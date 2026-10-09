---
title: "Viewed-scope index, contentless FTS search, CLI-first agent integration"
description: "Index only what the user views (plus link targets and siblings), sync the whole project lazily on content search, shrink the registry DB, and make `mdview open --json` the primary agent path."
status: pending
priority: P1
effort: "3d"
branch: feat/viewed-scope-index
tags: [refactor, backend, database, frontend, cli]
blockedBy: []
blocks: []
created: 2026-10-09
---

# Viewed-scope index, contentless FTS search, CLI-first agent integration

## Overview

mdview is a local dev tool for reading docs that agents write. Today a first
`view_file` on a project spawns a full background scan that copies the text of
every `.md` file into an FTS5 table. On the user's machine one project produced
2,372 rows / 24 MB, 90% of it agent skill packs under `.grok/` and `.omp/`.
FTS deletes filter on `UNINDEXED` columns, so every upsert/delete scans the
whole FTS table (measured 2.2 ms per lookup vs 0.04 ms by rowid), making
refresh and cleanup O(n²). Each agent session also spawns its own `mdview mcp`
process (measured ~1.4 MB private memory each; small but avoidable).

This plan changes *when* and *what* mdview indexes, keeps FTS5 + bm25 for
search, and moves agents to the CLI.

## Decisions (accepted with the user, 2026-10-09)

| # | Decision |
|---|---|
| D1 | **CLI-first agents.** Agent instruction block and `/mdview` skill use `mdview open --json` as the primary path; MCP stays as a fallback for clients without a shell. `mdview doctor --fix` adds a Claude Code permission `Bash(mdview open:*)` and no longer registers the MCP server by default (opt in with `--mcp`). CLI JSON gains a `path` field for parity with MCP `structuredContent`. |
| D2 | **No migrations.** The registry is disposable local state. The new schema lives in `registry-v4.db` (old binaries keep `registry.db`; `doctor --fix` removes the legacy file). On a version mismatch the store drops and recreates all tables inside one `BEGIN IMMEDIATE` transaction that re-reads and always stamps `user_version`. The v1–v3 migration chain and backfills are deleted. The "back up before DB change" rule is waived for this DB by the user because it is a rebuildable cache. A daemon started by an older binary is restarted automatically. |
| D3 | **FTS fixes.** `files.fts_rowid` links a file to its FTS row, so FTS delete/update is by rowid (O(log n)). Skip the FTS write when `content_hash` is unchanged. Batch writes (~200 files per transaction). Skip files whose size+mtime are unchanged. Prune rows whose file no longer exists on disk (only when `!exists`, never because a file is gitignored). |
| D4 | **Contentless FTS.** `fts5(title, content, content='', contentless_delete=1, tokenize='unicode61 remove_diacritics 2')`. Title and content are inserted pre-folded by `fold::fold` (NFD, drop marks, `đ`→`d`, lowercase) and queries are folded the same way, because `remove_diacritics 2` alone does not fold `đ`. Excerpts are built from disk for the top results with the same fold, marked with private-use sentinels, never with raw `<mark>` text. |
| D5 | **Viewed-scope indexing.** No background full-repo scan on register/open/view_file. Rendering a file reads it once for index + links + render, then indexes (in the background) its link targets (1 hop) and its sibling `.md` files. Link resolution checks the filesystem, so links to not-yet-indexed files are never shown as broken. Only markdown files inside the canonical project root (symlinks resolved) and outside excluded dirs are ever indexed, linked or rendered — `.env`, `Cargo.toml`, `.git/config` never enter the index. |
| D6 | **Search UX.** One box (the Cmd/Ctrl+K palette): typing = fuzzy match on path/title over a cached filesystem listing, recent files first; Enter = content search. Content search lazily syncs the whole project (hidden dirs included, `.gitignore` + exclude patterns respected) and then runs bm25. Scope toggle *Whole project / This folder*; sort toggle *Relevance (default) / Newest*; a status line "Synced N files in X s". Default scope is the whole project. A sync is single-flight per project and skipped if the previous one finished < 10 s ago; sync errors are shown, not swallowed; all blocking engine work runs in `spawn_blocking`. Searching a project counts as access; the CLI search without a project searches already-indexed rows only (no sync, no access touch). |
| D7 | **TTL.** Drop the per-file TTL. Change the project TTL from 30 to **14 days** (the file TTL was 7 days); after a sweep that deletes data, VACUUM when more than 25% of pages are free. |
| D8 | **Watcher scope.** Watch (non-recursively) only directories that contain indexed rows (stubs included); the engine hints new directories immediately and a 5 s reconcile catches the rest, so projects registered after daemon start are watched too. The watcher decides reloads from its own content-hash memory, so indexing by views/searches never swallows a reload. |
| D9 | **Hidden dirs stay included** in sync and search. |

