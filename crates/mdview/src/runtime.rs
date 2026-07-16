//! Shared runtime helpers: build the engine, and spawn/await the daemon.
//! Lock + health live in `mdview_core::daemon` (shared with the desktop shell).

use anyhow::Result;
use mdview_core::config::{self, Config};
use mdview_core::daemon;
use mdview_core::{Engine, SqliteStore};
use std::time::Duration;

pub use mdview_core::daemon::{read_lock, remove_lock, running_daemon, write_lock, DaemonInfo};

/// Open the shared registry DB + config and build an Engine.
pub fn build_engine() -> Result<Engine> {
    let config = Config::load();
    let store = SqliteStore::open(&config::registry_db_path())?;
    Ok(Engine::new(store, config))
}

/// How long a spawn-gate lock may sit before its owner is presumed dead and the
/// gate is stolen — comfortably longer than the readiness poll below.
const SPAWN_GATE_STALE: Duration = Duration::from_secs(15);

/// The spawn-gate lock path: a sibling of the daemon lock. Its existence means
/// "some invocation is currently spawning the daemon".
fn spawn_gate_path() -> std::path::PathBuf {
    daemon::lock_path().with_extension("spawning")
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
    path: std::path::PathBuf,
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

/// Ensure a daemon is running and resolve its real bind `(host, port)` — the
/// connectivity values, spawning a daemon if none is up. This is the shared
/// basis for the *display* URL builders below; it never mutates connectivity.
fn ensure_bind() -> (String, u16) {
    if let Some(info) = daemon::running_daemon() {
        return (info.host, info.port);
    }
    // Serialize the cold-start spawn: parallel `open`/`view_file` invocations
    // must not each launch a daemon (two daemons fight over the port and the
    // SQLite registry, and the loser becomes an unkillable orphan). Only the
    // gate holder spawns; if the gate is unusable we degrade to the old
    // unguarded spawn — never worse than before. `_gate` is held across the
    // whole readiness wait so no second invocation spawns during the window.
    let _gate = acquire_spawn_gate_at(&spawn_gate_path(), SPAWN_GATE_STALE);
    match &_gate {
        Gate::Acquired(_) => {
            // Re-check under the gate: another spawner may have just finished.
            if let Some(info) = daemon::running_daemon() {
                return (info.host, info.port);
            }
            if let Err(e) = spawn_daemon_detached() {
                eprintln!("mdview: failed to auto-spawn daemon: {e}");
            }
        }
        Gate::Held => {} // another invocation is spawning; just wait below.
        Gate::Unavailable => {
            if let Err(e) = spawn_daemon_detached() {
                eprintln!("mdview: failed to auto-spawn daemon: {e}");
            }
        }
    }
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(100));
        if let Some(info) = daemon::running_daemon() {
            return (info.host, info.port);
        }
    }
    // Daemon never answered: surface it rather than silently handing back a
    // config-default URL that looks live. The URL is still returned for the
    // caller to print, but the operator now sees why it may not respond.
    eprintln!("mdview: daemon did not become ready in time; the viewer URL may not respond yet.");
    let cfg = Config::load();
    bind_fallback(daemon::read_lock(), &cfg)
}

/// Pure fallback decision for `ensure_bind()`'s timeout branch (unit-tested,
/// no I/O). `serve()` writes the daemon lock with the real bound `(host,
/// port)` immediately after `bind_with_retry` succeeds — before the daemon
/// answers its own health check — so a lock found here holds the real bound
/// port even though `running_daemon()`'s poll timed out. Only the configured
/// port is used when no lock exists at all (the daemon was never spawned).
fn bind_fallback(lock: Option<DaemonInfo>, cfg: &Config) -> (String, u16) {
    match lock {
        Some(info) => (info.host, info.port),
        None => (cfg.server.host.clone(), cfg.server.port),
    }
}

/// Ensure a daemon is running and return every viewable base URL (spawns one
/// if needed). When the daemon binds a wildcard host (`0.0.0.0` / `::`) and
/// no `hostname` override is set, this is one URL per reachable machine IP so
/// a caller (e.g. a remote agent) can pick an address that routes to it.
/// Otherwise it is a single URL. Display values only — connectivity
/// (`DaemonInfo.host`, health) is never derived from this.
pub fn ensure_daemon_bases() -> Vec<String> {
    let (host, port) = ensure_bind();
    display_urls_for(&host, port)
}

/// Every viewable base URL for an already-bound `(bind_host, port)` — the
/// display side of `ensure_daemon_bases` without the spawn/readiness wait, so a
/// process that has *already* bound its listener (e.g. `serve()`) can print the
/// same multi-IP list. Applies the `hostname` override and wildcard→machine-IP
/// expansion via `build_display_urls`. Display values only.
pub fn display_urls_for(bind_host: &str, port: u16) -> Vec<String> {
    let cfg = Config::load();
    build_display_urls(
        cfg.server.hostname.as_deref(),
        bind_host,
        port,
        &machine_ipv4s(),
    )
}

/// True if `host` is a wildcard "any interface" bind address, whose literal
/// form is useless as a link — the case that warrants listing real IPs.
fn is_wildcard(host: &str) -> bool {
    matches!(host, "0.0.0.0" | "::" | "[::]")
}

