//! Shared runtime helpers: build the engine, and spawn/await the daemon.
//! Lock + health + the spawn-gate/readiness coordination live in
//! `mdview_core::daemon` (shared with the desktop shell); this module wraps
//! that shared `ensure_bind` with the CLI's own spawn strategy and stderr
//! reporting.

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

/// How many times, and how often, the CLI polls for daemon readiness after
/// spawning it before giving up and falling back (2s total).
const READY_POLL_ATTEMPTS: u32 = 20;
const READY_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Ensure a daemon is running and resolve its real bind `(host, port)` — the
/// connectivity values, spawning a daemon if none is up. This is the shared
/// basis for the *display* URL builders below; it never mutates connectivity.
/// Delegates the actual spawn-gate/readiness coordination to
/// `mdview_core::daemon::ensure_bind`, so the CLI and the desktop shell agree
/// on one implementation; only the spawn strategy and error reporting differ.
fn ensure_bind() -> (String, u16) {
    let result = daemon::ensure_bind(
        READY_POLL_ATTEMPTS,
        READY_POLL_INTERVAL,
        || spawn_daemon_detached().map_err(|e| std::io::Error::other(e.to_string())),
        |e| eprintln!("mdview: failed to auto-spawn daemon: {e}"),
    );
    match result {
        Ok(bind) => bind,
        Err(bind) => {
            // Daemon never answered: surface it rather than silently handing
            // back a config-default URL that looks live. The URL is still
            // returned for the caller to print, but the operator now sees
            // why it may not respond.
            eprintln!(
                "mdview: daemon did not become ready in time; the viewer URL may not respond yet."
            );
            bind
        }
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
/// The detach logic itself lives in `mdview_core::process` (shared with the
/// desktop shell's own spawn path) — re-exported here so existing call sites
/// and the existing unit test keep working unchanged.
pub(crate) use mdview_core::process::apply_detach;

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

/// Spawn `mdview refresh <project_id>` fully detached, so a newly-registered
/// project's full recursive scan happens off the calling process — `register`/
/// `open`/the MCP tool return as soon as the project row exists (and, for
/// `open`, the one requested file is viewable) instead of blocking on the
/// whole repo. Safe to run alongside an already-running daemon: CLI commands
/// and the daemon already share the same SQLite registry concurrently.
pub fn spawn_refresh_detached(project_id: &str) -> Result<()> {
    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("refresh")
        .arg(project_id)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    apply_detach(&mut cmd);
    cmd.spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{build_display_urls, is_wildcard};
    use std::time::Duration;

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
}
