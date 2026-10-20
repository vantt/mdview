//! `mdview uninstall`: undo what `doctor --fix` set up, then remove the binary.
//!
//! Every change is planned first by inspecting the current state, shown to the
//! user, and only applied after confirmation (`--yes` skips the prompt,
//! `--dry-run` stops after the plan). The data directory (`~/.mdview`: config
//! and index) is kept unless `--purge` is given.

use crate::doctor::{
    claude_config_path, claude_desktop_configs, claude_settings_path, home, managed_block_range,
    skill_path, timestamped_backup_path, AGENT_FILES, CLAUDE_PERMISSION,
};
use anyhow::{bail, Context, Result};
use mdview_core::config;
use serde_json::Value;
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

/// One planned change: what it does (shown to the user) and how to do it.
struct Step {
    what: String,
    irreversible: bool,
    apply: Box<dyn FnOnce() -> Result<()>>,
}

impl Step {
    fn new(what: impl Into<String>, apply: impl FnOnce() -> Result<()> + 'static) -> Self {
        Self {
            what: what.into(),
            irreversible: false,
            apply: Box::new(apply),
        }
    }

    /// A step that cannot be redone by re-running uninstall (deleting the
    /// binary or the data directory); skipped once any earlier step failed.
    fn irreversible(what: impl Into<String>, apply: impl FnOnce() -> Result<()> + 'static) -> Self {
        Self {
            irreversible: true,
            ..Self::new(what, apply)
        }
    }
}

// ── Pure edits (unit-tested) ──

/// `text` with `permission` removed from `permissions.allow`, or `None` when it
/// is absent or the file is not the expected JSON shape (never rewritten then).
fn remove_permission(text: &str, permission: &str) -> Option<String> {
    let mut root: Value = serde_json::from_str(text).ok()?;
    let allow = root
        .get_mut("permissions")?
        .get_mut("allow")?
        .as_array_mut()?;
    let before = allow.len();
    allow.retain(|v| v.as_str() != Some(permission));
    (allow.len() != before).then(|| pretty(&root))
}

/// `text` with the `mcpServers.mdview` entry removed, or `None` when absent.
fn remove_json_mcp(text: &str) -> Option<String> {
    let mut root: Value = serde_json::from_str(text).ok()?;
    root.get_mut("mcpServers")?
        .as_object_mut()?
        .remove("mdview")?;
    Some(pretty(&root))
}

/// `text` with the `[mcp_servers.mdview]` table removed (comments and other
/// settings preserved), or `None` when absent or the TOML does not parse.
fn remove_toml_mcp(text: &str) -> Option<String> {
    let mut doc = text.parse::<toml_edit::DocumentMut>().ok()?;
    doc.get_mut("mcp_servers")?
        .as_table_like_mut()?
        .remove("mdview")?;
    Some(doc.to_string())
}

/// `text` with mdview's managed instruction block removed and the blank-line
/// gap `doctor --fix` added around it closed up, or `None` when absent. An
/// empty result means the file held nothing but the block.
fn strip_managed_block(text: &str) -> Option<String> {
    let (start, end) = managed_block_range(text)?;
    let before = text[..start].trim_end_matches(['\n', '\r']);
    let after = text[end..].trim_start_matches(['\n', '\r']);
    Some(match (before.is_empty(), after.is_empty()) {
        (true, _) => after.to_string(),
        (false, true) => format!("{before}\n"),
        (false, false) => format!("{before}\n\n{after}"),
    })
}

fn pretty(v: &Value) -> String {
    let mut s = serde_json::to_string_pretty(v).expect("serializing a JSON value");
    s.push('\n');
    s
}

// ── Planning: one function per thing `doctor --fix` (or install) creates ──

