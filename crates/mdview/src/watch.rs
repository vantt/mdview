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
use mdview_core::indexer::{content_hash, is_markdown};
use mdview_core::Engine;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

type WatchDebouncer = Debouncer<RecommendedWatcher, FileIdMap>;

/// How often the watched set is recomputed from the index.
const RECONCILE_EVERY: Duration = Duration::from_secs(5);
/// How often the reconcile thread wakes to check for hints and shutdown.
const POLL_TICK: Duration = Duration::from_millis(250);

/// Keeps the watcher and its reconcile thread alive; dropping it stops both.
pub struct WatchHandle {
    _inner: Arc<Mutex<WatchDebouncer>>,
    stop: Arc<AtomicBool>,
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum ReloadKind {
    /// The file's content actually changed (`index_docs` reported so).
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

/// Directories to start and stop watching to go from `current` to `desired`.
fn diff_watch_sets(
    current: &HashSet<PathBuf>,
    desired: &HashSet<PathBuf>,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let to_add = desired.difference(current).cloned().collect();
    let to_remove = current.difference(desired).cloned().collect();
    (to_add, to_remove)
}

/// Watch the directories that hold indexed rows (non-recursively). The returned
/// handle must be kept alive for the daemon's lifetime.
pub fn spawn_watchers(
    engine: Arc<Engine>,
    reload_tx: broadcast::Sender<String>,
) -> Result<WatchHandle> {
    let debounce = Duration::from_millis(engine.config.indexing.debounce_ms.max(50));
    let cb_engine = engine.clone();
    let watched: Arc<Mutex<HashSet<PathBuf>>> = Arc::default();
    let cb_watched = watched.clone();
    // Last content hash seen per file; owned by the event callback.
    let mut seen: HashMap<PathBuf, String> = HashMap::new();

    let debouncer = new_debouncer(debounce, None, move |res: DebounceEventResult| {
        if let Ok(events) = res {
            let paths: Vec<_> = events.into_iter().flat_map(|e| e.paths.clone()).collect();
            // A watched directory that vanished no longer has a live watch;
            // forget it so the next reconcile can re-add it if it returns.
            if let Ok(mut set) = cb_watched.lock() {
                set.retain(|d| paths.iter().all(|p| p != d) || d.exists());
            }
            let events = reindex_paths(&cb_engine, &paths, &mut seen);
            if let Some(payload) = broadcast_payload(&events) {
                let _ = reload_tx.send(payload);
            }
        }
    })?;

    let inner = Arc::new(Mutex::new(debouncer));
    let stop = Arc::new(AtomicBool::new(false));
    let (hint_tx, hint_rx) = mpsc::channel();
    engine.set_dir_hint_sender(hint_tx);
    spawn_reconciler(
        engine,
        Arc::downgrade(&inner),
        watched,
        stop.clone(),
        hint_rx,
        RECONCILE_EVERY,
        POLL_TICK,
    )?;
    Ok(WatchHandle {
        _inner: inner,
        stop,
    })
}

/// Thread that keeps the watched set equal to the indexed directories: hints
/// add a directory at once, and every `reconcile_every` the full set is
/// recomputed. Exits when `stop` is set or the debouncer is gone.
fn spawn_reconciler(
    engine: Arc<Engine>,
    debouncer: Weak<Mutex<WatchDebouncer>>,
    watched: Arc<Mutex<HashSet<PathBuf>>>,
    stop: Arc<AtomicBool>,
    hints: Receiver<PathBuf>,
    reconcile_every: Duration,
    tick: Duration,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("mdview-watch-reconcile".into())
        .spawn(move || {
            // Reconcile immediately so already-indexed directories are covered at start.
            let mut last_reconcile: Option<Instant> = None;
            loop {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                let Some(deb) = debouncer.upgrade() else {
                    return;
                };
                match hints.recv_timeout(tick) {
                    Ok(dir) => {
                        if dir.is_dir() {
                            let mut set = watched.lock().unwrap();
                            if !set.contains(&dir) {
                                if let Some(dir) = watch_dir(&deb, dir) {
                                    set.insert(dir);
                                }
                            }
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => {
                        // The engine dropped its sender; keep reconciling only.
                        std::thread::sleep(tick);
                    }
                }
                if last_reconcile.is_none_or(|t| t.elapsed() >= reconcile_every) {
                    last_reconcile = Some(Instant::now());
                    reconcile(&engine, &deb, &watched);
                }
            }
        })
}

/// Start watching `dir` non-recursively; `None` (logged) if the watch failed.
fn watch_dir(deb: &Mutex<WatchDebouncer>, dir: PathBuf) -> Option<PathBuf> {
    let mut deb = deb.lock().ok()?;
    if let Err(e) = deb.watcher().watch(&dir, RecursiveMode::NonRecursive) {
        tracing::debug!("watch {} failed: {e}", dir.display());
        return None;
    }
    deb.cache().add_root(&dir, RecursiveMode::NonRecursive);
    Some(dir)
}

fn reconcile(engine: &Engine, deb: &Mutex<WatchDebouncer>, watched: &Mutex<HashSet<PathBuf>>) {
    let desired: HashSet<PathBuf> = match engine.store.indexed_dirs() {
        Ok(dirs) => dirs.into_iter().filter(|d| d.is_dir()).collect(),
        Err(e) => {
            tracing::debug!("reading indexed directories failed: {e}");
            return;
        }
    };
    let mut set = watched.lock().unwrap();
    let (to_add, to_remove) = diff_watch_sets(&set, &desired);
    for dir in to_remove {
        // `unwatch` only: `remove_root` would also drop nested roots from the cache.
        if let Ok(mut d) = deb.lock() {
            if let Err(e) = d.watcher().unwatch(&dir) {
                tracing::debug!("unwatch {} failed: {e}", dir.display());
            }
        }
        set.remove(&dir);
    }
    for dir in to_add {
        if let Some(dir) = watch_dir(deb, dir) {
            set.insert(dir);
        }
    }
}

/// Reindex the given paths incrementally. Returns one [`ReloadEvent`] per path
/// that actually warrants telling a browser about. `seen` is the watcher's own
/// memory of each file's last content hash: a reload is decided from it, not
/// from the index, because views and searches may already have indexed the new
/// content. A touch that leaves a file's bytes unchanged produces no event.
fn reindex_paths(
    engine: &Engine,
    paths: &[PathBuf],
    seen: &mut HashMap<PathBuf, String>,
) -> Vec<ReloadEvent> {
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
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            let hash = content_hash(&String::from_utf8_lossy(&bytes));
            // Refresh the index and outgoing links (keeps backlinks live); the
            // result is ignored — the reload decision is the watcher's own.
            let _ = engine.index_file_incremental(project, path);
            // Files the index refuses (excluded, outside the root) have no row.
            let indexed = matches!(engine.store.file_state(&project.id, &rel_path), Ok(Some(_)));
            let changed = seen.get(path) != Some(&hash);
            seen.insert(path.clone(), hash);
            if indexed && changed {
                events.push(ReloadEvent {
                    kind: ReloadKind::Changed,
                    project_id: project.id.clone(),
                    rel_path,
                });
            }
        } else {
            // Removed/renamed away — drop from index (survives atomic-save because
            // the debounced batch also carries the recreated path). Always
            // reported: there is no content left to hash.
            seen.remove(path);
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

#[cfg(test)]
mod tests {
    use super::*;
    use mdview_core::{Config, SqliteStore};
    use std::fs;

    fn engine_with_project(dir: &std::path::Path) -> Engine {
        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let (project, _) = engine.ensure_project(dir, None).unwrap();
        // `ensure_project` no longer scans (D-async-index) — index whatever's
        // already on disk now so these tests exercise the watcher's
        // incremental reindex against a known baseline, same as before.
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if is_markdown(&path) {
                engine.index_file_incremental(&project, &path).unwrap();
            }
        }
        engine
    }

    #[test]
    fn changing_a_files_content_emits_one_changed_event() {
        let dir = tempdir();
        let file = dir.path().join("a.md");
        fs::write(&file, "one").unwrap();
        let engine = engine_with_project(dir.path());

        fs::write(&file, "two").unwrap();
        let events = reindex_paths(&engine, &[file], &mut HashMap::new());

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, ReloadKind::Changed);
        assert_eq!(events[0].rel_path, "a.md");
    }

    /// A touch that leaves bytes identical must not
    /// produce an event, or every browser viewing this file flickers for no
    /// reason.
    #[test]
    fn touching_a_file_without_changing_its_bytes_emits_nothing() {
        let dir = tempdir();
        let file = dir.path().join("a.md");
        fs::write(&file, "same content").unwrap();
        let engine = engine_with_project(dir.path());

        // The watcher has already seen this content once.
        let mut seen = HashMap::new();
        assert_eq!(
            reindex_paths(&engine, std::slice::from_ref(&file), &mut seen).len(),
            1
        );

        // Re-write the exact same bytes -- a stand-in for a touch/checkout that
        // doesn't actually alter content.
        fs::write(&file, "same content").unwrap();
        let events = reindex_paths(&engine, &[file], &mut seen);

        assert!(events.is_empty(), "expected no event, got {events:?}");
    }

    /// A view or search may index the new content before the watcher's event
    /// arrives; the reload must still be emitted.
    #[test]
    fn a_change_already_indexed_by_someone_else_still_emits_changed() {
        let dir = tempdir();
        let file = dir.path().join("a.md");
        fs::write(&file, "one").unwrap();
        let engine = engine_with_project(dir.path());
        let mut seen = HashMap::new();
        reindex_paths(&engine, std::slice::from_ref(&file), &mut seen);

        fs::write(&file, "two").unwrap();
        let project = engine.list_projects().unwrap().remove(0);
        assert!(engine.index_file_incremental(&project, &file).unwrap());

        let events = reindex_paths(&engine, &[file], &mut seen);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, ReloadKind::Changed);
    }

    #[test]
    fn diff_watch_sets_returns_additions_and_removals() {
        let set = |v: &[&str]| v.iter().map(PathBuf::from).collect::<HashSet<_>>();
        let (add, remove) = diff_watch_sets(&set(&["/a", "/b"]), &set(&["/b", "/c"]));
        assert_eq!(add, vec![PathBuf::from("/c")]);
        assert_eq!(remove, vec![PathBuf::from("/a")]);
        let (add, remove) = diff_watch_sets(&set(&["/a"]), &set(&["/a"]));
        assert!(add.is_empty() && remove.is_empty());
    }

    /// Hints add a directory immediately, and dropping the handle's stop flag
    /// ends the thread.
    #[test]
    fn reconciler_watches_hinted_dirs_and_stops_on_request() {
        let dir = tempdir();
        // The directory must hold an indexed row, or the reconcile pass drops it.
        fs::write(dir.path().join("a.md"), "x").unwrap();
        let engine = Arc::new(engine_with_project(dir.path()));
        let deb = Arc::new(Mutex::new(
            new_debouncer(Duration::from_millis(50), None, |_: DebounceEventResult| {}).unwrap(),
        ));
        let watched: Arc<Mutex<HashSet<PathBuf>>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let handle = spawn_reconciler(
            engine,
            Arc::downgrade(&deb),
            watched.clone(),
            stop.clone(),
            rx,
            Duration::from_secs(3600),
            Duration::from_millis(10),
        )
        .unwrap();

        tx.send(dir.path().to_path_buf()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !watched.lock().unwrap().contains(dir.path()) {
            assert!(Instant::now() < deadline, "hinted dir was never watched");
            std::thread::sleep(Duration::from_millis(10));
        }

        stop.store(true, Ordering::Relaxed);
        join_with_timeout(handle, Duration::from_secs(5));
    }

    #[test]
    fn reconciler_exits_when_the_debouncer_is_gone() {
        let dir = tempdir();
        let engine = Arc::new(engine_with_project(dir.path()));
        let deb = Arc::new(Mutex::new(
            new_debouncer(Duration::from_millis(50), None, |_: DebounceEventResult| {}).unwrap(),
        ));
        let (_tx, rx) = mpsc::channel();
        let handle = spawn_reconciler(
            engine,
            Arc::downgrade(&deb),
            Arc::default(),
            Arc::new(AtomicBool::new(false)),
            rx,
            Duration::from_secs(3600),
            Duration::from_millis(10),
        )
        .unwrap();
        drop(deb);
        join_with_timeout(handle, Duration::from_secs(5));
    }

    fn join_with_timeout(handle: JoinHandle<()>, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while !handle.is_finished() {
            assert!(Instant::now() < deadline, "thread did not stop in time");
            std::thread::sleep(Duration::from_millis(10));
        }
        handle.join().unwrap();
    }

    #[test]
    fn removing_a_file_emits_a_removed_event_even_with_no_content_to_hash() {
        let dir = tempdir();
        let file = dir.path().join("a.md");
        fs::write(&file, "gone soon").unwrap();
        let engine = engine_with_project(dir.path());

        fs::remove_file(&file).unwrap();
        let events = reindex_paths(&engine, &[file], &mut HashMap::new());

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

        let events = reindex_paths(&engine, &[file], &mut HashMap::new());

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
        let events = reindex_paths(&engine, &[a, b], &mut HashMap::new());

        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.kind == ReloadKind::Changed));
    }

    #[test]
    fn non_markdown_paths_are_ignored() {
        let dir = tempdir();
        let engine = engine_with_project(dir.path());
        let file = dir.path().join("notes.txt");
        fs::write(&file, "irrelevant").unwrap();

        let events = reindex_paths(&engine, &[file], &mut HashMap::new());

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
