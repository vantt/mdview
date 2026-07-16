//! CLI surface (PRD §5.6). Registry commands operate on the shared SQLite DB;
//! the running daemon serves from the same DB. `serve`/`mcp` are the long-running
//! modes.

use crate::runtime;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use mdview_core::indexer;
use mdview_core::Config;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "mdview",
    version,
    about = "Multi-project markdown viewer for AI agent workflows"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the daemon (HTTP server + file watcher + live reload).
    Serve {
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        host: Option<String>,
    },
    /// Register a project (recursive scan + index).
    Register {
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Get the browser URL for a markdown file (indexing it if needed).
    Open {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// List registered projects.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Full-text search across projects.
    Search {
        query: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Show daemon status.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Re-scan a project (or all) to reconcile the index.
    Refresh { project: Option<String> },
    /// Remove a project from the registry (files are not deleted).
    Unregister { project_id: String },
    /// Stop the running daemon.
    Stop,
    /// Restart the daemon: stop the running one (if any), then start a fresh
    /// detached daemon. Useful after changing config (host, port, theme).
    Restart,
    /// Diagnose & auto-fix integration with Claude Code.
    Doctor {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        fix: bool,
    },
    /// Run the MCP server over stdio (used by Claude Code).
    Mcp,
    /// Edit configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Print the version (same value as `--version`).
    Version,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Open the config file (`~/.mdview/config.toml`) in $EDITOR.
    Edit,
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Serve { port, host } => cmd_serve(port, host),
        Command::Register { path, name, json } => cmd_register(&path, name.as_deref(), json),
        Command::Open { path, json } => cmd_open(&path, json),
        Command::List { json } => cmd_list(json),
        Command::Search {
            query,
            project,
            limit,
            json,
        } => cmd_search(&query, project.as_deref(), limit, json),
        Command::Status { json } => cmd_status(json),
        Command::Refresh { project } => cmd_refresh(project.as_deref()),
        Command::Unregister { project_id } => cmd_unregister(&project_id),
        Command::Stop => cmd_stop(),
        Command::Restart => cmd_restart(),
        Command::Doctor { json, dry_run, fix } => crate::doctor::run(json, dry_run, fix),
        Command::Mcp => crate::mcp::run(),
        Command::Config { action } => match action {
            ConfigAction::Edit => cmd_config_edit(),
        },
        Command::Version => {
            // Single source of truth: the workspace Cargo package version.
            println!("mdview {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

/// Resolved editor to spawn: the program name, its leading args (from
/// splitting a "program --flag"-shaped editor string), and the raw combined
/// string (unsplit, for error messages).
struct ResolvedEditor {
    display: String,
    program: String,
    args: Vec<String>,
}

/// Resolve which editor to spawn: prefer `visual` (`$VISUAL`), then `editor`
/// (`$EDITOR`), else a platform default (`notepad` on Windows, `vi`
/// elsewhere). Editors passed with args (e.g. "code --wait") are split on
/// whitespace; the config path is appended separately by the caller.
fn resolve_editor(visual: Option<String>, editor: Option<String>) -> ResolvedEditor {
    let editor = visual
        .or(editor)
        .unwrap_or_else(|| if cfg!(windows) { "notepad" } else { "vi" }.to_string());
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or("vi").to_string();
    let args: Vec<String> = parts.map(str::to_string).collect();
    ResolvedEditor {
        display: editor,
        program,
        args,
    }
}

/// Classify the post-edit re-read of the config file into the message
/// `cmd_config_edit` prints: valid TOML (saved), invalid TOML (warning with
/// the parse error), or an unreadable file (re-read error).
fn classify_config_edit_outcome(path: &Path, read_result: std::io::Result<String>) -> String {
    match read_result {
        Ok(text) => match toml::from_str::<Config>(&text) {
            Ok(_) => format!(
                "Saved {}.\nRestart the daemon to apply: mdview restart",
                path.display()
            ),
            Err(e) => format!(
                "Warning: {} is not valid TOML ({e}).\nmdview will ignore it and fall back to defaults until you fix it.",
                path.display()
            ),
        },
        Err(e) => format!("Could not re-read {}: {e}", path.display()),
    }
}

fn cmd_config_edit() -> Result<()> {
    let path = mdview_core::config::config_path();
    // Materialize the file with current (or default) values so the editor opens
    // a fully-populated config, not an empty/absent file.
    Config::load()
        .save()
        .with_context(|| format!("writing {}", path.display()))?;

    let resolved = resolve_editor(std::env::var("VISUAL").ok(), std::env::var("EDITOR").ok());
    let status = std::process::Command::new(&resolved.program)
        .args(&resolved.args)
        .arg(&path)
        .status()
        .with_context(|| format!("launching editor '{}'", resolved.display))?;
    if !status.success() {
        println!("Editor exited without success; config left unchanged on disk.");
        return Ok(());
    }

    // Validate what the user saved: a broken TOML would otherwise be silently
    // ignored (Config::load falls back to defaults), so warn loudly instead.
    println!(
        "{}",
        classify_config_edit_outcome(&path, std::fs::read_to_string(&path))
    );
    Ok(())
}

fn cmd_serve(port: Option<u16>, host: Option<String>) -> Result<()> {
    // Apply overrides by persisting to config before the daemon reads it.
    if port.is_some() || host.is_some() {
        let mut cfg = mdview_core::Config::load();
        if let Some(p) = port {
            cfg.server.port = p;
        }
        if let Some(h) = host {
            cfg.server.host = h;
        }
        cfg.save().ok();
    }
    if runtime::running_daemon().is_some() {
        println!("A mdview daemon is already running.");
        return Ok(());
    }
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(crate::server::serve())
}

fn cmd_register(path: &Path, name: Option<&str>, json: bool) -> Result<()> {
    let engine = runtime::build_engine()?;
    let project = engine.register(path, name)?;
    let count = engine.file_count(&project.id)?;
    if json {
        println!(
            "{}",
            serde_json::json!({
                "project_id": project.id, "name": project.name,
                "root_path": project.root_path, "file_count": count,
                "url": format!("/p/{}/", project.id)
            })
        );
    } else {
        println!(
            "Registered '{}' ({}) — {} markdown files",
            project.name, project.id, count
        );
        println!("  {}", project.root_path.display());
    }
    Ok(())
}

fn cmd_open(path: &Path, json: bool) -> Result<()> {
    let abs =
        std::fs::canonicalize(path).with_context(|| format!("no such file: {}", path.display()))?;
    let engine = runtime::build_engine()?;
    let root = find_project_root(&engine, &abs);
    let rel = indexer::rel_path_str(&root, &abs);
    let vf = engine.view_file(&root, &rel)?;
    let urls: Vec<String> = runtime::ensure_daemon_bases()
        .iter()
        .map(|base| format!("{base}{}", vf.url))
        .collect();
    if json {
        println!("{}", open_json(&urls, &vf.project_id));
    } else if urls.len() > 1 {
        println!("{}", format_url_choices(&urls));
    } else {
        println!("{}", urls.first().cloned().unwrap_or_default());
    }
    Ok(())
}

/// Pure JSON-shape builder for `cmd_open --json` (unit-tested without a live
/// daemon): `url` stays the primary (first) URL for back-compat, `urls` is
/// the full list mirroring the MCP `structuredContent` contract
/// (decision `d88c028b`).
fn open_json(urls: &[String], project_id: &str) -> serde_json::Value {
    let primary = urls.first().cloned().unwrap_or_default();
    serde_json::json!({ "url": primary, "urls": urls, "project_id": project_id })
}

/// Multi-line "pick a reachable IP" framing for text-mode output when
/// `ensure_daemon_bases()` returns more than one URL, matching the format
/// `mcp.rs::handle_tool_call` already uses (D3: CLI/MCP text parity).
fn format_url_choices(urls: &[String]) -> String {
    let lines = urls
        .iter()
        .map(|u| format!("  {u}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("Viewable at (pick a reachable IP):\n{lines}")
}

/// Find the registered project root containing `file`, else the nearest ancestor
/// with a project marker, else the file's parent directory.
fn find_project_root(engine: &mdview_core::Engine, file: &Path) -> PathBuf {
    if let Ok(projects) = engine.list_projects() {
        if let Some(p) = projects.iter().find(|p| file.starts_with(&p.root_path)) {
            return p.root_path.clone();
        }
    }
    const MARKERS: &[&str] = &[".mdview.json", ".git", "CLAUDE.md", "README.md"];
    let mut dir = file.parent();
    while let Some(d) = dir {
        if MARKERS.iter().any(|m| d.join(m).exists()) {
            return d.to_path_buf();
        }
        dir = d.parent();
    }
    file.parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn cmd_list(json: bool) -> Result<()> {
    let engine = runtime::build_engine()?;
    let projects = engine.list_projects()?;
    if json {
        let arr: Vec<_> = projects
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id, "name": p.name, "root_path": p.root_path,
                    "file_count": engine.file_count(&p.id).unwrap_or(0)
                })
            })
            .collect();
        println!("{}", serde_json::json!({ "projects": arr }));
    } else if projects.is_empty() {
        println!("No projects registered.");
    } else {
        for p in &projects {
            let c = engine.file_count(&p.id).unwrap_or(0);
            println!("{:<20} {:>5} files  {}", p.id, c, p.root_path.display());
        }
    }
    Ok(())
}

fn cmd_search(query: &str, project: Option<&str>, limit: usize, json: bool) -> Result<()> {
    let engine = runtime::build_engine()?;
    let results = engine.search(query, project, limit)?;
    if json {
        println!("{}", serde_json::json!({ "results": results }));
    } else if results.is_empty() {
        println!("No matches.");
    } else {
        for r in &results {
            println!("{}  {}\n  {}", r.title, r.url, r.excerpt.replace('\n', " "));
        }
    }
    Ok(())
}

fn cmd_status(json: bool) -> Result<()> {
    let engine = runtime::build_engine()?;
    let daemon = runtime::running_daemon();
    let projects = engine.list_projects()?;
    let files = engine.store.total_file_count()?;
    if json {
        println!(
            "{}",
            serde_json::json!({
                "running": daemon.is_some(),
                "server_url": daemon.as_ref().map(|d| format!("http://{}:{}", d.host, d.port)),
                "version": env!("CARGO_PKG_VERSION"),
                "project_count": projects.len(),
                "indexed_file_count": files,
            })
        );
    } else {
        match &daemon {
            Some(d) => println!("running: http://{}:{} (pid {})", d.host, d.port, d.pid),
            None => println!("running: no"),
        }
        println!("projects: {}  indexed files: {}", projects.len(), files);
    }
    Ok(())
}

fn cmd_refresh(project: Option<&str>) -> Result<()> {
    let engine = runtime::build_engine()?;
    match project {
        Some(id) => {
            let n = engine.refresh(id)?;
            println!("Reindexed {n} files in '{id}'.");
        }
        None => {
            for p in engine.list_projects()? {
                let n = engine.refresh(&p.id)?;
                println!("{}: {n} files", p.id);
            }
        }
    }
    Ok(())
}

fn cmd_unregister(id: &str) -> Result<()> {
    let engine = runtime::build_engine()?;
    engine.unregister(id)?;
    println!("Unregistered '{id}'.");
    Ok(())
}

/// Stop the daemon named by the lock file, if any. Removes the lock either way
/// (a failed kill means the process is already gone). Returns `(pid, killed_ok)`
/// when a lock existed, or `None` when no daemon was recorded.
fn stop_daemon() -> Option<(u32, bool)> {
    let info = runtime::read_lock()?;
    #[cfg(unix)]
    let ok = std::process::Command::new("kill")
        .arg(info.pid.to_string())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    #[cfg(not(unix))]
    let ok = std::process::Command::new("taskkill")
        .args(["/PID", &info.pid.to_string(), "/F"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    // Clear the lock unless the daemon is genuinely orphaned — a kill that
    // failed while the daemon still answers on its port. Deleting the lock in
    // that case would strand a live daemon that `stop`/`status` can no longer
    // reach and let `restart` spawn a second one. A failed kill on an already
    // dead process (health check fails) is a stale lock and is cleared.
    let orphaned = !ok && runtime::running_daemon().is_some();
    if !orphaned {
        runtime::remove_lock();
    }
    Some((info.pid, ok))
}

/// Map `stop_daemon()`'s outcome to `cmd_stop`'s exact printed message: a
/// successful kill, a failed kill (process likely already gone), or no
/// daemon recorded at all.
fn stop_outcome_message(outcome: Option<(u32, bool)>) -> String {
    match outcome {
        Some((pid, true)) => format!("Stopped daemon (pid {pid})."),
        Some((pid, false)) => format!("Could not stop pid {pid}. It may already be gone."),
        None => "No daemon running.".to_string(),
    }
}

/// Map `stop_daemon()`'s outcome to `cmd_restart`'s exact printed message
/// for the stop phase: a daemon was found and a stop was attempted (the
/// kill result itself doesn't change the message here, since restart
/// proceeds to spawn either way), or no daemon was running to begin with.
fn restart_stop_message(outcome: Option<(u32, bool)>) -> String {
    match outcome {
        Some((pid, _)) => format!("Stopped daemon (pid {pid})."),
        None => "No daemon was running.".to_string(),
    }
}

fn cmd_stop() -> Result<()> {
    println!("{}", stop_outcome_message(stop_daemon()));
    Ok(())
}

fn cmd_restart() -> Result<()> {
    println!("{}", restart_stop_message(stop_daemon()));
    // Wait for the old process to actually exit so the port and lock are free.
    for _ in 0..30 {
        if runtime::running_daemon().is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // Start a fresh daemon fully detached so it outlives this CLI invocation.
    runtime::spawn_daemon_detached()?;
    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Some(info) = runtime::running_daemon() {
            let primary = runtime::ensure_daemon_bases()
                .into_iter()
                .next()
                .unwrap_or_default();
            println!("Started daemon (pid {}) at {}", info.pid, primary);
            return Ok(());
        }
    }
    println!("Started daemon; not yet confirmed up (check `mdview status`).");
    Ok(())
}

#[cfg(test)]
mod config_edit_tests {
    use super::*;

    #[test]
    fn visual_takes_precedence_over_editor() {
        let resolved = resolve_editor(Some("code".to_string()), Some("vim".to_string()));
        assert_eq!(resolved.program, "code");
        assert!(resolved.args.is_empty());
    }

    #[test]
    fn platform_default_when_neither_set() {
        let resolved = resolve_editor(None, None);
        let expected = if cfg!(windows) { "notepad" } else { "vi" };
        assert_eq!(resolved.program, expected);
        assert!(resolved.args.is_empty());
    }

    #[test]
    fn editor_falls_back_when_visual_unset() {
        let resolved = resolve_editor(None, Some("vim".to_string()));
        assert_eq!(resolved.program, "vim");
        assert!(resolved.args.is_empty());
    }

    #[test]
    fn editor_with_args_splits_program_and_args() {
        let resolved = resolve_editor(Some("code --wait".to_string()), None);
        assert_eq!(resolved.program, "code");
        assert_eq!(resolved.args, vec!["--wait".to_string()]);
        assert_eq!(resolved.display, "code --wait");
    }

    #[test]
    fn valid_toml_reports_saved() {
        let path = Path::new("/tmp/mdview-test-config.toml");
        let msg = classify_config_edit_outcome(path, Ok(String::new()));
        assert!(msg.starts_with("Saved "));
        assert!(msg.contains("Restart the daemon to apply: mdview restart"));
    }

    #[test]
    fn invalid_toml_reports_warning() {
        let path = Path::new("/tmp/mdview-test-config.toml");
        let msg = classify_config_edit_outcome(path, Ok("not = [valid".to_string()));
        assert!(msg.starts_with("Warning: "));
        assert!(msg.contains("is not valid TOML"));
    }

    #[test]
    fn unreadable_file_reports_read_error() {
        let path = Path::new("/tmp/mdview-test-config.toml");
        let err = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let msg = classify_config_edit_outcome(path, Err(err));
        assert!(msg.starts_with("Could not re-read "));
    }
}

#[cfg(test)]
mod stop_restart_message_tests {
    use super::*;

    #[test]
    fn stop_outcome_message_reports_success() {
        assert_eq!(
            stop_outcome_message(Some((1234, true))),
            "Stopped daemon (pid 1234)."
        );
    }

    #[test]
    fn stop_outcome_message_reports_failed_kill() {
        assert_eq!(
            stop_outcome_message(Some((1234, false))),
            "Could not stop pid 1234. It may already be gone."
        );
    }

    #[test]
    fn stop_outcome_message_reports_no_daemon() {
        assert_eq!(stop_outcome_message(None), "No daemon running.");
    }

    #[test]
    fn restart_stop_message_reports_stopped_regardless_of_kill_result() {
        assert_eq!(
            restart_stop_message(Some((1234, true))),
            "Stopped daemon (pid 1234)."
        );
        assert_eq!(
            restart_stop_message(Some((1234, false))),
            "Stopped daemon (pid 1234)."
        );
    }

    #[test]
    fn restart_stop_message_reports_no_daemon_was_running() {
        assert_eq!(restart_stop_message(None), "No daemon was running.");
    }
}

#[cfg(test)]
mod open_url_shape_tests {
    use super::*;

    fn urls(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn open_json_single_url_sets_url_and_urls_consistently() {
        let u = urls(&["http://127.0.0.1:7700"]);
        let v = open_json(&u, "proj1");
        assert_eq!(v["url"], "http://127.0.0.1:7700");
        assert_eq!(v["urls"], serde_json::json!(["http://127.0.0.1:7700"]));
        assert_eq!(v["project_id"], "proj1");
    }

    #[test]
    fn open_json_multi_url_keeps_primary_as_first_and_lists_all() {
        let u = urls(&["http://192.168.1.5:7700", "http://10.0.0.2:7700"]);
        let v = open_json(&u, "proj1");
        assert_eq!(v["url"], "http://192.168.1.5:7700");
        assert_eq!(v["urls"], serde_json::json!(u));
        assert_eq!(v["url"], v["urls"][0]);
    }

    #[test]
    fn open_json_empty_urls_defaults_primary_to_empty_string() {
        let v = open_json(&[], "proj1");
        assert_eq!(v["url"], "");
        assert_eq!(v["urls"], serde_json::json!(Vec::<String>::new()));
    }

    #[test]
    fn format_url_choices_lists_every_url_indented_with_pick_framing() {
        let u = urls(&["http://192.168.1.5:7700", "http://10.0.0.2:7700"]);
        let text = format_url_choices(&u);
        assert_eq!(
            text,
            "Viewable at (pick a reachable IP):\n  http://192.168.1.5:7700\n  http://10.0.0.2:7700"
        );
    }
}
