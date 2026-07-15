//! SQLite adapter: project registry + file index + FTS5 search.
//! Behind a `Mutex<Connection>` so it is Send+Sync for the async daemon.

use crate::domain::{IndexedFile, Project, SearchResult};
use crate::error::Result;
use rusqlite::{params, Connection};
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

    pub fn upsert_file(&self, f: &IndexedFile, content: &str) -> Result<()> {
        let c = self.conn.lock().unwrap();
        c.execute(
            "INSERT INTO files(project_id,rel_path,abs_path,title,size_bytes,modified_at)
             VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(project_id,rel_path) DO UPDATE SET
               abs_path=?3, title=?4, size_bytes=?5, modified_at=?6",
            params![
                f.project_id,
                f.rel_path,
                f.abs_path.to_string_lossy(),
                f.title,
                f.size_bytes as i64,
                f.modified_at
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
        Ok(())
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

    pub fn file_count(&self, project_id: &str) -> Result<usize> {
        let c = self.conn.lock().unwrap();
        let n: i64 = c.query_row(
            "SELECT COUNT(*) FROM files WHERE project_id=?1",
            params![project_id],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    pub fn total_file_count(&self) -> Result<usize> {
        let c = self.conn.lock().unwrap();
        let n: i64 = c.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))?;
        Ok(n as usize)
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
