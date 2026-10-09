//! Application core: the facade the HTTP/MCP/CLI adapters call. Owns the store,
//! config, and renderer, and implements the high-level use cases (view_file,
//! render, search, registry) — including implicit project auto-create (FR-04).

use crate::code_source::{self, DirListing, SourceContent};
use crate::config::Config;
use crate::domain::{IndexedFile, Project, RenderedPage};
use crate::error::{Error, Result};
use crate::indexer::{self, IndexService, IndexedDoc};
use crate::link_resolver::ProjectFs;
use crate::render::{HighlightedSource, RenderService};
use crate::repository::SqliteStore;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::Mutex;

/// Most neighbours indexed per page view, and the write batch size.
const NEIGHBOUR_CAP: usize = 200;

pub struct Engine {
    pub store: SqliteStore,
    pub config: Config,
    render: RenderService,
    /// Where `hint_dir` reports directories that gained an indexed row; set
    /// once by the filesystem watcher, absent in the CLI and in tests.
    dir_hint: Mutex<Option<Sender<PathBuf>>>,
    /// Projects with an `index_neighbours` call in flight.
    neighbours_running: Mutex<HashSet<String>>,
}

/// A page opened by `Engine::view_page`.
#[derive(Debug, Clone)]
pub struct ViewedPage {
    pub file: IndexedFile,
    pub page: RenderedPage,
    /// Absolute paths worth indexing next (see `Engine::index_neighbours`).
    pub neighbours: Vec<PathBuf>,
}

/// Clears a project's in-flight mark when `index_neighbours` ends, on any path.
struct NeighbourGuard<'a> {
    running: &'a Mutex<HashSet<String>>,
    project_id: &'a str,
}

impl Drop for NeighbourGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut running) = self.running.lock() {
            running.remove(self.project_id);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ViewFile {
    pub url: String,
    pub project_id: String,
    pub rel_path: String,
    /// Short code for this file — the `<code>` in `/s/<code>`.
    pub code: String,
    /// Whether this call just created the project (as opposed to reusing an
    /// existing one).
    pub is_new_project: bool,
}

impl Engine {
    pub fn new(store: SqliteStore, config: Config) -> Self {
        Self {
            store,
            config,
            render: RenderService::new(),
            dir_hint: Mutex::new(None),
            neighbours_running: Mutex::new(HashSet::new()),
        }
    }

    /// Install the watcher's directory-hint sender (called once at startup).
    pub fn set_dir_hint_sender(&self, tx: Sender<PathBuf>) {
        *self.dir_hint.lock().unwrap() = Some(tx);
    }

    /// Best-effort, non-blocking notify that `dir` now holds an indexed row,
    /// so the watcher covers it without waiting for its periodic reconcile.
    pub fn hint_dir(&self, dir: &Path) {
        if let Some(tx) = self.dir_hint.lock().unwrap().as_ref() {
            let _ = tx.send(dir.to_path_buf());
        }
    }

    pub(crate) fn max_bytes(&self) -> u64 {
        self.config
            .indexing
            .max_file_size_mb
            .saturating_mul(1024 * 1024)
    }

    /// Canonicalize when possible; otherwise fall back to the given path.
    fn canonical(root: &Path) -> PathBuf {
        std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
    }

    /// Find the project owning `root`, or create it (implicit registration).
    /// Never scans — a brand-new project's row is created empty and its
    /// second return value is `true`. File content is indexed on demand
    /// (`view_page`, from the HTTP path when a visitor opens a page) or
    /// by `sync_project` when a content search needs the whole project.
    pub fn ensure_project(&self, root: &Path, name: Option<&str>) -> Result<(Project, bool)> {
        let root = Self::canonical(root);
        if let Some(mut p) = self.store.find_project_by_root(&root)? {
            p.last_seen_at = indexer::now_rfc3339();
            self.store.upsert_project(&p)?;
            return Ok((p, false));
        }
        let id = self.unique_id(&indexer::slug_from_root(&root))?;
        let name = name.map(|s| s.to_string()).unwrap_or_else(|| {
            root.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&id)
                .to_string()
        });
        let now = indexer::now_rfc3339();
        let project = Project {
            id,
            name,
            root_path: root,
            created_at: now.clone(),
            last_seen_at: now,
        };
        self.store.upsert_project(&project)?;
        Ok((project, true))
    }

    fn unique_id(&self, base: &str) -> Result<String> {
        if self.store.get_project(base)?.is_none() {
            return Ok(base.to_string());
        }
        for n in 2..10_000 {
            let cand = format!("{base}-{n}");
            if self.store.get_project(&cand)?.is_none() {
                return Ok(cand);
            }
        }
        Err(Error::Other("could not allocate project id".into()))
    }

    /// The core `mdview_view_file` use case: ensure the project exists and
    /// hand back its app URL. Deliberately does *not* content-index the file;
    /// the URL is computable from the project id + rel path alone, and the
    /// content gets indexed when a browser really requests the page
    /// (`view_page` in the HTTP handler). Only markdown files can be
    /// viewed.
    ///
    /// It does register the path (`register_known_path`, a stat + one cheap
    /// insert — no content read) so `/s/<code>` resolves in O(1) via
    /// `path_hash` from the moment this call returns, instead of needing
    /// `resolve_short_code`'s full-tree fallback scan on the first click.
    /// Best-effort: a failure here (e.g. the file vanished mid-call) must
    /// never fail `view_file` itself, since the URL is valid either way.
    pub fn view_file(&self, project_root: &Path, rel_path: &str) -> Result<ViewFile> {
        let (project, is_new_project) = self.ensure_project(project_root, None)?;
        let abs = project.root_path.join(rel_path);
        let abs = crate::link_resolver::normalize(&abs);
        let rel = indexer::rel_path_str(&project.root_path, &abs);
        if rel.is_empty() {
            return Err(Error::PathOutsideProject(abs));
        }
        if !indexer::is_markdown(&abs) {
            return Err(Error::InvalidPath(format!("not a markdown file: {rel}")));
        }
        let code = crate::short_link::short_code(&crate::short_link::path_hash(&project.id, &rel));
        self.register_known_path_stub(&project, &abs, &rel);
        Ok(ViewFile {
            url: format!("/p/{}/{}", project.id, rel),
            project_id: project.id,
            rel_path: rel,
            code,
            is_new_project,
        })
    }