/// The machine's externally-usable IPv4 addresses (loopback and link-local
/// excluded), sorted and deduped. Empty when only loopback/link-local exist.
fn machine_ipv4s() -> Vec<String> {
    let mut out: Vec<String> = if_addrs::get_if_addrs()
        .into_iter()
        .flatten()
        .filter(|i| !i.is_loopback())
        .filter_map(|i| match i.ip() {
            std::net::IpAddr::V4(v4) if !v4.is_link_local() => Some(v4.to_string()),
            _ => None,
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Pure display-URL builder (unit-tested; no I/O). Precedence:
/// 1. a non-empty `host_name` override → that single URL;
/// 2. a wildcard `bind_host` with machine IPs → one URL per IP;
/// 3. a wildcard `bind_host` with no external IP → single `127.0.0.1` URL;
/// 4. any other `bind_host` → that single URL.
fn build_display_urls(
    host_name: Option<&str>,
    bind_host: &str,
    port: u16,
    machine_ips: &[String],
) -> Vec<String> {
    let url = |h: &str| format!("http://{h}:{port}");
    if let Some(name) = host_name.map(str::trim).filter(|h| !h.is_empty()) {
        return vec![url(name)];
    }
    if is_wildcard(bind_host) {
        if machine_ips.is_empty() {
            return vec![url("127.0.0.1")];
        }
        return machine_ips.iter().map(|ip| url(ip)).collect();
    }
    vec![url(bind_host)]
}

/// Spawn `mdview serve` fully detached, so MCP/CLI can guarantee a viewer is up
/// and the daemon outlives whatever process spawned it. Without the detach the
/// daemon shares its spawner's session/process-group and dies with it (SIGHUP
/// when the terminal/session closes, or a process-group-directed SIGTERM).
/// Apply the platform detach settings to `cmd` so a spawned child outlives its
/// spawner: a new session on Unix (`setsid`), a detached console + new process
/// group on Windows. Extracted from `spawn_daemon_detached` so the detach itself
/// is testable without launching the full daemon.
fn apply_detach(cmd: &mut std::process::Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: setsid() is async-signal-safe and is the only call made in the
        // forked child before exec. It puts the child in its own new session (as
        // session leader), detaching it from the spawner's controlling terminal
        // and process group so neither a SIGHUP on session close nor a
        // process-group-directed signal can reach it.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS: no inherited console. CREATE_NEW_PROCESS_GROUP: the
        // daemon does not receive Ctrl+C/Ctrl+Break sent to the spawner's group.
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
}

pub fn spawn_daemon_detached() -> Result<()> {
    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("serve")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    apply_detach(&mut cmd);
    cmd.spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        acquire_spawn_gate_at, bind_fallback, build_display_urls, is_wildcard, DaemonInfo, Gate,
    };
    use mdview_core::config::Config;
    use std::time::Duration;

    fn gate_tmp(label: &str) -> std::path::PathBuf {
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

    // The daemon-detach behavior (setsid) had no automated guard — the function
    // was once "detached" in name only. This exercises the real `apply_detach`
    // on a throwaway child and asserts it lands in its own session.
    #[cfg(unix)]
    #[test]
    fn apply_detach_puts_child_in_its_own_session() {
        use std::process::{Command, Stdio};
        let mut cmd = Command::new("sleep");
        cmd.arg("0.4")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        super::apply_detach(&mut cmd);
        let mut child = cmd.spawn().expect("spawn sleep");
        let pid = child.id() as i32;
        // pre_exec runs setsid before exec; give the child a moment to get there.
        std::thread::sleep(Duration::from_millis(80));
        let child_sid = unsafe { libc::getsid(pid) };
        let my_sid = unsafe { libc::getsid(0) };
        // Reap the child before asserting so a failed assert can't leak a process.
        child.kill().ok();
        child.wait().ok();
        // A detached child leads its own session: getsid(child) == child pid, and
        // it differs from the test process's session.
        assert_eq!(child_sid, pid, "detached child must lead its own session");
        assert_ne!(
            child_sid, my_sid,
            "detached child must not share our session"
        );
    }

    fn ips(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn host_name_override_wins_even_over_wildcard() {
        let urls = build_display_urls(Some("my.local"), "0.0.0.0", 7700, &ips(&["192.168.1.5"]));
        assert_eq!(urls, vec!["http://my.local:7700"]);
    }

    #[test]
    fn blank_host_name_is_ignored() {
        let urls = build_display_urls(Some("  "), "127.0.0.1", 7700, &[]);
        assert_eq!(urls, vec!["http://127.0.0.1:7700"]);
    }

    #[test]
    fn wildcard_lists_every_machine_ip() {
        let urls = build_display_urls(None, "0.0.0.0", 7700, &ips(&["192.168.1.5", "10.0.0.2"]));
        assert_eq!(
            urls,
            vec!["http://192.168.1.5:7700", "http://10.0.0.2:7700"]
        );
    }

    #[test]
    fn wildcard_with_no_ip_falls_back_to_loopback() {
        let urls = build_display_urls(None, "0.0.0.0", 7700, &[]);
        assert_eq!(urls, vec!["http://127.0.0.1:7700"]);
    }

    #[test]
    fn specific_bind_host_is_single_and_unchanged() {
        let urls = build_display_urls(None, "192.168.1.9", 7700, &ips(&["192.168.1.9"]));
        assert_eq!(urls, vec!["http://192.168.1.9:7700"]);
    }

    #[test]
    fn wildcard_detection() {
        assert!(is_wildcard("0.0.0.0"));
        assert!(is_wildcard("::"));
        assert!(is_wildcard("[::]"));
        assert!(!is_wildcard("127.0.0.1"));
        assert!(!is_wildcard("192.168.1.1"));
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
