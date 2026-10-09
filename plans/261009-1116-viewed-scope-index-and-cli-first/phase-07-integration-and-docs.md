---
phase: 7
title: "Integration, docs, end-to-end verification"
status: pending
priority: P1
effort: "4h"
dependencies: [2, 3, 4, 5, 6]
---

# Phase 7: Integration, docs, end-to-end verification

## Goal

Merge wave A, apply recorded handoff notes, update the owning specs, and
prove the whole flow against a real daemon in an isolated home directory.

## Files to Create / Modify

- Merge results of phases 2–6 on `feat/viewed-scope-index`.
- Any file listed in a phase's "Handoff notes".
- `docs/specs/daemon.md`, `docs/specs/settings.md` and any other spec that
  states indexing, cleanup TTL, search or agent-wiring behaviour — only
  claims that changed.
- `crates/mdview/tests/e2e_open.rs` only if it breaks.
- Report: `plans/reports/cook-261009-viewed-scope-index.md`.

## Tasks & Steps

1. Merge worktree branches in order P6, P5, P4, P2, P3; resolve `server.rs`
   conflicts by keeping each owner's function bodies and test modules.
2. Apply every "Handoff notes" item; re-run the full gate.
3. Dead-code grep: `spawn_refresh_detached`, `index_project`,
   `reindex_links`, `file_abs_paths`, `last_accessed`, `migration_`,
   `FILE_TTL`, `upsert_file`, `ensure_indexed`, `render_file` → none.
4. Update specs/docs for D1–D9 with evidence from code.
5. **End-to-end smoke, isolated.** Never touch the user's daemon (port 7700)
   or `~/.mdview`. Build release; run with `HOME=<scratchpad>/home` and a
   config with `port = 7799`, `host = "127.0.0.1"`; log in with the token
   from that isolated config (do not print it).
   - `mdview open --json <repo>/README.md` → JSON has `path`; DB holds one
     stub row; no background process spawned.
   - GET the page → 200; poll until neighbours are indexed; DB now holds the
     README, its markdown link targets and root-level siblings only.
   - A doc linking to `.env`/`Cargo.toml`: those never appear in the DB.
   - GET `/p/<id>/_search?q=sqlite` → results with `<mark>`, status line;
     DB holds every non-excluded `.md` under the repo (hidden dirs too).
   - Repeat within 10 s → "Index reused"; after 10 s → `0 read`.
   - Edit a viewed file on disk → WebSocket receives a `changed` event.
   - Stop the isolated daemon, delete the scratch home, confirm with
     `pgrep -af "mdview serve"` that only the user's original daemon runs.
6. Measure the isolated DB size after a full content-search sync of
   `/home/vantt/projects/workshop-ecosystem` (read-only use) vs the 24 MB
   baseline; record it.
7. Write the report; mark phases complete.

## Verification

- `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
- All smoke steps pass; no stray processes.
