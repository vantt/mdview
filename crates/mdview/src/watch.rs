//! Filesystem watcher: notify-debouncer-full (200ms) → incremental reindex →
//! broadcast a reload-signal. Watches each project known at daemon start
//! (PRD FR-08/FR-09/FR-09b).
//!
//! The broadcast is unscoped on purpose (every connected browser receives every
//! message) — see `docs/history/scoped-live-reload/CONTEXT.md` D1. Each event
//! carries the `(project_id, rel_path)` it is actually about, and the client
//! (`assets/app.js`) compares that against its own `location.pathname` before
//! deciding to reload. That keeps the server free of any per-connection
//! "which socket is viewing which file" state.

use anyhow::Result;
use mdview_core::Engine;
use notify::{RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

pub type WatchHandle = Debouncer<notify::RecommendedWatcher, FileIdMap>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum ReloadKind {
    /// The file's content actually changed (`upsert_file` reported so).
    Changed,
    /// The file left the index. Always reported regardless of content-hash —
    /// there is no new content to hash, and a browser viewing this exact file
    /// needs to know it is gone.
    Removed,
}

/// One file whose viewers should reload. `project_id`/`rel_path` are what the
/// client matches against its own URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ReloadEvent {
    kind: ReloadKind,
    project_id: String,
    rel_path: String,
}

/// Build a debouncer watching every registered project. The returned handle
/// must be kept alive for the daemon's lifetime.
pub fn spawn_watchers(
    engine: Arc<Engine>,
    reload_tx: broadcast::Sender<String>,
) -> Result<WatchHandle> {
    let debounce = Duration::from_millis(engine.config.indexing.debounce_ms.max(50));
    let cb_engine = engine.clone();

    let mut debouncer = new_debouncer(debounce, None, move |res: DebounceEventResult| {
        if let Ok(events) = res {
            let paths: Vec<_> = events.into_iter().flat_map(|e| e.paths.clone()).collect();
            let events = reindex_paths(&cb_engine, &paths);
            if let Some(payload) = broadcast_payload(&events) {
                let _ = reload_tx.send(payload);
            }
        }
    })?;

    for project in engine.list_projects().unwrap_or_default() {
        let root = project.root_path.clone();
        if root.exists() {
            debouncer
                .watcher()
                .watch(&root, RecursiveMode::Recursive)
                .ok();
            debouncer.cache().add_root(&root, RecursiveMode::Recursive);
        }
    }
    Ok(debouncer)
}

/// Reindex the given paths incrementally. Returns one [`ReloadEvent`] per path
/// that actually warrants telling a browser about — a touch that leaves a
/// file's bytes unchanged produces none.
fn reindex_paths(engine: &Engine, paths: &[std::path::PathBuf]) -> Vec<ReloadEvent> {
    let projects = engine.list_projects().unwrap_or_default();
    let mut events = Vec::new();

    for path in paths {
        if !is_markdown(path) {
            continue;
        }
        let Some(project) = projects.iter().find(|p| path.starts_with(&p.root_path)) else {
            continue;
        };
        let rel_path = mdview_core::indexer::rel_path_str(&project.root_path, path);
        if rel_path.is_empty() {
            continue;
        }
        if path.exists() {
            // Reindex the file and refresh its outgoing links (keeps backlinks live).
            // Only a real content change (not just a touch) is worth telling a
            // browser about — see D2, CONTEXT.md.
            if let Ok(true) = engine.index_file_incremental(project, path) {
                events.push(ReloadEvent {
                    kind: ReloadKind::Changed,
                    project_id: project.id.clone(),
                    rel_path,
                });
            }
        } else {
            // Removed/renamed away — drop from index (survives atomic-save because
            // the debounced batch also carries the recreated path). Always
            // reported: there is no content left to hash, and D4 says a removal
            // is never something to filter out.
            let _ = engine.remove_file(project, path);
            events.push(ReloadEvent {
                kind: ReloadKind::Removed,
                project_id: project.id.clone(),
                rel_path,
            });
        }
    }
    events
}

/// The exact wire message `app.js`'s `ws.onmessage` parses. `None` for an
/// empty batch — a debounce tick where nothing warranted an event sends
/// nothing at all, rather than an empty `{"events":[]}` no-op message.
fn broadcast_payload(events: &[ReloadEvent]) -> Option<String> {
    if events.is_empty() {
        return None;
    }
    serde_json::to_string(&serde_json::json!({ "events": events })).ok()
}

