//! File listings served to the sidebar and the jump palette.
//!
//! Both read a short-lived cached walk of the project directory, so they show
//! every markdown file whether or not it has been indexed. Titles come from the
//! index when the file's content is indexed, else from the filename.

use crate::domain::IndexedFile;
use crate::engine::Engine;
use crate::error::{Error, Result};
use crate::fuzzy::{self, FuzzyHit};
use crate::indexer;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

/// How long a directory walk is reused before the next listing re-walks.
const LISTING_TTL: Duration = Duration::from_secs(3);

/// One markdown file found on disk.
#[derive(Debug, Clone)]
pub struct ListingEntry {
    pub rel_path: String,
    pub modified: SystemTime,
}

type ListingCache = Mutex<HashMap<PathBuf, (Instant, Arc<Vec<ListingEntry>>)>>;

fn cache() -> &'static ListingCache {
    static CACHE: OnceLock<ListingCache> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn walk(root: &Path, exclude: &[String]) -> Vec<ListingEntry> {
    indexer::scan_markdown_files(root, exclude)
        .into_iter()
        .filter_map(|abs| {
            let modified = std::fs::metadata(&abs).and_then(|m| m.modified()).ok()?;
            let rel_path = indexer::rel_path_str(root, &abs);
            (!rel_path.is_empty()).then_some(ListingEntry { rel_path, modified })
        })
        .collect()
}

/// Cached walk of `root` (the lock is held only to read or insert, never
/// during the walk, so concurrent callers may each walk once).
fn listing_with_ttl(root: &Path, exclude: &[String], ttl: Duration) -> Arc<Vec<ListingEntry>> {
    if let Some((at, entries)) = cache().lock().unwrap().get(root) {
        if at.elapsed() < ttl {
            return Arc::clone(entries);
        }
    }
    let entries = Arc::new(walk(root, exclude));
    cache()
        .lock()
        .unwrap()
        .insert(root.to_path_buf(), (Instant::now(), Arc::clone(&entries)));
    entries
}

fn filename_title(rel_path: &str) -> String {
    rel_path.rsplit('/').next().unwrap_or(rel_path).to_string()
}

impl Engine {
    fn project_listing(&self, project_id: &str) -> Result<(PathBuf, Arc<Vec<ListingEntry>>)> {
        let project = self
            .store
            .get_project(project_id)?
            .ok_or_else(|| Error::ProjectNotFound(project_id.to_string()))?;
        let entries = listing_with_ttl(
            &project.root_path,
            &self.config.indexing.exclude_patterns,
            LISTING_TTL,
        );
        Ok((project.root_path, entries))
    }

    /// Titles of content-indexed files, keyed by relative path.
    fn indexed_titles(&self, project_id: &str) -> Result<HashMap<String, String>> {
        Ok(self
            .store
            .file_states(project_id)?
            .into_iter()
            .filter(|(_, s)| s.content_indexed())
            .map(|(rel, s)| (rel, s.title))
            .collect())
    }

    /// Files shown in the project sidebar: every markdown file on disk, sorted
    /// by relative path.
    pub fn sidebar_files(&self, project_id: &str) -> Result<Vec<IndexedFile>> {
        let (root, entries) = self.project_listing(project_id)?;
        let mut titles = self.indexed_titles(project_id)?;
        let mut files: Vec<IndexedFile> = entries
            .iter()
            .map(|e| {
                let abs_path = root.join(&e.rel_path);
                IndexedFile {
                    project_id: project_id.to_string(),
                    title: titles
                        .remove(&e.rel_path)
                        .unwrap_or_else(|| filename_title(&e.rel_path)),
                    size_bytes: std::fs::metadata(&abs_path).map(|m| m.len()).unwrap_or(0),
                    modified_at: time::OffsetDateTime::from(e.modified)
                        .format(&time::format_description::well_known::Rfc3339)
                        .unwrap_or_default(),
                    abs_path,
                    rel_path: e.rel_path.clone(),
                }
            })
            .collect();
        files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        Ok(files)
    }

