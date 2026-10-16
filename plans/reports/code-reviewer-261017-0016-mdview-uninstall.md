# Code Review: `mdview uninstall` / `remove`

Date: 2026-10-17 (Asia/Saigon). Read-only review; no source modified.

## Scope
- `crates/mdview/src/uninstall.rs` (new, 518 LOC); wiring in `cli.rs`, `main.rs`; `pub(crate)` helpers in `doctor.rs`, `update.rs`.
- Cross-checked against `doctor.rs` writers (`ensure_claude_permission`, `register_json_mcp`, `register_toml_mcp`, `write_agent_snippet`, `check_skill_at`), `mdview_core::config::write_atomic`, `runtime::stop_daemon_pid`, `install.sh`, `install.ps1`.
- Empirical: built into the scratchpad, ran `mdview uninstall --yes --purge` with an isolated HOME and cwd (exe at `~/.mdview/bin/mdview`, a 0600 `~/.claude.json` holding a secret env in another MCP entry, a pre-existing `.claude.json.bak`, and `CLAUDE.md -> AGENTS.md` symlink). Findings F1, F3, F4, F6 reproduced there.

## Overall
The pure edit functions are fine and well tested. The problems are in how they are applied: file mode and symlinks are not kept, the content written comes from planning time and may be stale, the Windows default-install `--purge` path breaks, and partial failures do not abort before the steps that cannot be undone.

## High

### F1. Rewriting `~/.claude.json` / settings / Codex config widens permissions (0600 -> 0664/0644)
`uninstall.rs:109` -> `config::write_atomic` (`mdview-core/src/config.rs:213-224`) writes a fresh temp file with the default umask mode and renames it over the target. The original mode is lost.
Reproduced: `.claude.json` 600 -> 664 after uninstall. `~/.claude.json` is 0600 on real machines (verified on this host), and other MCP entries' `env` blocks often hold API tokens. Those become readable by other local users. (The `.bak` copy keeps 0600 because `fs::copy` copies permissions, so only the live file is exposed.) `doctor.rs` `register_json_mcp`/`register_toml_mcp`/`ensure_claude_permission` have the same bug, but uninstall adds another way to trigger it.
Fix: in `write_atomic` (one place, which also fixes doctor), read `fs::metadata(path)` when it exists and `set_permissions` on the temp file before the rename. Alternatively, create the temp file with `OpenOptions` + `mode(orig_mode)` on unix.

## Medium

### F2. Stale content is written back (planned at prompt time, applied after confirmation)
`uninstall.rs:98-111` and `202-221`: the file is read and `updated` is computed while planning. The closure later writes that captured string, after an interactive prompt that can wait for any length of time. Claude Code rewrites `~/.claude.json` constantly (startup counters, project state, OAuth/account fields). Any write made between the plan and the "y" is silently rolled back. The `.bak` holds the newer copy, but nothing tells the user. The `delete` decision for AGENTS.md/CLAUDE.md (`208`) is also stale: content the user adds during the prompt gets deleted with the file.
Fix: keep the planning read only for the description. Inside the closure, re-read the file, re-run `edit`/`strip_managed_block`, skip if the result is now `None`, and recompute `delete` from the fresh text.

### F3. Symlinked AGENTS.md/CLAUDE.md are replaced with regular files
`uninstall.rs:218` calls `write_atomic(&path, ..)` on the link path, and the rename replaces the symlink. Reproduced: with `CLAUDE.md -> AGENTS.md` (a common setup), both end up as separate regular files. `plan_config_edit` handles symlinks (`canonicalize` at `105`) but the instruction-file path does not. Both links are also planned as separate steps using the same stale text.
Fix: write to `fs::canonicalize(&path)`, and dedupe planned instruction files by canonical path (not just dirs, `194`). For the delete case, delete the target and then the link, or only the link when the target is shared.

