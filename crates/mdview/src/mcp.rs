//! Minimal MCP server over stdio (newline-delimited JSON-RPC 2.0), kept for
//! clients without a shell (Claude Desktop). Exposes the single tool
//! `mdview_view_file` and is a thin proxy: each call runs `mdview view-file
//! --json` as a child process, so the engine and registry are never loaded here
//! and an `mdview update` takes effect on the very next call without
//! reconnecting. Hand-rolled to avoid a heavy SDK dependency.

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::Path;
use std::process::{Command, Stdio};

const PROTOCOL_VERSION: &str = "2024-11-05";

pub fn run() -> Result<()> {
    // Resolved once, before any update can replace the binary: afterwards Linux
    // reports a deleted inode for /proc/self/exe and Windows the parked file.
    let exe = std::env::current_exe().context("locating the mdview binary")?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        // Notifications have no id and expect no response.
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let response = match method {
            "initialize" => Some(ok(
                id,
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "mdview", "version": env!("CARGO_PKG_VERSION") }
                }),
            )),
            "tools/list" => Some(ok(id, json!({ "tools": [tool_schema()] }))),
            "tools/call" => Some(handle_tool_call(id, &exe, &req)),
            "ping" => Some(ok(id, json!({}))),
            _ if id.is_some() => Some(err(id, -32601, "method not found")),
            _ => None, // notification
        };

        if let Some(resp) = response {
            writeln!(stdout, "{resp}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

fn tool_schema() -> Value {
    json!({
        "name": "mdview_view_file",
        "description": "Make a markdown file viewable in the browser and return its URL. \
    Auto-registers the project on first use. Pass the project root and the file path relative to that root.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "project_root": { "type": "string", "description": "Absolute path to the project root" },
                "relative_path": { "type": "string", "description": "Markdown file path relative to project_root" }
            },
            "required": ["project_root", "relative_path"]
        }
    })
}

fn handle_tool_call(id: Option<Value>, exe: &Path, req: &Value) -> Value {
    let args = req
        .get("params")
        .and_then(|p| p.get("arguments"))
        .cloned()
        .unwrap_or(json!({}));
    let name = req
        .get("params")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("");
    if name != "mdview_view_file" {
        return err(id, -32602, "unknown tool");
    }
    let root = args
        .get("project_root")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let rel = args
        .get("relative_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if root.is_empty() || rel.is_empty() {
        return tool_error(id, "project_root and relative_path are required");
    }

    match run_view_file(exe, root, rel).and_then(|stdout| view_result(&stdout)) {
        Ok(result) => ok(id, result),
        Err(msg) => tool_error(id, &msg),
    }
}

/// Run `<exe> view-file --json <root> <rel>` and return its stdout. A non-zero
/// exit is reported with the child's stderr, which carries the real cause.
fn run_view_file(exe: &Path, root: &str, rel: &str) -> Result<String, String> {
    let mut cmd = Command::new(exe);
    // `--` keeps a path that starts with `-` from being read as a flag.
    cmd.args(["view-file", "--json", "--", root, rel])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("view_file failed: could not run {}: {e}", exe.display()))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let detail = stderr.trim();
        return Err(format!(
            "view_file failed: {}",
            if detail.is_empty() {
                format!("mdview exited with {}", out.status)
            } else {
                detail.to_string()
            }
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// What `mdview view-file --json` prints.
#[derive(Deserialize)]
struct ViewJson {
    urls: Vec<String>,
    #[serde(default)]
    long_urls: Vec<String>,
    path: String,
    code: String,
    project_id: String,
    rel_path: String,
}

/// Map the child's JSON to the MCP tool result (`content` + `structuredContent`).
/// Pure so the mapping is testable without spawning anything.
fn view_result(stdout: &str) -> Result<Value, String> {
    let v: ViewJson = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("view_file failed: unexpected output from mdview ({e})"))?;
    // Primary URL kept for back-compat with clients reading `url`.
    let primary = v.urls.first().cloned().unwrap_or_default();
    let text = viewable_text(&v.urls, &v.rel_path, &v.project_id);
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": {
            "url": primary,
            "urls": v.urls,
            "long_url": v.long_urls.first().cloned().unwrap_or_default(),
            "long_urls": v.long_urls,
            "path": v.path,
            "code": v.code,
            "project_id": v.project_id
        }
    }))
}

