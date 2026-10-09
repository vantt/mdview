---
phase: 6
title: "CLI-first agent integration"
status: pending
priority: P1
effort: "5h"
dependencies: [1]
---

# Phase 6: CLI-first agent integration

## Goal

Make `mdview open --json` the primary agent path, stop the background
full-repo scan, restart a daemon from an older binary automatically, keep
`doctor` read-only on the registry, and have `doctor --fix` grant the CLI
permission safely instead of registering MCP by default.

## Ownership (wave A)

- Own: `crates/mdview/src/cli.rs`, `crates/mdview/src/runtime.rs`,
  `crates/mdview/src/mcp.rs`, `crates/mdview/src/doctor.rs`,
  `docs/mdview-agents-template.md`, `docs/mdview-skill-template.md`,
  `CLAUDE.md`, `AGENTS.md`, `README.md`, `docs/usage.md`.
- Do not edit any other file. Record needs under "Handoff notes".

## Context

- `cmd_open` (`cli.rs:262`), `cmd_register` (`cli.rs:225`) and the MCP tool
  (`mcp.rs:109`) call `runtime::spawn_refresh_detached`.
- `open_json` (`cli.rs:302`) lacks `path` (MCP returns `vf.url`).
- `runtime::ensure_bind` (`runtime.rs:33`) checks health only; a daemon
  started by an older binary keeps serving. `DaemonInfo.version`
  (`mdview-core/src/daemon.rs:26`) and `daemon::daemon_version` exist;
  `doctor.rs:154–175` already compares versions.
- `doctor.rs:199–214` `check_index_schema` opens the store with
  `SqliteStore::open`, which after Phase 1 creates/resets the DB — wrong for
  a diagnostic and for `--dry-run`.
- `register_json_mcp` (`doctor.rs:295–326`) falls back to `{}` on parse
  failure and ignores backup errors (`let _ = std::fs::copy`) with a fixed
  `.bak` name — unsafe to copy for `~/.claude/settings.json`.
- Agent block markers are `<!-- mdview:START -->` / `<!-- mdview:END -->`
  (`doctor.rs:483–484`); templates are `include_str!`'d (`:471`, `:597`);
  a test asserts the skill mentions `mdview_view_file` and `mdview open`
  (`:715–716`). The `--json` CLI change to the templates and to the repo's
  `CLAUDE.md`/`AGENTS.md` is already committed (Phase 1 step 0).
- Phase 1 made `Engine::view_file` reject non-markdown files, renamed the
  registry to `registry-v4.db` (`config::registry_db_path`) and added
  `config::legacy_registry_paths()`.

## Tasks & Steps

1. **No background scan.** Remove `spawn_refresh_detached` and its calls.
   `cmd_register` prints that files are indexed on view/search; `mdview
   refresh` stays as the explicit full sync.
2. **JSON parity.** Add `"path": vf.url` to `open_json` + test.
3. **Daemon version.** When a daemon is running and its version (lock file
   `version`, falling back to `daemon_version`) differs from
   `env!("CARGO_PKG_VERSION")` or is missing, stop it the way `mdview stop`
   does, then spawn a fresh one. Put the decision in a pure
   `needs_restart(running: Option<&str>, current: &str) -> bool` with tests.
4. **MCP tool text.** Description: the file is indexed when its URL is
   opened; the project is indexed on its first content search.
5. **Doctor.**
   - `check_index_schema`: never open the store read-write. Registry file
     missing → "will be created on first use"; otherwise open read-only
     (`OpenFlags::SQLITE_OPEN_READ_ONLY`) and report `user_version` vs
     `SCHEMA_VERSION` ("will be rebuilt on next start" on mismatch). Drop
     the "unhashed rows" metric. Legacy `registry.db*` files present →
     report as removable; delete them on `--fix` only.
   - `check_claude_permission`: ensure `"Bash(mdview open:*)"` is in
     `permissions.allow` of `~/.claude/settings.json`. Missing file → create.
     File that does not parse as a JSON object → `Manual`, never write.
     Write only after a successful timestamped backup
     (`settings.json.mdview-<unix>.bak`); abort on backup failure.
     Idempotent; `--dry-run` writes nothing; all other keys (including
     `permissions.deny`) preserved; a symlinked path is written through to
     its target.
   - MCP registration checks run only with a new `--mcp` flag (clap in
     `cli.rs`); otherwise reported as skipped, existing registrations left
     alone.
   - Tests: permission into missing/empty/existing settings; malformed JSON
     untouched; `deny` preserved; backup failure aborts; dry-run writes
     nothing; idempotency; schema check never creates a DB file.
6. **Templates (CLI first).** Agents template: lead with
   `mdview open --json <absolute-path-to-file.md>` and its JSON fields
   (`url`, `urls`, `long_url`, `long_urls`, `path`, `code`, `project_id`);
   `mdview_view_file` is "if you have no shell". Skill template: same order,
   keep the multi-URL guidance and both names (doctor test). Mirror the
   block into the repo's `CLAUDE.md` and `AGENTS.md` between the existing
   `mdview:START` / `mdview:END` markers.
7. **User docs.** `README.md` and `docs/usage.md`: CLI-first wiring,
   `doctor --fix` adds the permission, `--mcp` registers MCP, indexing on
   view/search, 14-day idle-project cleanup, upgrade note (the registry is
   rebuilt as `registry-v4.db`; restart agent sessions so old `mdview mcp`
   processes exit; older builds keep using `registry.db`).

## Verification

- `cargo test -p mdview doctor`
- `cargo test -p mdview cli`
- `cargo test -p mdview runtime`
- `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
- `grep -rn "spawn_refresh_detached" crates/` returns nothing.

## Handoff notes

_(record cross-ownership needs here)_