/// Rewrite `path` through `edit`, re-reading it at apply time so changes made
/// while the prompt waited (Claude Code rewrites `~/.claude.json` often) are
/// kept. Writes through symlinks; backs up with a timestamped copy.
fn rewrite_config(path: &Path, edit: impl Fn(&str) -> Option<String>) -> Result<()> {
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let text = match std::fs::read_to_string(&target) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e).with_context(|| format!("reading {}", target.display())),
    };
    let Some(updated) = edit(&text) else {
        return Ok(());
    };
    let backup = timestamped_backup_path(&target);
    std::fs::copy(&target, &backup)
        .with_context(|| format!("backing up to {}", backup.display()))?;
    config::write_atomic(&target, updated.as_bytes())
        .with_context(|| format!("writing {}", target.display()))
}

/// Plan an edit of the config at `path` when it currently needs one.
fn plan_config_edit(
    steps: &mut Vec<Step>,
    path: PathBuf,
    what: String,
    edit: impl Fn(&str) -> Option<String> + 'static,
) {
    let needed = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| edit(&t))
        .is_some();
    if needed {
        steps.push(Step::new(what, move || rewrite_config(&path, edit)));
    }
}

fn plan_daemon(steps: &mut Vec<Step>) {
    let Some(info) = crate::runtime::running_daemon() else {
        return;
    };
    steps.push(Step::new(
        format!("stop the running daemon (pid {})", info.pid),
        move || {
            if let Some((pid, false)) = crate::runtime::stop_daemon_pid(info.pid) {
                bail!("could not stop pid {pid}");
            }
            for _ in 0..30 {
                if crate::runtime::running_daemon().is_none() {
                    return Ok(());
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            bail!("daemon pid {} is still running", info.pid)
        },
    ));
}

fn plan_integrations(steps: &mut Vec<Step>) {
    let settings = claude_settings_path();
    let what = format!(
        "remove `{CLAUDE_PERMISSION}` from {} (backup first)",
        settings.display()
    );
    plan_config_edit(steps, settings, what, |t| {
        remove_permission(t, CLAUDE_PERMISSION)
    });

    let gemini = home()
        .join(".gemini")
        .join("config")
        .join("mcp_config.json");
    let json_configs = [claude_config_path(), gemini]
        .into_iter()
        .chain(claude_desktop_configs());
    for path in json_configs {
        let what = format!(
            "unregister the mdview MCP server from {} (backup first)",
            path.display()
        );
        plan_config_edit(steps, path, what, remove_json_mcp);
    }
    let codex = home().join(".codex").join("config.toml");
    let what = format!(
        "unregister the mdview MCP server from {} (backup first)",
        codex.display()
    );
    plan_config_edit(steps, codex, what, remove_toml_mcp);

    let skill = skill_path();
    if skill.exists() {
        steps.push(Step::new(
            format!("remove the /mdview skill {}", skill.display()),
            move || {
                remove_if_present(&skill)?;
                // The directory is mdview's own; keep it only if the user
                // put something else in it.
                if let Some(dir) = skill.parent() {
                    let _ = std::fs::remove_dir(dir);
                }
                Ok(())
            },
        ));
    }
}

/// Plan removal of the managed block from AGENTS.md/CLAUDE.md in `dir`. Files
/// are grouped by their real path so a `CLAUDE.md -> AGENTS.md` symlink pair
/// is edited once; a file is deleted only when it held nothing but the block
/// and no symlink points at it, otherwise the stripped text is written through.
fn plan_instruction_blocks(steps: &mut Vec<Step>, dir: &Path) {
    let mut groups: Vec<(PathBuf, Vec<PathBuf>)> = Vec::new();
    for name in AGENT_FILES {
        let path = dir.join(name);
        let Ok(real) = std::fs::canonicalize(&path) else {
            continue;
        };
        match groups.iter_mut().find(|(r, _)| *r == real) {
            Some((_, names)) => names.push(path),
            None => groups.push((real, vec![path])),
        }
    }
    for (real, names) in groups {
        let has_block = std::fs::read_to_string(&real)
            .ok()
            .and_then(|t| strip_managed_block(&t))
            .is_some();
        if !has_block {
            continue;
        }
        // Ask the filesystem rather than compare `p` with `real`: on Windows
        // `canonicalize` returns a `\\?\` verbatim path that never equals the
        // plain one, which made every file look aliased.
        let aliased = names.len() > 1
            || names
                .iter()
                .any(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()));
        let shown = names
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(" = ");
        steps.push(Step::new(
            format!("remove the MDView block from {shown}"),
            move || {
                let Some(updated) = std::fs::read_to_string(&real)
                    .ok()
                    .and_then(|t| strip_managed_block(&t))
                else {
                    return Ok(());
                };
                if updated.trim().is_empty() && !aliased {
                    // `doctor --fix` created the file just for the block.
                    remove_if_present(&real)
                } else {
                    config::write_atomic(&real, updated.as_bytes())
                        .with_context(|| format!("writing {}", real.display()))
                }
            },
        ));
    }
}

/// Registered project roots other than `cwd` whose instruction files still
/// carry the managed block. They are reported, never edited: those files are
/// often shared through git, and `doctor --fix` only writes where it is run.
fn other_projects_with_block(cwd: &Path) -> Vec<PathBuf> {
    if !config::registry_db_path().exists() {
        return Vec::new();
    }
    let Ok(projects) = crate::runtime::build_engine().and_then(|e| Ok(e.list_projects()?)) else {
        return Vec::new();
    };
    let cwd = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    projects
        .into_iter()
        .map(|p| p.root_path)
        .filter(|root| std::fs::canonicalize(root).ok().as_deref() != Some(cwd.as_path()))
        .filter(|root| {
            AGENT_FILES.iter().any(|name| {
                std::fs::read_to_string(root.join(name))
                    .map(|t| managed_block_range(&t).is_some())
                    .unwrap_or(false)
            })
        })
        .collect()
}

/// Paths a Windows cleanup must delete after this process exits, because a
/// running `.exe` cannot be deleted (only renamed).
#[derive(Default)]
struct Deferred {
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
}

type SharedDeferred = std::rc::Rc<std::cell::RefCell<Deferred>>;

fn plan_data_dir(steps: &mut Vec<Step>, exe: &Path, deferred: &SharedDeferred) {
    let dir = config::data_dir();
    if !dir.exists() {
        return;
    }
    // Compare resolved paths: on Windows `exe` is canonical (`\\?\C:\…`) while
    // `data_dir()` is not, and `starts_with` would never match across forms.
    let holds_exe = exe.starts_with(std::fs::canonicalize(&dir).unwrap_or_else(|_| dir.clone()));
    let deferred = deferred.clone();
    steps.push(Step::irreversible(
        format!(
            "delete the data directory {} (config and index)",
            dir.display()
        ),
        move || {
            if cfg!(windows) && holds_exe {
                deferred.borrow_mut().dirs.push(dir);
                return Ok(());
            }
            std::fs::remove_dir_all(&dir).with_context(|| format!("deleting {}", dir.display()))
        },
    ));
}

/// `mdview` entries on `PATH` that are symlinks to `exe`: removing only the
/// real file would leave them dangling.
fn path_symlinks_to(exe: &Path) -> Vec<PathBuf> {
    let name = format!("mdview{}", std::env::consts::EXE_SUFFIX);
    let Some(paths) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    let mut links: Vec<PathBuf> = std::env::split_paths(&paths)
        .map(|d| d.join(&name))
        .filter(|p| {
            p.symlink_metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
                && std::fs::canonicalize(p).ok().as_deref() == Some(exe)
        })
        .collect();
    links.dedup();
    links
}

fn plan_binary(steps: &mut Vec<Step>, exe: PathBuf, deferred: &SharedDeferred) {
    for link in path_symlinks_to(&exe) {
        steps.push(Step::irreversible(
            format!("remove the symlink {}", link.display()),
            move || remove_or_hint(&link),
        ));
    }
    let deferred = deferred.clone();
    steps.push(Step::irreversible(
        format!("remove the binary {}", exe.display()),
        move || {
            if cfg!(windows) {
                deferred.borrow_mut().files.push(exe);
                return Ok(());
            }
            remove_or_hint(&exe)
        },
    ));
}

/// Delete `path`; an already-missing file counts as removed.
fn remove_if_present(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other.with_context(|| format!("removing {}", path.display())),
    }
}

