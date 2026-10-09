---
phase: 1
title: "Foundation: schema, store, sync, contracts"
status: pending
priority: P1
effort: "1.5d"
dependencies: []
---

# Phase 1: Foundation: schema, store, sync, contracts

## Goal

Replace the registry schema and store API (rowid-linked contentless FTS over
diacritic-folded text, no migrations, `BEGIN IMMEDIATE` batched writes), add
the single-flight incremental project sync, confine every indexed path to
markdown inside the canonical project root, switch link resolution to the
filesystem, and create the module contracts wave A fills in — leaving the
workspace compiling and green.

## Context

- Store: `crates/mdview-core/src/repository.rs` (`from_conn` :31, `SCHEMA`
  :673, migrations :496–648, cleanup `cleanup_stale` :415, search :444).
- FTS deletes today filter on `UNINDEXED` columns (`repository.rs:93`, `:134`,
  `:198`) — a full FTS scan per call.
- Contentless FTS5 columns read back as NULL and `snippet()` is unavailable;
  `search` currently drops failing rows with `filter_map(|r| r.ok())`
  (`repository.rs:479`), which would silently return nothing.
- `unicode61 remove_diacritics 2` does **not** fold `đ` (verified on SQLite
  3.46: content "được" does not match query "duoc"). Folding must be done by
  mdview before insert and on the query.
- `resolve_to_rel` tries the exact path first (`link_resolver.rs:76–81`);
  `index_file` has no extension check (`indexer.rs:42–75`); containment is
  lexical (`normalize` :37, `rel_path_str`); `asset_path` already
  canonicalizes (`engine.rs:431–433`). A filesystem lookup without a markdown
  + canonical check would index `.env`, `Cargo.toml`, or symlinked files
  outside the root.
- `save_file` (`engine.rs:343–356`) only writes rows already in the index
  (`tests/e2e_open.rs:612–620` relies on the 404) — keep that guard.
- `DaemonInfo.version` exists (`daemon.rs:26`); `doctor.rs:154` already
  compares it (used by P6).
- `unicode-normalization 0.1.25` is already in `Cargo.lock` (transitive).

## Files to Create / Modify

- Modify: `crates/mdview-core/src/repository.rs`, `indexer.rs`,
  `link_resolver.rs`, `domain.rs`, `engine.rs`, `lib.rs`, `config.rs`
- Modify: root `Cargo.toml`, `crates/mdview-core/Cargo.toml`, `Cargo.lock`
  (add `unicode-normalization` as a direct dependency)
- Create: `crates/mdview-core/src/fold.rs`, `sync.rs`, `search.rs`,
  `listing.rs`, `snippet.rs` (module doc comment only; P2 owns it)
- Modify (call sites / compile fixes only): `crates/mdview/src/server.rs`,
  `cli.rs`, `views.rs` (`project_list_page` label only), `watch.rs` (test
  helper only)
- Modify: `crates/mdview/src/cleanup.rs`

## Contracts (wave A codes against these exact names)

