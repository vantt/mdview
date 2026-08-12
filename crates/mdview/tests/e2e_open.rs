//! Real end-to-end coverage for `mdview open` (D3/PBI-14): spawns the actual
//! compiled binary as a daemon, then runs `open --json` against it and
//! asserts the returned URL's port matches the daemon's real bound port.
//! This exercises the D3 happy path (loopback bind, no timeout fallback);
//! `bound-port-truth-1`'s own unit tests separately cover D2's stale-lock
//! fallback, which this test does not hit.

use mdview_core::daemon::{health_check, DaemonInfo};
use std::io::{Read, Write};
use std::net::TcpStream;
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

/// Raw `GET {path}` against a real daemon, reading the full response until
/// the connection closes (the daemon is asked to `Connection: close`).
/// Returns (status code, body). No HTTP client dependency needed for one
/// route per test file — mirrors `mdview_core::daemon`'s own raw-socket
/// health check.
fn http_get(host: &str, port: u16, path: &str) -> (u16, String) {
    let mut stream =
        TcpStream::connect((host, port)).unwrap_or_else(|e| panic!("connect {host}:{port}: {e}"));
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let req = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).unwrap();
    let mut buf = String::new();
    stream.read_to_string(&mut buf).unwrap();
    let status = buf
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("could not parse status line from response: {buf:?}"));
    let body = buf.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (status, body)
}

fn write_file(path: &Path, body: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

/// Registers a project via `mdview open <readme>` (the same mechanism the
/// existing test above uses — both the daemon and the CLI share the same
/// on-disk index under `HOME`, so registering via the CLI is visible to the
/// already-running daemon immediately) and returns its `project_id`.
fn open_project(bin: &str, home: &Path, readme: &Path) -> String {
    let output = Command::new(bin)
        .args(["open", readme.to_str().unwrap(), "--json"])
        .env("HOME", home)
        .output()
        .expect("run mdview open");
    assert!(
        output.status.success(),
        "mdview open failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&output.stdout).trim())
            .expect("mdview open --json printed valid JSON");
    json["project_id"]
        .as_str()
        .expect("project_id present")
        .to_string()
}

#[test]
fn code_section_lists_dirs_highlights_files_and_denies_sensitive_paths() {
    let bin = env!("CARGO_BIN_EXE_mdview");
    let home = scratch_home("code");
    let root = home.join("proj");

    write_file(&root.join("README.md"), b"# Hello\n");
    write_file(&root.join("src/lib.rs"), b"pub fn hello() {}\n");
    write_file(&root.join(".git/config"), b"[core]\n");
    write_file(&root.join("secret.pem"), b"-----BEGIN PRIVATE KEY-----");

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
        "daemon never answered /health"
    );

    let project_id = open_project(bin, &home, &root.join("README.md"));

    // 1. Root directory listing: the "src" folder is present.
    let (status, body) = http_get(&info.host, info.port, &format!("/p/{project_id}/_code/"));
    assert_eq!(status, 200);
    assert!(body.contains("src"), "dir listing missing src/: {body}");

    // 2. A source file renders highlighted with a line-numbered gutter.
    let (status, body) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/src/lib.rs"),
    );
    assert_eq!(status, 200);
    assert!(body.contains("id=\"L1\""), "missing line anchor: {body}");
    assert!(body.contains("class=\""), "missing syntect class: {body}");

    // 3. A path under the project's own git directory is denied.
    let (git_status, git_body) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/.git/config"),
    );
    assert_eq!(git_status, 404);

    // 4. Traversal outside the project root is denied.
    let (trav_status, _) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/../../../../../../etc/passwd"),
    );
    assert_eq!(trav_status, 404);

    // 5. Denied and missing paths return byte-identical bodies — a
    //    different message would itself disclose that the denied file
    //    exists.
    let (missing_status, missing_body) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/does-not-exist.txt"),
    );
    assert_eq!(missing_status, 404);
    assert_eq!(
        git_body, missing_body,
        "denied and missing paths must return identical bodies"
    );

    // 6. A denylisted extension (not just a denylisted directory) is denied.
    let (pem_status, _) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/secret.pem"),
    );
    assert_eq!(pem_status, 404);

    // 7. The same markdown file renders as highlighted SOURCE through the
    //    Code section, not as rendered markdown — the two sections stay
    //    distinct.
    let (status, body) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/README.md"),
    );
    assert_eq!(status, 200);
    assert!(body.contains("id=\"L1\""), "not rendered as source: {body}");
    assert!(
        !body.contains("fg-prose"),
        "markdown pipeline leaked into the Code section: {body}"
    );

    // 8. The Docs page still renders normally and now carries the Docs|Code
    //    section switch — the only change this item makes to that page.
    let (status, body) = http_get(&info.host, info.port, &format!("/p/{project_id}/README.md"));
    assert_eq!(status, 200);
    assert!(
        body.contains("section-switch"),
        "Docs page missing the section switch: {body}"
    );
}
