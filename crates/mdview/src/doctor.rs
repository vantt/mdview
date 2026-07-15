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
    checks.push(check_agent_instruction(dry_run, fix));

    if as_json {
        let arr: Vec<Value> = checks
            .iter()
            .map(|c| json!({ "check": c.name, "status": c.status.label(), "detail": c.detail }))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "checks": arr }))?
        );
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
            std::env::split_paths(&paths)
                .any(|d| d.join("mdview").exists() || d.join("mdview.exe").exists())
        })
        .unwrap_or(false);
    if found {
        Check {
            name: "binary in PATH".into(),
            status: Status::Ok,
            detail: "mdview found on PATH".into(),
        }
    } else {
        let exe = std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
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
        Check {
            name: "config".into(),
            status: Status::Ok,
            detail: path.display().to_string(),
        }
    } else if dry_run {
        Check {
            name: "config".into(),
            status: Status::Manual,
            detail: format!("missing: {} (would create default)", path.display()),
        }
    } else {
        match Config::default().save() {
            Ok(_) => Check {
                name: "config".into(),
                status: Status::Fixed,
                detail: format!("created default {}", path.display()),
            },
            Err(e) => Check {
                name: "config".into(),
                status: Status::Manual,
                detail: format!("could not create config: {e}"),
            },
        }
    }
}

fn check_daemon() -> Check {
    match runtime::running_daemon() {
        Some(info) => Check {
            name: "daemon".into(),
            status: Status::Ok,
            detail: format!(
                "running on http://{}:{} (pid {})",
                info.host, info.port, info.pid
            ),
        },
        None => Check {
            name: "daemon".into(),
            status: Status::Warn,
            detail: "not running — start with `mdview serve`".into(),
        },
    }
}

fn claude_config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude.json")
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
        return Check {
            name: "MCP registration".into(),
            status: Status::Ok,
            detail: format!("mdview registered in {}", path.display()),
        };
    }
    if dry_run || !fix {
        return Check {
            name: "MCP registration".into(),
            status: Status::Manual,
            detail: format!(
                "not registered in {} — run `mdview doctor --fix`",
                path.display()
            ),
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
    match serde_json::to_vec_pretty(&root)
        .map_err(anyhow::Error::from)
        .and_then(|b| config::write_atomic(&path, &b).map_err(anyhow::Error::from))
    {
        Ok(_) => Check {
            name: "MCP registration".into(),
            status: Status::Fixed,
            detail: format!("registered mdview in {} (backup .bak)", path.display()),
        },
        Err(e) => Check {
            name: "MCP registration".into(),
            status: Status::Manual,
            detail: format!("write failed: {e}"),
        },
    }
}

/// Preamble + `---` + the actual agent-facing snippet; only the latter half
/// gets copied into AGENTS.md/CLAUDE.md.
const AGENT_TEMPLATE: &str = include_str!("../../../docs/mdview-agents-template.md");

fn agent_instruction_snippet() -> &'static str {
    AGENT_TEMPLATE
        .split_once("\n---\n")
        .map(|(_, snippet)| snippet.trim_start())
        .unwrap_or(AGENT_TEMPLATE)
}

/// Markers delimiting mdview's managed instruction block. Everything between
/// them is owned by `doctor --fix`; a sync replaces only that region and never
/// touches the user's own content around it.
const MDVIEW_START: &str = "<!-- mdview:START -->";
const MDVIEW_END: &str = "<!-- mdview:END -->";

/// The full managed block: the current snippet wrapped in the markers.
fn agent_block() -> String {
    format!(
        "{MDVIEW_START}\n{}\n{MDVIEW_END}\n",
        agent_instruction_snippet().trim_end()
    )
}

/// Byte range `[start, end)` of the managed block (markers included), or `None`
/// when a well-formed block (START before END) is not present.
fn managed_block_range(text: &str) -> Option<(usize, usize)> {
    let start = text.find(MDVIEW_START)?;
    let end = text[start..].find(MDVIEW_END)? + start + MDVIEW_END.len();
    Some((start, end))
}

/// True when the file already carries the current managed block verbatim, so a
/// `--fix` would be a no-op.
fn agent_block_in_sync(text: &str) -> bool {
    managed_block_range(text)
        .map(|(s, e)| text[s..e].trim_end() == agent_block().trim_end())
        .unwrap_or(false)
}