fn is_markdown(p: &Path) -> bool {
    matches!(
        p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("md") | Some("markdown")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdview_core::{Config, SqliteStore};
    use std::fs;

    fn engine_with_project(dir: &std::path::Path) -> Engine {
        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        engine.ensure_project(dir, None).unwrap();
        engine
    }

    #[test]
    fn changing_a_files_content_emits_one_changed_event() {
        let dir = tempdir();
        let file = dir.path().join("a.md");
        fs::write(&file, "one").unwrap();
        let engine = engine_with_project(dir.path());

        fs::write(&file, "two").unwrap();
        let events = reindex_paths(&engine, &[file]);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, ReloadKind::Changed);
        assert_eq!(events[0].rel_path, "a.md");
    }

    /// The whole point of D2: a touch that leaves bytes identical must not
    /// produce an event, or every browser viewing this file flickers for no
    /// reason.
    #[test]
    fn touching_a_file_without_changing_its_bytes_emits_nothing() {
        let dir = tempdir();
        let file = dir.path().join("a.md");
        fs::write(&file, "same content").unwrap();
        let engine = engine_with_project(dir.path());

        // Re-write the exact same bytes -- a stand-in for a touch/checkout that
        // doesn't actually alter content.
        fs::write(&file, "same content").unwrap();
        let events = reindex_paths(&engine, &[file]);

        assert!(events.is_empty(), "expected no event, got {events:?}");
    }

    #[test]
    fn removing_a_file_emits_a_removed_event_even_with_no_content_to_hash() {
        let dir = tempdir();
        let file = dir.path().join("a.md");
        fs::write(&file, "gone soon").unwrap();
        let engine = engine_with_project(dir.path());

        fs::remove_file(&file).unwrap();
        let events = reindex_paths(&engine, &[file]);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, ReloadKind::Removed);
        assert_eq!(events[0].rel_path, "a.md");
    }

    #[test]
    fn a_new_files_first_index_emits_a_changed_event() {
        let dir = tempdir();
        let engine = engine_with_project(dir.path());
        let file = dir.path().join("new.md");
        fs::write(&file, "brand new").unwrap();

        let events = reindex_paths(&engine, &[file]);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, ReloadKind::Changed);
    }

    #[test]
    fn multiple_changed_files_in_one_batch_all_get_events() {
        let dir = tempdir();
        let a = dir.path().join("a.md");
        let b = dir.path().join("b.md");
        fs::write(&a, "a1").unwrap();
        fs::write(&b, "b1").unwrap();
        let engine = engine_with_project(dir.path());

        fs::write(&a, "a2").unwrap();
        fs::write(&b, "b2").unwrap();
        let events = reindex_paths(&engine, &[a, b]);

        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.kind == ReloadKind::Changed));
    }

    #[test]
    fn non_markdown_paths_are_ignored() {
        let dir = tempdir();
        let engine = engine_with_project(dir.path());
        let file = dir.path().join("notes.txt");
        fs::write(&file, "irrelevant").unwrap();

        let events = reindex_paths(&engine, &[file]);

        assert!(events.is_empty());
    }

    #[test]
    fn events_serialize_with_the_shape_the_client_expects() {
        let ev = ReloadEvent {
            kind: ReloadKind::Changed,
            project_id: "p1".into(),
            rel_path: "docs/a.md".into(),
        };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["kind"], "changed");
        assert_eq!(json["project_id"], "p1");
        assert_eq!(json["rel_path"], "docs/a.md");
    }

    /// The exact envelope shape `app.js` parses (`payload.events`) — this is
    /// the wire-format contract between `spawn_watchers` and the client, the
    /// one piece the sandboxed test environment's inotify watch exhaustion
    /// made impossible to prove via a real filesystem event end-to-end (see
    /// plan.md's build notes).
    #[test]
    fn broadcast_payload_wraps_events_in_the_envelope_the_client_expects() {
        let events = vec![ReloadEvent {
            kind: ReloadKind::Changed,
            project_id: "p1".into(),
            rel_path: "docs/a.md".into(),
        }];
        let payload = broadcast_payload(&events).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(parsed["events"][0]["kind"], "changed");
        assert_eq!(parsed["events"][0]["project_id"], "p1");
        assert_eq!(parsed["events"][0]["rel_path"], "docs/a.md");
    }

    /// An empty batch (every path in the debounce tick was a no-op touch)
    /// must send nothing at all, not an empty envelope -- this is the actual
    /// fix for the "chớp chớp" flicker: no message means no reload anywhere.
    #[test]
    fn broadcast_payload_is_none_for_an_empty_batch() {
        assert_eq!(broadcast_payload(&[]), None);
    }

    /// Minimal per-test temp dir -- no extra dependency, just a unique path
    /// under the OS temp dir, cleaned up on drop.
    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn tempdir() -> TempDir {
        let p = std::env::temp_dir().join(format!(
            "mdview-watch-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
}