/// Delete `path`, turning a permission error into the exact command to run.
fn remove_or_hint(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => bail!(
            "no permission ({e}); remove it with: sudo rm {}",
            path.display()
        ),
        _ => remove_if_present(path),
    }
}

/// A path as `cmd` builtins accept it: `canonicalize` yields `\\?\C:\…` and
/// `\\?\UNC\server\share\…`, which `del`/`rmdir` do not understand.
#[cfg_attr(not(windows), allow(dead_code))]
fn cmd_path(p: &Path) -> String {
    let s = p.display().to_string();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else {
        s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s)
    }
}

/// Hand the paths a running Windows `.exe` pins to a detached `cmd` that
/// deletes them once this process has exited. Returns the paths it could not
/// schedule (a `%` would be expanded by `cmd`, so those are left to the user).
#[cfg(windows)]
fn spawn_deferred_cleanup(deferred: &Deferred) -> Result<Vec<PathBuf>> {
    let mut manual = Vec::new();
    // ~2s pause so this process has exited and released its image.
    let mut script = String::from("ping -n 3 127.0.0.1 >NUL");
    let mut scheduled = 0;
    let all = deferred
        .files
        .iter()
        .map(|f| ("del /F /Q", f))
        .chain(deferred.dirs.iter().map(|d| ("rmdir /S /Q", d)));
    for (verb, path) in all {
        let p = cmd_path(path);
        if p.contains('%') || p.contains('"') {
            manual.push(path.clone());
            continue;
        }
        script.push_str(&format!(" & {verb} \"{p}\""));
        scheduled += 1;
    }
    if scheduled == 0 {
        return Ok(manual);
    }
    let mut cmd = std::process::Command::new("cmd");
    {
        use std::os::windows::process::CommandExt;
        cmd.raw_arg(format!("/C {script}"));
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    crate::runtime::apply_detach(&mut cmd);
    cmd.spawn()
        .context("scheduling deletion of the running binary")?;
    Ok(manual)
}

#[cfg(not(windows))]
fn spawn_deferred_cleanup(_: &Deferred) -> Result<Vec<PathBuf>> {
    Ok(Vec::new())
}

fn confirm() -> Result<bool> {
    if !std::io::stdin().is_terminal() {
        bail!("refusing to uninstall without confirmation; re-run with --yes");
    }
    print!("Proceed? [y/N] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES" | "Yes"))
}