```rust
// fold.rs — the ONE text fold used by FTS insert, FTS query and excerpts.
/// NFD, drop combining marks, map đ/Đ → d, lowercase.
pub fn fold(s: &str) -> String;

// domain.rs
pub struct SearchResult { pub project_id: String, pub rel_path: String, pub title: String,
                          pub excerpt: String, pub url: String, pub score: f64,
                          pub modified_at: String }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchSort { #[default] Relevance, Recent }
#[derive(Debug, Clone, Default, Serialize)]
pub struct SyncStats { pub files_seen: usize, pub files_read: usize, pub files_removed: usize,
                       pub elapsed_ms: u128, pub skipped_recent: bool }
#[derive(Debug, Clone, Default)]
pub struct SearchOutcome { pub results: Vec<SearchResult>, pub sync: SyncStats,
                           pub sync_error: Option<String> }

// indexer.rs
pub struct IndexedDoc { pub file: IndexedFile, pub content: String, pub links: Vec<String> }
pub fn is_markdown(p: &Path) -> bool;                       // .md / .markdown, case-insensitive
pub fn is_excluded(rel: &str, exclude: &[String]) -> bool;  // any path component equals a pattern
/// Canonicalize `abs`; Some(canonical) only if it is a markdown file inside the
/// canonical `root` and no component of its root-relative path is excluded.
pub fn confine(root: &Path, abs: &Path, exclude: &[String]) -> Option<PathBuf>;
impl IndexService {
    /// confine + size cap + read + title. None if skipped.
    pub fn read_file(project: &Project, abs: &Path, max_bytes: u64, exclude: &[String])
        -> Option<(IndexedFile, String)>;
    /// read_file + render::extract_internal_links with ProjectFs.
    pub fn build_doc(project: &Project, abs: &Path, max_bytes: u64, exclude: &[String])
        -> Option<IndexedDoc>;
}

// link_resolver.rs
/// Link-target existence answered by the filesystem: `confine(root, abs, exclude).is_some()`.
pub struct ProjectFs<'a> { pub root: &'a Path, pub exclude: &'a [String] }
impl IndexLookup for ProjectFs<'_> { /* ... */ }

// repository.rs
pub const SCHEMA_VERSION: i64 = 4;
#[derive(Debug, Clone)]
pub struct FileState { pub rel_path: String, pub abs_path: PathBuf, pub title: String,
                       pub size_bytes: u64, pub modified_at: String, pub content_hash: String,
                       pub fts_rowid: i64 }
impl FileState { pub fn content_indexed(&self) -> bool { !self.content_hash.is_empty() } }
impl SqliteStore {
    /// One BEGIN IMMEDIATE txn. Upserts rows, rewrites FTS only when content_hash
    /// changed or fts_rowid == 0, replaces each doc's outgoing links.
    /// Returns per-doc "content changed".
    pub fn index_docs(&self, docs: &[IndexedDoc]) -> Result<Vec<bool>>;
    pub fn register_known_path(&self, f: &IndexedFile) -> Result<()>;   // unchanged semantics
    pub fn delete_file(&self, project_id: &str, rel_path: &str) -> Result<()>;
    pub fn delete_files(&self, project_id: &str, rel_paths: &[String]) -> Result<usize>;
    pub fn delete_project(&self, id: &str) -> Result<()>;
    pub fn file_state(&self, project_id: &str, rel_path: &str) -> Result<Option<FileState>>;
    pub fn file_states_for(&self, project_id: &str, rel_paths: &[String])
        -> Result<HashMap<String, FileState>>;
    pub fn file_states(&self, project_id: &str) -> Result<HashMap<String, FileState>>;
    /// Parent dirs of ALL file rows (stubs included), across all projects.
    pub fn indexed_dirs(&self) -> Result<HashSet<PathBuf>>;
    /// Selects only `files` columns (never files_fts.<col> / snippet()); propagates row errors.
    pub fn search(&self, query: &str, project_id: Option<&str>, dir_prefix: Option<&str>,
                  sort: SearchSort, limit: usize) -> Result<Vec<SearchResult>>; // excerpt = ""
    pub fn cleanup_stale(&self, project_cutoff: &str) -> Result<usize>;
    pub fn vacuum_if_fragmented(&self) -> Result<bool>; // freelist_count * 4 > page_count
}

// engine.rs (P3 owns this file in wave A; these are fixed contracts)
impl Engine {
    /// Install the watcher's directory-hint sender (P5 calls it once).
    pub fn set_dir_hint_sender(&self, tx: std::sync::mpsc::Sender<PathBuf>);
    /// Best-effort, non-blocking notify that `dir` now holds an indexed row.
    pub fn hint_dir(&self, dir: &Path);
}
// sync.rs
impl Engine {
    /// Single-flight per project; skips (skipped_recent = true) when the last
    /// completed sync of this project finished < 10 s ago.
    pub fn sync_project(&self, project_id: &str) -> Result<SyncStats>;
}
// search.rs
impl Engine {
    /// touch project + sync (errors logged and returned in sync_error, query still runs) + search.
    pub fn search_content(&self, project_id: &str, query: &str, dir_prefix: Option<&str>,
                          sort: SearchSort, limit: usize) -> Result<SearchOutcome>;
    /// Already-indexed rows only, all projects, no sync, no touch (CLI without a project).
    pub fn search_indexed(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;
}
// listing.rs
impl Engine {
    pub fn sidebar_files(&self, project_id: &str) -> Result<Vec<IndexedFile>>;
    pub fn jump_files(&self, project_id: &str, query: &str, limit: usize) -> Result<Vec<FuzzyHit>>;
}
```

## Tasks & Steps

0. **Branch and baseline.** `git switch -c feat/viewed-scope-index`. Commit the
   already-modified `CLAUDE.md`, `AGENTS.md`, `docs/mdview-agents-template.md`,
   `docs/mdview-skill-template.md` (the `--json` CLI change) as
   `docs: use mdview open --json in agent instructions` before any code
   change, so P6's worktree starts with them. Commit the plan directory too.
