//! Single-daemon coordination: the `~/.mdview/daemon.lock` (pid + port), a
//! health probe, and the spawn/readiness coordination every launcher (CLI and
//! desktop shell) needs to bring a daemon up and agree on one server (PRD
//! §7.1/§7.5).

use crate::config::{self, write_atomic, Config};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonInfo {
    pub pid: u32,
    pub host: String,
    pub port: u16,
    pub started_at: String,
    /// Version of the binary that started this daemon.
    ///
    /// Optional because lock files written by builds before this field existed
    /// have to keep parsing — a daemon that predates it reads as `None`, which is
    /// itself the signal that it is older than the current binary.
    #[serde(default)]
    pub version: Option<String>,
}

impl DaemonInfo {
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

pub fn lock_path() -> PathBuf {
    config::daemon_lock_path()
}

pub fn write_lock(info: &DaemonInfo) -> Result<()> {
    let bytes =
        serde_json::to_vec_pretty(info).map_err(|e| crate::error::Error::Other(e.to_string()))?;
    write_atomic(&lock_path(), &bytes)
}

pub fn read_lock() -> Option<DaemonInfo> {
    let text = std::fs::read_to_string(lock_path()).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn remove_lock() {
    let _ = std::fs::remove_file(lock_path());
}

/// The daemon in the lock, but only if it actually answers on its port.
/// A stale lock (process gone) reads as not-running.
pub fn running_daemon() -> Option<DaemonInfo> {
    let info = read_lock()?;
    if health_check(&info.host, info.port) {
        Some(info)
    } else {
        None
    }
}

/// True if `host` is a wildcard bind address (`0.0.0.0` / `::` / `[::]`).
/// `mdview`'s `runtime.rs` keeps its own copy for the *display* URL builder
/// (list every machine IP for a wildcard bind) — a different concern from
/// this module's "can a client on this machine dial it back" question below,
/// so that duplicate is intentional, not drift.
fn is_wildcard(host: &str) -> bool {
    matches!(host, "0.0.0.0" | "::" | "[::]")
}

/// `host`, or the loopback address a same-machine client can dial if `host`
/// is a wildcard bind address nothing can connect to directly.
fn substitute_loopback(host: &str) -> &str {
    if is_wildcard(host) {
        if host == "0.0.0.0" {
            "127.0.0.1"
        } else {
            "::1"
        }
    } else {
        host
    }
}

/// A URL a client on this machine (browser, WebView) can actually navigate
/// to for `(host, port)`. A wildcard bind host is not itself dialable, so it
/// is replaced with loopback; any other host is used unchanged. For callers
/// that need every LAN-reachable address instead (e.g. printing a link for a
/// remote viewer), see `mdview`'s `runtime::display_urls_for` — this
/// function is for a client that is always on the same machine as the
/// daemon, which the desktop shell's WebView always is (PRD §7.5).
pub fn loopback_url(host: &str, port: u16) -> String {
    let resolved = substitute_loopback(host);
    // An IPv6 literal (the only kind of host this predicate can produce that
    // contains a colon) needs brackets in a URL, or `host:port` is ambiguous.
    if resolved.contains(':') {
        format!("http://[{resolved}]:{port}")
    } else {
        format!("http://{resolved}:{port}")
    }
}

/// The version the *running* daemon reports on `/health`, not the version in the
/// lock file.
///
/// Upgrading the binary does not restart a daemon that is already serving, so the
/// live process can be older than the CLI that just wrote to the database. That
/// mismatch is invisible until a short link 404s, because the old process has no
/// `/s/` route. `None` means the daemon did not answer, or answered without a
/// version — both of which mean "older than this build" for a caller's purposes.
pub fn daemon_version(host: &str, port: u16) -> Option<String> {
    let body = health_body(host, port)?;
    let key = "\"version\"";
    let start = body.find(key)? + key.len();
    let rest = &body[start..];
    let open = rest.find('"')? + 1;
    let close = rest[open..].find('"')? + open;
    Some(rest[open..close].to_string())
}

/// Minimal blocking HTTP GET /health; true if it looks like mdview.
pub fn health_check(host: &str, port: u16) -> bool {
    match health_body(host, port) {
        Some(buf) => buf.contains("\"mdview\"") || buf.contains("200 OK"),
        None => false,
    }
}

/// Raw `GET /health` response text, or `None` if the daemon did not answer.
fn health_body(host: &str, port: u16) -> Option<String> {
    // A wildcard-bound daemon can't be dialed back on its own bind address on
    // every platform (e.g. WSAEADDRNOTAVAIL on macOS/Windows), so connect to
    // loopback instead. Only the connect target changes -- the `Host:` header
    // below keeps using the original, unsubstituted `host`.
    let connect_host = substitute_loopback(host);
    let mut stream = TcpStream::connect(format!("{connect_host}:{port}")).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok();
    stream
        .set_write_timeout(Some(Duration::from_millis(500)))
        .ok();
    let req = format!("GET /health HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).ok()?;
    let mut buf = String::new();
    let _ = stream.take(4096).read_to_string(&mut buf);
    Some(buf)
}

/// How long a spawn-gate lock may sit before its owner is presumed dead and
/// the gate is stolen — comfortably longer than any caller's readiness poll.
const SPAWN_GATE_STALE: Duration = Duration::from_secs(15);

/// The spawn-gate lock path: a sibling of the daemon lock. Its existence
/// means "some invocation is currently spawning the daemon".
fn spawn_gate_path() -> PathBuf {
    lock_path().with_extension("spawning")
}

/// Outcome of trying to become the daemon spawner.
enum Gate {
    /// We own the gate and must do the spawn. The guard is held only for its
    /// `Drop` (which removes the gate file), never read — hence `dead_code`.
    Acquired(#[allow(dead_code)] SpawnGate),
    /// Another live invocation holds the gate — wait for the daemon, don't spawn.
    Held,
    /// The gate file could not be used at all — caller should spawn unguarded.
    Unavailable,
}

/// RAII guard that removes the spawn-gate file when the spawner is done.
struct SpawnGate {
    path: PathBuf,
}

impl Drop for SpawnGate {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Atomically claim the spawn gate at `path` via `create_new` (O_EXCL), so
/// exactly one racer wins. An existing gate older than `stale_after` is assumed
/// abandoned (its owner died mid-spawn) and stolen.
fn acquire_spawn_gate_at(path: &std::path::Path, stale_after: Duration) -> Gate {
    let claim = |p: &std::path::Path| {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(p)
    };
    match claim(path) {
        Ok(_) => Gate::Acquired(SpawnGate {
            path: path.to_path_buf(),
        }),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let stale = std::fs::metadata(path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .map(|age| age > stale_after)
                .unwrap_or(true); // unreadable/future mtime → treat as stale
            if !stale {
                return Gate::Held;
            }
            let _ = std::fs::remove_file(path);
            match claim(path) {
                Ok(_) => Gate::Acquired(SpawnGate {
                    path: path.to_path_buf(),
                }),
                Err(_) => Gate::Held, // lost the steal race to another invocation
            }
        }
        // Directory missing/unwritable etc. — the gate is unusable here.
        Err(_) => Gate::Unavailable,
    }
}

/// Pure fallback decision for `ensure_bind()`'s timeout branch (unit-tested,
/// no I/O). The daemon's own `serve()` writes the lock with the real bound
/// `(host, port)` immediately after binding — before it answers its own
/// health check — so a lock found here holds the real bound port even
/// though `running_daemon()`'s poll timed out. Only the configured port is
/// used when no lock exists at all (the daemon was never spawned).
fn bind_fallback(lock: Option<DaemonInfo>, cfg: &Config) -> (String, u16) {
    match lock {
        Some(info) => (info.host, info.port),
        None => (cfg.server.host.clone(), cfg.server.port),
    }
}

/// Ensure a daemon is running, spawning one via `spawn` if none is up yet,
/// and resolve its real bind `(host, port)`. Shared by the CLI
/// (`mdview::runtime`) and the desktop shell so both launchers use the exact
/// same spawn-gate/readiness coordination (PRD §7.1/§7.5) instead of two
/// independently-drifting copies. `spawn` is injected because the CLI and
/// the desktop shell resolve/spawn the daemon binary differently
/// (`current_exe()` re-invoked with `serve`, vs a sibling `mdview` binary).
///
/// `on_spawn_error` fires at most once, synchronously, if `spawn` itself
/// errors (the process could not even be started) — the caller reports that
/// however fits its UI (stderr for the CLI, a dialog for the desktop shell)
/// instead of failing silently.
///
/// Returns `Ok((host, port))` once the daemon answers its health check.
/// Returns `Err((host, port))` if it never becomes healthy within
/// `poll_attempts * poll_interval` — the best-effort fallback bind a caller
/// can still show, paired with the fact that it is not confirmed live.
pub fn ensure_bind(
    poll_attempts: u32,
    poll_interval: Duration,
    spawn: impl FnOnce() -> std::io::Result<()>,
    on_spawn_error: impl FnOnce(&std::io::Error),
) -> std::result::Result<(String, u16), (String, u16)> {
    if let Some(info) = running_daemon() {
        return Ok((info.host, info.port));
    }
    // Serialize the cold-start spawn: parallel invocations must not each
    // launch a daemon (two daemons fight over the port and the SQLite
    // registry, and the loser becomes an unkillable orphan). Only the gate
    // holder spawns; if the gate is unusable we degrade to the old unguarded
    // spawn — never worse than before. `_gate` is held across the whole
    // readiness wait so no second invocation spawns during the window.
    let _gate = acquire_spawn_gate_at(&spawn_gate_path(), SPAWN_GATE_STALE);
    match &_gate {
        Gate::Acquired(_) => {
            // Re-check under the gate: another spawner may have just finished.
            if let Some(info) = running_daemon() {
                return Ok((info.host, info.port));
            }
            if let Err(e) = spawn() {
                on_spawn_error(&e);
            }
        }
        Gate::Held => {} // another invocation is spawning; just wait below.
        Gate::Unavailable => {
            if let Err(e) = spawn() {
                on_spawn_error(&e);
            }
        }
    }
    for _ in 0..poll_attempts {
        std::thread::sleep(poll_interval);
        if let Some(info) = running_daemon() {
            return Ok((info.host, info.port));
        }
    }
    // Daemon never answered: the caller decides how to surface this rather
    // than silently handing back a config-default URL that looks live.
    let cfg = Config::load();
    Err(bind_fallback(read_lock(), &cfg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_info_serde_roundtrip() {
        let info = DaemonInfo {
            pid: 42,
            host: "127.0.0.1".into(),
            port: 7700,
            started_at: "2026-07-15T00:00:00Z".into(),
            version: Some("0.5.2".into()),
        };
        let s = serde_json::to_string(&info).unwrap();
        let back: DaemonInfo = serde_json::from_str(&s).unwrap();
        assert_eq!(back.pid, 42);
        assert_eq!(back.base_url(), "http://127.0.0.1:7700");
        assert_eq!(back.version.as_deref(), Some("0.5.2"));
    }

    /// A lock file written before `version` existed must still parse — a daemon
    /// from an older build is exactly the case this field exists to detect, so
    /// failing to read its lock would defeat the purpose.
    #[test]
    fn a_lock_file_without_version_still_parses() {
        let legacy =
            r#"{"pid":42,"host":"127.0.0.1","port":7700,"started_at":"2026-07-15T00:00:00Z"}"#;
        let info: DaemonInfo = serde_json::from_str(legacy).unwrap();
        assert_eq!(info.pid, 42);
        assert_eq!(info.version, None);
    }

    #[test]
    fn daemon_version_is_none_when_nothing_answers() {
        assert_eq!(daemon_version("127.0.0.1", 59_998), None);
    }

    #[test]
    fn health_check_false_on_dead_port() {
        // Nothing listening on this port → false, no panic.
        assert!(!health_check("127.0.0.1", 59_999));
    }

    #[test]
    fn health_check_dials_loopback_for_wildcard_host() {
        // A daemon bound to a wildcard host ("0.0.0.0") can't be dialed back
        // on that address on every platform (macOS/Windows reject it), so
        // health_check must substitute loopback at the connect call site.
        // This test proves that substitution by listening on 127.0.0.1 only
        // and calling health_check with "0.0.0.0".
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                // Drain the request so the client's write doesn't block/hang.
                let mut buf = [0u8; 512];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n");
            }
        });

        assert!(health_check("0.0.0.0", port));

        handle.join().unwrap();
    }

    #[test]
    fn loopback_url_substitutes_for_wildcard_hosts() {
        assert_eq!(loopback_url("0.0.0.0", 7700), "http://127.0.0.1:7700");
        assert_eq!(loopback_url("::", 7700), "http://[::1]:7700");
        assert_eq!(loopback_url("[::]", 7700), "http://[::1]:7700");
    }

    #[test]
    fn loopback_url_leaves_a_concrete_host_unchanged() {
        assert_eq!(loopback_url("192.168.1.9", 7700), "http://192.168.1.9:7700");
        assert_eq!(loopback_url("127.0.0.1", 7700), "http://127.0.0.1:7700");
    }

    fn gate_tmp(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mdview-gate-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("daemon.spawning")
    }

    #[test]
    fn spawn_gate_grants_one_holder_then_blocks_until_released() {
        let path = gate_tmp("excl");
        let g1 = acquire_spawn_gate_at(&path, Duration::from_secs(15));
        assert!(matches!(g1, Gate::Acquired(_)));
        assert!(path.exists());
        // A second racer, while the gate is held and fresh, must be blocked.
        assert!(matches!(
            acquire_spawn_gate_at(&path, Duration::from_secs(15)),
            Gate::Held
        ));
        // Dropping the guard releases the gate file...
        drop(g1);
        assert!(!path.exists());
        // ...and it can be claimed again.
        assert!(matches!(
            acquire_spawn_gate_at(&path, Duration::from_secs(15)),
            Gate::Acquired(_)
        ));
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn spawn_gate_steals_a_stale_lock() {
        let path = gate_tmp("stale");
        std::fs::write(&path, b"").unwrap();
        // stale_after = 0 → an existing gate is immediately abandoned and stolen.
        assert!(matches!(
            acquire_spawn_gate_at(&path, Duration::from_secs(0)),
            Gate::Acquired(_)
        ));
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn bind_fallback_prefers_the_lock_port_over_the_config_port() {
        let mut cfg = Config::default();
        cfg.server.port = 7700;
        cfg.server.host = "127.0.0.1".into();
        let lock = DaemonInfo {
            pid: 1234,
            host: "127.0.0.1".into(),
            port: 7701, // bind_with_retry auto-incremented past the configured port
            started_at: "2026-07-16T00:00:00Z".into(),
            version: None,
        };
        assert_eq!(
            bind_fallback(Some(lock), &cfg),
            ("127.0.0.1".to_string(), 7701)
        );
    }

    #[test]
    fn bind_fallback_uses_config_port_when_no_lock_exists() {
        let mut cfg = Config::default();
        cfg.server.port = 7700;
        cfg.server.host = "127.0.0.1".into();
        assert_eq!(bind_fallback(None, &cfg), ("127.0.0.1".to_string(), 7700));
    }
}
