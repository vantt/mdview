//! SQLite adapter: project registry + file index + FTS5 search.
//! Behind a `Mutex<Connection>` so it is Send+Sync for the async daemon.
//!
//! The registry is a disposable cache: a database of another schema version is
//! dropped and rebuilt rather than migrated. Full text lives in a contentless
//! FTS5 table (`files_fts`) linked to `files` by rowid (`files.fts_rowid`), so
//! every FTS delete/update is a rowid lookup, never a scan.

use crate::domain::{IndexedFile, Project, SearchResult, SearchSort};
use crate::error::Result;
use crate::fold::fold;
use crate::indexer::{self, IndexedDoc};
use crate::short_link;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Schema version this build expects. A database stamped with any other
/// version is dropped and recreated on open.
pub const SCHEMA_VERSION: i64 = 4;

/// What the index knows about one file — enough for a caller to decide whether
/// re-reading it from disk is worth it.
#[derive(Debug, Clone)]
pub struct FileState {
    pub rel_path: String,
    pub abs_path: PathBuf,
    pub title: String,
    pub size_bytes: u64,
    pub modified_at: String,
    pub content_hash: String,
    pub fts_rowid: i64,
}

impl FileState {
    /// `false` for a `register_known_path` stub (path known, content unread).
    pub fn content_indexed(&self) -> bool {
        !self.content_hash.is_empty()
    }
}

pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    /// Open (creating if needed) the registry DB, rebuilding it when its
    /// schema version differs from [`SCHEMA_VERSION`].
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::from_conn(conn)
    }

    /// In-memory store (tests).
    pub fn open_in_memory() -> Result<Self> {
        Self::from_conn(Connection::open_in_memory()?)
    }

    fn from_conn(conn: Connection) -> Result<Self> {
        // Multiple processes share this DB (daemon + CLI + MCP) — without a
        // busy timeout a writer that loses the race gets an immediate
        // "database is locked" error instead of waiting the brief moment WAL
        // contention actually needs. Set first so the pragmas below wait too.
        conn.busy_timeout(std::time::Duration::from_secs(15)).ok();
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "foreign_keys", "ON").ok();
        init_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Run `f` inside one `BEGIN IMMEDIATE` transaction. IMMEDIATE takes the
    /// write lock up front, so the busy timeout applies across processes
    /// instead of a deferred transaction failing instantly with
    /// `SQLITE_BUSY_SNAPSHOT` when it tries to upgrade.
    fn write_txn<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let c = self.conn.lock().unwrap();
        c.execute_batch("BEGIN IMMEDIATE")?;
        let out = f(&c).and_then(|v| c.execute_batch("COMMIT").map(|_| v).map_err(Into::into));
        if out.is_err() {
            let _ = c.execute_batch("ROLLBACK");
        }
        out
    }

    // ---- projects ----

    pub fn upsert_project(&self, p: &Project) -> Result<()> {
        let c = self.conn.lock().unwrap();
        c.execute(
            "INSERT INTO projects(id,name,root_path,created_at,last_seen_at)
             VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(id) DO UPDATE SET name=?2, root_path=?3, last_seen_at=?5",
            params![
                p.id,
                p.name,
                p.root_path.to_string_lossy(),
                p.created_at,
                p.last_seen_at
            ],
        )?;
        Ok(())
    }

    pub fn get_project(&self, id: &str) -> Result<Option<Project>> {
        let c = self.conn.lock().unwrap();
        let mut stmt = c.prepare(
            "SELECT id,name,root_path,created_at,last_seen_at FROM projects WHERE id=?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        Ok(rows.next()?.map(row_to_project))
    }

    pub fn find_project_by_root(&self, root: &Path) -> Result<Option<Project>> {
        let c = self.conn.lock().unwrap();
        let mut stmt = c.prepare(
            "SELECT id,name,root_path,created_at,last_seen_at FROM projects WHERE root_path=?1",
        )?;
        let mut rows = stmt.query(params![root.to_string_lossy()])?;
        Ok(rows.next()?.map(row_to_project))
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let c = self.conn.lock().unwrap();
        let mut stmt = c.prepare("SELECT id,name,root_path,created_at,last_seen_at FROM projects ORDER BY last_seen_at DESC")?;
        let rows = stmt.query_map([], |r| Ok(row_to_project(r)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Drop a project and every row it owns. Deletes only registry rows —
    /// never anything on disk.
    pub fn delete_project(&self, id: &str) -> Result<()> {
        self.write_txn(|c| delete_project_in(c, id))
    }

    // ---- files ----

    /// Upsert a batch of documents in one transaction: file rows, FTS rows and
    /// each doc's outgoing links. The FTS row is rewritten only when the
    /// content hash changed (or the row has none yet), so re-indexing an
    /// unchanged file costs no FTS work. Returns, per doc and in order,
    /// whether its *content* changed from what was stored (a brand-new row
    /// counts as changed).
    pub fn index_docs(&self, docs: &[IndexedDoc]) -> Result<Vec<bool>> {
        if docs.is_empty() {
            return Ok(Vec::new());
        }
        self.write_txn(|c| {
            let mut changed = Vec::with_capacity(docs.len());
            for doc in docs {
                changed.push(index_doc_in(c, doc)?);
            }
            Ok(changed)
        })
    }

    /// Register that `rel_path` exists at `abs_path`, without reading its
    /// content — cheap enough to call on every `view_file`. This is what lets
    /// `/s/<code>` resolve in O(1) from the moment `view_file` hands the code
    /// out, instead of needing `resolve_short_code`'s full-tree fallback scan
    /// the first time the link is clicked.
    ///
    /// `content_hash` stays at its `''` default — the sentinel for "row
    /// exists, content not read yet" — so [`FileState::content_indexed`] can
    /// tell a stub from a real row. A no-op if the row already exists (stub or
    /// real): never overwrites real indexed data with a placeholder.
    pub fn register_known_path(&self, f: &IndexedFile) -> Result<()> {
        let c = self.conn.lock().unwrap();
        c.execute(
            "INSERT INTO files(project_id,rel_path,abs_path,title,size_bytes,modified_at,path_hash,content_hash)
             VALUES(?1,?2,?3,?4,?5,?6,?7,'')
             ON CONFLICT(project_id,rel_path) DO NOTHING",
            params![
                f.project_id,
                f.rel_path,
                f.abs_path.to_string_lossy(),
                f.title,
                f.size_bytes as i64,
                f.modified_at,
                short_link::path_hash(&f.project_id, &f.rel_path),
            ],
        )?;
        Ok(())
    }

    /// Whether `rel_path`'s row (if any) carries real content-derived data —
    /// `false` for a `register_known_path` stub or a missing row.
    pub fn is_content_indexed(&self, project_id: &str, rel_path: &str) -> Result<bool> {
        Ok(self
            .file_state(project_id, rel_path)?
            .is_some_and(|s| s.content_indexed()))
    }

    pub fn delete_file(&self, project_id: &str, rel_path: &str) -> Result<()> {
        self.write_txn(|c| delete_file_in(c, project_id, rel_path).map(|_| ()))
    }

    /// Delete many files in one transaction; returns how many rows existed.
    pub fn delete_files(&self, project_id: &str, rel_paths: &[String]) -> Result<usize> {
        if rel_paths.is_empty() {
            return Ok(0);
        }
        self.write_txn(|c| {
            let mut n = 0;
            for rel in rel_paths {
                if delete_file_in(c, project_id, rel)? {
                    n += 1;
                }
            }
            Ok(n)
        })
    }

    // ---- file state ----

    pub fn file_state(&self, project_id: &str, rel_path: &str) -> Result<Option<FileState>> {
        let c = self.conn.lock().unwrap();
        file_state_in(&c, project_id, rel_path)
    }

    /// States for the given paths; paths with no row are absent from the map.
    pub fn file_states_for(
        &self,
        project_id: &str,
        rel_paths: &[String],
    ) -> Result<HashMap<String, FileState>> {
        let c = self.conn.lock().unwrap();
        let mut out = HashMap::with_capacity(rel_paths.len());
        for rel in rel_paths {
            if let Some(state) = file_state_in(&c, project_id, rel)? {
                out.insert(rel.clone(), state);
            }
        }
        Ok(out)
    }

    /// State of every file row of a project (stubs included), keyed by rel path.
    pub fn file_states(&self, project_id: &str) -> Result<HashMap<String, FileState>> {
        let c = self.conn.lock().unwrap();
        let mut stmt = c.prepare(&format!(
            "SELECT {FILE_STATE_COLS} FROM files WHERE project_id=?1"
        ))?;
        let rows = stmt.query_map(params![project_id], row_to_file_state)?;
        let mut out = HashMap::new();
        for r in rows {
            let state = r?;
            out.insert(state.rel_path.clone(), state);
        }
        Ok(out)
    }

    /// Parent directories of ALL file rows (stubs included), across all
    /// projects — the set the filesystem watcher needs to cover.
    pub fn indexed_dirs(&self) -> Result<HashSet<PathBuf>> {
        let c = self.conn.lock().unwrap();
        let mut stmt = c.prepare("SELECT abs_path FROM files")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = HashSet::new();
        for r in rows {
            if let Some(parent) = Path::new(&r?).parent() {
                out.insert(parent.to_path_buf());
            }
        }
        Ok(out)
    }

    // ---- links / backlinks (FR-18) ----

    /// Files that link *to* `target_rel` → (source_rel, title).
    pub fn backlinks(&self, project_id: &str, target_rel: &str) -> Result<Vec<(String, String)>> {
        let c = self.conn.lock().unwrap();
        let mut stmt = c.prepare(
            "SELECT l.source_rel, COALESCE(f.title, l.source_rel)
             FROM links l
             LEFT JOIN files f ON f.project_id = l.project_id AND f.rel_path = l.source_rel
             WHERE l.project_id = ?1 AND l.target_rel = ?2
             ORDER BY l.source_rel",
        )?;
        let rows = stmt.query_map(params![project_id, target_rel], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn get_file(&self, project_id: &str, rel_path: &str) -> Result<Option<IndexedFile>> {
        let c = self.conn.lock().unwrap();
        let mut stmt = c.prepare("SELECT project_id,abs_path,rel_path,title,size_bytes,modified_at FROM files WHERE project_id=?1 AND rel_path=?2")?;
        let mut rows = stmt.query(params![project_id, rel_path])?;
        Ok(rows.next()?.map(row_to_file))
    }

    pub fn list_files(&self, project_id: &str) -> Result<Vec<IndexedFile>> {
        let c = self.conn.lock().unwrap();
        let mut stmt = c.prepare("SELECT project_id,abs_path,rel_path,title,size_bytes,modified_at FROM files WHERE project_id=?1 ORDER BY rel_path")?;
        let rows = stmt.query_map(params![project_id], |r| Ok(row_to_file(r)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// The file a short code points at, or `None` when nothing matches.
    ///
    /// The pattern is built in Rust and bound as one parameter. Concatenating in
    /// SQL (`path_hash GLOB ?1 || '*'`) returns the same rows but makes the
    /// right-hand side an expression, which disables SQLite's GLOB index
    /// optimisation and silently turns this into a full table scan — see
    /// `short_link::hash_prefix_pattern`.
    ///
    /// Two files sharing a 12-character prefix is ~1.8e-5 likely even at 100k
    /// files, so the tie-break only has to be *stable*, not clever: order by the
    /// primary key and take the first.
    pub fn find_by_hash_prefix(&self, code: &str) -> Result<Option<(String, String)>> {
        if code.is_empty() {
            return Ok(None);
        }
        let c = self.conn.lock().unwrap();
        let mut stmt = c.prepare(
            "SELECT project_id, rel_path FROM files
             WHERE path_hash GLOB ?1
             ORDER BY project_id, rel_path
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![short_link::hash_prefix_pattern(code)])?;
        match rows.next()? {
            Some(r) => Ok(Some((r.get(0)?, r.get(1)?))),
            None => Ok(None),
        }
    }

    /// Query plan for [`find_by_hash_prefix`], so a test can prove it still uses
    /// the hash index rather than only proving it returns the right row.
    #[cfg(test)]
    fn hash_prefix_query_plan(&self, code: &str) -> Result<String> {
        let c = self.conn.lock().unwrap();
        let mut stmt = c.prepare(
            "EXPLAIN QUERY PLAN
             SELECT project_id, rel_path FROM files
             WHERE path_hash GLOB ?1
             ORDER BY project_id, rel_path
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![short_link::hash_prefix_pattern(code)])?;
        let mut plan = String::new();
        while let Some(r) = rows.next()? {
            plan.push_str(&r.get::<_, String>(3)?);
            plan.push('\n');
        }
        Ok(plan)
    }

    pub fn file_count(&self, project_id: &str) -> Result<usize> {
        let c = self.conn.lock().unwrap();
        let n: i64 = c.query_row(
            "SELECT COUNT(*) FROM files WHERE project_id=?1",
            params![project_id],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    /// `(schema version, files still missing a short-link code)` — what `mdview
    /// doctor` reports.
    pub fn schema_report(&self) -> Result<(i64, usize)> {
        let c = self.conn.lock().unwrap();
        let version: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let unhashed: i64 =
            c.query_row("SELECT COUNT(*) FROM files WHERE path_hash=''", [], |r| {
                r.get(0)
            })?;
        Ok((version, unhashed as usize))
    }

    #[cfg(test)]
    pub(crate) fn backdate_project_for_test(&self, project_id: &str, ts: &str) {
        let c = self.conn.lock().unwrap();
        c.execute(
            "UPDATE projects SET last_seen_at=?2 WHERE id=?1",
            params![project_id, ts],
        )
        .unwrap();
    }

    pub fn total_file_count(&self) -> Result<usize> {
        let c = self.conn.lock().unwrap();
        let n: i64 = c.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))?;
        Ok(n as usize)
    }

    // ---- access tracking / cleanup ----

    /// Record that a project was actually viewed (any file within it opened).
    pub fn touch_project_access(&self, project_id: &str) -> Result<()> {
        let c = self.conn.lock().unwrap();
        c.execute(
            "UPDATE projects SET last_seen_at=?2 WHERE id=?1",
            params![project_id, crate::indexer::now_rfc3339()],
        )?;
        Ok(())
    }

    /// Drop every project not seen since `project_cutoff` (an RFC3339 string;
    /// lexicographic comparison sorts correctly for RFC3339's fixed-width
    /// fields), with all its files, FTS rows and links, in one transaction.
    /// Returns the number of projects removed.
    ///
    /// Deletes only rows in this registry — never touches a project's real
    /// files on disk (same guarantee as `delete_project`).
    pub fn cleanup_stale(&self, project_cutoff: &str) -> Result<usize> {
        self.write_txn(|c| {
            let stale: Vec<String> = {
                let mut stmt = c.prepare("SELECT id FROM projects WHERE last_seen_at < ?1")?;
                let rows = stmt.query_map(params![project_cutoff], |r| r.get::<_, String>(0))?;
                rows.collect::<std::result::Result<_, _>>()?
            };
            for id in &stale {
                delete_project_in(c, id)?;
            }
            Ok(stale.len())
        })
    }

    /// `VACUUM` when more than a quarter of the database file is free pages.
    /// Returns whether it ran.
    pub fn vacuum_if_fragmented(&self) -> Result<bool> {
        let c = self.conn.lock().unwrap();
        let free: i64 = c.query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
        let total: i64 = c.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        if free * 4 > total {
            c.execute_batch("VACUUM")?;
            return Ok(true);
        }
        Ok(false)
    }

    // ---- search (FTS5) ----

    /// Ranked content search over already-indexed rows. Selects only `files`
    /// columns: a contentless FTS table reads back NULL for its own columns
    /// and has no `snippet()`, so excerpts are built elsewhere (`excerpt` is
    /// empty here). Row errors propagate instead of silently dropping hits.
    ///
    /// `dir_prefix` limits results to files under that folder (`%`, `_` and
    /// `\` in the folder name are matched literally).
    pub fn search(
        &self,
        query: &str,
        project_id: Option<&str>,
        dir_prefix: Option<&str>,
        sort: SearchSort,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let fts_query = fts_sanitize(query);
        if fts_query.is_empty() {
            return Ok(vec![]);
        }
        let like = dir_prefix
            .map(|d| d.trim_matches('/'))
            .filter(|d| !d.is_empty())
            .map(|d| format!("{}/%", escape_like(d)));
        let order = match sort {
            SearchSort::Relevance => "bm25(files_fts)",
            SearchSort::Recent => "f.modified_at DESC, bm25(files_fts)",
        };
        let sql = format!(
            "SELECT f.project_id, f.rel_path, f.title, f.modified_at, bm25(files_fts)
             FROM files_fts
             JOIN files f ON f.fts_rowid = files_fts.rowid
             WHERE files_fts MATCH ?1
               AND (?2 IS NULL OR f.project_id = ?2)
               AND (?3 IS NULL OR f.rel_path LIKE ?3 ESCAPE '\\')
             ORDER BY {order}
             LIMIT ?4"
        );
        let c = self.conn.lock().unwrap();
        let mut stmt = c.prepare(&sql)?;
        let rows = stmt.query_map(params![fts_query, project_id, like, limit as i64], |r| {
            let project_id: String = r.get(0)?;
            let rel_path: String = r.get(1)?;
            Ok(SearchResult {
                url: format!("/p/{project_id}/{rel_path}"),
                project_id,
                rel_path,
                title: r.get(2)?,
                excerpt: String::new(),
                modified_at: r.get(3)?,
                score: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
}

/// Create the schema, or rebuild it when the stored version differs — all in
/// one `BEGIN IMMEDIATE` transaction that re-reads `user_version` after taking
/// the write lock, so two processes opening the same stale file cannot both
/// reset it (the loser would otherwise wipe rows the winner already wrote).
/// `user_version` is always stamped, so a fresh database is recognised as
/// current on the next open.
fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let out = (|| -> Result<()> {
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let has_tables: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )?;
        if version == SCHEMA_VERSION && has_tables > 0 {
            return Ok(());
        }
        if has_tables > 0 {
            conn.execute_batch(
                "DROP TABLE IF EXISTS files_fts;
                 DROP TABLE IF EXISTS links;
                 DROP TABLE IF EXISTS files;
                 DROP TABLE IF EXISTS projects;",
            )?;
            tracing::info!(
                found = version,
                expected = SCHEMA_VERSION,
                "registry schema version differs; rebuilt the index (it is a rebuildable cache)"
            );
        }
        conn.execute_batch(SCHEMA)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(())
    })();
    match out {
        Ok(()) => conn.execute_batch("COMMIT").map_err(Into::into),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS files (
    project_id TEXT NOT NULL,
    rel_path TEXT NOT NULL,
    abs_path TEXT NOT NULL,
    title TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    modified_at TEXT NOT NULL,
    path_hash TEXT NOT NULL DEFAULT '',
    content_hash TEXT NOT NULL DEFAULT '',
    fts_rowid INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(project_id, rel_path)
);
CREATE INDEX IF NOT EXISTS idx_files_project ON files(project_id);
CREATE INDEX IF NOT EXISTS idx_files_hash ON files(path_hash);
CREATE INDEX IF NOT EXISTS idx_files_fts ON files(fts_rowid);
CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
    title,
    content,
    content='',
    contentless_delete=1,
    tokenize='unicode61 remove_diacritics 2'
);
CREATE TABLE IF NOT EXISTS links (
    project_id TEXT NOT NULL,
    source_rel TEXT NOT NULL,
    target_rel TEXT NOT NULL,
    PRIMARY KEY(project_id, source_rel, target_rel)
);
CREATE INDEX IF NOT EXISTS idx_links_target ON links(project_id, target_rel);
"#;

const FILE_STATE_COLS: &str =
    "rel_path,abs_path,title,size_bytes,modified_at,content_hash,fts_rowid";

fn row_to_file_state(r: &rusqlite::Row) -> rusqlite::Result<FileState> {
    Ok(FileState {
        rel_path: r.get(0)?,
        abs_path: PathBuf::from(r.get::<_, String>(1)?),
        title: r.get(2)?,
        size_bytes: r.get::<_, i64>(3)? as u64,
        modified_at: r.get(4)?,
        content_hash: r.get(5)?,
        fts_rowid: r.get(6)?,
    })
}

fn file_state_in(c: &Connection, project_id: &str, rel_path: &str) -> Result<Option<FileState>> {
    let mut stmt = c.prepare_cached(&format!(
        "SELECT {FILE_STATE_COLS} FROM files WHERE project_id=?1 AND rel_path=?2"
    ))?;
    Ok(stmt
        .query_row(params![project_id, rel_path], row_to_file_state)
        .optional()?)
}

/// Upsert one doc inside an open write transaction.
fn index_doc_in(c: &Connection, doc: &IndexedDoc) -> Result<bool> {
    let f = &doc.file;
    let new_hash = indexer::content_hash(&doc.content);
    let old: Option<(String, i64)> = c
        .query_row(
            "SELECT content_hash, fts_rowid FROM files WHERE project_id=?1 AND rel_path=?2",
            params![f.project_id, f.rel_path],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (old_hash, old_rowid) = match &old {
        Some((h, id)) => (Some(h.as_str()), *id),
        None => (None, 0),
    };
    c.execute(
        "INSERT INTO files(project_id,rel_path,abs_path,title,size_bytes,modified_at,path_hash,content_hash)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
         ON CONFLICT(project_id,rel_path) DO UPDATE SET
           abs_path=?3, title=?4, size_bytes=?5, modified_at=?6, path_hash=?7, content_hash=?8",
        params![
            f.project_id,
            f.rel_path,
            f.abs_path.to_string_lossy(),
            f.title,
            f.size_bytes as i64,
            f.modified_at,
            short_link::path_hash(&f.project_id, &f.rel_path),
            new_hash,
        ],
    )?;
    let changed = old_hash != Some(new_hash.as_str());
    if changed || old_rowid == 0 {
        if old_rowid > 0 {
            c.prepare_cached("DELETE FROM files_fts WHERE rowid=?1")?
                .execute(params![old_rowid])?;
        }
        c.prepare_cached("INSERT INTO files_fts(title, content) VALUES(?1, ?2)")?
            .execute(params![fold(&f.title), fold(&doc.content)])?;
        let rowid = c.last_insert_rowid();
        c.execute(
            "UPDATE files SET fts_rowid=?3 WHERE project_id=?1 AND rel_path=?2",
            params![f.project_id, f.rel_path, rowid],
        )?;
    }
    c.execute(
        "DELETE FROM links WHERE project_id=?1 AND source_rel=?2",
        params![f.project_id, f.rel_path],
    )?;
    let mut ins = c.prepare_cached(
        "INSERT OR IGNORE INTO links(project_id,source_rel,target_rel) VALUES(?1,?2,?3)",
    )?;
    for target in &doc.links {
        ins.execute(params![f.project_id, f.rel_path, target])?;
    }
    Ok(changed)
}

/// Delete one file's row, FTS row (by rowid) and outgoing links. Returns
/// whether the row existed.
fn delete_file_in(c: &Connection, project_id: &str, rel_path: &str) -> Result<bool> {
    let rowid: Option<i64> = c
        .query_row(
            "SELECT fts_rowid FROM files WHERE project_id=?1 AND rel_path=?2",
            params![project_id, rel_path],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(id) = rowid.filter(|id| *id > 0) {
        c.prepare_cached("DELETE FROM files_fts WHERE rowid=?1")?
            .execute(params![id])?;
    }
    c.execute(
        "DELETE FROM files WHERE project_id=?1 AND rel_path=?2",
        params![project_id, rel_path],
    )?;
    c.execute(
        "DELETE FROM links WHERE project_id=?1 AND source_rel=?2",
        params![project_id, rel_path],
    )?;
    Ok(rowid.is_some())
}

fn delete_project_in(c: &Connection, id: &str) -> Result<()> {
    let rowids: Vec<i64> = {
        let mut stmt =
            c.prepare("SELECT fts_rowid FROM files WHERE project_id=?1 AND fts_rowid>0")?;
        let rows = stmt.query_map(params![id], |r| r.get::<_, i64>(0))?;
        rows.collect::<std::result::Result<_, _>>()?
    };
    {
        let mut del = c.prepare_cached("DELETE FROM files_fts WHERE rowid=?1")?;
        for rowid in rowids {
            del.execute(params![rowid])?;
        }
    }
    c.execute("DELETE FROM files WHERE project_id=?1", params![id])?;
    c.execute("DELETE FROM links WHERE project_id=?1", params![id])?;
    c.execute("DELETE FROM projects WHERE id=?1", params![id])?;
    Ok(())
}

fn row_to_project(r: &rusqlite::Row) -> Project {
    Project {
        id: r.get_unwrap(0),
        name: r.get_unwrap(1),
        root_path: PathBuf::from(r.get_unwrap::<_, String>(2)),
        created_at: r.get_unwrap(3),
        last_seen_at: r.get_unwrap(4),
    }
}

fn row_to_file(r: &rusqlite::Row) -> IndexedFile {
    IndexedFile {
        project_id: r.get_unwrap(0),
        abs_path: PathBuf::from(r.get_unwrap::<_, String>(1)),
        rel_path: r.get_unwrap(2),
        title: r.get_unwrap(3),
        size_bytes: r.get_unwrap::<_, i64>(4) as u64,
        modified_at: r.get_unwrap(5),
    }
}

/// Escape `\`, `%` and `_` so a folder name is matched literally by `LIKE ... ESCAPE '\'`.
fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Make a user query safe for FTS5 MATCH: fold it exactly as the indexed text
/// was folded, keep alphanumerics, and quote each token as a prefix search.
/// Avoids syntax errors from FTS special chars.
fn fts_sanitize(query: &str) -> String {
    fold(query)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\"*"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{IndexedFile, Project};

    fn sample_project() -> Project {
        Project {
            id: "p1".into(),
            name: "P1".into(),
            root_path: PathBuf::from("/proj"),
            created_at: "2026-07-15T00:00:00Z".into(),
            last_seen_at: "2026-07-15T00:00:00Z".into(),
        }
    }

    fn file(rel: &str, title: &str) -> IndexedFile {
        IndexedFile {
            project_id: "p1".into(),
            abs_path: PathBuf::from("/proj").join(rel),
            rel_path: rel.into(),
            title: title.into(),
            size_bytes: 10,
            modified_at: "2026-07-15T00:00:00Z".into(),
        }
    }

    fn doc(rel: &str, title: &str, content: &str) -> IndexedDoc {
        IndexedDoc {
            file: file(rel, title),
            content: content.into(),
            links: Vec::new(),
        }
    }

    fn fts_rows(s: &SqliteStore) -> i64 {
        let c = s.conn.lock().unwrap();
        c.query_row("SELECT COUNT(*) FROM files_fts", [], |r| r.get(0))
            .unwrap()
    }

    /// The v3 schema text, so a test can build a database as the previous
    /// generation left it.
    const V3_SCHEMA: &str = "
        CREATE TABLE projects (id TEXT PRIMARY KEY, name TEXT NOT NULL, root_path TEXT NOT NULL,
            created_at TEXT NOT NULL, last_seen_at TEXT NOT NULL);
        CREATE TABLE files (project_id TEXT NOT NULL, rel_path TEXT NOT NULL, abs_path TEXT NOT NULL,
            title TEXT NOT NULL, size_bytes INTEGER NOT NULL, modified_at TEXT NOT NULL,
            path_hash TEXT NOT NULL DEFAULT '', content_hash TEXT NOT NULL DEFAULT '',
            PRIMARY KEY(project_id, rel_path));
        CREATE VIRTUAL TABLE files_fts USING fts5(project_id UNINDEXED, rel_path UNINDEXED, title, content);
        CREATE TABLE links (project_id TEXT NOT NULL, source_rel TEXT NOT NULL, target_rel TEXT NOT NULL,
            PRIMARY KEY(project_id, source_rel, target_rel));
        INSERT INTO files VALUES('old','a.md','/x/a.md','A',1,'t','h','c');
        INSERT INTO files_fts(project_id,rel_path,title,content) VALUES('old','a.md','A','body');
        PRAGMA user_version = 3;";

    fn v3_db_file(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "mdview-v3-{tag}-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
        }
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(V3_SCHEMA).unwrap();
        path
    }

    fn cleanup_db_file(path: &Path) {
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
        }
    }

    fn user_version(s: &SqliteStore) -> i64 {
        let c = s.conn.lock().unwrap();
        c.query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn a_stale_schema_version_is_rebuilt_empty() {
        let path = v3_db_file("reset");
        let s = SqliteStore::open(&path).unwrap();
        assert_eq!(user_version(&s), SCHEMA_VERSION);
        assert_eq!(s.total_file_count().unwrap(), 0);
        assert_eq!(fts_rows(&s), 0);
        // The rebuilt schema is the live one.
        s.upsert_project(&sample_project()).unwrap();
        s.index_docs(&[doc("a.md", "A", "alpha")]).unwrap();
        assert_eq!(
            s.search("alpha", None, None, SearchSort::Relevance, 5)
                .unwrap()
                .len(),
            1
        );
        drop(s);
        cleanup_db_file(&path);
    }

    #[test]
    fn a_fresh_database_keeps_its_rows_across_opens() {
        let path = std::env::temp_dir().join(format!("mdview-fresh-{}.db", std::process::id()));
        cleanup_db_file(&path);
        {
            let s = SqliteStore::open(&path).unwrap();
            assert_eq!(user_version(&s), SCHEMA_VERSION);
            s.upsert_project(&sample_project()).unwrap();
            s.index_docs(&[doc("a.md", "A", "alpha")]).unwrap();
        }
        let s = SqliteStore::open(&path).unwrap();
        assert_eq!(s.file_count("p1").unwrap(), 1);
        assert!(s.get_project("p1").unwrap().is_some());
        drop(s);
        cleanup_db_file(&path);
    }

    #[test]
    fn concurrent_opens_of_a_stale_file_reset_it_once() {
        let path = v3_db_file("race");
        let handles: Vec<_> = (0..2)
            .map(|i| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let s = SqliteStore::open(&path).unwrap();
                    let mut p = sample_project();
                    p.id = format!("p{i}");
                    p.root_path = PathBuf::from(format!("/proj{i}"));
                    s.upsert_project(&p).unwrap();
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let s = SqliteStore::open(&path).unwrap();
        assert_eq!(user_version(&s), SCHEMA_VERSION);
        assert_eq!(s.list_projects().unwrap().len(), 2);
        drop(s);
        cleanup_db_file(&path);
    }

    #[test]
    fn fts_delete_is_a_rowid_lookup() {
        let s = SqliteStore::open_in_memory().unwrap();
        let c = s.conn.lock().unwrap();
        let mut stmt = c
            .prepare("EXPLAIN QUERY PLAN DELETE FROM files_fts WHERE rowid=?1")
            .unwrap();
        let mut rows = stmt.query(params![1]).unwrap();
        let mut plan = String::new();
        while let Some(r) = rows.next().unwrap() {
            plan.push_str(&r.get::<_, String>(3).unwrap());
            plan.push('\n');
        }
        assert!(
            plan.contains("VIRTUAL TABLE INDEX 0:="),
            "FTS delete must be a rowid lookup, got plan: {plan}"
        );
    }

    #[test]
    fn unchanged_content_keeps_the_fts_row_and_changed_content_replaces_it() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        let first = s.index_docs(&[doc("a.md", "A", "old_token")]).unwrap();
        assert_eq!(first, vec![true]);
        let id1 = s.file_state("p1", "a.md").unwrap().unwrap().fts_rowid;
        assert!(id1 > 0);

        let same = s.index_docs(&[doc("a.md", "A", "old_token")]).unwrap();
        assert_eq!(same, vec![false]);
        assert_eq!(s.file_state("p1", "a.md").unwrap().unwrap().fts_rowid, id1);

        let changed = s.index_docs(&[doc("a.md", "A", "new_token")]).unwrap();
        assert_eq!(changed, vec![true]);
        assert!(s.file_state("p1", "a.md").unwrap().unwrap().fts_rowid > 0);
        assert_eq!(fts_rows(&s), 1, "the old FTS row must be gone");
        let q = |t: &str| {
            s.search(t, Some("p1"), None, SearchSort::Relevance, 5)
                .unwrap()
        };
        assert!(q("old_token").is_empty());
        assert_eq!(q("new_token").len(), 1);
    }

    #[test]
    fn index_docs_replaces_outgoing_links_and_reports_backlinks() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        let mut d = doc("a.md", "A", "x");
        d.links = vec!["b.md".into(), "c.md".into()];
        s.index_docs(&[d]).unwrap();
        assert_eq!(s.backlinks("p1", "b.md").unwrap().len(), 1);

        let mut d = doc("a.md", "A", "x");
        d.links = vec!["c.md".into()];
        s.index_docs(&[d]).unwrap();
        assert!(s.backlinks("p1", "b.md").unwrap().is_empty());
        assert_eq!(s.backlinks("p1", "c.md").unwrap().len(), 1);
    }

    #[test]
    fn stubs_have_no_content_state_and_never_overwrite_real_rows() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.register_known_path(&file("a.md", "a.md")).unwrap();
        let st = s.file_state("p1", "a.md").unwrap().unwrap();
        assert!(!st.content_indexed());
        assert_eq!(st.fts_rowid, 0);

        s.index_docs(&[doc("a.md", "Real", "body")]).unwrap();
        s.register_known_path(&file("a.md", "a.md")).unwrap();
        let st = s.file_state("p1", "a.md").unwrap().unwrap();
        assert!(st.content_indexed());
        assert_eq!(st.title, "Real");
        assert_eq!(s.file_states("p1").unwrap().len(), 1);
        assert_eq!(
            s.file_states_for("p1", &["a.md".into(), "zzz.md".into()])
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn indexed_dirs_cover_stubs_across_projects() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.register_known_path(&file("docs/a.md", "a")).unwrap();
        s.index_docs(&[doc("b.md", "B", "x")]).unwrap();
        let dirs = s.indexed_dirs().unwrap();
        assert!(dirs.contains(&PathBuf::from("/proj/docs")));
        assert!(dirs.contains(&PathBuf::from("/proj")));
    }

    #[test]
    fn index_docs_records_the_path_hash() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.index_docs(&[doc("docs/a.md", "Alpha", "alpha")]).unwrap();

        let code = short_link::short_code(&short_link::path_hash("p1", "docs/a.md"));
        assert_eq!(
            s.find_by_hash_prefix(&code).unwrap(),
            Some(("p1".into(), "docs/a.md".into()))
        );
    }

    #[test]
    fn re_indexing_keeps_the_same_hash() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.index_docs(&[doc("docs/a.md", "Alpha", "first")]).unwrap();
        let code = short_link::short_code(&short_link::path_hash("p1", "docs/a.md"));

        // Same path, new content/title — the link handed out earlier must survive.
        let mut changed = doc("docs/a.md", "Alpha v2", "second");
        changed.file.size_bytes = 999;
        s.index_docs(&[changed]).unwrap();

        assert_eq!(
            s.find_by_hash_prefix(&code).unwrap(),
            Some(("p1".into(), "docs/a.md".into()))
        );
    }

    #[test]
    fn unknown_code_resolves_to_nothing() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.index_docs(&[doc("docs/a.md", "Alpha", "alpha")]).unwrap();

        assert_eq!(s.find_by_hash_prefix("ffffffffffff").unwrap(), None);
        assert_eq!(s.find_by_hash_prefix("").unwrap(), None);
    }

    /// Regression guard with teeth: a functional test passes whether or not the
    /// query uses the index, because both forms return the same rows. Only the
    /// query plan distinguishes the fast path from a silent full scan.
    #[test]
    fn prefix_lookup_uses_the_hash_index() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        let docs: Vec<_> = (0..200)
            .map(|i| doc(&format!("docs/f{i}.md"), "T", "body"))
            .collect();
        s.index_docs(&docs).unwrap();
        let plan = s.hash_prefix_query_plan("a3f9c1d20b74").unwrap();
        assert!(
            plan.contains("idx_files_hash"),
            "prefix lookup must hit idx_files_hash, got plan: {plan}"
        );
    }

    #[test]
    fn project_and_file_roundtrip() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.index_docs(&[
            doc("docs/a.md", "Alpha", "alpha content here"),
            doc("src/b.md", "Beta", "beta words"),
        ])
        .unwrap();

        assert_eq!(s.file_count("p1").unwrap(), 2);
        assert_eq!(
            s.get_file("p1", "docs/a.md").unwrap().unwrap().title,
            "Alpha"
        );

        let found = s.find_project_by_root(Path::new("/proj")).unwrap();
        assert_eq!(found.unwrap().id, "p1");
    }

    #[test]
    fn delete_file_removes_from_index_and_fts() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.index_docs(&[doc("docs/a.md", "Alpha", "unique_token_xyz")])
            .unwrap();
        let q = || {
            s.search(
                "unique_token_xyz",
                Some("p1"),
                None,
                SearchSort::Relevance,
                10,
            )
            .unwrap()
            .len()
        };
        assert_eq!(q(), 1);
        s.delete_file("p1", "docs/a.md").unwrap();
        assert_eq!(s.file_count("p1").unwrap(), 0);
        assert_eq!(q(), 0);
        assert_eq!(fts_rows(&s), 0);
    }

    #[test]
    fn delete_files_and_delete_project_drop_fts_rows() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.index_docs(&[
            doc("a.md", "A", "one"),
            doc("b.md", "B", "two"),
            doc("c.md", "C", "three"),
        ])
        .unwrap();
        let removed = s
            .delete_files("p1", &["a.md".into(), "missing.md".into()])
            .unwrap();
        assert_eq!(removed, 1);
        assert_eq!(fts_rows(&s), 2);
        s.delete_project("p1").unwrap();
        assert_eq!(fts_rows(&s), 0);
        assert_eq!(s.total_file_count().unwrap(), 0);
    }

    #[test]
    fn touch_project_access_bumps_last_seen_at() {
        let s = SqliteStore::open_in_memory().unwrap();
        let mut p = sample_project();
        p.last_seen_at = "2000-01-01T00:00:00Z".into();
        s.upsert_project(&p).unwrap();

        s.touch_project_access("p1").unwrap();

        let refreshed = s.get_project("p1").unwrap().unwrap();
        assert_ne!(refreshed.last_seen_at, "2000-01-01T00:00:00Z");
    }

    #[test]
    fn cleanup_stale_removes_a_stale_project_with_all_its_rows() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.index_docs(&[doc("docs/a.md", "A", "content")]).unwrap();
        s.backdate_project_for_test("p1", "2000-01-01T00:00:00Z");

        let removed = s.cleanup_stale("2020-01-01T00:00:00Z").unwrap();

        assert_eq!(removed, 1);
        assert!(s.get_project("p1").unwrap().is_none());
        assert!(s.get_file("p1", "docs/a.md").unwrap().is_none());
        assert_eq!(fts_rows(&s), 0);
    }

    #[test]
    fn cleanup_stale_leaves_everything_when_nothing_is_old_enough() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.index_docs(&[doc("docs/a.md", "A", "content")]).unwrap();

        let removed = s.cleanup_stale("2000-01-01T00:00:00Z").unwrap();

        assert_eq!(removed, 0);
        assert!(s.get_file("p1", "docs/a.md").unwrap().is_some());
    }

    #[test]
    fn vacuum_runs_only_when_fragmented() {
        let path = std::env::temp_dir().join(format!("mdview-vac-{}.db", std::process::id()));
        cleanup_db_file(&path);
        let s = SqliteStore::open(&path).unwrap();
        assert!(!s.vacuum_if_fragmented().unwrap());

        {
            // Free a lot of pages the way a large cleanup would.
            let c = s.conn.lock().unwrap();
            c.execute_batch("CREATE TABLE junk(x BLOB)").unwrap();
            for _ in 0..200 {
                c.execute("INSERT INTO junk VALUES(zeroblob(8000))", [])
                    .unwrap();
            }
            c.execute_batch("DROP TABLE junk").unwrap();
        }
        assert!(s.vacuum_if_fragmented().unwrap());
        assert!(!s.vacuum_if_fragmented().unwrap());
        drop(s);
        cleanup_db_file(&path);
    }

    #[test]
    fn fts_search_finds_by_content_and_title() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.index_docs(&[
            doc("docs/a.md", "Deployment Guide", "how to deploy the service"),
            doc("docs/b.md", "Other", "unrelated text"),
        ])
        .unwrap();

        let by_content = s
            .search("deploy", Some("p1"), None, SearchSort::Relevance, 10)
            .unwrap();
        assert_eq!(by_content.len(), 1);
        assert_eq!(by_content[0].rel_path, "docs/a.md");
        assert_eq!(by_content[0].title, "Deployment Guide");
        assert!(by_content[0].url.contains("/p/p1/docs/a.md"));
        assert!(by_content[0].excerpt.is_empty());

        let by_title = s
            .search("deployment", None, None, SearchSort::Relevance, 10)
            .unwrap();
        assert_eq!(by_title.len(), 1);
    }

    #[test]
    fn search_folds_diacritics_and_the_d_stroke() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.index_docs(&[
            doc("a.md", "Hướng dẫn", "Đường đi được tới tài liệu"),
            doc("b.md", "Other", "nothing relevant"),
        ])
        .unwrap();
        let hits = |q: &str| {
            s.search(q, None, None, SearchSort::Relevance, 10)
                .unwrap()
                .len()
        };
        assert_eq!(hits("duoc"), 1);
        assert_eq!(hits("được"), 1);
        assert_eq!(hits("tai lieu"), 1);
        assert_eq!(hits("tài liệu"), 1);
        assert_eq!(hits("huong dan"), 1);
        assert_eq!(hits("absent"), 0);
        assert_eq!(hits("!!!"), 0);
    }

    #[test]
    fn search_dir_prefix_matches_literally_and_only_below_the_folder() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.index_docs(&[
            doc("100%_done/a.md", "A", "shared"),
            doc("100x_done/b.md", "B", "shared"),
            doc("100%_done/deep/c.md", "C", "shared"),
            doc("top.md", "T", "shared"),
        ])
        .unwrap();
        let rels = |dir: Option<&str>| {
            let mut v: Vec<_> = s
                .search("shared", Some("p1"), dir, SearchSort::Relevance, 10)
                .unwrap()
                .into_iter()
                .map(|r| r.rel_path)
                .collect();
            v.sort();
            v
        };
        assert_eq!(
            rels(Some("100%_done")),
            vec!["100%_done/a.md", "100%_done/deep/c.md"]
        );
        assert_eq!(
            rels(Some("100%_done/")),
            vec!["100%_done/a.md", "100%_done/deep/c.md"]
        );
        assert_eq!(rels(Some("")).len(), 4);
        assert_eq!(rels(None).len(), 4);
    }

    #[test]
    fn search_recent_orders_by_modified_time() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        let mut old = doc("old.md", "Old", "shared shared shared shared");
        old.file.modified_at = "2020-01-01T00:00:00Z".into();
        let mut new = doc("new.md", "New", "shared");
        new.file.modified_at = "2026-01-01T00:00:00Z".into();
        s.index_docs(&[old, new]).unwrap();

        let recent = s
            .search("shared", None, None, SearchSort::Recent, 10)
            .unwrap();
        assert_eq!(recent[0].rel_path, "new.md");
        assert_eq!(recent[0].modified_at, "2026-01-01T00:00:00Z");
        let relevant = s
            .search("shared", None, None, SearchSort::Relevance, 10)
            .unwrap();
        assert_eq!(relevant[0].rel_path, "old.md");
    }
}
