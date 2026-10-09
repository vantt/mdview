//! File listings served to the sidebar and the jump palette.

use crate::domain::IndexedFile;
use crate::engine::Engine;
use crate::error::Result;
use crate::fuzzy::{self, FuzzyHit};

impl Engine {
    /// Files shown in the project sidebar.
    pub fn sidebar_files(&self, project_id: &str) -> Result<Vec<IndexedFile>> {
        self.store.list_files(project_id)
    }

    /// Fuzzy file-jump: rank a project's files by a fuzzy match of `query`
    /// against their relative paths. Ordered by descending match score.
    pub fn jump_files(&self, project_id: &str, query: &str, limit: usize) -> Result<Vec<FuzzyHit>> {
        let files = self.sidebar_files(project_id)?;
        Ok(fuzzy::rank_files(&files, project_id, query, limit))
    }
}
