//! Minimal MCP server over stdio (newline-delimited JSON-RPC 2.0).
//! Exposes the single tool `mdview_view_file` (PRD §5.5). Hand-rolled to avoid
//! a heavy SDK dependency; the protocol surface here is intentionally small.

use crate::runtime;
use anyhow::Result;
use mdview_core::config::registry_db_path;
use mdview_core::{Config, Engine, SqliteStore};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::Path;

const PROTOCOL_VERSION: &str = "2024-11-05";

pub fn run() -> Result<()> {
    let engine = Engine::new(SqliteStore::open(&registry_db_path())?, Config::load());
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
            "initialize" => Some(ok(id, json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "mdview", "version": env!("CARGO_PKG_VERSION") }
            }))),
            "tools/list" => Some(ok(id, json!({ "tools": [tool_schema()] }))),
            "tools/call" => Some(handle_tool_call(id, &engine, &req)),
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
Auto-registers the project on first use and indexes the file immediately. \
Pass the project root and the file path relative to that root.",
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

fn handle_tool_call(id: Option<Value>, engine: &Engine, req: &Value) -> Value {
    let args = req.get("params").and_then(|p| p.get("arguments")).cloned().unwrap_or(json!({}));
    let name = req
        .get("params")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("");
    if name != "mdview_view_file" {
        return err(id, -32602, "unknown tool");
    }
    let root = args.get("project_root").and_then(|v| v.as_str()).unwrap_or("");
    let rel = args.get("relative_path").and_then(|v| v.as_str()).unwrap_or("");
    if root.is_empty() || rel.is_empty() {
        return tool_error(id, "project_root and relative_path are required");
    }

    match engine.view_file(Path::new(root), rel) {
        Ok(vf) => {
            // Ensure a daemon is up so the URL is actually viewable.
            let base = runtime::ensure_daemon_base();
            let full = format!("{base}{}", vf.url);
            let text = format!("Viewable at: {full}\nproject_id: {}", vf.project_id);
            ok(id, json!({
                "content": [{ "type": "text", "text": text }],
                "structuredContent": { "url": full, "path": vf.url, "project_id": vf.project_id }
            }))
        }
        Err(e) => tool_error(id, &format!("view_file failed: {e}")),
    }
}

fn ok(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}
fn err(id: Option<Value>, code: i64, msg: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": msg } })
}
/// Tool-level error: reported inside a successful result with isError=true (MCP convention).
fn tool_error(id: Option<Value>, msg: &str) -> Value {
    ok(id, json!({ "content": [{ "type": "text", "text": msg }], "isError": true }))
}
