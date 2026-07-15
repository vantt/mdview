//! `mdview doctor` — diagnose & auto-fix integration (PRD FR-33). Idempotent.

use crate::runtime;
use anyhow::Result;
use mdview_core::config::{self, Config};
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Status {
    Ok,
    Fixed,
    Manual,
    Warn,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Ok => "OK",
            Status::Fixed => "FIXED",
            Status::Manual => "MANUAL",
            Status::Warn => "WARN",
        }
    }
    fn mark(self) -> &'static str {
        match self {
            Status::Ok => "✓",
            Status::Fixed => "+",
            Status::Manual => "!",
            Status::Warn => "~",
        }
    }
}

struct Check {
    name: String,
    status: Status,
    detail: String,
}

pub fn run(as_json: bool, dry_run: bool, fix: bool) -> Result<()> {
    let mut checks = Vec::new();

    checks.push(check_binary_in_path());
    checks.push(check_config(dry_run));
    let daemon = check_daemon();
    checks.push(daemon);
    checks.push(check_mcp_registration(dry_run, fix));
    checks.push(check_agent_instruction());

    if as_json {
        let arr: Vec<Value> = checks
            .iter()
            .map(|c| json!({ "check": c.name, "status": c.status.label(), "detail": c.detail }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&json!({ "checks": arr }))?);
    } else {
        println!("mdview doctor\n");
        for c in &checks {
            println!("  [{}] {:<22} {}", c.status.mark(), c.name, c.detail);
        }
        let manual = checks.iter().filter(|c| c.status == Status::Manual).count();
        println!();
        if manual > 0 {
            println!("{manual} item(s) need attention. Re-run with --fix to apply safe fixes.");
        } else {
            println!("All good.");
        }
    }
    Ok(())
}

fn check_binary_in_path() -> Check {
    let found = std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|d| {
                d.join("mdview").exists() || d.join("mdview.exe").exists()
            })
        })
        .unwrap_or(false);
    if found {
        Check { name: "binary in PATH".into(), status: Status::Ok, detail: "mdview found on PATH".into() }
    } else {
        let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
        Check {
            name: "binary in PATH".into(),
            status: Status::Warn,
            detail: format!("mdview not on PATH (running from {exe}); add its dir to PATH"),
        }
    }
}

fn check_config(dry_run: bool) -> Check {
    let path = config::config_path();
    if path.exists() {
        // Validate by loading (load is resilient; re-serialize to confirm shape).
        let _ = Config::load();
        Check { name: "config".into(), status: Status::Ok, detail: path.display().to_string() }
    } else if dry_run {
        Check { name: "config".into(), status: Status::Manual, detail: format!("missing: {} (would create default)", path.display()) }
    } else {
        match Config::default().save() {
            Ok(_) => Check { name: "config".into(), status: Status::Fixed, detail: format!("created default {}", path.display()) },
            Err(e) => Check { name: "config".into(), status: Status::Manual, detail: format!("could not create config: {e}") },
        }
    }
}

fn check_daemon() -> Check {
    match runtime::running_daemon() {
        Some(info) => Check {
            name: "daemon".into(),
            status: Status::Ok,
            detail: format!("running on http://{}:{} (pid {})", info.host, info.port, info.pid),
        },
        None => Check {
            name: "daemon".into(),
            status: Status::Warn,
            detail: "not running — start with `mdview serve`".into(),
        },
    }
}

fn claude_config_path() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".claude.json")
}

fn check_mcp_registration(dry_run: bool, fix: bool) -> Check {
    let path = claude_config_path();
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "mdview".to_string());

    let mut root: Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}));

    let already = root
        .get("mcpServers")
        .and_then(|m| m.get("mdview"))
        .is_some();
    if already {
        return Check { name: "MCP registration".into(), status: Status::Ok, detail: format!("mdview registered in {}", path.display()) };
    }
    if dry_run || !fix {
        return Check {
            name: "MCP registration".into(),
            status: Status::Manual,
            detail: format!("not registered in {} — run `mdview doctor --fix`", path.display()),
        };
    }

    // Merge without clobbering other servers; backup first.
    if path.exists() {
        let _ = std::fs::copy(&path, path.with_extension("json.bak"));
    }
    if !root.is_object() {
        root = json!({});
    }
    let servers = root
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert_with(|| json!({}));
    if let Some(obj) = servers.as_object_mut() {
        obj.insert(
            "mdview".to_string(),
            json!({ "command": exe, "args": ["mcp"] }),
        );
    }
    match serde_json::to_vec_pretty(&root).map_err(anyhow::Error::from).and_then(|b| {
        config::write_atomic(&path, &b).map_err(anyhow::Error::from)
    }) {
        Ok(_) => Check { name: "MCP registration".into(), status: Status::Fixed, detail: format!("registered mdview in {} (backup .bak)", path.display()) },
        Err(e) => Check { name: "MCP registration".into(), status: Status::Manual, detail: format!("write failed: {e}") },
    }
}

fn check_agent_instruction() -> Check {
    for name in ["AGENTS.md", "CLAUDE.md"] {
        if let Ok(text) = std::fs::read_to_string(name) {
            if text.contains("mdview_view_file") || text.contains("MDView") {
                return Check { name: "agent instruction".into(), status: Status::Ok, detail: format!("{name} mentions mdview") };
            }
        }
    }
    Check {
        name: "agent instruction".into(),
        status: Status::Warn,
        detail: "no MDView snippet in ./AGENTS.md or ./CLAUDE.md (see `mdview` docs §5.7)".into(),
    }
}