## Non-goals

- No MCP over HTTP / single shared MCP process (rejected: per-process cost measured small).
- No semantic search, no cross-project link resolution.
- No change to auth, short-link format, editor, or Code section behaviour.

## Phases

| # | Phase | Depends on | Parallel group | Status |
|---|---|---|---|---|
| 1 | [Foundation: schema, store, sync, contracts](./phase-01-foundation-store-and-sync.md) | — | sequential | Pending |
| 2 | [Content search UX and excerpts](./phase-02-content-search-ux.md) | 1 | wave A | Pending |
| 3 | [Index on render: one read + neighbours](./phase-03-index-on-render.md) | 1 | wave A | Pending |
| 4 | [Filesystem listing, sidebar and jump palette](./phase-04-listing-sidebar-jump.md) | 1 | wave A | Pending |
| 5 | [Watcher scoped to indexed dirs](./phase-05-scoped-watcher.md) | 1 | wave A | Pending |
| 6 | [CLI-first agent integration](./phase-06-cli-first-agents.md) | 1 | wave A | Pending |
| 7 | [Integration, docs, end-to-end verification](./phase-07-integration-and-docs.md) | 2–6 | sequential | Pending |

### Execution model

- Work on branch `feat/viewed-scope-index` (never `main`). Phase 1 is committed
  on that branch before wave A starts.
- Wave A phases run in parallel, each in its own git worktree branched from the
  Phase 1 commit, and each commits only the files it owns (see the ownership
  table). Phase 7 merges them back in order 6 → 5 → 4 → 2 → 3.
- Every phase must leave `cargo fmt --all --check`, `cargo clippy --workspace
  --all-targets -- -D warnings` and `cargo test --workspace` green in its own
  worktree.

### File ownership in wave A (disjoint)

| File | Owner |
|---|---|
| `crates/mdview-core/src/search.rs`, `snippet.rs`, `crates/mdview/assets/app.css` | P2 |
| `crates/mdview/src/server.rs` → `SearchQuery`, `search_page`, their private helpers, `mod search_page_tests` | P2 |
| `crates/mdview/src/views.rs` → `search_page`, `highlight_excerpt`, their private helpers and tests | P2 |
| `crates/mdview-core/src/engine.rs`, `render.rs`, `domain.rs` | P3 |
| `crates/mdview/src/server.rs` → `project_path`, its private helpers, `mod project_path_tests` | P3 |
| `crates/mdview-core/src/listing.rs`, `fuzzy.rs`, `crates/mdview/assets/app.js` | P4 |
| `crates/mdview/src/server.rs` → `JumpQuery`, `jump_search`, `default_jump_limit`, `mod jump_tests` | P4 |
| `crates/mdview/src/watch.rs` | P5 |
| `crates/mdview/src/cli.rs`, `runtime.rs`, `mcp.rs`, `doctor.rs`, `docs/mdview-agents-template.md`, `docs/mdview-skill-template.md`, `CLAUDE.md`, `AGENTS.md`, `README.md`, `docs/usage.md` | P6 |
| `crates/mdview-core/src/repository.rs`, `indexer.rs`, `sync.rs`, `fold.rs`, `link_resolver.rs`, `config.rs`, `lib.rs`, `Cargo.toml`s, `Cargo.lock`, `crates/mdview/src/cleanup.rs`, rest of `views.rs` and `server.rs` | frozen after P1 (only P7 may touch) |