    /// `stat` + `register_known_path`, best-effort: a failure (file vanished,
    /// permission denied, path outside the project) must never fail the
    /// caller, since the URL/redirect stays valid either way — real
    /// content-indexing is `view_page`'s job, this only ever needs to
    /// make `path_hash` resolvable. Shared by `view_file` and
    /// `resolve_short_code`'s scan fallback. A path `confine` rejects (symlink
    /// out of the root, excluded directory) never gets a row.
    fn register_known_path_stub(&self, project: &Project, abs: &Path, rel: &str) {
        if indexer::confine(
            &project.root_path,
            abs,
            &self.config.indexing.exclude_patterns,
        )
        .is_none()
        {
            return;
        }
        let Ok(meta) = std::fs::metadata(abs) else {
            return;
        };
        let stub = IndexedFile {
            project_id: project.id.clone(),
            abs_path: abs.to_path_buf(),
            rel_path: rel.to_string(),
            title: indexer::filename(abs),
            size_bytes: meta.len(),
            modified_at: meta
                .modified()
                .ok()
                .and_then(|t| {
                    time::OffsetDateTime::from(t)
                        .format(&time::format_description::well_known::Rfc3339)
                        .ok()
                })
                .unwrap_or_default(),
        };
        if self.store.register_known_path(&stub).is_ok() {
            if let Some(dir) = abs.parent() {
                self.hint_dir(dir);
            }
        }
    }

    /// Register a project explicitly (CLI). Same as ensure_project + optional
    /// name — returns whether the project was newly created so the caller can
    /// kick off a background scan.
    pub fn register(&self, root: &Path, name: Option<&str>) -> Result<(Project, bool)> {
        self.ensure_project(root, name)
    }

    pub fn unregister(&self, project_id: &str) -> Result<()> {
        self.store.delete_project(project_id)
    }

    /// Read a file once and index it (row, FTS, outgoing links). Used by the
    /// filesystem watcher, and `save_file`. Returns whether
    /// the file's *content* actually changed (`false` for a file `confine`
    /// rejects or one that could not be read).
    pub fn index_file_incremental(&self, project: &Project, abs: &Path) -> Result<bool> {
        let Some(doc) = IndexService::build_doc(
            project,
            abs,
            self.max_bytes(),
            &self.config.indexing.exclude_patterns,
        ) else {
            return Ok(false);
        };
        let changed = self.store.index_docs(std::slice::from_ref(&doc))?;
        if let Some(dir) = doc.file.abs_path.parent() {
            self.hint_dir(dir);
        }
        Ok(changed.first().copied().unwrap_or(false))
    }

    /// Drop a file from the index (and its outgoing links).
    pub fn remove_file(&self, project: &Project, abs: &Path) -> Result<()> {
        IndexService::remove_file(&self.store, project, abs)
    }

    /// Resolve `/s/<code>` to `(project_id, rel_path)`. Content-indexing the
    /// file, if it isn't already, is the redirect target's job
    /// (`view_page`, called from the `/p/...` handler) — this only needs
    /// to answer "which file".
    ///
    /// `find_by_hash_prefix` reads the `path_hash` column, which is now set
    /// the moment `view_file` hands the code out (`register_known_path`'s
    /// stub row) — so in the common case this resolves in O(1) without ever
    /// touching the filesystem. The scan below is a safety net for a code
    /// whose stub is missing (a link from before this existed, or a registry
    /// restored from an older backup): a filename-only scan (no content
    /// reads) of every registered project, hashing each candidate to find the
    /// one the code belongs to, then registering just that file's stub so the
    /// redirect target can index it.
    ///
    /// The scan ignores `.gitignore` (unlike a full project scan) so it stays
    /// in parity with the long URL: `view_page` indexes whatever file is
    /// named regardless of `.gitignore`, and a link handed out by `view_file`
    /// for such a file must resolve here too, not 404 forever.
    pub fn resolve_short_code(&self, code: &str) -> Result<Option<(String, String)>> {
        if let Some(hit) = self.store.find_by_hash_prefix(code)? {
            return Ok(Some(hit));
        }
        for project in self.store.list_projects()? {
            for abs in indexer::scan_markdown_files_ignoring_gitignore(
                &project.root_path,
                &self.config.indexing.exclude_patterns,
            ) {
                let rel = indexer::rel_path_str(&project.root_path, &abs);
                if rel.is_empty() {
                    continue;
                }
                let hash = crate::short_link::path_hash(&project.id, &rel);
                if hash.starts_with(code) {
                    self.register_known_path_stub(&project, &abs, &rel);
                    return Ok(Some((project.id, rel)));
                }
            }
        }
        Ok(None)
    }

    /// Files that link to `rel_path` → (source_rel, title). FR-18 backlinks.
    pub fn backlinks(&self, project_id: &str, rel_path: &str) -> Result<Vec<(String, String)>> {
        self.store.backlinks(project_id, rel_path)
    }

