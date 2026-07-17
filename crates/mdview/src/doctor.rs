//! `mdview doctor` — diagnose & auto-fix integration (PRD FR-33). Idempotent.

use crate::runtime;
use anyhow::Result;
use mdview_core::config::{self, Config};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Status {
    Ok,
    Fixed,
    Manual,
    Warn,
    /// The target tool isn't installed on this machine, so nothing was written
    /// for it (never install blindly).
    Skip,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Ok => "OK",
            Status::Fixed => "FIXED",
            Status::Manual => "MANUAL",
            Status::Warn => "WARN",
            Status::Skip => "SKIP",
        }
    }
    fn mark(self) -> &'static str {
        match self {
            Status::Ok => "✓",
            Status::Fixed => "+",
            Status::Manual => "!",
            Status::Warn => "~",
            Status::Skip => "–",
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
    checks.push(check_mcp_claude(dry_run, fix));
    checks.push(check_mcp_codex(dry_run, fix));
    checks.push(check_mcp_antigravity(dry_run, fix));
    checks.push(check_agent_instruction(dry_run, fix));
    checks.push(check_skill(dry_run, fix));

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

/// A binary of this name is resolvable on `PATH` (with a `.exe` fallback for
/// Windows) — one of the "is this tool installed" signals.
fn bin_on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths)
                .any(|d| d.join(name).exists() || d.join(format!("{name}.exe")).exists())
        })
        .unwrap_or(false)
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn current_exe_str() -> String {
    std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "mdview".to_string())
}

/// The `<name>.bak` sibling of `path`, written before an in-place config edit.
fn backup_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".bak");
    path.with_file_name(name)
}

// ── Detection: only ever register for a tool that is actually installed ──

fn claude_present() -> bool {
    home().join(".claude.json").exists() || home().join(".claude").is_dir() || bin_on_path("claude")
}
fn codex_present() -> bool {
    home().join(".codex").is_dir() || bin_on_path("codex")
}
fn antigravity_present() -> bool {
    // Antigravity (IDE / CLI / 2.0) shares ~/.gemini/config for its MCP config.
    home().join(".gemini").join("config").is_dir() || bin_on_path("antigravity")
}

/// Register the `mdview` MCP server in a JSON config using the standard
/// `mcpServers` object — Claude Code's `~/.claude.json` and Antigravity's
/// `~/.gemini/config/mcp_config.json` share this shape. Merge-safe; backs up first.
fn register_json_mcp(path: &Path, dry_run: bool, fix: bool) -> (Status, String) {
    let mut root: Value = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}));

    let already = root
        .get("mcpServers")
        .and_then(|m| m.get("mdview"))
        .is_some();
    if already {
        return (Status::Ok, format!("registered in {}", path.display()));
    }
    if dry_run || !fix {
        return (
            Status::Manual,
            format!(
                "not registered in {} — run `mdview doctor --fix`",
                path.display()
            ),
        );
    }

    if !root.is_object() {
        root = json!({});
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if path.exists() {
        let _ = std::fs::copy(path, backup_path(path));
    }
    let servers = root
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert_with(|| json!({}));
    if let Some(obj) = servers.as_object_mut() {
        obj.insert(
            "mdview".to_string(),
            json!({ "command": current_exe_str(), "args": ["mcp"] }),
        );
    }
    match serde_json::to_vec_pretty(&root)
        .map_err(anyhow::Error::from)
        .and_then(|b| config::write_atomic(path, &b).map_err(anyhow::Error::from))
    {
        Ok(_) => (
            Status::Fixed,
            format!("registered in {} (backup .bak)", path.display()),
        ),
        Err(e) => (Status::Manual, format!("write failed: {e}")),
    }
}