/// The human-readable half of the tool result.
///
/// Pure on purpose: the caller resolves the daemon's base URLs (which starts a
/// daemon), so keeping the formatting separate is what makes this behaviour
/// testable at all.
///
/// Each URL is a markdown link on its own line: Claude Desktop renders that as
/// clickable, while a bare URL after an arrow or one in backticks is not
/// reliably linked.
///
/// The file's path rides along as the link text next to the short link, because
/// the link itself is opaque — without it, a transcript full of `/s/…` codes
/// tells a reader nothing about which document each one was.
fn viewable_text(urls: &[String], rel_path: &str, project_id: &str) -> String {
    let viewable = if urls.len() > 1 {
        let lines = urls
            .iter()
            .map(|u| format!("[{rel_path}]({u})"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("Viewable at (pick a reachable IP):\n{lines}")
    } else {
        let primary = urls.first().map(String::as_str).unwrap_or_default();
        format!("Viewable at:\n[{rel_path}]({primary})")
    };
    format!("{viewable}\nproject_id: {project_id}")
}

fn ok(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}
fn err(id: Option<Value>, code: i64, msg: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": msg } })
}
/// Tool-level error: reported inside a successful result with isError=true (MCP convention).
fn tool_error(id: Option<Value>, msg: &str) -> Value {
    ok(
        id,
        json!({ "content": [{ "type": "text", "text": msg }], "isError": true }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_base_renders_a_single_line() {
        let text = viewable_text(
            &["http://design-lap:7700/s/a3f9c1d20b74".into()],
            "docs/history/short-link/DISCUSSION.md",
            "mdview",
        );
        assert_eq!(
            text,
            "Viewable at:\n[docs/history/short-link/DISCUSSION.md]\
             (http://design-lap:7700/s/a3f9c1d20b74)\nproject_id: mdview"
        );
        assert_eq!(text.lines().count(), 3);
    }

    #[test]
    fn several_bases_render_one_line_each() {
        let text = viewable_text(
            &[
                "http://192.168.1.10:7700/s/a3f9c1d20b74".into(),
                "http://10.0.0.5:7700/s/a3f9c1d20b74".into(),
            ],
            "docs/a.md",
            "mdview",
        );
        assert!(text.contains("pick a reachable IP"));
        assert!(text.contains("\n[docs/a.md](http://192.168.1.10:7700/s/a3f9c1d20b74)\n"));
        assert!(text.contains("\n[docs/a.md](http://10.0.0.5:7700/s/a3f9c1d20b74)\n"));
    }

    /// The whole point of the feature: the emitted line has to stay inside a
    /// terminal width, which the full path did not.
    #[test]
    fn the_short_line_fits_in_a_terminal() {
        let deep = "docs/history/short-link-for-file-urls/DISCUSSION.md";
        let text = viewable_text(
            &["http://design-lap:7700/s/a3f9c1d20b74".into()],
            deep,
            "mdview",
        );
        let url_line = text.lines().nth(1).unwrap();
        let url = url_line.rsplit('(').next().unwrap().trim_end_matches(')');
        assert!(url.len() <= 40, "short url grew to {}: {url}", url.len());
    }

    #[test]
    fn child_json_maps_to_the_tool_result() {
        let stdout = r#"{"url":"http://h:7700/s/abc","urls":["http://h:7700/s/abc"],
            "long_url":"http://h:7700/p/mdview/a.md","long_urls":["http://h:7700/p/mdview/a.md"],
            "path":"/p/mdview/a.md","code":"abc","project_id":"mdview","rel_path":"a.md"}"#;
        let r = view_result(stdout).unwrap();
        assert_eq!(
            r["content"][0]["text"],
            "Viewable at:\n[a.md](http://h:7700/s/abc)\nproject_id: mdview"
        );
        let s = &r["structuredContent"];
        assert_eq!(s["url"], "http://h:7700/s/abc");
        assert_eq!(s["urls"], json!(["http://h:7700/s/abc"]));
        assert_eq!(s["long_url"], "http://h:7700/p/mdview/a.md");
        assert_eq!(s["long_urls"], json!(["http://h:7700/p/mdview/a.md"]));
        assert_eq!(s["path"], "/p/mdview/a.md");
        assert_eq!(s["code"], "abc");
        assert_eq!(s["project_id"], "mdview");
    }

    #[test]
    fn several_urls_keep_the_first_as_primary() {
        let stdout = r#"{"urls":["http://a/s/x","http://b/s/x"],"long_urls":["http://a/p","http://b/p"],
            "path":"/p","code":"x","project_id":"p","rel_path":"d.md"}"#;
        let r = view_result(stdout).unwrap();
        assert_eq!(r["structuredContent"]["url"], "http://a/s/x");
        assert_eq!(r["structuredContent"]["long_url"], "http://a/p");
        assert!(r["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("pick a reachable IP"));
    }

    #[test]
    fn bad_child_output_is_an_error_not_a_panic() {
        assert!(view_result("not json").is_err());
        assert!(view_result("{}").is_err());
        assert!(view_result("").is_err());
    }
}
