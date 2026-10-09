//! Content search: sync the project lazily, then rank with bm25.

use crate::domain::{Project, SearchOutcome, SearchResult, SearchSort, SyncStats};
use crate::engine::Engine;
use crate::error::{Error, Result};
use crate::{indexer, snippet};
use std::collections::HashMap;
use std::io::Read;

/// Words per excerpt.
const EXCERPT_WORDS: usize = 24;

impl Engine {
    /// Count the search as access to the project, sync the whole project
    /// (a sync failure is logged and returned in `sync_error`; the query still
    /// runs over what is already indexed), then search it.
    pub fn search_content(
        &self,
        project_id: &str,
        query: &str,
        dir_prefix: Option<&str>,
        sort: SearchSort,
        limit: usize,
    ) -> Result<SearchOutcome> {
        if self.store.get_project(project_id)?.is_none() {
            return Err(Error::ProjectNotFound(project_id.to_string()));
        }
        let _ = self.store.touch_project_access(project_id);
        let (sync, sync_error) = match self.sync_project(project_id) {
            Ok(stats) => (stats, None),
            Err(e) => {
                tracing::warn!(
                    project = project_id,
                    "project sync failed before search: {e}"
                );
                (SyncStats::default(), Some(e.to_string()))
            }
        };
        let mut results = self
            .store
            .search(query, Some(project_id), dir_prefix, sort, limit)?;
        self.fill_excerpts(query, &mut results);
        Ok(SearchOutcome {
            results,
            sync,
            sync_error,
        })
    }

    /// Already-indexed rows only, across all projects. Never syncs and never
    /// touches a project's access time (the CLI without a project).
    pub fn search_indexed(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let mut results = self
            .store
            .search(query, None, None, SearchSort::Relevance, limit)?;
        self.fill_excerpts(query, &mut results);
        Ok(results)
    }

    /// Build each excerpt from the file on disk. A file that vanished, is too
    /// large, is unreadable or no longer passes `confine` keeps an empty
    /// excerpt; the hit itself is still listed.
    fn fill_excerpts(&self, query: &str, results: &mut [SearchResult]) {
        let terms = snippet::query_terms(query);
        let max_bytes = self.max_bytes();
        let exclude = &self.config.indexing.exclude_patterns;
        let mut projects: HashMap<String, Option<Project>> = HashMap::new();
        for r in results.iter_mut() {
            let project = projects
                .entry(r.project_id.clone())
                .or_insert_with(|| self.store.get_project(&r.project_id).ok().flatten());
            let Some(project) = project else { continue };
            let Some(abs) = indexer::confine(
                &project.root_path,
                &project.root_path.join(&r.rel_path),
                exclude,
            ) else {
                continue;
            };
            if let Some(text) = read_capped(&abs, max_bytes) {
                r.excerpt = snippet::excerpt(&text, &terms, EXCERPT_WORDS);
            }
        }
    }
}

/// Lossy UTF-8 text of `path`, or `None` when unreadable or over `max_bytes`.
fn read_capped(path: &std::path::Path, max_bytes: u64) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut buf = Vec::new();
    // One byte past the cap distinguishes "exactly at" from "over".
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut buf)
        .ok()?;
    if buf.len() as u64 > max_bytes {
        return None;
    }
    Some(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::repository::SqliteStore;
    use crate::snippet::{MARK_CLOSE, MARK_OPEN};

    fn fixture(tag: &str) -> (Engine, std::path::PathBuf, String) {
        let dir = std::env::temp_dir().join(format!("mdview-search-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("a.md"),
            "# A\n\nNội dung có tài liệu quan trọng.\n",
        )
        .unwrap();
        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let (project, _) = engine.register(&dir, None).unwrap();
        (engine, dir, project.id)
    }

    #[test]
    fn search_content_fills_excerpts_from_disk() {
        let (engine, dir, id) = fixture("fill");
        let out = engine
            .search_content(&id, "tai lieu", None, SearchSort::Relevance, 10)
            .unwrap();
        assert_eq!(out.results.len(), 1);
        let ex = &out.results[0].excerpt;
        assert!(ex.contains(&format!("{MARK_OPEN}tài{MARK_CLOSE}")), "{ex}");
        assert!(ex.contains(&format!("{MARK_OPEN}liệu{MARK_CLOSE}")), "{ex}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_indexed_fills_excerpts_and_survives_a_missing_file() {
        let (engine, dir, id) = fixture("indexed");
        engine.sync_project(&id).unwrap();
        let hits = engine.search_indexed("quan trong", 10).unwrap();
        assert!(hits[0].excerpt.contains(MARK_OPEN), "{:?}", hits[0].excerpt);

        std::fs::remove_file(dir.join("a.md")).unwrap();
        let hits = engine.search_indexed("quan trong", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].excerpt, "");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn oversized_file_gets_empty_excerpt() {
        let (_engine, dir, _) = fixture("cap");
        let path = dir.join("a.md");
        assert!(read_capped(&path, 1024 * 1024).is_some());
        assert!(read_capped(&path, 4).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
