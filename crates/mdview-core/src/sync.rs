//! Incremental, single-flight sync of a whole project into the index.
//!
//! Run lazily by content search — never on register/open/view — so a project
//! the user only skims never pays for a full scan. Files whose size and mtime
//! match their indexed row are not re-read, and writes go in batches.

use crate::domain::SyncStats;
use crate::engine::Engine;
use crate::error::{Error, Result};
use crate::indexer::{self, IndexService, IndexedDoc};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

/// A sync that finished more recently than this makes the next call a no-op.
const MIN_SYNC_INTERVAL: Duration = Duration::from_secs(10);
/// Documents written per transaction.
const BATCH_SIZE: usize = 200;

/// When a project's last sync completed; holding the lock *is* the single-flight.
type Slot = Arc<Mutex<Option<Instant>>>;

/// Process-wide, so the daemon's request threads and any other in-process
/// caller share one flight per project.
fn slot_for(project_id: &str) -> Slot {
    static SLOTS: OnceLock<Mutex<HashMap<String, Slot>>> = OnceLock::new();
    let mut slots = lock(SLOTS.get_or_init(Default::default));
    slots.entry(project_id.to_string()).or_default().clone()
}

/// A poisoned lock only means another sync panicked; the data (a timestamp)
/// is still valid.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Engine {
    /// Bring the index of `project_id` in line with the files on disk.
    /// Single-flight per project; skips (`skipped_recent = true`) when the last
    /// completed sync of this project finished less than 10 s ago.
    pub fn sync_project(&self, project_id: &str) -> Result<SyncStats> {
        self.sync_project_within(project_id, MIN_SYNC_INTERVAL)
    }

    pub(crate) fn sync_project_within(
        &self,
        project_id: &str,
        window: Duration,
    ) -> Result<SyncStats> {
        let project = self
            .store
            .get_project(project_id)?
            .ok_or_else(|| Error::ProjectNotFound(project_id.to_string()))?;
        let slot = slot_for(project_id);
        let mut last_done = lock(&slot);
        if last_done.is_some_and(|t| t.elapsed() < window) {
            return Ok(SyncStats {
                skipped_recent: true,
                ..SyncStats::default()
            });
        }

        let started = Instant::now();
        let mut stats = self.run_sync(&project)?;
        stats.elapsed_ms = started.elapsed().as_millis();
        *last_done = Some(Instant::now());
        Ok(stats)
    }

    fn run_sync(&self, project: &crate::domain::Project) -> Result<SyncStats> {
        let exclude = &self.config.indexing.exclude_patterns;
        let max_bytes = self.max_bytes();
        let known = self.store.file_states(&project.id)?;
        let mut stats = SyncStats::default();
        let mut seen: HashSet<String> = HashSet::new();
        let mut batch: Vec<IndexedDoc> = Vec::new();
        let mut hinted: HashSet<PathBuf> = HashSet::new();

        for abs in indexer::scan_markdown_files(&project.root_path, exclude) {
            let rel = indexer::rel_path_str(&project.root_path, &abs);
            if rel.is_empty() || indexer::is_excluded(&rel, exclude) {
                continue;
            }
            stats.files_seen += 1;
            let unchanged = match (known.get(&rel), std::fs::metadata(&abs)) {
                (Some(state), Ok(meta)) => {
                    state.content_indexed()
                        && state.size_bytes == meta.len()
                        && state.modified_at == indexer::modified_rfc3339(&meta)
                }
                _ => false,
            };
            seen.insert(rel);
            if unchanged {
                continue;
            }
            let Some(doc) = IndexService::build_doc(project, &abs, max_bytes, exclude) else {
                continue;
            };
            stats.files_read += 1;
            if let Some(dir) = doc.file.abs_path.parent() {
                hinted.insert(dir.to_path_buf());
            }
            batch.push(doc);
            if batch.len() >= BATCH_SIZE {
                self.store.index_docs(&batch)?;
                batch.clear();
            }
        }
        self.store.index_docs(&batch)?;

        // Prune rows the walk did not see only when the file is really gone,
        // or its path is now excluded. A file the walk skipped because it is
        // gitignored but that still exists (e.g. one the user viewed) stays.
        let gone: Vec<String> = known
            .iter()
            .filter(|(rel, _)| !seen.contains(*rel))
            .filter(|(rel, state)| {
                indexer::is_excluded(rel, exclude) || !state.abs_path.try_exists().unwrap_or(true)
            })
            .map(|(rel, _)| rel.clone())
            .collect();
        stats.files_removed = self.store.delete_files(&project.id, &gone)?;

        for dir in &hinted {
            self.hint_dir(dir);
        }
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::domain::{IndexedFile, SearchSort};
    use crate::repository::SqliteStore;
    use std::path::Path;

    fn write(dir: &Path, rel: &str, body: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    /// Project ids derive from the directory name and sync state is keyed by
    /// project id process-wide, so every test gets its own directory name.
    fn setup(tag: &str) -> (Engine, crate::domain::Project, PathBuf) {
        let dir = std::env::temp_dir().join(format!("mdview-sync-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let (project, _) = engine.register(&dir, None).unwrap();
        (engine, project, dir)
    }

    const NO_WINDOW: Duration = Duration::ZERO;

    #[test]
    fn indexes_new_files_then_skips_inside_the_window() {
        let (engine, project, dir) = setup("new");
        write(&dir, "a.md", "# A\nalpha");
        write(&dir, "docs/b.md", "# B\nbeta");
        write(&dir, "notes.txt", "not markdown");

        let first = engine.sync_project(&project.id).unwrap();
        assert!(!first.skipped_recent);
        assert_eq!(first.files_seen, 2);
        assert_eq!(first.files_read, 2);
        assert_eq!(engine.file_count(&project.id).unwrap(), 2);

        let second = engine.sync_project(&project.id).unwrap();
        assert!(second.skipped_recent);
        assert_eq!(second.files_read, 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn after_the_window_unchanged_files_are_not_re_read() {
        let (engine, project, dir) = setup("unchanged");
        write(&dir, "a.md", "# A\nalpha");
        engine.sync_project_within(&project.id, NO_WINDOW).unwrap();

        let again = engine.sync_project_within(&project.id, NO_WINDOW).unwrap();
        assert!(!again.skipped_recent);
        assert_eq!(again.files_seen, 1);
        assert_eq!(again.files_read, 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_modified_file_is_re_read_and_a_deleted_one_is_pruned() {
        let (engine, project, dir) = setup("modify");
        write(&dir, "a.md", "# A\nalpha");
        write(&dir, "b.md", "# B\nbeta");
        engine.sync_project_within(&project.id, NO_WINDOW).unwrap();

        write(
            &dir,
            "a.md",
            "# A\nalpha plus a much longer body than before",
        );
        std::fs::remove_file(dir.join("b.md")).unwrap();
        let stats = engine.sync_project_within(&project.id, NO_WINDOW).unwrap();

        assert_eq!(stats.files_read, 1);
        assert_eq!(stats.files_removed, 1);
        assert!(engine
            .store
            .get_file(&project.id, "b.md")
            .unwrap()
            .is_none());
        let hits = engine
            .store
            .search("longer", Some(&project.id), None, SearchSort::Relevance, 5)
            .unwrap();
        assert_eq!(hits.len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_gitignored_but_viewed_row_is_kept() {
        let (engine, project, dir) = setup("ignored");
        write(&dir, ".ignore", "notes/\n");
        write(&dir, "a.md", "# A");
        write(&dir, "notes/scratch.md", "# Scratch");
        let vf = engine.view_file(&dir, "notes/scratch.md").unwrap();
        assert!(engine.ensure_indexed(&project, "notes/scratch.md").unwrap());
        assert_eq!(vf.project_id, project.id);

        let stats = engine.sync_project_within(&project.id, NO_WINDOW).unwrap();

        assert_eq!(stats.files_seen, 1, "the walk must skip the ignored folder");
        assert_eq!(stats.files_removed, 0);
        assert!(engine
            .store
            .get_file(&project.id, "notes/scratch.md")
            .unwrap()
            .is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_row_under_an_excluded_directory_is_pruned() {
        let (engine, project, dir) = setup("excluded");
        write(&dir, "a.md", "# A");
        write(&dir, "target/x.md", "# X");
        let abs = dir.join("target/x.md");
        engine
            .store
            .register_known_path(&IndexedFile {
                project_id: project.id.clone(),
                abs_path: abs,
                rel_path: "target/x.md".into(),
                title: "x.md".into(),
                size_bytes: 3,
                modified_at: String::new(),
            })
            .unwrap();

        let stats = engine.sync_project_within(&project.id, NO_WINDOW).unwrap();

        assert_eq!(stats.files_removed, 1);
        assert!(engine
            .store
            .get_file(&project.id, "target/x.md")
            .unwrap()
            .is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn concurrent_syncs_run_once() {
        let (engine, project, dir) = setup("flight");
        for i in 0..30 {
            write(&dir, &format!("f{i}.md"), &format!("# F{i}"));
        }
        let engine = Arc::new(engine);
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let engine = engine.clone();
                let id = project.id.clone();
                std::thread::spawn(move || engine.sync_project(&id).unwrap())
            })
            .collect();
        let results: Vec<SyncStats> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        assert_eq!(results.iter().filter(|s| !s.skipped_recent).count(), 1);
        assert_eq!(engine.file_count(&project.id).unwrap(), 30);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn syncing_an_unknown_project_is_an_error() {
        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        assert!(matches!(
            engine.sync_project("nope"),
            Err(Error::ProjectNotFound(_))
        ));
    }
}