/// Register the `mdview` MCP server in a TOML config using `[mcp_servers.<name>]`
/// — Codex's `~/.codex/config.toml`. Uses `toml_edit` so the user's existing
/// settings and comments survive; a malformed file is left untouched, never clobbered.
fn register_toml_mcp(path: &Path, dry_run: bool, fix: bool) -> (Status, String) {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let parsed = existing.parse::<toml_edit::DocumentMut>();

    let already = parsed
        .as_ref()
        .ok()
        .and_then(|d| d.get("mcp_servers").and_then(|m| m.get("mdview")))
        .is_some();
    if already {
        return (Status::Ok, format!("registered in {}", path.display()));
    }
    if dry_run || !fix {
        return (
            Status::Manual,
            format!(
                "not registered in {} — run `mdview doctor --fix`",
                path.display()
            ),
        );
    }

    let mut doc = match parsed {
        Ok(d) => d,
        Err(_) if existing.trim().is_empty() => toml_edit::DocumentMut::new(),
        Err(e) => {
            return (
                Status::Warn,
                format!("{} is not valid TOML ({e}); left unchanged", path.display()),
            );
        }
    };

    // Build explicit (non-inline) tables so this renders as `[mcp_servers.mdview]`.
    // An inline `mcp_servers = { ... }` would clash with a later
    // `[mcp_servers.other]` section (a TOML duplicate-key error).
    use toml_edit::{Array, Item, Table};
    if !doc.contains_key("mcp_servers") {
        let mut parent = Table::new();
        parent.set_implicit(true); // omit the bare `[mcp_servers]` header
        doc.insert("mcp_servers", Item::Table(parent));
    }
    let mut server = Table::new();
    server.insert("command", toml_edit::value(current_exe_str()));
    let mut args = Array::new();
    args.push("mcp");
    server.insert("args", toml_edit::value(args));
    doc["mcp_servers"]["mdview"] = Item::Table(server);

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if path.exists() {
        let _ = std::fs::copy(path, backup_path(path));
    }
    match config::write_atomic(path, doc.to_string().as_bytes()) {
        Ok(_) => (
            Status::Fixed,
            format!("registered in {} (backup .bak)", path.display()),
        ),
        Err(e) => (Status::Manual, format!("write failed: {e}")),
    }
}

fn skipped(name: &str, tool: &str) -> Check {
    Check {
        name: name.to_string(),
        status: Status::Skip,
        detail: format!("{tool} not detected — skipped"),
    }
}

fn check_mcp_claude(dry_run: bool, fix: bool) -> Check {
    let name = "MCP · Claude Code";
    if !claude_present() {
        return skipped(name, "Claude Code");
    }
    let (status, detail) = register_json_mcp(&claude_config_path(), dry_run, fix);
    Check {
        name: name.into(),
        status,
        detail,
    }
}

fn check_mcp_codex(dry_run: bool, fix: bool) -> Check {
    let name = "MCP · Codex";
    if !codex_present() {
        return skipped(name, "Codex");
    }
    let path = home().join(".codex").join("config.toml");
    let (status, detail) = register_toml_mcp(&path, dry_run, fix);
    Check {
        name: name.into(),
        status,
        detail,
    }
}

