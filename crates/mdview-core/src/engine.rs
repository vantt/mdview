//! Application core: the facade the HTTP/MCP/CLI adapters call. Owns the store,
//! config, and renderer, and implements the high-level use cases (view_file,
//! render, search, registry) — including implicit project auto-create (FR-04).

use crate::code_source::{self, DirListing, SourceContent};
use crate::config::Config;
use crate::domain::{IndexedFile, Project, RenderedPage, SearchResult};
use crate::error::{Error, Result};
use crate::fuzzy::{self, FuzzyHit};
use crate::indexer::{self, IndexService};
use crate::render::{self, HighlightedSource, RenderService};
use crate::repository::SqliteStore;
use std::path::{Path, PathBuf};

pub struct Engine {
    pub store: SqliteStore,
    pub config: Config,
    render: RenderService,
}

#[derive(Debug, Clone)]
pub struct ViewFile {
    pub url: String,
    pub project_id: String,
    pub rel_path: String,
    /// Short code for this file — the `<code>` in `/s/<code>`.
    pub code: String,
    /// Whether this call just created the project (as opposed to reusing an
    /// existing one). Callers use this to decide whether a background index
    /// needs kicking off — see `view_file`'s doc comment.
    pub is_new_project: bool,
}

impl Engine {
    pub fn new(store: SqliteStore, config: Config) -> Self {
        Self {
            store,
            config,
            render: RenderService::new(),
        }
    }

