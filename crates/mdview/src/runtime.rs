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

/// Ensure a daemon is running and return its base URL (spawns one if needed).
///
/// The returned host is a display value: when `config.server.host_name` is
/// set it replaces the bind/connect host in the URL text only — the daemon
/// still binds and is health-checked on its real host/IP (`DaemonInfo.host`).
pub fn ensure_daemon_base() -> String {
    if let Some(info) = daemon::running_daemon() {
        return display_base_url(&info.host, info.port);
    }
    let _ = spawn_daemon_detached();
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(100));
        if let Some(info) = daemon::running_daemon() {
            return display_base_url(&info.host, info.port);
        }
    }
    let cfg = Config::load();
    display_base_url(&cfg.server.host, cfg.server.port)
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

/// Spawn `mdview serve` detached, so MCP/CLI can guarantee a viewer is up.
pub fn spawn_daemon_detached() -> Result<()> {
    let exe = std::env::current_exe()?;
    std::process::Command::new(exe)
        .arg("serve")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}
