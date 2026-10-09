---
phase: 2
title: "Content search UX and excerpts"
status: pending
priority: P1
effort: "4h"
dependencies: [1]
---

# Phase 2: Content search UX and excerpts

## Goal

Give content search real excerpts (read from disk, matched with the shared
fold), scope and sort toggles, a sync status line and visible errors, and run
the search off the async workers.

## Ownership (wave A)

- Own: `crates/mdview-core/src/search.rs`, `crates/mdview-core/src/snippet.rs`,
  `crates/mdview/assets/app.css`.
- In `crates/mdview/src/server.rs`: `SearchQuery`, `search_page`, new private
  helpers used only by them, and a new `#[cfg(test)] mod search_page_tests`.
- In `crates/mdview/src/views.rs`: `search_page`, `highlight_excerpt`, new
  private helpers used only by them, and tests for them.
- Do not edit any other file. Record needs under "Handoff notes".

## Context

- Phase 1: `Engine::search_content(project_id, query, dir_prefix, sort, limit) -> SearchOutcome`
  (results with `excerpt = ""`, `sync: SyncStats`, `sync_error`),
  `Engine::search_indexed` (CLI, no sync), `mdview_core::fold::fold`.
- Today `views::highlight_excerpt` (`views.rs:756`) escapes the excerpt and
  turns literal `<mark>` text back into tags — any document containing the
  text `<mark>` would inject markup once excerpts come from raw disk text.
- `esc` (`views.rs:909`) HTML-escapes only; it does not URL-encode.
- UI copy is English. Default scope is the whole project (D6).

## Tasks & Steps

1. **`snippet.rs`.**
   - Markers are private-use sentinels: `pub const MARK_OPEN: char = '\u{E000}'`,
     `pub const MARK_CLOSE: char = '\u{E001}'`. Strip any pre-existing
     `\u{E000}`/`\u{E001}` from the content first.
   - `pub fn query_terms(query: &str) -> Vec<String>`: split on
     non-alphanumeric like `fts_sanitize`, fold each with `fold::fold`.
   - `pub fn excerpt(content: &str, terms: &[String], max_words: usize) -> String`:
     collapse whitespace; split into words; fold each word; pick the window of
     `max_words` (callers pass 24) with the most prefix matches; wrap each
     matching original word in the sentinels; prefix/suffix `…` when cut. No
     match → first `max_words` words.
   - Tests: "tai lieu" marks "tài liệu"; "duoc" marks "được"; prefix
     ("index" marks "indexing"); window centring; no-match fallback;
     multi-byte safety; content containing literal `<mark>` and sentinel
     chars is neutralised.
2. **`search.rs`.** Fill `excerpt` for each result in both `search_content`
   and `search_indexed`: resolve the absolute path with
   `indexer::confine(project.root_path, root.join(rel), exclude)`, respect
   `max_file_size_mb`, empty excerpt on any failure.
3. **`views.rs` `highlight_excerpt`.** `esc` the text, then replace the
   sentinels with `<mark class="fg-mark">` / `</mark>`. Literal `<mark>` text
   stays escaped.
4. **`server.rs`.** `SearchQuery { q, scope: project|dir (default project), dir, sort: relevance|recent (default relevance) }`.
   Private helper `fn search_params(&SearchQuery) -> (Option<String> /*dir_prefix*/, SearchSort)`:
   normalise `dir` (trim `/`, reject any `..` or absolute component → no
   prefix); prefix only when `scope=dir` and `dir` non-empty. `search_page`
   runs `search_content` inside `tokio::task::spawn_blocking` (clone the
   `Arc<Engine>`); a join or engine error renders an error message, not an
   empty result list.
5. **`views.rs` `search_page`.** Signature
   `search_page(project, query, dir: &str, scope_is_dir: bool, sort: SearchSort, outcome: Result<&SearchOutcome, &str>)`.
   Render the query form (hidden `scope`, `dir`, `sort`); a toggle row of
   links *Whole project* / *This folder* (only when `dir` non-empty) and
   *Relevance* / *Newest*, built with a private `search_href` that
   percent-encodes every query value (`q`, `scope`, `dir`, `sort`) and then
   HTML-escapes the URL; the active option has `aria-current="true"`; a
   status line `Synced {files_seen} files ({files_read} read) in {s:.2} s`,
   or `Index reused (synced moments ago)` when `skipped_recent`, or
   `Sync failed: {err} — showing indexed results` when `sync_error`; results
   with title, rel path and excerpt.
6. **`app.css`.** Toggle row and status line using existing `fg-*` tokens;
   no new colours.
7. **Tests.** `search_page_tests`: defaults, `scope=dir` with empty `dir`,
   `..` rejection, `dir` containing `a&sort=recent#x` is percent-encoded in
   every toggle href. View tests: toggles, sentinels → `<mark>`, literal
   `<mark>` escaped, error and skipped status lines.

## Verification

- `cargo test -p mdview-core snippet`
- `cargo test -p mdview-core search`
- `cargo test -p mdview search_page`
- `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`

## Handoff notes

_(record cross-ownership needs here)_
