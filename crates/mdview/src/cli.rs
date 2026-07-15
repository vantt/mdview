//! CLI surface (PRD §5.6). Registry commands operate on the shared SQLite DB;
//! the running daemon serves from the same DB. `serve`/`mcp` are the long-running
//! modes.

use crate::runtime;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use mdview_core::indexer;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "mdview", version, about = "Multi-project markdown viewer for AI agent workflows")]
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
    Refresh {
        project: Option<String>,
    },
    /// Remove a project from the registry (files are not deleted).
    Unregister {
        project_id: String,
    },
    /// Stop the running daemon.
    Stop,
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
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Serve { port, host } => cmd_serve(port, host),
        Command::Register { path, name, json } => cmd_register(&path, name.as_deref(), json),
        Command::Open { path, json } => cmd_open(&path, json),
        Command::List { json } => cmd_list(json),
        Command::Search { query, project, limit, json } => cmd_search(&query, project.as_deref(), limit, json),
        Command::Status { json } => cmd_status(json),
        Command::Refresh { project } => cmd_refresh(project.as_deref()),
        Command::Unregister { project_id } => cmd_unregister(&project_id),
        Command::Stop => cmd_stop(),
        Command::Doctor { json, dry_run, fix } => crate::doctor::run(json, dry_run, fix),
        Command::Mcp => crate::mcp::run(),
    }
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
        println!("{}", serde_json::json!({
            "project_id": project.id, "name": project.name,
            "root_path": project.root_path, "file_count": count,
            "url": format!("/p/{}/", project.id)
        }));
    } else {
        println!("Registered '{}' ({}) — {} markdown files", project.name, project.id, count);
        println!("  {}", project.root_path.display());
    }
    Ok(())
}

fn cmd_open(path: &Path, json: bool) -> Result<()> {
    let abs = std::fs::canonicalize(path).with_context(|| format!("no such file: {}", path.display()))?;
    let engine = runtime::build_engine()?;
    let root = find_project_root(&engine, &abs);
    let rel = indexer::rel_path_str(&root, &abs);
    let vf = engine.view_file(&root, &rel)?;
    let base = runtime::ensure_daemon_base();
    let full = format!("{base}{}", vf.url);
    if json {
        println!("{}", serde_json::json!({ "url": full, "project_id": vf.project_id }));
    } else {
        println!("{full}");
    }
    Ok(())
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
    file.parent().unwrap_or_else(|| Path::new(".")).to_path_buf()
}

fn cmd_list(json: bool) -> Result<()> {
    let engine = runtime::build_engine()?;
    let projects = engine.list_projects()?;
    if json {
        let arr: Vec<_> = projects.iter().map(|p| serde_json::json!({
            "id": p.id, "name": p.name, "root_path": p.root_path,
            "file_count": engine.file_count(&p.id).unwrap_or(0)
        })).collect();
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
        println!("{}", serde_json::json!({
            "running": daemon.is_some(),
            "server_url": daemon.as_ref().map(|d| format!("http://{}:{}", d.host, d.port)),
            "version": env!("CARGO_PKG_VERSION"),
            "project_count": projects.len(),
            "indexed_file_count": files,
        }));
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

fn cmd_stop() -> Result<()> {
    match runtime::read_lock() {
        Some(info) => {
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
            if ok {
                runtime::remove_lock();
                println!("Stopped daemon (pid {}).", info.pid);
            } else {
                println!("Could not stop pid {}. It may already be gone.", info.pid);
                runtime::remove_lock();
            }
        }
        None => println!("No daemon running."),
    }
    Ok(())
}