    /// Open a markdown page: read the file once, render it, and index it when
    /// its content is new or changed (row, FTS, outgoing links). `Ok(None)`
    /// means the path is not a markdown file inside the project (missing,
    /// non-markdown, excluded, or a symlink out of the root) — the caller falls
    /// through to asset / folder / 404 handling. `Err` is a real failure.
    ///
    /// `neighbours` lists the page's link targets and sibling markdown files
    /// that are not indexed yet (or changed on disk); pass them to
    /// [`Engine::index_neighbours`] off the request path.
    pub fn view_page(&self, project_id: &str, rel_path: &str) -> Result<Option<ViewedPage>> {
        let project = self
            .store
            .get_project(project_id)?
            .ok_or_else(|| Error::ProjectNotFound(project_id.to_string()))?;
        let exclude = &self.config.indexing.exclude_patterns;
        let abs = crate::link_resolver::normalize(&project.root_path.join(rel_path));
        let Some((file, content)) =
            IndexService::read_file(&project, &abs, self.max_bytes(), exclude)
        else {
            return Ok(None);
        };
        let fs = ProjectFs {
            root: &project.root_path,
            exclude,
        };
        let page = self.render.render(
            &content,
            &file.abs_path,
            &project.id,
            &project.root_path,
            &fs,
        );

        let state = self.store.file_state(&project.id, &file.rel_path)?;
        let hash = indexer::content_hash(&content);
        let up_to_date = state
            .as_ref()
            .is_some_and(|s| s.content_indexed() && s.content_hash == hash);
        if !up_to_date {
            let doc = IndexedDoc {
                file: file.clone(),
                content,
                links: page.links.clone(),
            };
            self.store.index_docs(std::slice::from_ref(&doc))?;
        }
        self.record_access(&project.id);
        if let Some(dir) = file.abs_path.parent() {
            self.hint_dir(dir);
        }
        let neighbours = self.pending_neighbours(&project, &file, &page.links)?;
        Ok(Some(ViewedPage {
            file,
            page,
            neighbours,
        }))
    }

    /// Link targets and same-directory markdown siblings of `file` that still
    /// need indexing: missing, a stub, or whose size/mtime differ from the row.
    /// Every candidate passes `confine`; at most `NEIGHBOUR_CAP` are returned.
    fn pending_neighbours(
        &self,
        project: &Project,
        file: &IndexedFile,
        links: &[String],
    ) -> Result<Vec<PathBuf>> {
        let exclude = &self.config.indexing.exclude_patterns;
        let mut candidates: Vec<PathBuf> = links
            .iter()
            .map(|rel| project.root_path.join(rel))
            .collect();
        if let Some(dir) = file.abs_path.parent() {
            let mut siblings: Vec<PathBuf> = std::fs::read_dir(dir)
                .map(|rd| {
                    rd.flatten()
                        .map(|e| e.path())
                        .filter(|p| indexer::is_markdown(p))
                        .collect()
                })
                .unwrap_or_default();
            siblings.sort();
            candidates.extend(siblings);
        }

        let mut seen = std::collections::HashSet::new();
        let mut cands: Vec<(String, PathBuf, std::fs::Metadata)> = Vec::new();
        for abs in candidates {
            let rel = indexer::rel_path_str(&project.root_path, &abs);
            if rel.is_empty() || rel == file.rel_path || !seen.insert(rel.clone()) {
                continue;
            }
            if indexer::confine(&project.root_path, &abs, exclude).is_none() {
                continue;
            }
            let Ok(meta) = std::fs::metadata(&abs) else {
                continue;
            };
            cands.push((rel, abs, meta));
        }

        let rels: Vec<String> = cands.iter().map(|(rel, _, _)| rel.clone()).collect();
        let states = self.store.file_states_for(&project.id, &rels)?;
        Ok(cands
            .into_iter()
            .filter(|(rel, _, meta)| match states.get(rel) {
                Some(s) => {
                    !s.content_indexed()
                        || s.size_bytes != meta.len()
                        || s.modified_at != indexer::modified_rfc3339(meta)
                }
                None => true,
            })
            .map(|(_, abs, _)| abs)
            .take(NEIGHBOUR_CAP)
            .collect())
    }

    /// Index `paths` (a viewed page's neighbours) in batches. Single-flight per
    /// project: a call made while another is running for the same project does
    /// nothing and returns `Ok(0)`. Returns how many files were indexed.
    pub fn index_neighbours(&self, project_id: &str, paths: &[PathBuf]) -> Result<usize> {
        let _guard = {
            let mut running = self.neighbours_running.lock().unwrap();
            if !running.insert(project_id.to_string()) {
                return Ok(0);
            }
            NeighbourGuard {
                running: &self.neighbours_running,
                project_id,
            }
        };
        let project = self
            .store
            .get_project(project_id)?
            .ok_or_else(|| Error::ProjectNotFound(project_id.to_string()))?;
        let docs: Vec<IndexedDoc> = paths
            .iter()
            .filter_map(|abs| {
                IndexService::build_doc(
                    &project,
                    abs,
                    self.max_bytes(),
                    &self.config.indexing.exclude_patterns,
                )
            })
            .collect();
        for batch in docs.chunks(NEIGHBOUR_CAP) {
            self.store.index_docs(batch)?;
        }
        let dirs: std::collections::HashSet<&Path> = docs
            .iter()
            .filter_map(|d| d.file.abs_path.parent())
            .collect();
        for dir in dirs {
            self.hint_dir(dir);
        }
        Ok(docs.len())
    }

