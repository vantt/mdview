//! Background cleanup sweep: periodically drops registry records nobody has
//! accessed in a while. This only ever deletes rows in mdview's own SQLite
//! index (`repository::cleanup_stale`) — it never touches a project's real
//! files on disk. A cleaned-up file re-indexes itself the next time its full
//! URL is opened (`Engine::ensure_indexed`); a cleaned-up project has to be
//! reopened via MCP/CLI to be rediscovered, since its `root_path` is gone
//! from the registry too.

use mdview_core::indexer::cutoff_rfc3339;
use mdview_core::Engine;
use std::sync::Arc;
use std::time::Duration;

/// A file record not viewed in this long is dropped from the index.
const FILE_TTL_SECS: i64 = 7 * 24 * 60 * 60;
/// A project not seen (any view, MCP call, or CLI register) in this long is
/// dropped from the registry, taking its files with it.
const PROJECT_TTL_SECS: i64 = 30 * 24 * 60 * 60;
/// How often the sweep runs while the daemon is up.
const SWEEP_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Spawn the periodic sweep. Detached — runs for the daemon's process
/// lifetime, same as the filesystem watcher, with nothing to keep alive or
/// shut down explicitly.
pub fn spawn(engine: Arc<Engine>) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            ticker.tick().await;
            sweep_once(&engine, FILE_TTL_SECS, PROJECT_TTL_SECS);
        }
    });
}

/// TTLs are parameters (rather than reading the module constants directly)
/// so a test can force staleness deterministically — a negative TTL pushes
/// the cutoff into the future, making every real timestamp look stale
/// without needing to fake the clock or backdate any row.
fn sweep_once(engine: &Engine, file_ttl_secs: i64, project_ttl_secs: i64) {
    let file_cutoff = cutoff_rfc3339(file_ttl_secs);
    let project_cutoff = cutoff_rfc3339(project_ttl_secs);
    match engine.store.cleanup_stale(&file_cutoff, &project_cutoff) {
        Ok((files, projects)) if files > 0 || projects > 0 => {
            tracing::info!(
                files,
                projects,
                "cleanup sweep removed stale registry records"
            );
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("cleanup sweep failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdview_core::{Config, SqliteStore};

    /// Comfortably longer than any real TTL, so a cutoff built from it lands
    /// far in the past and nothing looks stale against it.
    const NEVER: i64 = 100 * 365 * 24 * 60 * 60;
    /// A cutoff in the near future — every real timestamp is stale against it.
    const IMMEDIATELY: i64 = -3600;

    fn write(dir: &std::path::Path, rel: &str, body: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    #[test]
    fn sweep_once_removes_files_past_the_file_ttl_but_leaves_a_fresh_project() {
        let dir = std::env::temp_dir().join(format!("mdview-sweep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, "a.md", "# A");

        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let (project, _) = engine.register(&dir, None).unwrap();
        // `register` no longer scans (D-async-index) — index the fixture file
        // now so the sweep has something real to test against.
        engine
            .index_file_incremental(&project, &dir.join("a.md"))
            .unwrap();

        sweep_once(&engine, IMMEDIATELY, NEVER);

        assert!(engine
            .store
            .get_file(&project.id, "a.md")
            .unwrap()
            .is_none());
        assert!(engine.get_project(&project.id).unwrap().is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sweep_once_removes_a_project_past_the_project_ttl_and_its_files_with_it() {
        let dir = std::env::temp_dir().join(format!("mdview-sweep-proj-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, "a.md", "# A");

        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let (project, _) = engine.register(&dir, None).unwrap();
        // `register` no longer scans (D-async-index) — index the fixture file
        // now so the sweep has something real to test against.
        engine
            .index_file_incremental(&project, &dir.join("a.md"))
            .unwrap();

        sweep_once(&engine, NEVER, IMMEDIATELY);

        assert!(engine.get_project(&project.id).unwrap().is_none());
        assert!(engine
            .store
            .get_file(&project.id, "a.md")
            .unwrap()
            .is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sweep_once_leaves_everything_when_both_ttls_are_generous() {
        let dir = std::env::temp_dir().join(format!("mdview-sweep-fresh-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, "a.md", "# A");

        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let (project, _) = engine.register(&dir, None).unwrap();
        // `register` no longer scans (D-async-index) — index the fixture file
        // now so the sweep has something real to test against.
        engine
            .index_file_incremental(&project, &dir.join("a.md"))
            .unwrap();

        sweep_once(&engine, NEVER, NEVER);

        assert!(engine
            .store
            .get_file(&project.id, "a.md")
            .unwrap()
            .is_some());
        assert!(engine.get_project(&project.id).unwrap().is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn documented_ttls_match_the_one_week_and_thirty_day_thresholds() {
        assert_eq!(FILE_TTL_SECS, 7 * 24 * 60 * 60);
        assert_eq!(PROJECT_TTL_SECS, 30 * 24 * 60 * 60);
    }
}
