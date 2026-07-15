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
