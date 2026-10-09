---
phase: 3
title: "Index on render: one read + neighbours"
status: pending
priority: P1
effort: "5h"
dependencies: [1]
---

# Phase 3: Index on render: one read + neighbours

## Goal

When a page is opened, read the file once to index it, compute its links and
render it; then, off the request path, index its link targets (1 hop) and its
sibling markdown files, so the viewed neighbourhood is searchable, titled and
backlinked without a full-repo scan.

## Ownership (wave A)

- Own: `crates/mdview-core/src/engine.rs`, `crates/mdview-core/src/render.rs`,
  `crates/mdview-core/src/domain.rs`.
- In `crates/mdview/src/server.rs`: `project_path`, new private helpers used
  only by it, and a new `#[cfg(test)] mod project_path_tests` if needed.
- Do not edit any other file. Record needs under "Handoff notes".

## Context

- `project_path` (`server.rs:515`) calls `ensure_indexed` then `render_file`;
  any path (even `.git/config`) is indexed and rendered today because no
  extension check exists. Phase 1 added `indexer::confine` (markdown +
  canonical root + excludes) and made `build_doc`/`ProjectFs` use it.
- Phase 1 store API: `index_docs`, `file_state`, `file_states_for`
  (`FileState` has `content_hash`, `fts_rowid`, size, mtime, title).
- Phase 1 engine contract: `hint_dir(&Path)` must be called for the directory
  of every file this phase indexes, so P5's watcher starts watching it at once.
- `RenderService::render` resolves every link during its AST walk
  (`render.rs` `walk`); `extract_internal_links` (`render.rs:325`) re-parses.
- The watcher decides live reload on its own (P5), so indexing here never
  suppresses a reload.

## Tasks & Steps

1. **`domain.rs`.** Add `pub links: Vec<String>` to `RenderedPage` (resolved
   project-relative internal targets, sorted, deduped).
2. **`render.rs`.** Collect resolved targets during `walk` into
   `RenderedPage.links`. Keep `extract_internal_links`. Test: two internal
   links + one external → exactly the two targets.
3. **`engine.rs`.**
   ```rust
   pub struct ViewedPage { pub file: IndexedFile, pub page: RenderedPage, pub neighbours: Vec<PathBuf> }
   /// Ok(None) = not a markdown file inside the project (caller falls through
   /// to asset / folder landing / 404). Err = real failure.
   pub fn view_page(&self, project_id: &str, rel_path: &str) -> Result<Option<ViewedPage>>;
   pub fn index_neighbours(&self, project_id: &str, paths: &[PathBuf]) -> Result<usize>;
   ```
   - `view_page`: project lookup; `confine` the path (None → `Ok(None)`);
     read once (size cap); render with `ProjectFs`; build the `IndexedDoc`
     from that content and `page.links`; call `index_docs` only when
     `file_state` is missing, a stub, or its `content_hash` differs; touch
     project access; `hint_dir` the parent.
   - Neighbours = link targets ∪ sibling `.md`/`.markdown` in the same
     directory (non-recursive `read_dir`), minus the file itself, each passed
     through `confine`; then `file_states_for` those rel paths and keep only
     missing / stub / size-or-mtime-changed ones. Cap at 200.
   - `index_neighbours`: per-project in-flight guard (`Mutex<HashSet<String>>`
     field on `Engine`; a second call for a project already running returns
     `Ok(0)`); `build_doc` each path; `index_docs` in batches of 200;
     `hint_dir` each parent.
   - Delete `ensure_indexed` and `render_file`; move their tests to
     `view_page`.
4. **`server.rs` `project_path`.** Run `view_page`, `sidebar_files` and
   `backlinks` inside one `tokio::task::spawn_blocking`. `Ok(Some(..))` →
   render the page as today; then, if `neighbours` is non-empty,
   `spawn_blocking(index_neighbours)` and log a warning on error.
   `Ok(None)` → existing asset / folder-landing / 404 logic unchanged.
   `Err` → `internal_error`.
5. **Tests (engine).**
   - fresh project: `view_page` indexes the file (searchable) and returns its
     link target and siblings as neighbours;
   - unchanged second view: no neighbours left after `index_neighbours`, and
     `fts_rowid` unchanged (`file_state`);
   - edit then view: re-indexed, new text searchable;
   - link to an existing unindexed `.md` renders as a normal link (no broken
     marker in HTML);
   - `.env`, `.git/config`, `Cargo.toml` → `Ok(None)` and never indexed;
     a link to them is not a neighbour;
   - symlink to a file outside the root → `Ok(None)`;
   - `node_modules/x.md` sibling is not a neighbour;
   - cap at 200; second concurrent `index_neighbours` for the same project
     returns `Ok(0)`.

## Verification

- `cargo test -p mdview-core engine`
- `cargo test -p mdview-core render`
- `cargo test -p mdview`
- `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`

## Handoff notes

_(record cross-ownership needs here)_