/// Upsert the managed block into `name`: replace the existing marker block in
/// place, or append a fresh one, creating the file if absent. Idempotent — the
/// user's surrounding content is preserved untouched.
fn write_agent_snippet(name: &str) -> Result<()> {
    let path = std::path::Path::new(name);
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let block = agent_block();
    let updated = if let Some((start, end)) = managed_block_range(&existing) {
        let mut out = String::with_capacity(existing.len());
        out.push_str(&existing[..start]);
        out.push_str(block.trim_end());
        out.push_str(&existing[end..]);
        out
    } else {
        let mut out = existing;
        if !out.is_empty() {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
        }
        out.push_str(&block);
        out
    };
    config::write_atomic(path, updated.as_bytes()).map_err(anyhow::Error::from)
}

fn check_agent_instruction(dry_run: bool, fix: bool) -> Check {
    const FILES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];

    let needs_sync: Vec<&str> = FILES
        .into_iter()
        .filter(|name| {
            !std::fs::read_to_string(name)
                .map(|t| agent_block_in_sync(&t))
                .unwrap_or(false)
        })
        .collect();

    if needs_sync.is_empty() {
        return Check {
            name: "agent instruction".into(),
            status: Status::Ok,
            detail: "AGENTS.md and CLAUDE.md carry the current MDView block".into(),
        };
    }

    if dry_run || !fix {
        return Check {
            name: "agent instruction".into(),
            status: Status::Manual,
            detail: format!(
                "missing or outdated MDView block in: {} (see `mdview` docs §5.7) — run `mdview doctor --fix`",
                needs_sync.join(", ")
            ),
        };
    }

    let mut fixed = Vec::new();
    let mut failed = Vec::new();
    for name in &needs_sync {
        match write_agent_snippet(name) {
            Ok(()) => fixed.push(*name),
            Err(e) => failed.push(format!("{name}: {e}")),
        }
    }
    if failed.is_empty() {
        Check {
            name: "agent instruction".into(),
            status: Status::Fixed,
            detail: format!(
                "synced MDView block (<!-- mdview:START/END -->) in {}",
                fixed.join(", ")
            ),
        }
    } else {
        Check {
            name: "agent instruction".into(),
            status: Status::Manual,
            detail: format!("write failed: {}", failed.join("; ")),
        }
    }
}

#[cfg(test)]
mod agent_instruction_tests {
    use super::*;

    fn tmp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "mdview-doctor-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn snippet_strips_preamble() {
        let s = agent_instruction_snippet();
        assert!(s.starts_with("## Documentation Viewing (MDView)"));
        assert!(!s.contains("Copy this snippet"));
    }

    #[test]
    fn block_sync_detection() {
        // A file that is exactly the managed block is in sync.
        assert!(agent_block_in_sync(&agent_block()));
        // No markers → not in sync.
        assert!(!agent_block_in_sync("nothing relevant here"));
        // Markers present but stale inner → not in sync.
        let stale = format!("{MDVIEW_START}\nold text\n{MDVIEW_END}\n");
        assert!(!agent_block_in_sync(&stale));
    }

    #[test]
    fn write_snippet_creates_missing_file() {
        let path = tmp_path("create");
        let name = path.to_str().unwrap();
        write_agent_snippet(name).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(MDVIEW_START) && text.contains(MDVIEW_END));
        assert!(agent_block_in_sync(&text));
        assert!(!path.with_extension("md.bak").exists());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn write_snippet_appends_preserving_existing() {
        let path = tmp_path("append");
        std::fs::write(&path, "# My project\n\nExisting content.\n").unwrap();
        let name = path.to_str().unwrap();
        write_agent_snippet(name).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("Existing content."));
        assert!(agent_block_in_sync(&text));
        // No backup clutter — the marker block makes re-runs safe on its own.
        assert!(!path.with_extension("md.bak").exists());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn write_snippet_replaces_block_in_place_idempotently() {
        let path = tmp_path("replace");
        // Pre-existing file with a STALE managed block plus user content around it.
        std::fs::write(
            &path,
            format!("# proj\n\n{MDVIEW_START}\nOLD STALE\n{MDVIEW_END}\n\ntail line\n"),
        )
        .unwrap();
        let name = path.to_str().unwrap();
        write_agent_snippet(name).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        // Stale inner replaced, surrounding user content preserved.
        assert!(!text.contains("OLD STALE"));
        assert!(agent_block_in_sync(&text));
        assert!(text.contains("# proj") && text.contains("tail line"));
        // Exactly one block — the region is replaced, never duplicated.
        assert_eq!(text.matches(MDVIEW_START).count(), 1);
        // A second run changes nothing (idempotent).
        write_agent_snippet(name).unwrap();
        assert_eq!(text, std::fs::read_to_string(&path).unwrap());
        std::fs::remove_file(&path).ok();
    }
}
