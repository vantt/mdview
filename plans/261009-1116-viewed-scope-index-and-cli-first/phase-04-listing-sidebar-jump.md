---
phase: 4
title: "Filesystem listing, sidebar and jump palette"
status: completed
priority: P1
effort: "4h"
dependencies: [1]
---

# Phase 4: Filesystem listing, sidebar and jump palette

## Goal

Make the sidebar and the Cmd/Ctrl+K palette independent of what is indexed:
both read a cached filesystem listing, titles come from the index when known,
typing fuzzy-matches path and title with recent files first, and Enter runs a
whole-project content search.

## Ownership (wave A)

- Own: `crates/mdview-core/src/listing.rs`, `crates/mdview-core/src/fuzzy.rs`,
  `crates/mdview/assets/app.js`.
- In `crates/mdview/src/server.rs`: `JumpQuery`, `jump_search`,
  `default_jump_limit`, and a new `#[cfg(test)] mod jump_tests` if needed.
- Do not edit any other file (no CSS — the new palette row reuses the
  existing two-span `.jump-item` markup). Record needs under "Handoff notes".

## Context

- Phase 1 `listing.rs` provides DB-backed `sidebar_files` and `jump_files`;
  `server.rs` callers already use them (sidebar call sites run inside P3's
  `spawn_blocking` in `project_path`; `project_home` and folder landing call
  `sidebar_files` directly).
- `fuzzy::rank_files` (`fuzzy.rs`) ranks `IndexedFile`s by `rel_path`;
  `FuzzyHit { rel_path, title, url, score }` is the palette JSON
  (`app.js:211–300`, fetch at `:275` with `encodeURIComponent`).
- Walk semantics: `indexer::scan_markdown_files` (hidden included,
  `.gitignore` respected, excludes pruned).
- Search URL contract (P2): `/p/:id/_search?q=&scope=project|dir&dir=&sort=`;
  default scope is the whole project.
- `jump_search` accepts an uncapped caller-supplied `limit` today.

## Tasks & Steps

1. **`listing.rs`.**
   - `struct ListingEntry { rel_path: String, modified: SystemTime }`.
   - Process-wide cache `OnceLock<Mutex<HashMap<PathBuf, (Instant, Arc<Vec<ListingEntry>>)>>>`
     keyed by canonical project root; TTL 3 s via a private
     `listing_with_ttl(root, exclude, ttl)` used by tests; hold the lock only
     to read/insert, never during the walk.
   - `sidebar_files`: entries → `IndexedFile`, title from `store.list_files`
     when that row is content-indexed, else filename; sorted by `rel_path`.
   - `jump_files`: blank query → the `limit` most recently modified entries;
     otherwise `fuzzy::rank_items`.
2. **`fuzzy.rs`.** Add `rank_items(items: &[(String /*rel*/, String /*title*/, SystemTime)], project_id, query, limit)`:
   score = max(nucleo score on path, on title); ties → newer first. Keep
   `FuzzyHit`'s shape unchanged and keep `rank_files` working (or make it a
   thin wrapper). Tests: title-only match found; newer wins a tie; blank
   query → empty.
3. **`server.rs` `jump_search`.** Clamp `limit` to 1..=50; run `jump_files`
   inside `tokio::task::spawn_blocking`.
4. **`app.js` palette.**
   - On open, fetch with an empty query and list recent files.
   - When `q` is non-empty, prepend a row "Search content for “q”" (title
     span) + "Enter" (path span), selected by default; Enter on it navigates
     to `/p/<pid>/_search?q=<q>&dir=<current folder>` with every value passed
     through `encodeURIComponent` (scope omitted → whole project, the toggle
     can narrow it). Arrow keys select file rows; Enter on a file row opens it.
   - Build rows with `textContent` only.
5. **Tests.** Listing cache expiry (short TTL), sidebar title merge, jump
   recent ordering, limit clamp.

## Verification

- `cargo test -p mdview-core listing`
- `cargo test -p mdview-core fuzzy`
- `cargo test -p mdview jump`
- `node --check crates/mdview/assets/app.js`
- `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`

## Handoff notes

_(record cross-ownership needs here)_
