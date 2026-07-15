//! Filesystem watcher: notify-debouncer-full (200ms) → incremental reindex →
//! broadcast a reload-signal. Watches each project known at daemon start
//! (PRD FR-08/FR-09/FR-09b).

use anyhow::Result;
use mdview_core::indexer::IndexService;
use mdview_core::Engine;
use notify::{RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

pub type WatchHandle = Debouncer<notify::RecommendedWatcher, FileIdMap>;

/// Build a debouncer watching every registered project. The returned handle
/// must be kept alive for the daemon's lifetime.
pub fn spawn_watchers(engine: Arc<Engine>, reload_tx: broadcast::Sender<String>) -> Result<WatchHandle> {
    let debounce = Duration::from_millis(engine.config.indexing.debounce_ms.max(50));
    let cb_engine = engine.clone();

    let mut debouncer = new_debouncer(debounce, None, move |res: DebounceEventResult| {
        if let Ok(events) = res {
            let paths: Vec<_> = events.into_iter().flat_map(|e| e.paths.clone()).collect();
            if reindex_paths(&cb_engine, &paths) {
                let _ = reload_tx.send("reload".to_string());
            }
        }
    })?;

    for project in engine.list_projects().unwrap_or_default() {
        let root = project.root_path.clone();
        if root.exists() {
            debouncer.watcher().watch(&root, RecursiveMode::Recursive).ok();
            debouncer.cache().add_root(&root, RecursiveMode::Recursive);
        }
    }
    Ok(debouncer)
}

/// Reindex the given paths incrementally. Returns true if anything relevant changed.
fn reindex_paths(engine: &Engine, paths: &[std::path::PathBuf]) -> bool {
    let projects = engine.list_projects().unwrap_or_default();
    let max_bytes = engine.config.indexing.max_file_size_mb.saturating_mul(1024 * 1024);
    let mut changed = false;

    for path in paths {
        if !is_markdown(path) {
            continue;
        }
        let Some(project) = projects.iter().find(|p| path.starts_with(&p.root_path)) else {
            continue;
        };
        if path.exists() {
            if IndexService::index_file(&engine.store, project, path, max_bytes).is_ok() {
                changed = true;
            }
        } else {
            // Removed/renamed away — drop from index (survives atomic-save because
            // the debounced batch also carries the recreated path).
            let _ = IndexService::remove_file(&engine.store, project, path);
            changed = true;
        }
    }
    changed
}

fn is_markdown(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref(),
        Some("md") | Some("markdown")
    )
}
