//! Content search: sync the project lazily, then rank with bm25.

use crate::domain::{SearchOutcome, SearchResult, SearchSort, SyncStats};
use crate::engine::Engine;
use crate::error::{Error, Result};

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
        let results = self
            .store
            .search(query, Some(project_id), dir_prefix, sort, limit)?;
        Ok(SearchOutcome {
            results,
            sync,
            sync_error,
        })
    }

    /// Already-indexed rows only, across all projects. Never syncs and never
    /// touches a project's access time (the CLI without a project).
    pub fn search_indexed(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.store
            .search(query, None, None, SearchSort::Relevance, limit)
    }
}