1. **Fold.** `fold.rs` per contract with tests: "Đường đi được" → "duong di
   duoc"; ASCII unchanged; idempotent.
2. **Schema, no migrations (D2).**
   - `config::registry_db_path()` → `data_dir().join("registry-v4.db")` (a new
     file per schema generation keeps old binaries, old daemons and old
     `mdview mcp` processes on their own file instead of failing every write
     against a reshaped one). Add `config::legacy_registry_paths()` returning
     `registry.db`, `registry.db-wal`, `registry.db-shm`.
   - New `SCHEMA`: `projects` (unchanged columns); `files` (no
     `last_accessed_at`; add `fts_rowid INTEGER NOT NULL DEFAULT 0`); indexes
     `idx_files_project`, `idx_files_hash(path_hash)`, `idx_files_fts(fts_rowid)`;
     `links` + `idx_links_target`;
     `CREATE VIRTUAL TABLE files_fts USING fts5(title, content, content='', contentless_delete=1, tokenize='unicode61 remove_diacritics 2')`.
   - `from_conn`: inside one `BEGIN IMMEDIATE` transaction re-read
     `user_version`; if it is neither 0-with-no-tables nor `SCHEMA_VERSION`,
     drop `files_fts`, `links`, `files`, `projects`; run `SCHEMA`; always set
     `user_version = SCHEMA_VERSION`; `COMMIT`. Log one `tracing::info!` when
     tables were dropped. Delete `MigrationStep`, `MIGRATIONS`, `migrate`,
     `migration_*`, `backfill_*`, `has_column` and their tests.
   - Keep `schema_report` returning `(user_version, rows with empty path_hash)`.
3. **Writes (D3).** Every write transaction uses `BEGIN IMMEDIATE` (so the
   15 s busy timeout applies across processes): `index_docs`, `delete_files`,
   `delete_project`, `cleanup_stale`. `index_docs`: per doc read
   `(content_hash, fts_rowid)` by primary key; upsert the row; if the hash is
   unchanged and `fts_rowid > 0` skip FTS; else delete the old FTS row by
   rowid (when > 0), `INSERT INTO files_fts(title, content) VALUES(fold(title), fold(content))`,
   store `last_insert_rowid()`; replace links. Delete `upsert_file` and
   `IndexService::index_file`/`index_project` (no remaining callers after
   step 8).
4. **Search (D4/D6).** `fts_sanitize` folds each token with `fold` before
   quoting as a prefix term. SQL:
   `SELECT f.project_id, f.rel_path, f.title, f.modified_at, bm25(files_fts) FROM files_fts JOIN files f ON f.fts_rowid = files_fts.rowid WHERE files_fts MATCH ?1 AND (?2 IS NULL OR f.project_id = ?2) AND (?3 IS NULL OR f.rel_path LIKE ?3 ESCAPE '\')`
   where `?3` is the escaped prefix + `/%`; `ORDER BY bm25(files_fts)` for
   `Relevance`, `f.modified_at DESC` for `Recent`; `LIMIT`. Collect with
   `collect::<Result<Vec<_>, _>>()?`.
5. **Cleanup (D7).** `cleanup_stale(project_cutoff)` deletes stale projects
   with all their rows in one transaction. Remove `touch_file_access` and its
   test helpers. `crates/mdview/src/cleanup.rs`: drop `FILE_TTL_SECS`; set
   `PROJECT_TTL_SECS = 14 * 24 * 60 * 60`; the periodic task runs
   `sweep_once` inside `tokio::task::spawn_blocking`; `sweep_once` calls
   `cleanup_stale` and, when anything was removed, `vacuum_if_fragmented`.
   Update module docs and tests (TTL test asserts 14 days).
6. **Confinement (security).** Implement `is_markdown`, `is_excluded`
   (moved from `engine.rs::is_excluded_path` and the walker's `filter_entry`
   predicate; both call sites use it), `confine`. `read_file`/`build_doc`
   return `None` for anything `confine` rejects. `ProjectFs` uses `confine`.
   `Engine::view_file` (CLI/MCP) returns an error for a non-markdown file.
   Tests: link to `.env` / `Cargo.toml` is broken and never indexed; a
   symlinked dir pointing outside the root is rejected; excluded component
   (`node_modules/x.md`) is rejected; `../` escape rejected.
