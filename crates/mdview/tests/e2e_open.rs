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

/// One raw HTTP/1.1 request/response over a fresh connection (the daemon is
/// asked to `Connection: close`). No HTTP client dependency needed for a
/// handful of routes per test file — mirrors `mdview_core::daemon`'s own
/// raw-socket health check.
struct RawResponse {
    status: u16,
    set_cookie: Option<String>,
    location: Option<String>,
    body: String,
}

fn raw_request(host: &str, port: u16, req: &str) -> RawResponse {
    let mut stream =
        TcpStream::connect((host, port)).unwrap_or_else(|e| panic!("connect {host}:{port}: {e}"));
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(req.as_bytes()).unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();

    let status = raw
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("could not parse status line from response: {raw:?}"));
    let headers = raw.split("\r\n\r\n").next().unwrap_or("");
    let set_cookie = headers.lines().find_map(|l| {
        l.to_lowercase()
            .strip_prefix("set-cookie: ")
            .map(|v| v.to_string())
    });
    let location = headers.lines().find_map(|l| {
        if l.to_lowercase().starts_with("location: ") {
            l.split_once(": ").map(|(_, v)| v.to_string())
        } else {
            None
        }
    });
    let body = raw.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    RawResponse {
        status,
        set_cookie,
        location,
        body,
    }
}

/// `GET {path}`, optionally with a `Cookie` header. Returns (status, body).
fn http_get(host: &str, port: u16, path: &str, cookie: Option<&str>) -> (u16, String) {
    let (status, _location, body) = http_get_with_location(host, port, path, cookie);
    (status, body)
}

/// Same as `http_get`, but also returns the `Location` header (for
/// asserting an auth-redirect target).
fn http_get_with_location(
    host: &str,
    port: u16,
    path: &str,
    cookie: Option<&str>,
) -> (u16, Option<String>, String) {
    let cookie_header = cookie
        .map(|c| format!("Cookie: {c}\r\n"))
        .unwrap_or_default();
    let req =
        format!("GET {path} HTTP/1.1\r\nHost: {host}\r\n{cookie_header}Connection: close\r\n\r\n");
    let res = raw_request(host, port, &req);
    (res.status, res.location, res.body)
}

/// `POST {path}` with a urlencoded form body. Returns (status, `Set-Cookie`
/// name=value pair if any).
fn http_post_form(host: &str, port: u16, path: &str, form: &str) -> (u16, Option<String>) {
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{form}",
        form.len()
    );
    let res = raw_request(host, port, &req);
    let cookie = res
        .set_cookie
        .map(|c| c.split(';').next().unwrap_or(&c).to_string());
    (res.status, cookie)
}

