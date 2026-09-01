//! SQLite adapter: project registry + file index + FTS5 search.
//! Behind a `Mutex<Connection>` so it is Send+Sync for the async daemon.

use crate::domain::{IndexedFile, Project, SearchResult};
use crate::error::Result;
use crate::short_link;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    /// Open (creating if needed) the registry DB and run migrations.
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
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "foreign_keys", "ON").ok();
        conn.execute_batch(SCHEMA)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
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

    pub fn delete_project(&self, id: &str) -> Result<()> {
        let c = self.conn.lock().unwrap();
        c.execute("DELETE FROM files WHERE project_id=?1", params![id])?;
        c.execute("DELETE FROM files_fts WHERE project_id=?1", params![id])?;
        c.execute("DELETE FROM links WHERE project_id=?1", params![id])?;
        c.execute("DELETE FROM projects WHERE id=?1", params![id])?;
        Ok(())
    }

    // ---- files ----

    /// Upsert a file's index row. Returns whether its *content* actually
    /// changed from what was stored before (a brand-new row counts as
    /// changed) — the filesystem watcher uses this to skip a live-reload
    /// broadcast for a touch that left bytes identical (see D2,
    /// `docs/history/scoped-live-reload/CONTEXT.md`).
    pub fn upsert_file(&self, f: &IndexedFile, content: &str) -> Result<bool> {
        let c = self.conn.lock().unwrap();
        let new_content_hash = crate::indexer::content_hash(content);
        let old_content_hash: Option<String> = c
            .query_row(
                "SELECT content_hash FROM files WHERE project_id=?1 AND rel_path=?2",
                params![f.project_id, f.rel_path],
                |r| r.get(0),
            )
            .optional()?;
        c.execute(
            "INSERT INTO files(project_id,rel_path,abs_path,title,size_bytes,modified_at,path_hash,content_hash,last_accessed_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)
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
                new_content_hash,
                crate::indexer::now_rfc3339(),
            ],
        )?;
        c.execute(
            "DELETE FROM files_fts WHERE project_id=?1 AND rel_path=?2",
            params![f.project_id, f.rel_path],
        )?;
        c.execute(
            "INSERT INTO files_fts(project_id,rel_path,title,content) VALUES(?1,?2,?3,?4)",
            params![f.project_id, f.rel_path, f.title, content],
        )?;
        Ok(old_content_hash.as_deref() != Some(new_content_hash.as_str()))
    }

    pub fn delete_file(&self, project_id: &str, rel_path: &str) -> Result<()> {
        let c = self.conn.lock().unwrap();
        c.execute(
            "DELETE FROM files WHERE project_id=?1 AND rel_path=?2",
            params![project_id, rel_path],
        )?;
        c.execute(
            "DELETE FROM files_fts WHERE project_id=?1 AND rel_path=?2",
            params![project_id, rel_path],
        )?;
        c.execute(
            "DELETE FROM links WHERE project_id=?1 AND source_rel=?2",
            params![project_id, rel_path],
        )?;
        Ok(())
    }

    // ---- links / backlinks (FR-18) ----

    /// Replace the set of outgoing internal links for a source file.
    pub fn set_file_links(
        &self,
        project_id: &str,
        source_rel: &str,
        targets: &[String],
    ) -> Result<()> {
        let c = self.conn.lock().unwrap();
        c.execute(
            "DELETE FROM links WHERE project_id=?1 AND source_rel=?2",
            params![project_id, source_rel],
        )?;
        for t in targets {
            c.execute(
                "INSERT OR IGNORE INTO links(project_id,source_rel,target_rel) VALUES(?1,?2,?3)",
                params![project_id, source_rel, t],
            )?;
        }
        Ok(())
    }

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

    /// Absolute paths of every indexed file in a project — the link resolver index.
    pub fn file_abs_paths(&self, project_id: &str) -> Result<HashSet<PathBuf>> {
        let c = self.conn.lock().unwrap();
        let mut stmt = c.prepare("SELECT abs_path FROM files WHERE project_id=?1")?;
        let rows = stmt.query_map(params![project_id], |r| r.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok()).map(PathBuf::from).collect())
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
    /// doctor` reports so an operator can see whether an upgrade finished.
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
    pub(crate) fn file_last_accessed(&self, project_id: &str, rel_path: &str) -> Option<String> {
        let c = self.conn.lock().unwrap();
        c.query_row(
            "SELECT last_accessed_at FROM files WHERE project_id=?1 AND rel_path=?2",
            params![project_id, rel_path],
            |r| r.get(0),
        )
        .optional()
        .unwrap()
    }

    #[cfg(test)]
    pub(crate) fn backdate_file_access_for_test(&self, project_id: &str, rel_path: &str, ts: &str) {
        let c = self.conn.lock().unwrap();
        c.execute(
            "UPDATE files SET last_accessed_at=?3 WHERE project_id=?1 AND rel_path=?2",
            params![project_id, rel_path, ts],
        )
        .unwrap();
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

    /// Record that `rel_path` was actually viewed — the signal the cleanup
    /// sweep (`cleanup_stale`) checks. Deliberately separate from
    /// `upsert_file`, which never touches this column on re-index: an edit
    /// (or the filesystem watcher noticing one) is not a view, so it must
    /// not reset a file's unaccessed clock.
    pub fn touch_file_access(&self, project_id: &str, rel_path: &str) -> Result<()> {
        let c = self.conn.lock().unwrap();
        c.execute(
            "UPDATE files SET last_accessed_at=?3 WHERE project_id=?1 AND rel_path=?2",
            params![project_id, rel_path, crate::indexer::now_rfc3339()],
        )?;
        Ok(())
    }

    /// Record that a project was actually viewed (any file within it opened).
    pub fn touch_project_access(&self, project_id: &str) -> Result<()> {
        let c = self.conn.lock().unwrap();
        c.execute(
            "UPDATE projects SET last_seen_at=?2 WHERE id=?1",
            params![project_id, crate::indexer::now_rfc3339()],
        )?;
        Ok(())
    }

    /// Drop every file not viewed since `file_cutoff`, and every project not
    /// seen since `project_cutoff` (which cascades its files, FTS rows, and
    /// links — same as `delete_project`). Both cutoffs are RFC3339 strings;
    /// lexicographic comparison sorts correctly for RFC3339's fixed-width
    /// fields. Projects are swept first so a file belonging to a
    /// just-deleted project isn't also counted in the file total. Returns
    /// `(files_removed, projects_removed)`.
    ///
    /// Deletes only rows in this registry — never touches a project's real
    /// files on disk (same guarantee as `delete_project`/`delete_file`).
    pub fn cleanup_stale(&self, file_cutoff: &str, project_cutoff: &str) -> Result<(usize, usize)> {
        let stale_projects: Vec<String> = {
            let c = self.conn.lock().unwrap();
            let mut stmt = c.prepare("SELECT id FROM projects WHERE last_seen_at < ?1")?;
            let rows = stmt.query_map(params![project_cutoff], |r| r.get::<_, String>(0))?;
            rows.filter_map(|r| r.ok()).collect()
        };
        for id in &stale_projects {
            self.delete_project(id)?;
        }

        let stale_files: Vec<(String, String)> = {
            let c = self.conn.lock().unwrap();
            let mut stmt =
                c.prepare("SELECT project_id, rel_path FROM files WHERE last_accessed_at < ?1")?;
            let rows = stmt.query_map(params![file_cutoff], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            rows.filter_map(|r| r.ok()).collect()
        };
        for (project_id, rel_path) in &stale_files {
            self.delete_file(project_id, rel_path)?;
        }

        Ok((stale_files.len(), stale_projects.len()))
    }

    // ---- search (FTS5) ----

    pub fn search(
        &self,
        query: &str,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let c = self.conn.lock().unwrap();
        let fts_query = fts_sanitize(query);
        if fts_query.is_empty() {
            return Ok(vec![]);
        }
        let sql = "SELECT project_id, rel_path, title,
                     snippet(files_fts, 3, '<mark>', '</mark>', '…', 12) AS excerpt,
                     bm25(files_fts) AS score
                   FROM files_fts
                   WHERE files_fts MATCH ?1
                     AND (?2 IS NULL OR project_id = ?2)
                   ORDER BY score
                   LIMIT ?3";
        let mut stmt = c.prepare(sql)?;
        let rows = stmt.query_map(params![fts_query, project_id, limit as i64], |r| {
            let project_id: String = r.get(0)?;
            let rel_path: String = r.get(1)?;
            let title: String = r.get(2)?;
            let excerpt: String = r.get(3)?;
            let score: f64 = r.get(4)?;
            Ok(SearchResult {
                url: format!("/p/{project_id}/{rel_path}"),
                project_id,
                rel_path,
                title,
                excerpt,
                score,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

/// Ordered, append-only migration steps.
///
/// `SCHEMA` above only ever runs `CREATE TABLE IF NOT EXISTS`, so a database
/// created by an older build keeps its old columns forever — anything new has to
/// be added here instead. To add a migration, append one entry; never edit or
/// reorder an existing one, because databases in the field have already run it.
///
/// This is SQLite's own `PRAGMA user_version` convention, which is also what
/// crates like `rusqlite_migration` implement underneath. With a single step,
/// that crate would only wrap this list, so the dependency is not earned yet;
/// the shape here is deliberately the one it expects, so adopting it later is a
/// mechanical swap rather than a redesign.
type MigrationStep = (i64, fn(&Connection) -> Result<()>);
const MIGRATIONS: &[MigrationStep] = &[
    (1, migration_1_path_hash),
    (2, migration_2_content_hash),
    (3, migration_3_last_accessed),
];

/// Schema version this build expects — the last entry in [`MIGRATIONS`].
pub const SCHEMA_VERSION: i64 = 3;

/// Bring an existing database up to [`SCHEMA_VERSION`].
///
/// Every step is additive (no row is dropped or rewritten) and stamps
/// `user_version` as soon as it finishes, so a run interrupted halfway resumes at
/// the next unfinished step instead of redoing completed ones, and running this
/// against an up-to-date database is a no-op.
fn migrate(conn: &Connection) -> Result<()> {
    let mut version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (target, step) in MIGRATIONS {
        if version >= *target {
            continue;
        }
        step(conn)?;
        conn.pragma_update(None, "user_version", target)?;
        version = *target;
    }
    Ok(())
}

/// v1 — short-link support. `path_hash` lets `/s/<code>` find a file without a
/// separate shortlink table, so a code's lifetime is its index row's lifetime.
fn migration_1_path_hash(conn: &Connection) -> Result<()> {
    // A database created by this build already has the column from SCHEMA; one
    // created by an older build does not, because `CREATE TABLE IF NOT EXISTS`
    // leaves an existing table alone. The index has to be created here rather
    // than in SCHEMA for the same reason: on a legacy database SCHEMA runs
    // first, while the column still does not exist.
    if !has_column(conn, "files", "path_hash")? {
        conn.execute(
            "ALTER TABLE files ADD COLUMN path_hash TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_files_hash ON files(path_hash)",
        [],
    )?;
    backfill_path_hash(conn)
}

/// v2 — scoped live-reload support. `content_hash` lets the watcher tell a real
/// edit apart from a touch that left bytes identical, so it can skip a needless
/// reload broadcast (see D2, `docs/history/scoped-live-reload/CONTEXT.md`).
fn migration_2_content_hash(conn: &Connection) -> Result<()> {
    if !has_column(conn, "files", "content_hash")? {
        conn.execute(
            "ALTER TABLE files ADD COLUMN content_hash TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    backfill_content_hash(conn)
}

/// Fill `content_hash` for every row still carrying the empty default, reading
/// each file's already-indexed content straight from `files_fts` rather than
/// touching disk — the same content that would otherwise need re-reading is
/// already sitting in that table from the last successful index.
///
/// The two tables are joined in Rust with a `HashMap`, one full scan of each
/// (O(n+m)), rather than a SQL `LEFT JOIN` on `files_fts`'s `project_id`/
/// `rel_path` — those columns are `UNINDEXED`, so SQLite has no index to join
/// through and falls back to a nested-loop scan (O(n×m)). Measured on 16,000
/// rows: the SQL join never finished in over three minutes against the real
/// production database; this version completes in under 50ms against an
/// equivalent synthetic table (tsk-155).
fn backfill_content_hash(conn: &Connection) -> Result<()> {
    let pending: Vec<(String, String)> = {
        let mut stmt =
            conn.prepare("SELECT project_id, rel_path FROM files WHERE content_hash = ''")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        rows.filter_map(|r| r.ok()).collect()
    };
    if pending.is_empty() {
        return Ok(());
    }

    let content_by_key: std::collections::HashMap<(String, String), String> = {
        let mut stmt = conn.prepare("SELECT project_id, rel_path, content FROM files_fts")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                (r.get::<_, String>(0)?, r.get::<_, String>(1)?),
                r.get::<_, String>(2)?,
            ))
        })?;
        rows.filter_map(|r| r.ok()).collect()
    };

    conn.execute_batch("BEGIN")?;
    for (project_id, rel_path) in &pending {
        let content = content_by_key
            .get(&(project_id.clone(), rel_path.clone()))
            .map(String::as_str)
            .unwrap_or("");
        conn.execute(
            "UPDATE files SET content_hash=?3 WHERE project_id=?1 AND rel_path=?2",
            params![project_id, rel_path, crate::indexer::content_hash(content)],
        )?;
    }
    conn.execute_batch("COMMIT")?;
    Ok(())
}

/// v3 — access-based cleanup support. `last_accessed_at` is the timestamp
/// `cleanup_stale` compares against to decide whether a file's index record
/// (never the real file on disk) is still worth keeping.
fn migration_3_last_accessed(conn: &Connection) -> Result<()> {
    if !has_column(conn, "files", "last_accessed_at")? {
        conn.execute(
            "ALTER TABLE files ADD COLUMN last_accessed_at TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    backfill_last_accessed(conn)
}

/// Seed every row still carrying the empty default to "now", so an upgrade
/// never makes a whole existing database instantly stale — the access clock
/// starts fresh from the moment of the upgrade, same grace period a
/// brand-new file gets.
fn backfill_last_accessed(conn: &Connection) -> Result<()> {
    conn.execute(
        "UPDATE files SET last_accessed_at=?1 WHERE last_accessed_at=''",
        params![crate::indexer::now_rfc3339()],
    )?;
    Ok(())
}

fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        if r.get::<_, String>(1)? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Fill `path_hash` for every row still carrying the empty default.
///
/// Scoped to empty values rather than rewriting the whole table so an
/// interrupted run costs only what it did not finish, and so a re-run after a
/// crash is cheap rather than a full rewrite of 15k+ rows.
fn backfill_path_hash(conn: &Connection) -> Result<()> {
    let pending: Vec<(String, String)> = {
        let mut stmt =
            conn.prepare("SELECT project_id, rel_path FROM files WHERE path_hash = ''")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        rows.filter_map(|r| r.ok()).collect()
    };
    if pending.is_empty() {
        return Ok(());
    }
    conn.execute_batch("BEGIN")?;
    for (project_id, rel_path) in &pending {
        conn.execute(
            "UPDATE files SET path_hash=?3 WHERE project_id=?1 AND rel_path=?2",
            params![
                project_id,
                rel_path,
                short_link::path_hash(project_id, rel_path)
            ],
        )?;
    }
    conn.execute_batch("COMMIT")?;
    Ok(())
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
    last_accessed_at TEXT NOT NULL DEFAULT '',
    PRIMARY KEY(project_id, rel_path)
);
CREATE INDEX IF NOT EXISTS idx_files_project ON files(project_id);
CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
    project_id UNINDEXED,
    rel_path UNINDEXED,
    title,
    content
);
CREATE TABLE IF NOT EXISTS links (
    project_id TEXT NOT NULL,
    source_rel TEXT NOT NULL,
    target_rel TEXT NOT NULL,
    PRIMARY KEY(project_id, source_rel, target_rel)
);
CREATE INDEX IF NOT EXISTS idx_links_target ON links(project_id, target_rel);
"#;

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

/// Make a user query safe for FTS5 MATCH: keep alphanumerics, quote each token
/// as a prefix search. Avoids syntax errors from FTS special chars.
fn fts_sanitize(query: &str) -> String {
    query
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

    /// A database as an older build left it: no `path_hash`, no `user_version`.
    fn legacy_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE files (
                 project_id TEXT NOT NULL,
                 rel_path TEXT NOT NULL,
                 abs_path TEXT NOT NULL,
                 title TEXT NOT NULL,
                 size_bytes INTEGER NOT NULL,
                 modified_at TEXT NOT NULL,
                 PRIMARY KEY(project_id, rel_path)
             );
             INSERT INTO files VALUES('mdview','docs/a.md','/x/docs/a.md','A',1,'t');
             INSERT INTO files VALUES('mdview','README.md','/x/README.md','R',1,'t');",
        )
        .unwrap();
        conn
    }

    #[test]
    fn migrate_backfills_a_legacy_database() {
        let store = SqliteStore::from_conn(legacy_conn()).unwrap();
        let c = store.conn.lock().unwrap();

        let version: i64 = c
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);

        let hash: String = c
            .query_row(
                "SELECT path_hash FROM files WHERE rel_path='docs/a.md'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hash, short_link::path_hash("mdview", "docs/a.md"));

        let unfilled: i64 = c
            .query_row("SELECT COUNT(*) FROM files WHERE path_hash=''", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(unfilled, 0, "every legacy row must be backfilled");
    }

    #[test]
    fn migrate_is_idempotent() {
        let store = SqliteStore::from_conn(legacy_conn()).unwrap();
        {
            let c = store.conn.lock().unwrap();
            // Second pass over an already-migrated database must change nothing
            // and must not fail on the column already existing.
            migrate(&c).unwrap();
            let version: i64 = c
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap();
            assert_eq!(version, SCHEMA_VERSION);
        }
        assert_eq!(
            store
                .find_by_hash_prefix(&short_link::short_code(&short_link::path_hash(
                    "mdview",
                    "docs/a.md"
                )))
                .unwrap(),
            Some(("mdview".into(), "docs/a.md".into()))
        );
    }

    /// Regression guard with teeth (tsk-155): the backfill's original SQL
    /// `LEFT JOIN` on `files_fts`'s `UNINDEXED` columns degraded into an
    /// O(n×m) nested-loop scan — 2.58s at 3,000 rows in an isolated repro,
    /// and it never finished at all within three minutes against the real
    /// 15,480-row production database. A functional test alone (like
    /// `migrate_backfills_a_legacy_database` above) would never catch a
    /// regression back to that shape, since both versions produce identical
    /// output — only wall-clock time distinguishes them. 4,000 rows here is
    /// enough to make the O(n×m) shape unmistakably slow while staying fast
    /// under the *correct* O(n+m) one.
    #[test]
    fn backfilling_thousands_of_rows_stays_fast() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE files (project_id TEXT NOT NULL, rel_path TEXT NOT NULL,
                 abs_path TEXT NOT NULL, title TEXT NOT NULL, size_bytes INTEGER NOT NULL,
                 modified_at TEXT NOT NULL, path_hash TEXT NOT NULL DEFAULT '',
                 content_hash TEXT NOT NULL DEFAULT '', PRIMARY KEY(project_id, rel_path));
             CREATE VIRTUAL TABLE files_fts USING fts5(
                 project_id UNINDEXED, rel_path UNINDEXED, title, content);",
        )
        .unwrap();
        const N: usize = 4000;
        conn.execute_batch("BEGIN").unwrap();
        for i in 0..N {
            let rel = format!("f{i}.md");
            conn.execute(
                "INSERT INTO files VALUES('p1',?1,?1,'T',1,'t','','')",
                params![rel],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO files_fts(project_id,rel_path,title,content) VALUES('p1',?1,'T',?2)",
                params![
                    rel,
                    format!("body {i} filler text to keep rows non-trivial")
                ],
            )
            .unwrap();
        }
        conn.execute_batch("COMMIT").unwrap();

        let start = std::time::Instant::now();
        backfill_content_hash(&conn).unwrap();
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs() < 2,
            "backfill of {N} rows took {elapsed:?} -- likely regressed back to an O(n*m) join"
        );

        let unfilled: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM files WHERE content_hash=''",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(unfilled, 0);
    }

    #[test]
    fn a_fresh_database_needs_no_alter_table() {
        // SCHEMA already carries path_hash, so migrate must recognise that and
        // still stamp the version rather than trying to add the column again.
        let store = SqliteStore::open_in_memory().unwrap();
        let c = store.conn.lock().unwrap();
        let version: i64 = c
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn upsert_file_records_the_path_hash() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.upsert_file(&file("docs/a.md", "Alpha"), "alpha").unwrap();

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
        s.upsert_file(&file("docs/a.md", "Alpha"), "first").unwrap();
        let code = short_link::short_code(&short_link::path_hash("p1", "docs/a.md"));

        // Same path, new content/title — the link handed out earlier must survive.
        let mut changed = file("docs/a.md", "Alpha v2");
        changed.size_bytes = 999;
        s.upsert_file(&changed, "second").unwrap();

        assert_eq!(
            s.find_by_hash_prefix(&code).unwrap(),
            Some(("p1".into(), "docs/a.md".into()))
        );
    }

    #[test]
    fn unknown_code_resolves_to_nothing() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.upsert_file(&file("docs/a.md", "Alpha"), "alpha").unwrap();

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
        for i in 0..200 {
            s.upsert_file(&file(&format!("docs/f{i}.md"), "T"), "body")
                .unwrap();
        }
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
        s.upsert_file(&file("docs/a.md", "Alpha"), "alpha content here")
            .unwrap();
        s.upsert_file(&file("src/b.md", "Beta"), "beta words")
            .unwrap();

        assert_eq!(s.file_count("p1").unwrap(), 2);
        assert_eq!(
            s.get_file("p1", "docs/a.md").unwrap().unwrap().title,
            "Alpha"
        );
        assert!(s
            .file_abs_paths("p1")
            .unwrap()
            .contains(&PathBuf::from("/proj/docs/a.md")));

        let found = s.find_project_by_root(Path::new("/proj")).unwrap();
        assert_eq!(found.unwrap().id, "p1");
    }

    #[test]
    fn delete_file_removes_from_index_and_fts() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.upsert_file(&file("docs/a.md", "Alpha"), "unique_token_xyz")
            .unwrap();
        assert_eq!(
            s.search("unique_token_xyz", Some("p1"), 10).unwrap().len(),
            1
        );
        s.delete_file("p1", "docs/a.md").unwrap();
        assert_eq!(s.file_count("p1").unwrap(), 0);
        assert_eq!(
            s.search("unique_token_xyz", Some("p1"), 10).unwrap().len(),
            0
        );
    }

    #[test]
    fn upsert_file_seeds_last_accessed_at_but_reindex_never_resets_it() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.upsert_file(&file("docs/a.md", "Alpha"), "v1").unwrap();
        assert!(!s.file_last_accessed("p1", "docs/a.md").unwrap().is_empty());

        // Back-date it, as if this file hasn't been viewed in a while.
        s.backdate_file_access_for_test("p1", "docs/a.md", "2000-01-01T00:00:00Z");

        // Re-indexing on a content change must NOT count as a view.
        s.upsert_file(&file("docs/a.md", "Alpha"), "v2").unwrap();
        assert_eq!(
            s.file_last_accessed("p1", "docs/a.md").unwrap(),
            "2000-01-01T00:00:00Z"
        );

        // An actual view does reset it.
        s.touch_file_access("p1", "docs/a.md").unwrap();
        assert_ne!(
            s.file_last_accessed("p1", "docs/a.md").unwrap(),
            "2000-01-01T00:00:00Z"
        );
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
    fn cleanup_stale_removes_unaccessed_files_but_keeps_recent_ones() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.upsert_file(&file("docs/old.md", "Old"), "stale").unwrap();
        s.upsert_file(&file("docs/new.md", "New"), "fresh").unwrap();
        s.backdate_file_access_for_test("p1", "docs/old.md", "2000-01-01T00:00:00Z");

        let (files_removed, projects_removed) = s
            .cleanup_stale("2020-01-01T00:00:00Z", "2000-01-01T00:00:00Z")
            .unwrap();

        assert_eq!(files_removed, 1);
        assert_eq!(projects_removed, 0);
        assert!(s.get_file("p1", "docs/old.md").unwrap().is_none());
        assert!(s.get_file("p1", "docs/new.md").unwrap().is_some());
    }

    #[test]
    fn cleanup_stale_removing_a_project_cascades_its_files_without_double_counting() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.upsert_file(&file("docs/a.md", "A"), "content").unwrap();
        s.backdate_project_for_test("p1", "2000-01-01T00:00:00Z");

        let (files_removed, projects_removed) = s
            .cleanup_stale("2020-01-01T00:00:00Z", "2020-01-01T00:00:00Z")
            .unwrap();

        // The project itself was stale, so its file left via cascade, not a
        // second time through the file sweep.
        assert_eq!(projects_removed, 1);
        assert_eq!(files_removed, 0);
        assert!(s.get_project("p1").unwrap().is_none());
        assert!(s.get_file("p1", "docs/a.md").unwrap().is_none());
    }

    #[test]
    fn cleanup_stale_leaves_everything_when_nothing_is_old_enough() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.upsert_file(&file("docs/a.md", "A"), "content").unwrap();

        let (files_removed, projects_removed) = s
            .cleanup_stale("2000-01-01T00:00:00Z", "2000-01-01T00:00:00Z")
            .unwrap();

        assert_eq!(files_removed, 0);
        assert_eq!(projects_removed, 0);
    }

    #[test]
    fn fts_search_finds_by_content_and_title() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.upsert_project(&sample_project()).unwrap();
        s.upsert_file(
            &file("docs/a.md", "Deployment Guide"),
            "how to deploy the service",
        )
        .unwrap();
        s.upsert_file(&file("docs/b.md", "Other"), "unrelated text")
            .unwrap();

        let by_content = s.search("deploy", Some("p1"), 10).unwrap();
        assert_eq!(by_content.len(), 1);
        assert_eq!(by_content[0].rel_path, "docs/a.md");
        assert!(by_content[0].url.contains("/p/p1/docs/a.md"));

        let by_title = s.search("deployment", None, 10).unwrap();
        assert_eq!(by_title.len(), 1);
    }
}