### F4. Windows `--purge` on the default install fails and deletes data only partly
`uninstall.rs:244`: `exe` is canonicalized (`update.rs:151`), so on Windows it starts with `\\?\C:\...`. `config::data_dir()` returns `C:\Users\..\.mdview`. `Path::starts_with` compares prefix components by parsed kind, and `VerbatimDisk('C') != Disk('C')`, so `holds_exe` is always false. `install.ps1:26` installs to `%USERPROFILE%\.mdview\bin` by default, so in the normal case `remove_dir_all` (`255`) runs on a directory that contains the running exe. It fails on `mdview.exe` after deleting some of the contents, the step reports a failure, and the deferred script only `del`s the exe. `.mdview\bin` (and anything else left) stays behind.
Fix: compare against `fs::canonicalize(&dir)` (or strip `\\?\` on both sides, e.g. with `dunce`) before `starts_with`.

### F5. Partial failure does not stop the steps that cannot be undone
`uninstall.rs:402-412`: every step runs regardless of earlier failures, and the binary is always removed last. That causes two problems:
- If the daemon cannot be stopped (`121-123`), or is still alive after the 3 s wait (`124-129` returns `Ok` anyway), `--purge` still deletes `~/.mdview` under a live daemon that has SQLite and lock files open. On Windows the open files make it fail partway.
- When any config edit fails (permission error, read-only file), the binary is still deleted, so the user cannot simply re-run `mdview uninstall` to finish the cleanup.

Fix: make the daemon step fatal (abort before any other change) and return `Err` if it is still running after the wait. Skip the binary and data-dir steps when any earlier step failed, and print "re-run `mdview uninstall` after fixing the above".

### F6. `--purge` exits 1 with a false failure when the binary lives under `~/.mdview`
`uninstall.rs:382` runs after `380`. `remove_dir_all(~/.mdview)` has already deleted `~/.mdview/bin/mdview`, so `remove_or_hint` (`306-311`) gets `NotFound` and the command ends with "1 step(s) failed". Reproduced (exit 1). This is a normal unix layout: `install.sh:35-39` falls back to `$HOME/.mdview/bin`, and `MDVIEW_INSTALL_DIR` can point there too.
Fix: treat `NotFound` as success in `remove_or_hint`, or on unix drop the binary step when `purge && holds_exe` (after fixing F4).

### F7. Git-tracked team files in every registered project are edited or deleted
`uninstall.rs:186-196` scans every registered root, but `doctor --fix` only ever writes the cwd (`doctor.rs:737-758`, relative path). In a shared repo where the team committed the managed block (this repo does: tracked `CLAUDE.md:3-31`, `AGENTS.md:7-35`), one developer uninstalling mdview modifies tracked files in every registered repo. If the committed AGENTS.md held only the block, the file is deleted (`208-216`). The plan lists these changes, but a user reading a long plan will easily miss "delete <repo>/AGENTS.md" for a repo they are not working in.
Options (needs a product decision): (a) default to cwd only and add an opt-in `--all-projects`; (b) in registered roots, strip the block but never delete the file; (c) skip files tracked by git (`git ls-files --error-unmatch`) and list them as "left in place (tracked)".

### F8. Windows: a running `mdview mcp` holds the exe, the deferred delete fails silently, and it still reports success
`uninstall.rs:328-346`: the script output goes to NUL and nothing verifies it, while `416` prints "mdview is uninstalled." Claude Code, Codex and Antigravity keep `mdview mcp` children alive for the whole session, and each one pins `mdview.exe`, so `del` fails and the binary stays. Uninstall never detects or mentions these processes. On unix they keep running from the deleted inode, which is harmless.
Fix: on Windows, after the `del`, check with `if exist "<exe>"` and fall back to a rename-to-`%TEMP%` plus a message. At minimum, print "close Claude Code/Codex sessions first; if mdview.exe remains, delete it manually", and do not claim success for the deferred part.

## Low

### F9. Overwrites the `.bak` that doctor made (the only pre-mdview copy)
`uninstall.rs:157,164` use the fixed-name `backup_path` (`<file>.bak`). Reproduced: the earlier `.claude.json.bak` was replaced. The user loses the pre-`doctor` snapshot and gets a post-doctor one instead. Fix: use `timestamped_backup_path` for all three MCP configs, as the settings edit already does.

### F10. Windows deferred-script path handling
- `323-326`: removing `\\?\` from `\\?\UNC\srv\share\...` leaves `UNC\srv\share\...`, which is invalid, so the delete fails silently. Map `\\?\UNC\` to `\\` (or use `dunce::simplified`).
- `330,333`: double quotes guard against `&`, `^` and spaces. `%NAME%` sequences inside a path are still expanded by `cmd /C`. This is rare, but a path containing `%` would target the wrong file. Escape `%` as `%%` cannot be done on a `/C` command line, so reject such paths or write a temp `.cmd` file instead.
- Quote stripping is safe: the command line after `/C` starts with `ping`, not `"`, so `cmd` keeps the inner quotes.

### F11. Leftover update artifacts on Windows
Parked `mdview.exe.old-<pid>` files from `mdview update` (`update.rs:159-162`) are not removed by uninstall. They are left in the install dir when running without `--purge`, or always with a custom `MDVIEW_INSTALL_DIR`, because no later run will ever clean them. Fix: add `<exe>.old*` siblings to the deferred file list.

### F12. JSON rewrite reorders keys
`serde_json` has no `preserve_order` (`Cargo.toml:18`), so `pretty()` (`81-85`) sorts every key in `~/.claude.json` / `settings.json` alphabetically (reproduced). There is no data loss, but it produces a large diff for dotfile-managed `settings.json`. Doctor does the same. Optional fix: enable `serde_json/preserve_order`.

## Coverage check: doctor artifacts vs uninstall
| doctor writes | uninstall handles |
|---|---|
| `~/.claude/settings.json` permission | yes (timestamped backup) |
| `~/.claude.json` mcpServers.mdview | yes |
| `~/.codex/config.toml` [mcp_servers.mdview] | yes (toml_edit, comments kept) |
| `~/.gemini/config/mcp_config.json` | yes |
| `~/.claude/skills/mdview/SKILL.md` (+ dir if empty) | yes |
| AGENTS.md/CLAUDE.md block (cwd only) | yes, and also every registered root (see F7) |
| `~/.mdview/config.toml`, registry | `--purge` only |
| doctor `.bak` / `.mdview-<ts>.bak` backups | left in place (fine, conservative) |
No autostart or service registration exists in the codebase (searched for launchd/systemd/Run keys), so none is missed.

## Unix ordering and sudo
Sudo refusal (`366-371`), the non-tty refusal (`355-357`), `--dry-run` returning before any change (`394-396`), PATH symlink detection limited to links that resolve to the running exe (`262-278`), and the skill dir being removed only when empty (`174-176`) all behave correctly.

## Recommended actions (priority)
1. F1: keep the file mode in `write_atomic` (fixes doctor too).
2. F2 + F3: recompute the edit at apply time and write through symlinks; dedupe by canonical file.
3. F5: daemon stop fatal; skip binary/data-dir removal after any failure.
4. F4 + F6: canonical `holds_exe`; `NotFound` counts as removed.
5. F7: product decision on the scope for registered roots.
6. F8–F11: Windows hardening; F9: timestamped backups.

## Unresolved questions
- F7: should uninstall touch registered roots other than cwd at all, and may it ever delete a tracked file?
- Should uninstall attempt to stop running `mdview mcp` processes, or only warn?

Status: DONE_WITH_CONCERNS
Summary: Uninstall works end to end on Linux but has verified data-safety defects: rewritten configs lose their 0600 mode, stale planned content overwrites concurrent edits, and symlinked AGENTS.md/CLAUDE.md are replaced. It also deletes the binary after failures and breaks `--purge` on the default Windows install (`\\?\` prefix mismatch).