A wave A phase that finds it needs a change in a file it does not own must not
make it; it records the need under "Handoff notes" in its phase file and P7
applies it.

## Success criteria

- [ ] Registry DB for a project where the user only viewed a few files holds only those files, their link targets and siblings (no full-repo scan without a content search).
- [ ] FTS delete/update use `fts_rowid`; no SQL statement filters `files_fts` by `project_id`/`rel_path` (`grep -n "files_fts WHERE" crates/mdview-core/src/repository.rs` shows only rowid filters / `MATCH`).
- [ ] Content search returns bm25-ranked results across the whole project; "duoc" finds "được" and "tai lieu" finds "tài liệu" with highlighted excerpts; scope and sort toggles; a sync status line; a sync failure is visible.
- [ ] Non-markdown files, files outside the canonical root (including via symlink) and excluded dirs are never indexed, rendered as pages, or offered as link targets.
- [ ] Request handlers never run sync, render or index work on async worker threads.
- [ ] Links to existing but unindexed files render as working links, never as broken.
- [ ] New schema lives in `registry-v4.db`; a stale-version DB is rebuilt atomically; no migration code remains; `doctor` never creates or resets the DB.
- [ ] `mdview open --json` includes `path`; agent instruction and skill templates are CLI-first; `doctor --fix` adds `Bash(mdview open:*)`.
- [ ] Cleanup removes projects idle > 14 days in one transaction and vacuums when fragmented; no per-file TTL remains.
- [ ] Watcher picks up edits in directories of indexed files (including right after `mdview open` and for projects registered after daemon start), and a reload is broadcast even when a view or search indexed the change first.
- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass.

## Risks

| Risk | Mitigation |
|---|---|
| Old daemon / old `mdview mcp` still running after upgrade | New file `registry-v4.db` isolates them; the CLI restarts a daemon whose version differs; docs say to restart agent sessions. |
| Two processes reset the schema at once | Re-read `user_version` and reset inside one `BEGIN IMMEDIATE`; concurrent-open test. |
| Cross-process write races (`SQLITE_BUSY_SNAPSHOT`) | Every write transaction is `BEGIN IMMEDIATE` so the busy timeout applies. |
| `contentless_delete` support | Bundled SQLite 3.46.0 (verified); test creates the table and deletes by rowid. |
| Parallel worktrees edit `server.rs` | Function-level ownership plus per-owner test modules; P7 merges. |
| Huge neighbour directories / repeated views | Cap 200, skip unchanged, per-project in-flight guard. |
| Repeated or concurrent searches | Single-flight + 10 s skip window, `spawn_blocking`. |

## Validation Log

### Session 1 — 2026-10-09 (autonomous; user away, decisions D1–D9 taken from the conversation)

### Verification Results
- Claims checked: 52
- Verified: 43 | Failed: 5 | Unverified: 4
- Tier: Full
- Failures (all corrected in the phase files): D7 wording said "keep" a 14-day TTL that was 30 days; agent-block markers are `mdview:START/END`, not `BEGIN`; `mcp.rs:103` → `:109`; `project_path` `~510` → `:515`; "Engine.render" was ambiguous — it meant the `render` field (now dropped as unneeded).
- Uncovered callers added to Phase 1: `Engine::list_files` wrapper removal, `save_file` indexed-row guard, `doctor` schema check (moved to Phase 6, read-only).

### Whole-Plan Consistency Sweep
Re-read plan.md and all phase files after applying validation and red-team changes; contract names (`fold`, `confine`, `FileState`, `file_states_for`, `index_docs`, `search_indexed`, `hint_dir`, `view_page -> Result<Option<_>>`, `registry-v4.db`) are used consistently; no references to removed `upsert_file`, `ensure_indexed`, `render_file` remain outside "delete" instructions.

