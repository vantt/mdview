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

/// Ensure a daemon is running and resolve its real bind `(host, port)` — the
/// connectivity values, spawning a daemon if none is up. This is the shared
/// basis for the *display* URL builders below; it never mutates connectivity.
fn ensure_bind() -> (String, u16) {
    if let Some(info) = daemon::running_daemon() {
        return (info.host, info.port);
    }
    if let Err(e) = spawn_daemon_detached() {
        eprintln!("mdview: failed to auto-spawn daemon: {e}");
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
    (cfg.server.host, cfg.server.port)
}

/// Ensure a daemon is running and return its base URL (spawns one if needed).
///
/// The returned host is a display value: when `config.server.host_name` is
/// set it replaces the bind/connect host in the URL text only — the daemon
/// still binds and is health-checked on its real host/IP (`DaemonInfo.host`).
pub fn ensure_daemon_base() -> String {
    let (host, port) = ensure_bind();
    display_base_url(&host, port)
}

/// Like [`ensure_daemon_base`], but returns *every* viewable base URL. When the
/// daemon binds a wildcard host (`0.0.0.0` / `::`) and no `host_name` override
/// is set, this is one URL per reachable machine IP so a caller (e.g. a remote
/// agent) can pick an address that routes to it. Otherwise it is the single URL
/// [`ensure_daemon_base`] would return. Display values only — connectivity
/// (`DaemonInfo.host`, health) is never derived from this.
pub fn ensure_daemon_bases() -> Vec<String> {
    let (host, port) = ensure_bind();
    let cfg = Config::load();
    let host_name = cfg.server.host_name.as_deref();
    build_display_urls(host_name, &host, port, &machine_ipv4s())
}

fn display_base_url(bind_host: &str, port: u16) -> String {
    let cfg = Config::load();
    let host = cfg
        .server
        .host_name
        .as_deref()
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .unwrap_or(bind_host);
    format!("http://{host}:{port}")
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
pub fn spawn_daemon_detached() -> Result<()> {
    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("serve")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: setsid() is async-signal-safe and is the only call made in the
        // forked child before exec. It puts the daemon in its own new session
        // (as session leader), detaching it from the spawner's controlling
        // terminal and process group so neither a SIGHUP on session close nor a
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

    cmd.spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{build_display_urls, is_wildcard};

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