    fn max_bytes(&self) -> u64 {
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
    /// second return value is `true`. Indexing is deliberately the caller's
    /// job: `view_file`/`register` return fast so an MCP/CLI response never
    /// blocks on a full recursive scan, while the actual file content gets
    /// indexed on demand (`ensure_indexed`, called synchronously from the
    /// HTTP path when a visitor really opens a page) or via a background
    /// `refresh` the app layer kicks off for a `true` return here.
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
    /// hand back its app URL. Deliberately does *not* index anything —
    /// indexing here would make every first-view of a large project block
    /// the MCP/CLI response on a full scan. The URL is computable from the
    /// project id + rel path alone; the actual content gets indexed either
    /// by the caller's background refresh (`is_new_project`) or, at the
    /// latest, synchronously when a browser really requests the page
    /// (`ensure_indexed` in the HTTP handler).
    pub fn view_file(&self, project_root: &Path, rel_path: &str) -> Result<ViewFile> {
        let (project, is_new_project) = self.ensure_project(project_root, None)?;
        let abs = project.root_path.join(rel_path);
        let abs = crate::link_resolver::normalize(&abs);
        let rel = indexer::rel_path_str(&project.root_path, &abs);
        if rel.is_empty() {
            return Err(Error::PathOutsideProject(abs));
        }
        let code = crate::short_link::short_code(&crate::short_link::path_hash(&project.id, &rel));
        Ok(ViewFile {
            url: format!("/p/{}/{}", project.id, rel),
            project_id: project.id,
            rel_path: rel,
            code,
            is_new_project,
        })
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

    /// Full re-scan of a project to reconcile drift (FR-09b).
    pub fn refresh(&self, project_id: &str) -> Result<usize> {
        let project = self
            .store
            .get_project(project_id)?
            .ok_or_else(|| Error::ProjectNotFound(project_id.to_string()))?;
        let n = IndexService::index_project(
            &self.store,
            &project,
            &self.config.indexing.exclude_patterns,
            self.max_bytes(),
        )?;
        self.reindex_links(&project)?;
        Ok(n)
    }

    /// Index a single file and (re)compute its outgoing links. Used by view_file
    /// and the filesystem watcher. Returns whether the file's *content* actually
    /// changed (see `IndexService::index_file`) — the watcher uses this to skip
    /// a live-reload broadcast for a touch that left bytes identical.
    pub fn index_file_incremental(&self, project: &Project, abs: &Path) -> Result<bool> {
        let changed = IndexService::index_file(&self.store, project, abs, self.max_bytes())?
            .map(|(_, changed)| changed)
            .unwrap_or(false);
        self.compute_file_links(project, abs)?;
        Ok(changed)
    }

    /// Drop a file from the index (and its outgoing links).
    pub fn remove_file(&self, project: &Project, abs: &Path) -> Result<()> {
        IndexService::remove_file(&self.store, project, abs)
    }

    /// If `rel_path` isn't in the index yet but exists on disk under the
    /// project root, index it now. Closes the race between the filesystem
    /// watcher noticing a new/renamed file and a request for its URL arriving
    /// first — the watcher's debounce, or a file written by something it
    /// isn't watching, would otherwise 404 a file that is really there.
    /// Returns whether the file is indexed after the call (already indexed,
    /// or newly indexed).
    pub fn ensure_indexed(&self, project: &Project, rel_path: &str) -> Result<bool> {
        if self.store.get_file(&project.id, rel_path)?.is_some() {
            return Ok(true);
        }
        let abs = crate::link_resolver::normalize(&project.root_path.join(rel_path));
        if indexer::rel_path_str(&project.root_path, &abs).is_empty() {
            return Ok(false);
        }
        self.index_file_incremental(project, &abs)?;
        Ok(self.store.get_file(&project.id, rel_path)?.is_some())
    }

    /// Resolve `/s/<code>` to `(project_id, rel_path)`, indexing the file on
    /// demand if needed.
    ///
    /// `view_file` hands out a code derived from the path hash without
    /// indexing anything, on the assumption a background refresh or the
    /// watcher will have indexed the file by the time anyone clicks the
    /// link. When that hasn't happened yet, the fast hash lookup misses —
    /// unlike `ensure_indexed`, there's no `rel_path` to index directly, so
    /// this falls back to a filename-only scan (no content reads) of every
    /// registered project, hashing each candidate to find the one the code
    /// belongs to, then indexes just that file.
    pub fn resolve_short_code(&self, code: &str) -> Result<Option<(String, String)>> {
        if let Some(hit) = self.store.find_by_hash_prefix(code)? {
            return Ok(Some(hit));
        }
        for project in self.store.list_projects()? {
            for abs in indexer::scan_markdown_files(
                &project.root_path,
                &self.config.indexing.exclude_patterns,
            ) {
                let rel = indexer::rel_path_str(&project.root_path, &abs);
                if rel.is_empty() {
                    continue;
                }
                let hash = crate::short_link::path_hash(&project.id, &rel);
                if hash.starts_with(code) {
                    self.index_file_incremental(&project, &abs)?;
                    return Ok(Some((project.id, rel)));
                }
            }
        }
        Ok(None)
    }

    /// Resolve and store the internal links a single file points to.
    fn compute_file_links(&self, project: &Project, abs: &Path) -> Result<()> {
        let rel = indexer::rel_path_str(&project.root_path, abs);
        if rel.is_empty() {
            return Ok(());
        }
        let content = std::fs::read_to_string(abs).unwrap_or_default();
        let index = self.store.file_abs_paths(&project.id)?;
        let targets = render::extract_internal_links(&content, abs, &project.root_path, &index);
        self.store.set_file_links(&project.id, &rel, &targets)
    }

    /// Recompute links for every file in a project (after a full scan).
    fn reindex_links(&self, project: &Project) -> Result<()> {
        let files = self.store.list_files(&project.id)?;
        let index = self.store.file_abs_paths(&project.id)?;
        for f in files {
            let content = std::fs::read_to_string(&f.abs_path).unwrap_or_default();
            let targets =
                render::extract_internal_links(&content, &f.abs_path, &project.root_path, &index);
            self.store
                .set_file_links(&project.id, &f.rel_path, &targets)?;
        }
        Ok(())
    }

    /// Files that link to `rel_path` → (source_rel, title). FR-18 backlinks.
    pub fn backlinks(&self, project_id: &str, rel_path: &str) -> Result<Vec<(String, String)>> {
        self.store.backlinks(project_id, rel_path)
    }

    /// Render a file for the viewer, rewriting internal links against the index.
    pub fn render_file(&self, project_id: &str, rel_path: &str) -> Result<RenderedPage> {
        let project = self
            .store
            .get_project(project_id)?
            .ok_or_else(|| Error::ProjectNotFound(project_id.to_string()))?;
        let file = self
            .store
            .get_file(project_id, rel_path)?
            .ok_or_else(|| Error::FileNotFound(rel_path.to_string()))?;
        let content = std::fs::read_to_string(&file.abs_path)?;
        let index = self.store.file_abs_paths(project_id)?;
        let page = self.render.render(
            &content,
            &file.abs_path,
            project_id,
            &project.root_path,
            &index,
        );
        self.record_access(project_id, rel_path);
        Ok(page)
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

    /// Record that `rel_path` in `project_id` was actually viewed — the
    /// signal the periodic cleanup sweep checks (see
    /// `repository::cleanup_stale`, called from the daemon). Best-effort:
    /// bookkeeping must never fail the view itself.
    fn record_access(&self, project_id: &str, rel_path: &str) {
        let _ = self.store.touch_file_access(project_id, rel_path);
        let _ = self.store.touch_project_access(project_id);
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        self.store.list_projects()
    }

    pub fn get_project(&self, id: &str) -> Result<Option<Project>> {
        self.store.get_project(id)
    }

    pub fn list_files(&self, project_id: &str) -> Result<Vec<IndexedFile>> {
        self.store.list_files(project_id)
    }

    pub fn file_count(&self, project_id: &str) -> Result<usize> {
        self.store.file_count(project_id)
    }

    pub fn search(
        &self,
        query: &str,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        self.store.search(query, project_id, limit)
    }

    /// Fuzzy file-jump: rank a project's files by a fuzzy match of `query`
    /// against their relative paths (name/path jump, complementing the
    /// content-based `search`). Ordered by descending match score.
    pub fn fuzzy_files(
        &self,
        project_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FuzzyHit>> {
        let files = self.store.list_files(project_id)?;
        Ok(fuzzy::rank_files(&files, project_id, query, limit))
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
        if is_excluded_path(&rel, &self.config.indexing.exclude_patterns) {
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
                // A no-op for non-markdown source (not a `files` row); for a
                // markdown file viewed via the raw Code section, this counts
                // the same as viewing its rendered page.
                self.record_access(project_id, rel_path);
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

/// True if any path component (by exact name equality) matches an exclude
/// pattern, mirroring `indexer::scan_markdown_files`'s filter semantics.
fn is_excluded_path(rel: &str, exclude_patterns: &[String]) -> bool {
    Path::new(rel)
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .any(|name| exclude_patterns.iter().any(|ex| ex == name))
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

        // view_file deliberately doesn't scan (that would block the caller on
        // a full recursive index) — nothing is indexed yet.
        assert_eq!(engine.file_count(&vf.project_id).unwrap(), 0);

        // Stand in for the background refresh a real caller kicks off on
        // `is_new_project`, or the on-demand `ensure_indexed` a browser visit
        // triggers.
        engine.refresh(&vf.project_id).unwrap();
        assert_eq!(engine.file_count(&vf.project_id).unwrap(), 2);

        // rendering rewrites the cross-folder link
        let page = engine
            .render_file(&vf.project_id, "docs/architecture.md")
            .unwrap();
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
    #[test]
    fn resolve_short_code_indexes_on_demand() {
        let dir = std::env::temp_dir().join(format!("mdview-eng-short-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, "docs/architecture.md", "# Arch");

        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let vf = engine.view_file(&dir, "docs/architecture.md").unwrap();

        // Nothing indexed yet, same as view_file_auto_creates_project_and_returns_url.
        assert_eq!(engine.file_count(&vf.project_id).unwrap(), 0);

        let (project_id, rel_path) = engine
            .resolve_short_code(&vf.code)
            .unwrap()
            .expect("short code should resolve even though nothing was indexed yet");
        assert_eq!(project_id, vf.project_id);
        assert_eq!(rel_path, "docs/architecture.md");

        // The lookup indexed the file as a side effect, same as ensure_indexed.
        assert_eq!(engine.file_count(&vf.project_id).unwrap(), 1);

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
        let project = engine.get_project(&vf.project_id).unwrap().unwrap();
        assert!(engine.ensure_indexed(&project, "docs/guide.md").unwrap());
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

    /// Viewing a file's rendered page must reset its "unaccessed" clock —
    /// otherwise the cleanup sweep would drop a file's index record while
    /// someone is actively reading it.
    #[test]
    fn render_file_touches_last_accessed_and_project_last_seen() {
        let dir = std::env::temp_dir().join(format!("mdview-access-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, "docs/a.md", "# A");

        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let vf = engine.view_file(&dir, "docs/a.md").unwrap();
        engine.refresh(&vf.project_id).unwrap();

        // Simulate a file/project that hasn't been viewed in a while.
        engine.store.backdate_file_access_for_test(
            &vf.project_id,
            "docs/a.md",
            "2000-01-01T00:00:00Z",
        );
        engine
            .store
            .backdate_project_for_test(&vf.project_id, "2000-01-01T00:00:00Z");

        engine.render_file(&vf.project_id, "docs/a.md").unwrap();

        assert_ne!(
            engine
                .store
                .file_last_accessed(&vf.project_id, "docs/a.md")
                .unwrap(),
            "2000-01-01T00:00:00Z",
            "render_file must bump last_accessed_at"
        );
        assert_ne!(
            engine
                .get_project(&vf.project_id)
                .unwrap()
                .unwrap()
                .last_seen_at,
            "2000-01-01T00:00:00Z",
            "render_file must bump the project's last_seen_at too"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Viewing a markdown file's raw source via the Code section counts as
    /// an access too, same as its rendered page.
    #[test]
    fn code_path_touches_last_accessed_for_an_indexed_markdown_file() {
        let dir = std::env::temp_dir().join(format!("mdview-code-access-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, "docs/a.md", "# A");

        let engine = Engine::new(SqliteStore::open_in_memory().unwrap(), Config::default());
        let vf = engine.view_file(&dir, "docs/a.md").unwrap();
        engine.refresh(&vf.project_id).unwrap();
        engine.store.backdate_file_access_for_test(
            &vf.project_id,
            "docs/a.md",
            "2000-01-01T00:00:00Z",
        );

        engine.code_path(&vf.project_id, "docs/a.md").unwrap();

        assert_ne!(
            engine
                .store
                .file_last_accessed(&vf.project_id, "docs/a.md")
                .unwrap(),
            "2000-01-01T00:00:00Z"
        );

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
}