    /// Overwrite the markdown file `rel_path` in `project_id` with `content`
    /// and re-index it, so the next render (and the sidebar title, search,
    /// backlinks) already reflect the new bytes without waiting for the
    /// watcher. Only files already in the index are writable — the index is
    /// the whitelist of what the viewer shows, so it is also the whitelist of
    /// what the viewer may edit; nothing here can create a file or touch a
    /// path the viewer never listed.
    ///
    /// `expected_hash`, when given, is the `content_hash` of the source the
    /// editor started from (the page ships it). If the bytes on disk no longer
    /// hash to it, someone (an agent, another tab) wrote the file meanwhile
    /// and the save is refused with `Error::Conflict` rather than silently
    /// clobbering their work; the caller may retry without a hash to force.
    /// Returns the hash of the newly written content.
    pub fn save_file(
        &self,
        project_id: &str,
        rel_path: &str,
        content: &str,
        expected_hash: Option<&str>,
    ) -> Result<String> {
        let project = self
            .store
            .get_project(project_id)?
            .ok_or_else(|| Error::ProjectNotFound(project_id.to_string()))?;
        let file = self
            .store
            .get_file(project_id, rel_path)?
            .ok_or_else(|| Error::FileNotFound(rel_path.to_string()))?;
        if let Some(expected) = expected_hash {
            let current = std::fs::read_to_string(&file.abs_path)?;
            if indexer::content_hash(&current) != expected {
                return Err(Error::Conflict(rel_path.to_string()));
            }
        }
        // Write to a sibling temp file and rename over the target so a crash
        // mid-write never leaves a truncated document behind.
        let tmp = file.abs_path.with_extension("mdview-save.tmp");
        std::fs::write(&tmp, content)?;
        if let Err(e) = std::fs::rename(&tmp, &file.abs_path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.into());
        }
        self.index_file_incremental(&project, &file.abs_path)?;
        Ok(indexer::content_hash(content))
    }

    /// Record that `project_id` was actually viewed — the signal the
    /// periodic cleanup sweep checks (see `repository::cleanup_stale`, called
    /// from the daemon). Best-effort: bookkeeping must never fail the view
    /// itself.
    fn record_access(&self, project_id: &str) {
        let _ = self.store.touch_project_access(project_id);
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        self.store.list_projects()
    }

    pub fn get_project(&self, id: &str) -> Result<Option<Project>> {
        self.store.get_project(id)
    }

    pub fn file_count(&self, project_id: &str) -> Result<usize> {
        self.store.file_count(project_id)
    }

    /// Resolve an on-disk absolute path for an asset/image request, guarding
    /// against path traversal (must stay within the project root), a
    /// safe-extension allowlist, and configured exclude patterns.
    pub fn asset_path(&self, project_id: &str, rel_path: &str) -> Result<PathBuf> {
        let project = self
            .store
            .get_project(project_id)?
            .ok_or_else(|| Error::ProjectNotFound(project_id.to_string()))?;
        let joined = crate::link_resolver::normalize(&project.root_path.join(rel_path));
        let canonical = std::fs::canonicalize(&joined).unwrap_or(joined);
        if !canonical.starts_with(&project.root_path) {
            return Err(Error::PathOutsideProject(canonical));
        }
        // Extension check runs on `canonical` (post symlink-resolution), never
        // on `rel_path`/the URL segment: a symlink named e.g. pretty.png can
        // point at an arbitrary file, and only the resolved target's real
        // extension is trustworthy.
        if !has_allowed_asset_extension(&canonical) {
            return Err(Error::PathOutsideProject(canonical));
        }
        // Exclude-pattern check mirrors scan_markdown_files's semantics
        // (indexer.rs): exact component-name equality, not glob/substring.
        // Matched against canonical-stripped-of-root components (same
        // post-resolution path already used above) rather than the raw
        // rel_path, and never against the full absolute canonical path
        // (which would false-positive-exclude a project root that happens to
        // sit under a directory literally named one of the patterns).
        let rel = indexer::rel_path_str(&project.root_path, &canonical);
        if indexer::is_excluded(&rel, &self.config.indexing.exclude_patterns) {
            return Err(Error::PathOutsideProject(canonical));
        }
        Ok(canonical)
    }

    /// Resolve a Code-section request: a directory listing, a highlighted
    /// text file, or a binary notice. Every filesystem access goes through
    /// `code_source` (never `asset_path`'s extension allowlist — the Code
    /// section serves arbitrary text, so identity of the file is what's
    /// gated, not its extension). The caller (HTTP layer) never touches
    /// `code_source` or the renderer directly; both are private to `Engine`.
    pub fn code_path(&self, project_id: &str, rel_path: &str) -> Result<CodeView> {
        let project = self
            .store
            .get_project(project_id)?
            .ok_or_else(|| Error::ProjectNotFound(project_id.to_string()))?;
        let exclude = &self.config.indexing.exclude_patterns;
        let abs = code_source::resolve_source_path(&project.root_path, rel_path, exclude)?;
        let _ = self.store.touch_project_access(project_id);
        if abs.is_dir() {
            let listing = code_source::list_dir(&project.root_path, rel_path, exclude)?;
            return Ok(CodeView::Dir(listing));
        }
        match code_source::read_source(&abs)? {
            SourceContent::Binary { size } => Ok(CodeView::Binary { size }),
            SourceContent::Text { text, truncated } => {
                let size = text.len() as u64;
                let highlighted = self.render.highlight_source(&abs, &text);
                self.record_access(project_id);
                Ok(CodeView::File {
                    highlighted,
                    truncated,
                    size,
                })
            }
        }
    }
}

/// Result of resolving a Code-section path — see `Engine::code_path`.
pub enum CodeView {
    Dir(DirListing),
    File {
        highlighted: HighlightedSource,
        truncated: bool,
        size: u64,
    },
    Binary {
        size: u64,
    },
}