## Red Team Review

### Session — 2026-10-09
**Findings:** 36 raw across 4 reviewers, deduplicated to 20 (18 accepted, 2 rejected)
**Severity breakdown (deduplicated):** 1 Critical, 10 High, 9 Medium

| # | Finding | Severity | Disposition | Applied To |
|---|---------|----------|-------------|------------|
| 1 | Schema reset TOCTOU across processes; fresh DB never stamped | Critical | Accept | Phase 1 (reset in one `BEGIN IMMEDIATE`, always stamp, `registry-v4.db`) |
| 2 | Filesystem link lookup indexes `.env`/`Cargo.toml`; no markdown gate | High | Accept | Phase 1 (`confine`, markdown-only), Phase 3 |
| 3 | Lexical containment lets symlinks escape the root | High | Accept | Phase 1 (`confine` canonicalizes), Phase 2, Phase 3 |
| 4 | `view_page` has no not-markdown fall-through | High | Accept | Phase 3 (`Result<Option<ViewedPage>>`) |
| 5 | Sync/search/render run on tokio workers, no single-flight | High | Accept | Phase 1 (single-flight, 10 s skip, sweep in `spawn_blocking`), Phases 2–4 |
| 6 | Deferred `BEGIN` → instant `database is locked` across processes | High | Accept | Phase 1 (`BEGIN IMMEDIATE` everywhere) |
| 7 | Old daemon/MCP keep running against reshaped schema | High | Accept | Phase 1 (new file), Phase 6 (version restart, docs) |
| 8 | Live reload swallowed when a view/search indexes first | High | Accept | Phase 5 (watcher-owned hash memory) |
| 9 | Scoped watcher misses edits during reconcile gap; stubs not watched | High | Accept | Phase 1 (`indexed_dirs` includes stubs, `hint_dir`), Phase 5 |
| 10 | `đ` not folded by `remove_diacritics 2` | High | Accept | Phase 1 (`fold.rs`, pre-folded FTS), Phase 2 |
| 11 | `doctor --dry-run` would reset the registry | High | Accept | Phase 6 (read-only schema check) |
| 12 | CLI search without project syncs and touches every project | High | Accept | Phase 1 (`search_indexed`) |
| 13 | Phase 3 needs hash/rowid getters from frozen store | High | Accept | Phase 1 (`FileState` fields, `file_state(s_for)`) |
| 14 | `settings.json` edit can wipe user permissions | High | Accept | Phase 6 (Manual on parse failure, timestamped backup, abort on backup failure) |
| 15 | Contentless columns read NULL; `filter_map(ok)` hides it | Medium | Accept | Phase 1 (select from `files`, propagate errors) |
| 16 | Query-plan test asserts the wrong string | Medium | Accept | Phase 1 (`INDEX 0:=`) |
| 17 | Watcher thread lifetime / non-Clone watcher / `remove_root` nesting | Medium | Accept | Phase 5 |
| 18 | Neighbours ignore excludes; no dedupe; excluded rows never pruned | Medium | Accept | Phases 1, 3 |
| 19 | Excerpt `<mark>` injection; `dir` not URL-encoded | Medium | Accept | Phase 2 (sentinels, percent-encoding), Phase 4 |
| 20 | Misc: invalid multi-filter `cargo test` commands; uncommitted P6 files; server test-module ownership; file counts mislabelled; gold plating (bm25 title weight, modified-time display, `pub(crate)` render) | Medium | Accept | All phases; plan.md ownership table |
| — | Refuse `mdview open` for roots equal to `$HOME` | High | Reject | Beyond D1; markdown-only + canonical confinement already blocks credential files; flagged for the user |
| — | Watch parents to catch recreated directories | Medium | Reject | Added complexity for a rare case; documented as a known limitation in Phase 5 |

### Whole-Plan Consistency Sweep
Same sweep as the Validation Log; no unresolved contradictions.
