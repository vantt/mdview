# Phase 1 foundation report

Status: done. fmt, clippy (-D warnings) and `cargo test --workspace` are green; both grep checks are clean.

## Contract deviations

None. All signatures in the phase-01 Contracts block are implemented as written.

## Things wave-A phases must know

- `Engine::sync_project_within(id, window)` is `pub(crate)` (tests use `Duration::ZERO` to bypass the 10 s skip window). Sync state is a process-wide map keyed by project id, so tests that sync must use a unique project id (unique temp dir name).
- `Engine::max_bytes()` is now `pub(crate)` (sync.rs needs it).
- `Engine::register_known_path_stub` takes `&Project` and refuses (via `confine`) any path that is not a markdown file inside the canonical root; `Engine::view_file` returns `Error::InvalidPath` for a non-markdown path.
- `record_access(project_id)` touches only the project.
- `index_file_incremental` is `build_doc` + `index_docs` and also calls `hint_dir` for the file's directory. It returns `false` for anything `confine` rejects.
- `search_page` in server.rs now calls `search_content(...)` directly on the async worker; the `spawn_blocking` wrapping is P2's. `jump_search`/sidebar call sites call `jump_files`/`sidebar_files` synchronously as before (P4).
- `SqliteStore::is_content_indexed` is kept (used by `ensure_indexed`) and now delegates to `file_state`.
- FTS rowids can be reused after a delete (SQLite reuses the max rowid); `fts_rowid` equality is therefore not a "content changed" signal, use `index_docs`' returned flags or `content_hash`.
- `snippet.rs` is a module-doc-only file for P2.
- `doctor.rs` still references `registry_db_path()`; it now points at `registry-v4.db`. `legacy_registry_paths()` exists for P6's `doctor --fix`.
- `spawn_refresh_detached` call sites in cli.rs/mcp.rs are untouched (P6). `mdview refresh` (the detached child) now runs `sync_project`, so a detached refresh within 10 s of another sync of the same project is a no-op.

## Out-of-list change

- `crates/mdview/src/auth.rs`: `cargo fmt --all` reformatted one pre-existing test assertion (the baseline was not fmt-clean). Included so `cargo fmt --all --check` passes.
- `code_source.rs` has a doc comment mentioning `engine::is_excluded_path` (now `indexer::is_excluded`); not in the ownership list, left unchanged.