fn check_mcp_antigravity(dry_run: bool, fix: bool) -> Check {
    let name = "MCP · Antigravity";
    if !antigravity_present() {
        return skipped(name, "Antigravity");
    }
    let path = home()
        .join(".gemini")
        .join("config")
        .join("mcp_config.json");
    let (status, detail) = register_json_mcp(&path, dry_run, fix);
    Check {
        name: name.into(),
        status,
        detail,
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

/// The mdview Claude Code skill (`/mdview <path>`), installed globally so it
/// works in any project. mdview owns this file entirely, so the check is a
/// whole-file content match rather than a shared marker block.
const SKILL_TEMPLATE: &str = include_str!("../../../docs/mdview-skill-template.md");

/// `~/.claude/skills/mdview/SKILL.md` — the global (not per-project) skill file.
fn skill_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude/skills/mdview/SKILL.md")
}

fn check_skill(dry_run: bool, fix: bool) -> Check {
    // The skill lives under ~/.claude/skills — only relevant to Claude Code.
    if !claude_present() {
        return skipped("Claude skill", "Claude Code");
    }
    check_skill_at(&skill_path(), dry_run, fix)
}

/// Install/verify the global mdview skill at `path`. Split from `check_skill` so
/// the write/idempotency logic is testable without touching the real HOME.
fn check_skill_at(path: &std::path::Path, dry_run: bool, fix: bool) -> Check {
    let in_sync = std::fs::read_to_string(path)
        .map(|t| t == SKILL_TEMPLATE)
        .unwrap_or(false);
    if in_sync {
        return Check {
            name: "skill".into(),
            status: Status::Ok,
            detail: "global /mdview skill is installed and current".into(),
        };
    }
    if dry_run || !fix {
        return Check {
            name: "skill".into(),
            status: Status::Manual,
            detail: format!(
                "global /mdview skill missing or outdated at {} — run `mdview doctor --fix`",
                path.display()
            ),
        };
    }
    let write = || -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        config::write_atomic(path, SKILL_TEMPLATE.as_bytes()).map_err(anyhow::Error::from)
    };
    match write() {
        Ok(()) => Check {
            name: "skill".into(),
            status: Status::Fixed,
            detail: format!("installed global /mdview skill at {}", path.display()),
        },
        Err(e) => Check {
            name: "skill".into(),
            status: Status::Manual,
            detail: format!("write failed: {e}"),
        },
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
    fn skill_template_is_a_valid_skill_file() {
        assert!(SKILL_TEMPLATE.starts_with("---"));
        assert!(SKILL_TEMPLATE.contains("name: mdview"));
        // Carries the CLI-vs-MCP guidance the feature is about.
        assert!(SKILL_TEMPLATE.contains("mdview_view_file"));
        assert!(SKILL_TEMPLATE.contains("mdview open"));
    }

    #[test]
    fn skill_installs_then_reports_in_sync_idempotently() {
        let base = tmp_path("skill");
        let path = base.join("skills/mdview/SKILL.md");
        // Missing → Manual on a dry run (no write).
        assert!(matches!(
            check_skill_at(&path, true, false).status,
            Status::Manual
        ));
        assert!(!path.exists());
        // --fix installs the file with the template content verbatim.
        assert!(matches!(
            check_skill_at(&path, false, true).status,
            Status::Fixed
        ));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), SKILL_TEMPLATE);
        // Re-running is a no-op: the file is already in sync.
        assert!(matches!(
            check_skill_at(&path, false, true).status,
            Status::Ok
        ));
        std::fs::remove_dir_all(&base).ok();
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

#[cfg(test)]
mod mcp_register_tests {
    use super::*;

    fn tmp(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "mdview-mcp-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn json_registers_preserving_others_and_is_idempotent() {
        let p = tmp("json");
        std::fs::write(&p, r#"{"mcpServers":{"other":{"command":"x"}},"foo":1}"#).unwrap();
        let (s, _) = register_json_mcp(&p, false, true);
        assert_eq!(s, Status::Fixed);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["mdview"]["args"][0].as_str(), Some("mcp"));
        assert_eq!(v["mcpServers"]["other"]["command"], "x"); // untouched
        assert_eq!(v["foo"], 1); // unrelated key untouched
        assert!(backup_path(&p).exists());
        let (s2, _) = register_json_mcp(&p, false, true);
        assert_eq!(s2, Status::Ok); // idempotent
        std::fs::remove_file(&p).ok();
        std::fs::remove_file(backup_path(&p)).ok();
    }

    #[test]
    fn json_dry_run_writes_nothing() {
        let p = tmp("jsondry");
        let (s, _) = register_json_mcp(&p, true, true);
        assert_eq!(s, Status::Manual);
        assert!(!p.exists());
    }

    #[test]
    fn toml_registers_preserving_user_config_and_is_idempotent() {
        let p = tmp("toml");
        std::fs::write(
            &p,
            "# my codex config\nmodel = \"gpt-5\"\n\n[mcp_servers.other]\ncommand = \"x\"\n",
        )
        .unwrap();
        let (s, _) = register_toml_mcp(&p, false, true);
        assert_eq!(s, Status::Fixed);
        let out = std::fs::read_to_string(&p).unwrap();
        assert!(out.contains("# my codex config")); // comment survives
        let doc: toml_edit::DocumentMut = out.parse().unwrap();
        assert_eq!(doc["model"].as_str(), Some("gpt-5")); // user setting survives
        assert_eq!(doc["mcp_servers"]["other"]["command"].as_str(), Some("x")); // other server survives
        assert!(doc["mcp_servers"]["mdview"]["command"].is_str()); // mdview added
        assert_eq!(
            doc["mcp_servers"]["mdview"]["args"]
                .as_array()
                .and_then(|a| a.get(0))
                .and_then(|v| v.as_str()),
            Some("mcp")
        );
        assert!(backup_path(&p).exists());
        let (s2, _) = register_toml_mcp(&p, false, true);
        assert_eq!(s2, Status::Ok); // idempotent
        std::fs::remove_file(&p).ok();
        std::fs::remove_file(backup_path(&p)).ok();
    }

    #[test]
    fn toml_creates_section_form_when_empty() {
        let p = tmp("tomlempty");
        let (s, _) = register_toml_mcp(&p, false, true);
        assert_eq!(s, Status::Fixed);
        let out = std::fs::read_to_string(&p).unwrap();
        // Section form, not an inline `mcp_servers = { ... }` (which would clash
        // with a later `[mcp_servers.other]`).
        assert!(out.contains("[mcp_servers.mdview]"), "got:\n{out}");
        let doc: toml_edit::DocumentMut = out.parse().unwrap();
        assert!(doc["mcp_servers"]["mdview"]["command"].is_str());
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn toml_malformed_is_left_unchanged() {
        let p = tmp("tomlbad");
        std::fs::write(&p, "this is [[ not valid toml").unwrap();
        let before = std::fs::read_to_string(&p).unwrap();
        let (s, _) = register_toml_mcp(&p, false, true);
        assert_eq!(s, Status::Warn);
        assert_eq!(std::fs::read_to_string(&p).unwrap(), before); // never clobbered
        std::fs::remove_file(&p).ok();
    }
}