/// Read the daemon's auto-generated login token from `config.toml` under
/// `home` (D3: `serve()` generates and persists one on first start when
/// none is configured) and exchange it for a session cookie.
fn login_and_get_cookie(host: &str, port: u16, home: &Path) -> String {
    let cfg_text = std::fs::read_to_string(home.join(".mdview/config.toml"))
        .expect("config.toml must exist after first daemon start (D3 auto-generates web_secret)");
    let token = cfg_text
        .lines()
        .find_map(|l| l.strip_prefix("web_secret = "))
        .map(|v| v.trim_matches('"').to_string())
        .expect("config.toml must contain an auto-generated web_secret");

    let (status, cookie) = http_post_form(host, port, "/api/login", &format!("token={token}"));
    assert_eq!(
        status, 303,
        "login with the auto-generated token must succeed"
    );
    cookie.expect("login must set a session cookie")
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

    // 0a. Unauthenticated: the Code section is gated exactly like Docs — a
    // browser page route redirects to /login rather than a bare 404.
    let (anon_status, anon_location, _) = http_get_with_location(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/"),
        None,
    );
    assert_eq!(
        anon_status, 303,
        "Code section must require login too, via redirect"
    );
    assert_eq!(anon_location.as_deref(), Some("/login"));

    // 0b. Sign in with the token `serve()` auto-generated on first start
    // (D3) and use the resulting cookie for every request below — without
    // it, every one of these would 404 on auth alone, proving nothing about
    // code_source's own guards.
    let cookie = login_and_get_cookie(&info.host, info.port, &home);
    let cookie = Some(cookie.as_str());

    // 1. Root directory listing: the "src" folder is present.
    let (status, body) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/"),
        cookie,
    );
    assert_eq!(status, 200);
    assert!(body.contains("src"), "dir listing missing src/: {body}");

    // 2. A source file renders highlighted with a line-numbered gutter.
    let (status, body) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/src/lib.rs"),
        cookie,
    );
    assert_eq!(status, 200);
    assert!(body.contains("id=\"L1\""), "missing line anchor: {body}");
    assert!(body.contains("class=\""), "missing syntect class: {body}");

    // 3. A path under the project's own git directory is denied.
    let (git_status, git_body) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/.git/config"),
        cookie,
    );
    assert_eq!(git_status, 404);

    // 4. Traversal outside the project root is denied.
    let (trav_status, _) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/../../../../../../etc/passwd"),
        cookie,
    );
    assert_eq!(trav_status, 404);

    // 5. Denied and missing paths return byte-identical bodies — a
    //    different message would itself disclose that the denied file
    //    exists.
    let (missing_status, missing_body) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/does-not-exist.txt"),
        cookie,
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
        cookie,
    );
    assert_eq!(pem_status, 404);

    // 7. The same markdown file renders as highlighted SOURCE through the
    //    Code section, not as rendered markdown — the two sections stay
    //    distinct.
    let (status, body) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/README.md"),
        cookie,
    );
    assert_eq!(status, 200);
    assert!(body.contains("id=\"L1\""), "not rendered as source: {body}");
    assert!(
        !body.contains("fg-prose"),
        "markdown pipeline leaked into the Code section: {body}"
    );

    // 8. The Docs page still renders normally and now carries the Docs|Code
    //    section switch — the only change this item makes to that page.
    let (status, body) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/README.md"),
        cookie,
    );
    assert_eq!(status, 200);
    assert!(
        body.contains("section-switch"),
        "Docs page missing the section switch: {body}"
    );
    // The topbar no longer repeats the path — that's the sticky breadcrumb's
    // job now (and the sidebar's own crumbs).
    assert!(
        !body.contains("class=\"crumb\""),
        "topbar should no longer show a redundant path crumb: {body}"
    );
    // The sticky breadcrumb is capped to the reading column's width (Docs
    // only), and carries the copy-as-markdown button in its right half —
    // moved out of the topbar.
    assert!(
        body.contains("breadcrumb breadcrumb--reading"),
        "Docs breadcrumb must be width-capped to match the article below it: {body}"
    );
    let breadcrumb_at = body.find("breadcrumb--reading").unwrap();
    let breadcrumb_end = body[breadcrumb_at..]
        .find("</nav>")
        .map(|i| breadcrumb_at + i)
        .expect("breadcrumb nav never closes");
    assert!(
        body[breadcrumb_at..breadcrumb_end].contains("id=\"copy-md\""),
        "copy-as-markdown button must live inside the breadcrumb, not the topbar: {body}"
    );
}

