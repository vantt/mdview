//! Real end-to-end coverage for `mdview open` (D3/PBI-14): spawns the actual
//! compiled binary as a daemon, then runs `open --json` against it and
//! asserts the returned URL's port matches the daemon's real bound port.
//! This exercises the D3 happy path (loopback bind, no timeout fallback);
//! `bound-port-truth-1`'s own unit tests separately cover D2's stale-lock
//! fallback, which this test does not hit.

use mdview_core::daemon::{health_check, DaemonInfo};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Kills and reaps the spawned daemon on drop, even if an assertion above
/// panics — a leaked daemon process would otherwise strand a listening port
/// across CI/test runs.
struct DaemonGuard(Child);

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn scratch_home(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mdview-e2e-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create scratch HOME dir");
    dir
}

fn lock_path(home: &Path) -> PathBuf {
    home.join(".mdview").join("daemon.lock")
}

/// Poll for `serve()`'s daemon.lock (written immediately after bind, per
/// `bound-port-truth-1`) so the test knows the daemon's real bound port
/// without hardcoding or pre-selecting one.
fn wait_for_lock(home: &Path, timeout: Duration) -> DaemonInfo {
    let path = lock_path(home);
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(info) = serde_json::from_str::<DaemonInfo>(&text) {
                return info;
            }
        }
        if Instant::now() >= deadline {
            panic!(
                "daemon.lock never appeared/parsed within {timeout:?} at {}",
                path.display()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn wait_for_health(host: &str, port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if health_check(host, port) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Extract the `host:port` authority's port from a `http://host:port/...` URL
/// without pulling in a URL-parsing dependency for one field.
fn port_of(url: &str) -> u16 {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let authority = after_scheme.split('/').next().unwrap_or(after_scheme);
    authority
        .rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or_else(|| panic!("could not parse port out of url {url}"))
}

#[test]
fn cmd_open_json_url_port_matches_real_daemon_bound_port() {
    let bin = env!("CARGO_BIN_EXE_mdview");
    let home = scratch_home("open");

    let doc_dir = home.join("docs");
    std::fs::create_dir_all(&doc_dir).unwrap();
    let doc_path = doc_dir.join("note.md");
    std::fs::write(&doc_path, "# Hello\n").unwrap();

    // --port 0 asks the OS for a free port (no hardcoded/pre-selected port,
    // no collision with any real daemon on this machine); serve() writes the
    // real bound port to daemon.lock right after bind.
    let child = Command::new(bin)
        .args(["serve", "--port", "0", "--host", "127.0.0.1"])
        .env("HOME", &home)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mdview serve");
    let _guard = DaemonGuard(child);

    let info = wait_for_lock(&home, Duration::from_secs(10));
    assert!(
        wait_for_health(&info.host, info.port, Duration::from_secs(10)),
        "daemon never answered /health on {}:{}",
        info.host,
        info.port
    );

    let output = Command::new(bin)
        .args(["open", doc_path.to_str().unwrap(), "--json"])
        .env("HOME", &home)
        .output()
        .expect("run mdview open");
    assert!(
        output.status.success(),
        "mdview open failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("mdview open --json did not print valid JSON ({e}): {stdout}"));

    let url = json["url"].as_str().expect("url field present");
    let urls = json["urls"].as_array().expect("urls field present");
    assert_eq!(
        urls.first().and_then(|v| v.as_str()),
        Some(url),
        "url must equal urls[0]"
    );

    let url_port = port_of(url);
    assert_eq!(
        url_port, info.port,
        "mdview open returned a URL whose port ({url_port}) does not match \
         the daemon's real bound port ({})",
        info.port
    );
}
