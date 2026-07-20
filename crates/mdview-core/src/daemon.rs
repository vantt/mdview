//! Single-daemon coordination: the `~/.mdview/daemon.lock` (pid + port) and a
//! health probe. Shared by the CLI/daemon and the desktop shell so every
//! launcher agrees on one server (PRD §7.1/§7.5).

use crate::config::{self, write_atomic};
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
/// Duplicated from `mdview`'s `runtime.rs::is_wildcard` rather than shared:
/// `mdview-core` cannot depend on the binary crate (wrong dependency
/// direction), and this is a 2-line pure predicate with no drift risk.
fn is_wildcard(host: &str) -> bool {
    matches!(host, "0.0.0.0" | "::" | "[::]")
}

/// Minimal blocking HTTP GET /health; true if it looks like mdview.
pub fn health_check(host: &str, port: u16) -> bool {
    // A wildcard-bound daemon can't be dialed back on its own bind address on
    // every platform (e.g. WSAEADDRNOTAVAIL on macOS/Windows), so connect to
    // loopback instead. Only the connect target changes -- the `Host:`
    // header below keeps using the original, unsubstituted `host`.
    let connect_host = if is_wildcard(host) {
        if host == "0.0.0.0" {
            "127.0.0.1"
        } else {
            "::1"
        }
    } else {
        host
    };
    let Ok(mut stream) = TcpStream::connect(format!("{connect_host}:{port}")) else {
        return false;
    };
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok();
    stream
        .set_write_timeout(Some(Duration::from_millis(500)))
        .ok();
    let req = format!("GET /health HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut buf = String::new();
    let _ = stream.take(4096).read_to_string(&mut buf);
    buf.contains("\"mdview\"") || buf.contains("200 OK")
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
        };
        let s = serde_json::to_string(&info).unwrap();
        let back: DaemonInfo = serde_json::from_str(&s).unwrap();
        assert_eq!(back.pid, 42);
        assert_eq!(back.base_url(), "http://127.0.0.1:7700");
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
}