/// The Code sidebar carries a `chap-crumbs` path right below the search
/// box, folders inside a `chap-folders` disclosure (collapsible/counted/
/// icon, same shape Docs' own chapter sidebar uses) followed by a separate
/// Files list, and a code file's breadcrumb right half carries its
/// language and size.
#[test]
fn code_view_sidebar_splits_folders_files_and_breadcrumb_shows_type_and_size() {
    let bin = env!("CARGO_BIN_EXE_mdview");
    let home = scratch_home("code-sidebar");
    let root = home.join("proj");

    write_file(&root.join("README.md"), b"# Hello\n");
    write_file(&root.join("src/lib.rs"), b"pub fn hello() {}\n");
    write_file(&root.join("docs/notes.md"), b"notes\n");
    write_file(&root.join("only-dirs/child/deep.rs"), b"pub fn deep() {}\n");

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
    let cookie = login_and_get_cookie(&info.host, info.port, &home);
    let cookie = Some(cookie.as_str());

    // 1. Root directory listing: folders render inside a `chap-folders`
    //    disclosure (with a count and a `chap-subfolder` icon per entry —
    //    "docs", "only-dirs" and "src" all at root, so count is 3), followed
    //    by a separate Files section, folders first.
    let (status, body) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/"),
        cookie,
    );
    assert_eq!(status, 200);
    // Sidebar breadcrumb sits right below the search box, before anything
    // else — just the project root at the top level (no ancestor segments).
    let search_at = body
        .find("fg-sidebar-search")
        .expect("sidebar search box missing");
    let crumbs_at = body.find("chap-crumbs").expect("sidebar crumbs missing");
    assert!(
        search_at < crumbs_at,
        "sidebar crumbs must render right after the search box: {body}"
    );
    let crumbs_end = body[crumbs_at..]
        .find("</div>")
        .map(|i| crumbs_at + i)
        .expect("crumbs div never closes");
    assert!(
        !body[crumbs_at..crumbs_end].contains("chap-sep"),
        "root-level crumbs must not show an ancestor separator: {body}"
    );
    assert!(
        body.contains("chap-folders__label\">Subfolders<"),
        "folder disclosure must be labelled Subfolders (matching Docs): {body}"
    );
    assert!(
        body.contains("chap-folders__count\">3<"),
        "folder count missing/wrong: {body}"
    );
    assert!(
        body.contains("chap-subfolder"),
        "folder entries missing their chap-subfolder (icon) class: {body}"
    );
    let folders_at = body
        .find("chap-folders__bar")
        .expect("Folders disclosure bar missing");
    let files_at = body
        .find("chap-sec\">Files")
        .expect("Files section header missing");
    assert!(
        folders_at < files_at,
        "Folders disclosure must render before Files: {body}"
    );
    // Root has a file (README.md) alongside the two folders, so the
    // disclosure must NOT auto-open (only an empty-of-files folder does).
    assert!(
        !body.contains("chap-folders is-open"),
        "Folders disclosure should stay collapsed when the folder has files too: {body}"
    );

    // 1b. A folder with only subfolders and no files of its own (only-dirs/)
    //     auto-opens its disclosure — otherwise the sidebar would show
    //     nothing until the user expands it by hand.
    let (status, body) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/only-dirs/"),
        cookie,
    );
    assert_eq!(status, 200);
    assert!(
        body.contains("chap-folders is-open"),
        "Folders disclosure should auto-open when the folder has no files: {body}"
    );

    // 1c. Two levels deep (only-dirs/child/), the sidebar crumbs show both
    //     ancestor segments in order, each a real link to that folder.
    let (status, body) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/only-dirs/child/"),
        cookie,
    );
    assert_eq!(status, 200);
    let only_dirs_seg = body
        .find(&format!(
            "href=\"/p/{project_id}/_code/only-dirs\">only-dirs<"
        ))
        .expect("crumbs missing the only-dirs ancestor segment");
    let child_seg = body
        .find(&format!(
            "href=\"/p/{project_id}/_code/only-dirs/child\">child<"
        ))
        .expect("crumbs missing the child ancestor segment");
    assert!(
        only_dirs_seg < child_seg,
        "crumb segments must render in path order (only-dirs before child): {body}"
    );

    // 2. A source file's breadcrumb right half carries its language and size.
    let (status, body) = http_get(
        &info.host,
        info.port,
        &format!("/p/{project_id}/_code/src/lib.rs"),
        cookie,
    );
    assert_eq!(status, 200);
    let right_start = body
        .find("breadcrumb__right")
        .expect("breadcrumb right half missing");
    let right_slice = &body[right_start..right_start + 300];
    assert!(
        right_slice.contains("breadcrumb__meta-lang"),
        "breadcrumb right half missing file type: {right_slice}"
    );
    assert!(
        right_slice.contains("breadcrumb__meta-size"),
        "breadcrumb right half missing file size: {right_slice}"
    );
    // The topbar no longer repeats the path, the main pane fills the
    // viewport height for its own scrollbar (content--code), and the old
    // line-count line is gone (its info already moved into the breadcrumb
    // above).
    assert!(
        !body.contains("class=\"crumb\""),
        "topbar should no longer show a redundant path crumb: {body}"
    );
    assert!(
        body.contains("content content--code"),
        "Code page's main pane must carry content--code for its own scroll area: {body}"
    );
    assert!(
        !body.contains("codeview__head"),
        "the standalone line-count line should be gone, not just relabelled: {body}"
    );
}

