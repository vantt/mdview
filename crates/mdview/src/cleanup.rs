//! Background cleanup sweep: periodically drops registry records of projects
//! nobody has used in a while. This only ever deletes rows in mdview's own
//! SQLite index (`repository::cleanup_stale`) — it never touches a project's
//! real files on disk. A cleaned-up project has to be reopened via MCP/CLI to
//! be rediscovered, since its `root_path` is gone from the registry too.

use mdview_core::indexer::cutoff_rfc3339;
use mdview_core::Engine;
use std::sync::Arc;
use std::time::Duration;

/// A project not seen (any view, search, MCP call, or CLI register) in this
/// long is dropped from the registry, taking its files with it.
const PROJECT_TTL_SECS: i64 = 14 * 24 * 60 * 60;
/// How often the sweep runs while the daemon is up.
const SWEEP_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Spawn the periodic sweep. Detached — runs for the daemon's process
/// lifetime, same as the filesystem watcher, with nothing to keep alive or
/// shut down explicitly. The sweep itself is blocking SQLite work, so it runs
/// on the blocking pool rather than a runtime worker.
pub fn spawn(engine: Arc<Engine>) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            ticker.tick().await;
            let engine = engine.clone();
            if let Err(e) =
                tokio::task::spawn_blocking(move || sweep_once(&engine, PROJECT_TTL_SECS)).await
            {
                tracing::warn!("cleanup sweep task failed: {e}");
            }
        }
    });
}

/// The TTL is a parameter (rather than reading the module constant directly)
/// so a test can force staleness deterministically — a negative TTL pushes
/// the cutoff into the future, making every real timestamp look stale
/// without needing to fake the clock or backdate any row.
fn sweep_once(engine: &Engine, project_ttl_secs: i64) {
    let project_cutoff = cutoff_rfc3339(project_ttl_secs);
    match engine.store.cleanup_stale(&project_cutoff) {
        Ok(0) => {}
        Ok(projects) => {
            tracing::info!(projects, "cleanup sweep removed stale registry records");
            match engine.store.vacuum_if_fragmented() {
                Ok(true) => tracing::info!("registry vacuumed after cleanup"),
                Ok(false) => {}
                Err(e) => tracing::warn!("registry vacuum failed: {e}"),
            }
        }
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

    fn indexed_engine(tag: &str) -> (Engine, mdview_core::domain::Project, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("mdview-sweep-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, "a.md", "# A");
        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let (project, _) = engine.register(&dir, None).unwrap();
        // `register` does not scan — index the fixture file so the sweep has
        // something real to test against.
        engine
            .index_file_incremental(&project, &dir.join("a.md"))
            .unwrap();
        (engine, project, dir)
    }

    #[test]
    fn sweep_once_removes_a_project_past_the_ttl_and_its_files_with_it() {
        let (engine, project, dir) = indexed_engine("proj");

        sweep_once(&engine, IMMEDIATELY);

        assert!(engine.get_project(&project.id).unwrap().is_none());
        assert!(engine
            .store
            .get_file(&project.id, "a.md")
            .unwrap()
            .is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sweep_once_leaves_everything_when_the_ttl_is_generous() {
        let (engine, project, dir) = indexed_engine("fresh");

        sweep_once(&engine, NEVER);

        assert!(engine
            .store
            .get_file(&project.id, "a.md")
            .unwrap()
            .is_some());
        assert!(engine.get_project(&project.id).unwrap().is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn documented_ttl_is_fourteen_days() {
        assert_eq!(PROJECT_TTL_SECS, 14 * 24 * 60 * 60);
    }
}
