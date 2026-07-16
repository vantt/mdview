//! Application core: the facade the HTTP/MCP/CLI adapters call. Owns the store,
//! config, and renderer, and implements the high-level use cases (view_file,
//! render, search, registry) — including implicit project auto-create (FR-04).

use crate::config::Config;
use crate::domain::{IndexedFile, Project, RenderedPage, SearchResult};
use crate::error::{Error, Result};
use crate::fuzzy::{self, FuzzyHit};
use crate::indexer::{self, IndexService};
use crate::render::{self, RenderService};
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

    /// Find the project owning `root`, or create + index it (implicit registration).
    pub fn ensure_project(&self, root: &Path, name: Option<&str>) -> Result<Project> {
        let root = Self::canonical(root);
        if let Some(mut p) = self.store.find_project_by_root(&root)? {
            p.last_seen_at = indexer::now_rfc3339();
            self.store.upsert_project(&p)?;
            return Ok(p);
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
        IndexService::index_project(
            &self.store,
            &project,
            &self.config.indexing.exclude_patterns,
            self.max_bytes(),
        )?;
        self.reindex_links(&project)?;
        Ok(project)
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

    /// The core `mdview_view_file` use case: ensure project, index the file now,
    /// return its app URL.
    pub fn view_file(&self, project_root: &Path, rel_path: &str) -> Result<ViewFile> {
        let project = self.ensure_project(project_root, None)?;
        let abs = project.root_path.join(rel_path);
        let abs = crate::link_resolver::normalize(&abs);
        self.index_file_incremental(&project, &abs)?;
        let rel = indexer::rel_path_str(&project.root_path, &abs);
        if rel.is_empty() {
            return Err(Error::PathOutsideProject(abs));
        }
        Ok(ViewFile {
            url: format!("/p/{}/{}", project.id, rel),
            project_id: project.id,
            rel_path: rel,
        })
    }

    /// Register a project explicitly (CLI). Same as ensure_project + optional name.
    pub fn register(&self, root: &Path, name: Option<&str>) -> Result<Project> {
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
    /// and the filesystem watcher.
    pub fn index_file_incremental(&self, project: &Project, abs: &Path) -> Result<()> {
        IndexService::index_file(&self.store, project, abs, self.max_bytes())?;
        self.compute_file_links(project, abs)
    }

    /// Drop a file from the index (and its outgoing links).
    pub fn remove_file(&self, project: &Project, abs: &Path) -> Result<()> {
        IndexService::remove_file(&self.store, project, abs)
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
        Ok(self.render.render(
            &content,
            &file.abs_path,
            project_id,
            &project.root_path,
            &index,
        ))
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

        // project auto-created + fully scanned (both files indexed)
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
        let project = engine.register(&dir, None).unwrap();

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