/// Extensions asset_path serves. Mirrors the 9 tokens
/// `crates/mdview/src/server.rs::content_type()` already recognizes;
/// mdview-core cannot import across the crate boundary, so keep this list in
/// sync if content_type() ever changes.
const ALLOWED_ASSET_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "svg", "webp", "ico", "bmp", "pdf",
];

fn has_allowed_asset_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .map(|e| ALLOWED_ASSET_EXTENSIONS.contains(&e.as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, body: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    #[test]
    fn view_file_auto_creates_project_and_returns_url() {
        let dir = std::env::temp_dir().join(format!("mdview-eng-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(
            &dir,
            "docs/architecture.md",
            "# Arch\nsee [api](../src/api/README.md)",
        );
        write(&dir, "src/api/README.md", "# API");

        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let vf = engine.view_file(&dir, "docs/architecture.md").unwrap();
        assert!(vf.url.starts_with("/p/"));
        assert!(vf.url.ends_with("/docs/architecture.md"));
        assert!(vf.is_new_project);

        // view_file deliberately doesn't content-index — but it does
        // register the viewed file's path as a stub, so it already counts as
        // a row...
        assert_eq!(engine.file_count(&vf.project_id).unwrap(), 1);
        assert!(!engine
            .store
            .is_content_indexed(&vf.project_id, "docs/architecture.md")
            .unwrap());

        // Stand in for the project-wide sync a content search triggers.
        engine.sync_project(&vf.project_id).unwrap();
        assert_eq!(engine.file_count(&vf.project_id).unwrap(), 2);
        assert!(engine
            .store
            .is_content_indexed(&vf.project_id, "docs/architecture.md")
            .unwrap());

        // rendering rewrites the cross-folder link
        let page = engine
            .view_page(&vf.project_id, "docs/architecture.md")
            .unwrap()
            .unwrap()
            .page;
        assert!(page
            .html
            .contains(&format!("/p/{}/src/api/README.md", vf.project_id)));

        // second call reuses the same project id
        let vf2 = engine.view_file(&dir, "src/api/README.md").unwrap();
        assert_eq!(vf.project_id, vf2.project_id);
        assert!(!vf2.is_new_project);

        // backlinks: architecture.md links to the API readme (FR-18)
        let back = engine
            .backlinks(&vf.project_id, "src/api/README.md")
            .unwrap();
        assert!(
            back.iter().any(|(rel, _)| rel == "docs/architecture.md"),
            "backlinks: {back:?}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// The short code `view_file` hands back must resolve even before any
    /// background refresh or watcher has indexed the file — otherwise a
    /// visitor who clicks the short link before that catch-up finishes gets
    /// a 404 while the long `/p/...` URL for the same file works fine.
    /// `view_file`'s stub row (`register_known_path`) is what makes this an
    /// O(1) `path_hash` lookup instead of a full-tree scan.
    #[test]
    fn resolve_short_code_resolves_from_the_view_file_stub_without_scanning() {
        let dir = std::env::temp_dir().join(format!("mdview-eng-short-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, "docs/architecture.md", "# Arch");

        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let vf = engine.view_file(&dir, "docs/architecture.md").unwrap();

        // The path is known (stub row) but content isn't read/indexed yet.
        assert_eq!(engine.file_count(&vf.project_id).unwrap(), 1);
        assert!(!engine
            .store
            .is_content_indexed(&vf.project_id, "docs/architecture.md")
            .unwrap());

        // Deleting the file from disk proves resolution comes from the DB
        // stub alone: a scan (which reads the filesystem) would find nothing.
        std::fs::remove_file(dir.join("docs/architecture.md")).unwrap();

        let (project_id, rel_path) = engine
            .resolve_short_code(&vf.code)
            .unwrap()
            .expect("short code should resolve from the stub even though nothing was content-indexed and the file is now gone");
        assert_eq!(project_id, vf.project_id);
        assert_eq!(rel_path, "docs/architecture.md");

        // resolve_short_code only answers "which file" — content-indexing
        // stays content-indexed only once something actually reads it.
        assert!(!engine
            .store
            .is_content_indexed(&vf.project_id, "docs/architecture.md")
            .unwrap());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// `view_page` must not mistake a `view_file` stub (path known,
    /// content unread — title still the filename placeholder) for a real
    /// index and skip indexing: the file's title, FTS content, and links must
    /// all still get filled in on first real view.
    #[test]
    fn view_page_upgrades_a_view_file_stub_to_a_real_index() {
        let dir = std::env::temp_dir().join(format!("mdview-eng-stub-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, "docs/guide.md", "# Real Title\nbody");

        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let vf = engine.view_file(&dir, "docs/guide.md").unwrap();
        let stub = engine
            .store
            .get_file(&vf.project_id, "docs/guide.md")
            .unwrap()
            .unwrap();
        assert_eq!(stub.title, "guide.md"); // placeholder: filename, not the real H1

        assert!(engine
            .view_page(&vf.project_id, "docs/guide.md")
            .unwrap()
            .is_some());
        assert!(engine
            .store
            .is_content_indexed(&vf.project_id, "docs/guide.md")
            .unwrap());

        let indexed = engine
            .store
            .get_file(&vf.project_id, "docs/guide.md")
            .unwrap()
            .unwrap();
        assert_eq!(indexed.title, "Real Title");

        let hits = engine
            .store
            .search(
                "body",
                Some(&vf.project_id),
                None,
                crate::domain::SearchSort::Relevance,
                10,
            )
            .unwrap();
        assert!(
            hits.iter().any(|h| h.rel_path == "docs/guide.md"),
            "expected docs/guide.md in FTS search results: {hits:?}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// A file matched by the project's own `.gitignore` still resolves via its
    /// short code, the same way it is still viewable via the long `/p/...`
    /// URL (`view_page` never consults `.gitignore`). Before this fix the
    /// fallback scan respected `.gitignore` and such a link 404'd forever.
    ///
    /// Registers the project directly (`register`, no stub) rather than going
    /// through `view_file`, so this actually exercises the scan fallback
    /// instead of short-circuiting on `view_file`'s own stub row — the
    /// scenario this test targets is a code whose stub is missing (an old
    /// link, or a registry restored from an older backup).
    #[test]
    fn resolve_short_code_finds_gitignored_file() {
        let dir = std::env::temp_dir().join(format!("mdview-eng-short-gi-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, ".gitignore", "notes/\n");
        write(&dir, "notes/scratch.md", "# Scratch");

        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let (project, _) = engine.register(&dir, None).unwrap();
        let code = crate::short_link::short_code(&crate::short_link::path_hash(
            &project.id,
            "notes/scratch.md",
        ));

        let (project_id, rel_path) = engine
            .resolve_short_code(&code)
            .unwrap()
            .expect("short code should resolve even though the file is gitignored");
        assert_eq!(project_id, project.id);
        assert_eq!(rel_path, "notes/scratch.md");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Same as `resolve_short_code_finds_gitignored_file`, but for a path
    /// excluded via the repo's local `.git/info/exclude` instead of a tracked
    /// `.gitignore` — the actual root cause of the reported 404 (a `.claude`
    /// worktree path is typically excluded this way, precisely so it need not
    /// be committed). `git_exclude` is a separate WalkBuilder toggle from
    /// `git_ignore`; the first version of this fix only disabled the latter.
    #[test]
    fn resolve_short_code_finds_locally_excluded_file() {
        let dir =
            std::env::temp_dir().join(format!("mdview-eng-short-gitexcl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".git/info")).unwrap();
        std::fs::write(dir.join(".git/info/exclude"), "worktrees/\n").unwrap();
        write(&dir, "worktrees/task-1/README.md", "# Task 1");

        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let (project, _) = engine.register(&dir, None).unwrap();
        let code = crate::short_link::short_code(&crate::short_link::path_hash(
            &project.id,
            "worktrees/task-1/README.md",
        ));

        let (project_id, rel_path) = engine
            .resolve_short_code(&code)
            .unwrap()
            .expect("short code should resolve even though the file is locally excluded");
        assert_eq!(project_id, project.id);
        assert_eq!(rel_path, "worktrees/task-1/README.md");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Saving from the editor must land on disk *and* in the index in one
    /// step (title/search must not lag until the watcher catches up), must
    /// refuse to clobber a file someone else changed since the editor loaded
    /// it, and must never write a path the index doesn't list.
    #[test]
    fn save_file_writes_reindexes_and_detects_conflicts() {
        let dir = std::env::temp_dir().join(format!("mdview-eng-save-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, "docs/guide.md", "# Old title\n");

        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let vf = engine.view_file(&dir, "docs/guide.md").unwrap();
        assert!(engine
            .view_page(&vf.project_id, "docs/guide.md")
            .unwrap()
            .is_some());
        let base = indexer::content_hash("# Old title\n");

        let new_hash = engine
            .save_file(
                &vf.project_id,
                "docs/guide.md",
                "# New title\n",
                Some(&base),
            )
            .unwrap();
        assert_eq!(new_hash, indexer::content_hash("# New title\n"));
        assert_eq!(
            std::fs::read_to_string(dir.join("docs/guide.md")).unwrap(),
            "# New title\n"
        );
        let file = engine
            .store
            .get_file(&vf.project_id, "docs/guide.md")
            .unwrap()
            .unwrap();
        assert_eq!(file.title, "New title");
        assert!(!dir.join("docs/guide.mdview-save.tmp").exists());

        // Stale base hash → conflict, disk untouched.
        let err = engine
            .save_file(&vf.project_id, "docs/guide.md", "# Clobber\n", Some(&base))
            .unwrap_err();
        assert!(matches!(err, Error::Conflict(_)), "got {err:?}");
        assert_eq!(
            std::fs::read_to_string(dir.join("docs/guide.md")).unwrap(),
            "# New title\n"
        );

        // No base hash → force overwrite.
        engine
            .save_file(&vf.project_id, "docs/guide.md", "# Forced\n", None)
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("docs/guide.md")).unwrap(),
            "# Forced\n"
        );

        // Unindexed path → refused, nothing created.
        let err = engine
            .save_file(&vf.project_id, "docs/other.md", "x", None)
            .unwrap_err();
        assert!(matches!(err, Error::FileNotFound(_)), "got {err:?}");
        assert!(!dir.join("docs/other.md").exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Viewing a page (or a file's raw source via the Code section) must
    /// reset the project's idle clock — otherwise the cleanup sweep would drop
    /// a project while someone is actively reading it.
    #[test]
    fn view_page_and_code_path_touch_the_project_last_seen() {
        let dir = std::env::temp_dir().join(format!("mdview-access-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, "docs/a.md", "# A");

        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let vf = engine.view_file(&dir, "docs/a.md").unwrap();
        assert!(engine
            .view_page(&vf.project_id, "docs/a.md")
            .unwrap()
            .is_some());

        let stale = "2000-01-01T00:00:00Z";
        let last_seen = |e: &Engine| e.get_project(&vf.project_id).unwrap().unwrap().last_seen_at;

        engine
            .store
            .backdate_project_for_test(&vf.project_id, stale);
        engine.view_page(&vf.project_id, "docs/a.md").unwrap();
        assert_ne!(
            last_seen(&engine),
            stale,
            "view_page must bump last_seen_at"
        );

        engine
            .store
            .backdate_project_for_test(&vf.project_id, stale);
        engine.code_path(&vf.project_id, "docs/a.md").unwrap();
        assert_ne!(
            last_seen(&engine),
            stale,
            "code_path must bump last_seen_at"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn view_file_refuses_non_markdown_and_never_stubs_symlinked_escapes() {
        let dir = std::env::temp_dir().join(format!("mdview-eng-nonmd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, "Cargo.toml", "[package]");
        write(&dir, "ok.md", "# ok");

        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let err = engine.view_file(&dir, "Cargo.toml").unwrap_err();
        assert!(matches!(err, Error::InvalidPath(_)), "got {err:?}");

        #[cfg(unix)]
        {
            let outside =
                std::env::temp_dir().join(format!("mdview-eng-outside-{}.md", std::process::id()));
            std::fs::write(&outside, "# outside").unwrap();
            std::os::unix::fs::symlink(&outside, dir.join("link.md")).unwrap();
            let vf = engine.view_file(&dir, "link.md").unwrap();
            assert!(engine
                .store
                .get_file(&vf.project_id, "link.md")
                .unwrap()
                .is_none());
            std::fs::remove_file(&outside).ok();
        }

        let vf = engine.view_file(&dir, "ok.md").unwrap();
        assert!(engine
            .store
            .get_file(&vf.project_id, "ok.md")
            .unwrap()
            .is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn asset_path_enforces_allowlist_exclude_patterns_and_traversal_guard() {
        let dir = std::env::temp_dir().join(format!("mdview-asset-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        write(&dir, "readme.md", "# root");
        write(&dir, "images/logo.png", "fake-png-bytes");
        write(&dir, "images/secret.env", "SECRET=1");
        write(&dir, "images/LOGO.PNG", "fake-png-bytes-upper");
        write(&dir, "node_modules/pkg/logo.png", "vendored-png-bytes");

        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let (project, _) = engine.register(&dir, None).unwrap();

        // allowed extension → Ok
        assert!(engine.asset_path(&project.id, "images/logo.png").is_ok());

        // uppercase extension → Ok (case-insensitive)
        assert!(engine.asset_path(&project.id, "images/LOGO.PNG").is_ok());

        // disallowed extension → Err
        assert!(engine.asset_path(&project.id, "images/secret.env").is_err());

        // allowed extension but inside an excluded directory → Err
        assert!(engine
            .asset_path(&project.id, "node_modules/pkg/logo.png")
            .is_err());

        // traversal escape → Err, unchanged
        assert!(engine
            .asset_path(&project.id, "../../../../../../../etc/passwd")
            .is_err());

        #[cfg(unix)]
        {
            // A symlink named with an allowed extension but pointing at a
            // disallowed-extension target must still be rejected: the
            // extension check runs on the canonicalized (resolved) path,
            // not the pre-resolution symlink name.
            let target = dir.join("images/secret.env");
            let link = dir.join("images/bypass.png");
            std::os::unix::fs::symlink(&target, &link).unwrap();
            assert!(engine.asset_path(&project.id, "images/bypass.png").is_err());

            // The highest-value vector: a symlink with an *allowed* extension
            // pointing at a readable file *outside* the project root. Its
            // extension passes, so only the containment guard (starts_with on
            // the canonical path) rejects it — lock that in.
            let outside =
                std::env::temp_dir().join(format!("mdview-outside-{}.png", std::process::id()));
            std::fs::write(&outside, "out-of-root-bytes").unwrap();
            let esc_link = dir.join("images/escape.png");
            std::os::unix::fs::symlink(&outside, &esc_link).unwrap();
            assert!(engine.asset_path(&project.id, "images/escape.png").is_err());
            std::fs::remove_file(&outside).ok();
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    fn view_engine(tag: &str) -> (Engine, Project, PathBuf) {
        let dir = std::env::temp_dir().join(format!("mdview-eng-vp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let (project, _) = engine.ensure_project(&dir, None).unwrap();
        (engine, project, dir)
    }

    fn hit_paths(engine: &Engine, project: &Project, query: &str) -> Vec<String> {
        engine
            .store
            .search(
                query,
                Some(&project.id),
                None,
                crate::domain::SearchSort::Relevance,
                20,
            )
            .unwrap()
            .into_iter()
            .map(|h| h.rel_path)
            .collect()
    }

    fn neighbour_rels(viewed: &ViewedPage, dir: &Path) -> Vec<String> {
        let root = std::fs::canonicalize(dir).unwrap();
        let mut rels: Vec<String> = viewed
            .neighbours
            .iter()
            .map(|p| indexer::rel_path_str(&root, p))
            .collect();
        rels.sort();
        rels
    }

    #[test]
    fn view_page_indexes_the_file_and_reports_links_and_siblings_as_neighbours() {
        let (engine, project, dir) = view_engine("fresh");
        write(&dir, "docs/a.md", "# A\nuniquealpha [b](../ref/b.md)");
        write(&dir, "docs/sib.md", "# Sibling");
        write(&dir, "ref/b.md", "# B");
        write(&dir, "docs/notes.txt", "not markdown");

        let viewed = engine.view_page(&project.id, "docs/a.md").unwrap().unwrap();
        assert_eq!(viewed.file.rel_path, "docs/a.md");
        assert_eq!(viewed.file.title, "A");
        assert_eq!(viewed.page.links, vec!["ref/b.md"]);
        assert_eq!(hit_paths(&engine, &project, "uniquealpha"), ["docs/a.md"]);
        assert_eq!(
            neighbour_rels(&viewed, &dir),
            ["docs/sib.md", "ref/b.md"],
            "link target and markdown sibling, not the viewed file or non-markdown"
        );

        assert_eq!(
            engine
                .index_neighbours(&project.id, &viewed.neighbours)
                .unwrap(),
            2
        );
        let b = engine.store.file_state(&project.id, "ref/b.md").unwrap();
        assert!(b.unwrap().content_indexed());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unchanged_second_view_has_no_neighbours_and_keeps_its_fts_row() {
        let (engine, project, dir) = view_engine("again");
        write(&dir, "a.md", "# A\n[b](b.md)");
        write(&dir, "b.md", "# B");

        let first = engine.view_page(&project.id, "a.md").unwrap().unwrap();
        engine
            .index_neighbours(&project.id, &first.neighbours)
            .unwrap();
        let before = engine
            .store
            .file_state(&project.id, "a.md")
            .unwrap()
            .unwrap();

        let second = engine.view_page(&project.id, "a.md").unwrap().unwrap();
        assert!(second.neighbours.is_empty(), "{:?}", second.neighbours);
        let after = engine
            .store
            .file_state(&project.id, "a.md")
            .unwrap()
            .unwrap();
        assert_eq!(before.fts_rowid, after.fts_rowid);
        assert_eq!(before.content_hash, after.content_hash);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn editing_a_file_then_viewing_reindexes_it() {
        let (engine, project, dir) = view_engine("edit");
        write(&dir, "a.md", "# A\nfirstword");
        engine.view_page(&project.id, "a.md").unwrap().unwrap();
        assert_eq!(hit_paths(&engine, &project, "firstword"), ["a.md"]);

        write(&dir, "a.md", "# A\nsecondword");
        engine.view_page(&project.id, "a.md").unwrap().unwrap();
        assert_eq!(hit_paths(&engine, &project, "secondword"), ["a.md"]);
        assert!(hit_paths(&engine, &project, "firstword").is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_link_to_an_unindexed_markdown_file_is_not_marked_broken() {
        let (engine, project, dir) = view_engine("unbroken");
        write(&dir, "a.md", "[b](b.md)");
        write(&dir, "b.md", "# B");

        let viewed = engine.view_page(&project.id, "a.md").unwrap().unwrap();
        assert!(
            !viewed.page.html.contains("broken-link"),
            "{}",
            viewed.page.html
        );
        assert!(viewed
            .page
            .html
            .contains(&format!("/p/{}/b.md", project.id)));
        assert!(engine
            .store
            .file_state(&project.id, "b.md")
            .unwrap()
            .is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn non_markdown_paths_are_never_viewed_indexed_or_offered_as_neighbours() {
        let (engine, project, dir) = view_engine("nonmd");
        write(&dir, ".env", "SECRET=1");
        write(&dir, ".git/config", "[core]");
        write(&dir, "Cargo.toml", "[package]");
        write(
            &dir,
            "a.md",
            "[e](.env) [c](Cargo.toml) [g](.git/config) [x](node_modules/x.md)",
        );
        write(&dir, "node_modules/x.md", "# vendored");

        for rel in [".env", ".git/config", "Cargo.toml", "missing.md"] {
            assert!(
                engine.view_page(&project.id, rel).unwrap().is_none(),
                "{rel}"
            );
            assert!(engine.store.file_state(&project.id, rel).unwrap().is_none());
        }
        assert!(engine
            .view_page(&project.id, "node_modules/x.md")
            .unwrap()
            .is_none());

        let viewed = engine.view_page(&project.id, "a.md").unwrap().unwrap();
        assert!(viewed.page.links.is_empty(), "{:?}", viewed.page.links);
        assert!(viewed.neighbours.is_empty(), "{:?}", viewed.neighbours);
        assert_eq!(engine.file_count(&project.id).unwrap(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_to_a_file_outside_the_root_is_not_a_page_or_neighbour() {
        let (engine, project, dir) = view_engine("symlink");
        let outside =
            std::env::temp_dir().join(format!("mdview-eng-vp-out-{}.md", std::process::id()));
        std::fs::write(&outside, "# outside").unwrap();
        write(&dir, "a.md", "# A");
        std::os::unix::fs::symlink(&outside, dir.join("link.md")).unwrap();

        assert!(engine.view_page(&project.id, "link.md").unwrap().is_none());
        let viewed = engine.view_page(&project.id, "a.md").unwrap().unwrap();
        assert!(viewed.neighbours.is_empty(), "{:?}", viewed.neighbours);
        std::fs::remove_file(&outside).ok();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn neighbours_are_capped_and_a_concurrent_call_for_the_project_is_a_no_op() {
        let (engine, project, dir) = view_engine("cap");
        write(&dir, "a.md", "# A");
        for i in 0..250 {
            write(&dir, &format!("s{i:03}.md"), "# S");
        }

        let viewed = engine.view_page(&project.id, "a.md").unwrap().unwrap();
        assert_eq!(viewed.neighbours.len(), NEIGHBOUR_CAP);

        engine
            .neighbours_running
            .lock()
            .unwrap()
            .insert(project.id.clone());
        assert_eq!(
            engine
                .index_neighbours(&project.id, &viewed.neighbours)
                .unwrap(),
            0
        );
        engine
            .neighbours_running
            .lock()
            .unwrap()
            .remove(&project.id);

        assert_eq!(
            engine
                .index_neighbours(&project.id, &viewed.neighbours)
                .unwrap(),
            NEIGHBOUR_CAP
        );
        assert!(
            engine.neighbours_running.lock().unwrap().is_empty(),
            "the in-flight mark must be cleared afterwards"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
