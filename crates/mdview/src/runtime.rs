//! Shared runtime helpers: build the engine, and manage the single-daemon lock
//! (`~/.mdview/daemon.lock`) so every launcher coordinates on one server.

use anyhow::Result;
use mdview_core::config::{self, Config};
use mdview_core::{Engine, SqliteStore};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

/// Open the shared registry DB + config and build an Engine.
pub fn build_engine() -> Result<Engine> {
    let config = Config::load();
    let store = SqliteStore::open(&config::registry_db_path())?;
    Ok(Engine::new(store, config))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonInfo {
    pub pid: u32,
    pub host: String,
    pub port: u16,
    pub started_at: String,
}

pub fn lock_path() -> PathBuf {
    config::daemon_lock_path()
}

pub fn write_lock(info: &DaemonInfo) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(info)?;
    config::write_atomic(&lock_path(), &bytes)?;
    Ok(())
}

pub fn read_lock() -> Option<DaemonInfo> {
    let text = std::fs::read_to_string(lock_path()).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn remove_lock() {
    let _ = std::fs::remove_file(lock_path());
}

/// Is a mdview daemon actually answering on the locked port? A stale lock
/// (process gone) reads as not-running.
pub fn running_daemon() -> Option<DaemonInfo> {
    let info = read_lock()?;
    if health_check(&info.host, info.port) {
        Some(info)
    } else {
        None
    }
}

/// Minimal blocking HTTP GET /health; true if it looks like mdview.
pub fn health_check(host: &str, port: u16) -> bool {
    let addr = format!("{host}:{port}");
    let Ok(mut stream) = TcpStream::connect(&addr) else {
        return false;
    };
    stream.set_read_timeout(Some(Duration::from_millis(500))).ok();
    stream.set_write_timeout(Some(Duration::from_millis(500))).ok();
    let req = format!("GET /health HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut buf = String::new();
    let _ = stream.take(4096).read_to_string(&mut buf);
    buf.contains("\"mdview\"") || buf.contains("200 OK")
}

/// Ensure a daemon is running and return its base URL (spawns one if needed).
pub fn ensure_daemon_base() -> String {
    if let Some(info) = running_daemon() {
        return format!("http://{}:{}", info.host, info.port);
    }
    let _ = spawn_daemon_detached();
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(100));
        if let Some(info) = running_daemon() {
            return format!("http://{}:{}", info.host, info.port);
        }
    }
    let cfg = Config::load();
    format!("http://{}:{}", cfg.server.host, cfg.server.port)
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