/// D3: a first start with no configured `web_secret` generates one, persists
/// it to `config.toml`, and prints it to stdout exactly once — the operator's
/// only way to learn it without opening the file.
#[test]
fn first_start_generates_and_prints_a_login_token_exactly_once() {
    use std::io::{BufRead, BufReader};

    let bin = env!("CARGO_BIN_EXE_mdview");
    let home = scratch_home("first-run-token");

    let mut child = Command::new(bin)
        .args(["serve", "--port", "0", "--host", "127.0.0.1"])
        .env("HOME", &home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mdview serve");
    let stdout = child.stdout.take().expect("piped stdout");
    let _guard = DaemonGuard(child);

    let info = wait_for_lock(&home, Duration::from_secs(10));
    assert!(
        wait_for_health(&info.host, info.port, Duration::from_secs(10)),
        "daemon never answered /health"
    );

    let cfg_text = std::fs::read_to_string(home.join(".mdview/config.toml")).unwrap();
    let token = cfg_text
        .lines()
        .find_map(|l| l.strip_prefix("web_secret = "))
        .map(|v| v.trim_matches('"').to_string())
        .expect("config.toml must contain an auto-generated web_secret");
    assert!(!token.is_empty());

    // serve() prints exactly 4 lines to stdout before it starts accepting
    // connections (the 3-line token notice, then the single-URL "serving
    // on" line — `host=127.0.0.1` guarantees exactly one display URL) and
    // nothing after; by the time the health check above succeeded, all 4
    // are already sitting in the pipe. Reading exactly 4 avoids blocking on
    // a 5th line the long-running daemon will never print.
    let reader = BufReader::new(stdout);
    let mut occurrences = 0;
    for line in reader.lines().map_while(Result::ok).take(4) {
        if line.contains(&token) {
            occurrences += 1;
        }
    }
    assert_eq!(
        occurrences, 1,
        "the generated token must be printed to stdout exactly once"
    );
}

/// tsk-46t: an unauthenticated browser page request redirects to `/login`
/// (a bare 404 has nowhere useful for a navigation to go), while an
/// unauthenticated API request stays an opaque 404 — the two extractors
/// (`AuthPage`/`AuthSession`) must not blur together.
#[test]
fn unauthenticated_page_redirects_to_login_while_api_stays_opaque_404() {
    let bin = env!("CARGO_BIN_EXE_mdview");
    let home = scratch_home("unauth-redirect");

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

    // Browser page route ("/"): redirect to /login, not a bare 404.
    let (page_status, page_location, _) = http_get_with_location(&info.host, info.port, "/", None);
    assert_eq!(page_status, 303, "unauthenticated page route must redirect");
    assert_eq!(page_location.as_deref(), Some("/login"));

    // API route ("/api/status"): stays an opaque 404 — a fetch() call has no
    // browser navigation to redirect, and the existing "don't confirm the
    // route exists" contract still applies to machine callers.
    let (api_status, _) = http_get(&info.host, info.port, "/api/status", None);
    assert_eq!(api_status, 404, "unauthenticated API route must stay 404");

    // /login itself stays open — required so the redirect target is
    // actually reachable.
    let (login_status, _) = http_get(&info.host, info.port, "/login", None);
    assert_eq!(login_status, 200);
}