    /// Fuzzy file-jump over the project listing. A blank query returns the
    /// `limit` most recently modified files; otherwise files are ranked by a
    /// fuzzy match of `query` against path and title.
    pub fn jump_files(&self, project_id: &str, query: &str, limit: usize) -> Result<Vec<FuzzyHit>> {
        let (_, entries) = self.project_listing(project_id)?;
        let mut titles = self.indexed_titles(project_id)?;
        let items: Vec<(String, String, SystemTime)> = entries
            .iter()
            .map(|e| {
                let title = titles
                    .remove(&e.rel_path)
                    .unwrap_or_else(|| filename_title(&e.rel_path));
                (e.rel_path.clone(), title, e.modified)
            })
            .collect();
        if query.trim().is_empty() {
            let mut recent = items;
            recent.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
            return Ok(recent
                .into_iter()
                .take(limit)
                .map(|(rel_path, title, _)| FuzzyHit {
                    url: format!("/p/{project_id}/{rel_path}"),
                    rel_path,
                    title,
                    score: 0,
                })
                .collect());
        }
        Ok(fuzzy::rank_items(&items, project_id, query, limit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::repository::SqliteStore;
    use std::fs;

    fn write(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    fn set_mtime(root: &Path, rel: &str, secs: u64) {
        let f = fs::File::options()
            .write(true)
            .open(root.join(rel))
            .unwrap();
        f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
            .unwrap();
    }

    /// Unique canonical temp dir, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let p =
                std::env::temp_dir().join(format!("mdview-listing-{tag}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Self(fs::canonicalize(p).unwrap())
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn setup(tag: &str) -> (TempDir, Engine, String) {
        let dir = TempDir::new(tag);
        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let (project, _) = engine.ensure_project(dir.path(), Some(tag)).unwrap();
        (dir, engine, project.id)
    }

    #[test]
    fn cache_reuses_within_ttl_and_rewalks_after() {
        let dir = TempDir::new("cache");
        let root = dir.path().to_path_buf();
        write(&root, "a.md", "# A");
        let first = listing_with_ttl(&root, &[], Duration::from_millis(150));
        assert_eq!(first.len(), 1);
        write(&root, "b.md", "# B");
        let cached = listing_with_ttl(&root, &[], Duration::from_millis(150));
        assert_eq!(cached.len(), 1, "within the TTL the walk is reused");
        std::thread::sleep(Duration::from_millis(200));
        let fresh = listing_with_ttl(&root, &[], Duration::from_millis(150));
        assert_eq!(fresh.len(), 2, "after the TTL the directory is re-walked");
    }

    #[test]
    fn sidebar_lists_unindexed_files_and_merges_indexed_titles() {
        let (dir, engine, id) = setup("sidebar");
        write(dir.path(), "docs/known.md", "# Known Title\nbody");
        write(dir.path(), "docs/unknown.md", "# Hidden Title\nbody");
        write(dir.path(), "notes.txt", "not markdown");
        let project = engine.get_project(&id).unwrap().unwrap();
        let docs = [indexer::IndexService::build_doc(
            &project,
            &project.root_path.join("docs/known.md"),
            10_000_000,
            &[],
        )
        .unwrap()];
        engine.store.index_docs(&docs).unwrap();

        let files = engine.sidebar_files(&id).unwrap();
        let rels: Vec<&str> = files.iter().map(|f| f.rel_path.as_str()).collect();
        assert_eq!(rels, ["docs/known.md", "docs/unknown.md"]);
        assert_eq!(files[0].title, "Known Title");
        assert_eq!(files[1].title, "unknown.md");
    }

    #[test]
    fn jump_blank_query_lists_most_recent_first_with_limit() {
        let (dir, engine, id) = setup("recent");
        for (rel, secs) in [("old.md", 1_000), ("mid.md", 2_000), ("new.md", 3_000)] {
            write(dir.path(), rel, "# t");
            set_mtime(dir.path(), rel, secs);
        }
        let hits = engine.jump_files(&id, "", 2).unwrap();
        let rels: Vec<&str> = hits.iter().map(|h| h.rel_path.as_str()).collect();
        assert_eq!(rels, ["new.md", "mid.md"]);
        assert_eq!(hits[0].url, format!("/p/{id}/new.md"));
    }

    #[test]
    fn jump_query_matches_title_of_indexed_file() {
        let (dir, engine, id) = setup("title");
        write(dir.path(), "x1.md", "# Quarterly Roadmap\n");
        write(dir.path(), "x2.md", "# Other\n");
        let project = engine.get_project(&id).unwrap().unwrap();
        let doc = indexer::IndexService::build_doc(
            &project,
            &project.root_path.join("x1.md"),
            10_000_000,
            &[],
        )
        .unwrap();
        engine.store.index_docs(&[doc]).unwrap();
        let hits = engine.jump_files(&id, "roadmap", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].rel_path, "x1.md");
    }
}