pub fn run(purge: bool, yes: bool, dry_run: bool) -> Result<()> {
    if crate::update::running_under_sudo() {
        bail!(
            "run `mdview uninstall` as your own user, not via sudo: it cleans up your home \
             directory, and prints the one `sudo rm` needed if the binary is root-owned"
        );
    }
    let exe = crate::update::current_exe_path()?;
    let cwd = std::env::current_dir().context("reading the current directory")?;
    let deferred = SharedDeferred::default();

    let mut steps = Vec::new();
    plan_daemon(&mut steps);
    plan_integrations(&mut steps);
    plan_instruction_blocks(&mut steps, &cwd);
    if purge {
        plan_data_dir(&mut steps, &exe, &deferred);
    }
    plan_binary(&mut steps, exe, &deferred);
    let elsewhere = other_projects_with_block(&cwd);

    println!("mdview uninstall will:");
    for s in &steps {
        println!("  - {}", s.what);
    }
    if !purge {
        println!(
            "  (keeping {}; add --purge to delete config and index too)",
            config::data_dir().display()
        );
    }
    if !elsewhere.is_empty() {
        println!("  (left untouched: the MDView block in AGENTS.md/CLAUDE.md of these registered projects;");
        println!("   run `mdview uninstall` from a project to strip it there, or edit by hand)");
        for root in &elsewhere {
            println!("     {}", root.display());
        }
    }
    if dry_run {
        return Ok(());
    }
    if !yes && !confirm()? {
        println!("Aborted; nothing changed.");
        return Ok(());
    }

    let mut failed = 0;
    for step in steps {
        if step.irreversible && failed > 0 {
            println!(
                "  - skipped: {} (fix the failures above, then re-run)",
                step.what
            );
            continue;
        }
        match (step.apply)() {
            Ok(()) => println!("  ✓ {}", step.what),
            Err(e) => {
                failed += 1;
                println!("  ✗ {}: {e:#}", step.what);
            }
        }
    }
    let manual = spawn_deferred_cleanup(&deferred.borrow())?;
    if failed > 0 {
        bail!("{failed} step(s) failed; see above");
    }
    for path in &manual {
        println!("Delete {} by hand once mdview has exited.", path.display());
    }
    if cfg!(windows) && manual.is_empty() {
        println!(
            "The binary is deleted a few seconds after this command exits. If it remains, \
             close running mdview processes (e.g. MCP sessions in your editor) and delete it."
        );
    }
    println!("mdview is uninstalled.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_removed_and_other_entries_kept() {
        let text = r#"{"permissions":{"allow":["Bash(ls:*)","Bash(mdview open:*)"],"deny":["X"]},"theme":"dark"}"#;
        let out: Value =
            serde_json::from_str(&remove_permission(text, CLAUDE_PERMISSION).unwrap()).unwrap();
        assert_eq!(
            out["permissions"]["allow"],
            serde_json::json!(["Bash(ls:*)"])
        );
        assert_eq!(out["permissions"]["deny"], serde_json::json!(["X"]));
        assert_eq!(out["theme"], "dark");
    }

    #[test]
    fn permission_absent_or_malformed_is_left_alone() {
        assert!(remove_permission(
            r#"{"permissions":{"allow":["Bash(ls:*)"]}}"#,
            CLAUDE_PERMISSION
        )
        .is_none());
        assert!(remove_permission("not json", CLAUDE_PERMISSION).is_none());
        assert!(remove_permission(r#"{"permissions":{"allow":"x"}}"#, CLAUDE_PERMISSION).is_none());
    }

    #[test]
    fn json_mcp_entry_removed_and_siblings_kept() {
        let text = r#"{"mcpServers":{"mdview":{"command":"mdview"},"other":{}},"k":1}"#;
        let out: Value = serde_json::from_str(&remove_json_mcp(text).unwrap()).unwrap();
        assert_eq!(out["mcpServers"], serde_json::json!({"other":{}}));
        assert_eq!(out["k"], 1);
        assert!(remove_json_mcp(r#"{"mcpServers":{"other":{}}}"#).is_none());
    }

    #[test]
    fn toml_mcp_table_removed_and_comments_kept() {
        let text = "# keep me\nmodel = \"x\"\n\n[mcp_servers.mdview]\ncommand = \"mdview\"\nargs = [\"mcp\"]\n\n[mcp_servers.other]\ncommand = \"o\"\n";
        let out = remove_toml_mcp(text).unwrap();
        assert!(out.contains("# keep me"));
        assert!(out.contains("[mcp_servers.other]"));
        assert!(!out.contains("mdview"));
        assert!(remove_toml_mcp("[mcp_servers.other]\ncommand = \"o\"\n").is_none());
        assert!(remove_toml_mcp("not = = toml").is_none());
    }

    #[test]
    fn managed_block_stripped_between_user_content() {
        let text =
            "# Rules\n\nmine\n\n<!-- mdview:START -->\nstuff\n<!-- mdview:END -->\n\n## Later\n";
        assert_eq!(
            strip_managed_block(text).unwrap(),
            "# Rules\n\nmine\n\n## Later\n"
        );
    }

    #[test]
    fn managed_block_appended_at_end_is_stripped_cleanly() {
        let text = "# Rules\nmine\n\n<!-- mdview:START -->\nstuff\n<!-- mdview:END -->\n";
        assert_eq!(strip_managed_block(text).unwrap(), "# Rules\nmine\n");
    }

    #[test]
    fn file_holding_only_the_block_becomes_empty() {
        let text = "<!-- mdview:START -->\nstuff\n<!-- mdview:END -->\n";
        assert_eq!(strip_managed_block(text).unwrap(), "");
        assert!(strip_managed_block("no block here").is_none());
    }

    fn scratch(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mdview-uninstall-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const BLOCK: &str = "<!-- mdview:START -->\nx\n<!-- mdview:END -->\n";

    #[test]
    fn instruction_blocks_planned_per_file() {
        let dir = scratch("files");
        std::fs::write(dir.join("AGENTS.md"), BLOCK).unwrap();
        std::fs::write(dir.join("CLAUDE.md"), format!("mine\n\n{BLOCK}")).unwrap();
        let mut steps = Vec::new();
        plan_instruction_blocks(&mut steps, &dir);
        assert_eq!(steps.len(), 2);
        for s in steps {
            (s.apply)().unwrap();
        }
        assert!(!dir.join("AGENTS.md").exists());
        assert_eq!(
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap(),
            "mine\n"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn instruction_block_reapplied_on_current_content() {
        let dir = scratch("fresh");
        std::fs::write(dir.join("AGENTS.md"), BLOCK).unwrap();
        let mut steps = Vec::new();
        plan_instruction_blocks(&mut steps, &dir);
        // Text added after planning (while the prompt waited) must survive.
        std::fs::write(dir.join("AGENTS.md"), format!("{BLOCK}\nnew notes\n")).unwrap();
        for s in steps {
            (s.apply)().unwrap();
        }
        assert_eq!(
            std::fs::read_to_string(dir.join("AGENTS.md")).unwrap(),
            "new notes\n"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_instruction_files_edited_once_and_kept_linked() {
        let dir = scratch("link");
        std::fs::write(dir.join("AGENTS.md"), BLOCK).unwrap();
        std::os::unix::fs::symlink("AGENTS.md", dir.join("CLAUDE.md")).unwrap();
        let mut steps = Vec::new();
        plan_instruction_blocks(&mut steps, &dir);
        assert_eq!(steps.len(), 1);
        for s in steps {
            (s.apply)().unwrap();
        }
        let meta = dir.join("CLAUDE.md").symlink_metadata().unwrap();
        assert!(meta.file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap(), "");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn config_rewrite_uses_current_content_and_backs_up() {
        let dir = scratch("cfg");
        let p = dir.join("c.json");
        std::fs::write(&p, r#"{"mcpServers":{"mdview":{}},"a":1}"#).unwrap();
        let mut steps = Vec::new();
        plan_config_edit(&mut steps, p.clone(), "x".into(), remove_json_mcp);
        std::fs::write(&p, r#"{"mcpServers":{"mdview":{}},"a":2}"#).unwrap();
        for s in steps {
            (s.apply)().unwrap();
        }
        let out: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(out, serde_json::json!({"mcpServers":{},"a":2}));
        let backups = std::fs::read_dir(&dir).unwrap().count() - 1;
        assert_eq!(backups, 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_file_counts_as_removed() {
        let dir = scratch("gone");
        assert!(remove_or_hint(&dir.join("nope")).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cmd_paths_drop_verbatim_prefixes() {
        assert_eq!(cmd_path(Path::new(r"\\?\C:\a\b.exe")), r"C:\a\b.exe");
        assert_eq!(
            cmd_path(Path::new(r"\\?\UNC\srv\share\x")),
            r"\\srv\share\x"
        );
        assert_eq!(cmd_path(Path::new("/usr/bin/x")), "/usr/bin/x");
    }
}