7. **Sync (D3/D6), `sync.rs`.** Single-flight: a process-wide
   `Mutex<HashMap<String, Arc<Mutex<Option<Instant>>>>>` keyed by project id;
   hold the per-project lock for the duration; if the stored instant is
   < 10 s old return `SyncStats { skipped_recent: true, .. }`. Walk with
   `scan_markdown_files(root, exclude)`; stat each file; skip when the state
   is content-indexed with equal size and `modified_at`; else `build_doc`
   into a batch flushed every 200 via `index_docs`; then `delete_files` for
   states whose file no longer exists **or** whose rel path is now excluded
   (never because a file is gitignored). Call `hint_dir` for parent dirs of
   newly indexed files. `Engine::refresh` is removed; callers use
   `sync_project`.
8. **Engine.** Add the `dir_hint` field (`Mutex<Option<Sender<PathBuf>>>`)
   and the two contract methods. `index_file_incremental` = `build_doc` +
   `index_docs` (one read). `save_file` uses the same, keeping its
   indexed-row guard. Delete `reindex_links`, `compute_file_links`, `search`,
   `fuzzy_files`, the `list_files` wrapper, `file_abs_paths` users;
   `record_access` touches only the project; `render_file`/`ensure_indexed`
   change only as far as compiling with `ProjectFs` requires (P3 replaces
   them). `register_known_path_stub` calls `hint_dir`.
9. **Contract modules.** `search.rs`: `search_content` and `search_indexed`
   per contract (excerpt empty; P2 fills). `listing.rs`: `sidebar_files` =
   `store.list_files`; `jump_files` = `fuzzy::rank_files` over it (P4
   replaces). Register `fold`, `sync`, `search`, `listing`, `snippet` in
   `lib.rs`.
10. **Call sites.** `server.rs`: `search_page` → `search_content(.., None,
    SearchSort::Relevance, 30)`, render `.results`; `jump_search` →
    `jump_files`; `project_home`, `project_path` sidebar and folder landing
    → `sidebar_files`. `cli.rs`: `cmd_search` with a project →
    `search_content`, without → `search_indexed`; `cmd_refresh` →
    `sync_project`, print `SyncStats`; `cmd_list` / register message say
    "indexed files". `views.rs` `project_list_page`: "{count} indexed files".
    Leave `spawn_refresh_detached` calls (P6 removes them).
11. **Tests.** Rewrite or delete tests that used removed APIs: `repository.rs`
    migration/schema (~767–907), `file_abs_paths` (~984), search
    (~999–1116), last-accessed/touch (~1011–1030), `cleanup_stale`
    (~1049–1092); `engine.rs` refresh/render_file/ensure_indexed/search/
    last-accessed (~575–900); `indexer.rs` `index_project` (~262);
    `cleanup.rs` file-TTL (~73, ~152); `watch.rs` setup helper. Add:
    - reset: build a v3 DB in-test from the old `SCHEMA` text with
      `user_version = 3` and a row; open → tables rebuilt, `user_version == 4`,
      row gone;
    - fresh DB opened twice keeps its rows (stamp happens on create);
    - two threads opening the same v3 file concurrently: rows written after
      the first reset survive;
    - FTS delete by rowid: `EXPLAIN QUERY PLAN` of the delete contains
      `VIRTUAL TABLE INDEX 0:=` (rowid lookup), not a bare `INDEX 0:`;
    - unchanged content keeps `fts_rowid`; changed content replaces it and the
      old text no longer matches;
    - `sync_project`: indexes new files; second run within 10 s is
      `skipped_recent`; after the window `files_read == 0`; modified file
      re-read; deleted file pruned; gitignored-but-viewed row kept; excluded
      row pruned;
    - search: `dir_prefix` with `%`/`_` in the folder name, `Recent` order,
      "duoc" matches "được", "tai lieu" matches "tài liệu", titles come back
      non-empty;
    - `save_file` on an unindexed path still returns `FileNotFound`.

## Verification

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `grep -n "files_fts WHERE" crates/mdview-core/src/repository.rs` shows only `rowid` filters and `MATCH`.
- `grep -rn "migration_\|backfill_\|last_accessed\|upsert_file\|index_project" crates/` returns nothing.

## Handoff

Commit on `feat/viewed-scope-index` as
`refactor(core): rowid-linked contentless FTS, incremental sync, schema reset`.
Wave A worktrees branch from this commit.
