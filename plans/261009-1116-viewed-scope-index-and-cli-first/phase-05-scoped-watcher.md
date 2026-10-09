---
phase: 5
title: "Watcher scoped to indexed dirs"
status: pending
priority: P2
effort: "4h"
dependencies: [1]
---

# Phase 5: Watcher scoped to indexed dirs

## Goal

Watch only directories that hold indexed rows (stubs included), non-
recursively; add a directory the moment the engine hints it; reconcile
periodically; and decide live reload from the watcher's own content-hash
memory, so indexing done by views or searches never swallows a reload.

## Ownership (wave A)

- Own: `crates/mdview/src/watch.rs` only.
- Do not edit any other file. Record needs under "Handoff notes".

## Context

- `spawn_watchers` (`watch.rs:46`) recursively watches every project root
  known at start (`watch.rs:63–72`); `WatchHandle` is a type alias
  (`watch.rs:22`) bound in `server.rs:65` as `let _watch = ...` (P5 must keep
  that call compiling unchanged — a newtype named `WatchHandle` is fine).
- `reindex_paths` broadcasts `Changed` only when `index_file_incremental`
  reports a DB content change (`watch.rs:98–104`). After this plan, views
  and searches also index changed files, so the DB flag can already be
  consumed — the watcher needs its own memory.
- Phase 1: `store.indexed_dirs()` (all rows, stubs included),
  `Engine::set_dir_hint_sender(Sender<PathBuf>)`, `indexer::is_markdown`.
- notify facts: `INotifyWatcher` is not `Clone`; `Debouncer::watcher()` and
  `cache()` take `&mut self`; `FileIdMap::remove_root` removes nested roots
  too (`notify-debouncer-full` `cache.rs:67–70`); dropping the debouncer
  stops it.

## Tasks & Steps

1. **Handle and lifetime.** `pub struct WatchHandle { _inner: Arc<Mutex<Debouncer<..>>>, stop: Arc<AtomicBool> }`
   with `Drop` setting `stop`. The reconcile thread holds a `Weak` to the
   debouncer and the `stop` flag; it exits when `stop` is set or the `Weak`
   no longer upgrades.
2. **Watch set.** Track the watched set in the thread's own `HashSet<PathBuf>`.
   Add with `RecursiveMode::NonRecursive` + `cache().add_root(dir, NonRecursive)`;
   remove with `unwatch` only (do not call `remove_root`, which would drop
   nested roots from the cache). Pure helper
   `diff_watch_sets(current, desired) -> (to_add, to_remove)`.
3. **Hints + reconcile.** In `spawn_watchers`, create an mpsc channel, call
   `engine.set_dir_hint_sender(tx)`. Thread loop: `recv_timeout(5 s)`; a hint
   adds that dir immediately (if it exists); on timeout recompute
   `indexed_dirs()` and apply the diff (only dirs that exist). A `Remove`
   event for a watched directory drops it from the set so the next
   reconcile can re-add it if it reappears. Errors are logged at debug.
4. **Reload decision.** Keep a `HashMap<PathBuf, String>` of the last
   content hash the watcher saw per file (seeded lazily). On a change event
   for a markdown file: read it, hash with `indexer::content_hash`, index it
   via `index_file_incremental` (result ignored for broadcast), and emit
   `Changed` when the hash differs from the remembered one or none is
   remembered; then remember it. Removal behaviour unchanged (`Removed`
   always).
5. **Tests.** `diff_watch_sets`; reload still emitted when the DB was
   already updated by someone else before the watcher ran (index the new
   content first, then `reindex_paths` → one `Changed`); identical rewrite
   after a remembered hash → no event; dropping `WatchHandle` stops the
   thread (join with timeout); existing `reindex_paths` tests updated to the
   Phase 1 API.

## Verification

- `cargo test -p mdview watch`
- `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`

## Known limitation (documented, accepted)

A directory deleted and recreated loses its rows, so it is not re-watched
until a file in it is viewed or searched again; reloading the page recovers.

## Handoff notes

_(record cross-ownership needs here)_
