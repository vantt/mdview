//! Recursive scan (WalkBuilder, respects .gitignore) + indexing service.
//! Incremental single-file updates keep steady-state cost low (PRD FR-09b);
//! a full re-scan reconciles drift.

use crate::domain::{IndexedFile, Project};
use crate::error::Result;
use crate::repository::SqliteStore;
use ignore::WalkBuilder;
use std::path::{Component, Path, PathBuf};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const MARKDOWN_EXTS: &[&str] = &["md", "markdown"];

pub struct IndexService;

impl IndexService {
    /// Full recursive scan → (re)index every markdown file under the project root.
    /// Returns the number of files indexed.
    pub fn index_project(
        store: &SqliteStore,
        project: &Project,
        exclude: &[String],
        max_bytes: u64,
    ) -> Result<usize> {
        let files = scan_markdown_files(&project.root_path, exclude);
        let mut n = 0;
        for abs in files {
            if Self::index_file(store, project, &abs, max_bytes)?.is_some() {
                n += 1;
            }
        }
        Ok(n)
    }

    /// Index (or refresh) a single file. Returns None if skipped (too big / unreadable).
    pub fn index_file(
        store: &SqliteStore,
        project: &Project,
        abs: &Path,
        max_bytes: u64,
    ) -> Result<Option<IndexedFile>> {
        let meta = match std::fs::metadata(abs) {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };
        if meta.len() > max_bytes {
            return Ok(None);
        }
        let content = match std::fs::read_to_string(abs) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };
        let rel = rel_path_str(&project.root_path, abs);
        if rel.is_empty() {
            return Ok(None);
        }
        let title = extract_title(&content).unwrap_or_else(|| filename(abs));
        let modified_at = meta
            .modified()
            .ok()
            .and_then(|t| OffsetDateTime::from(t).format(&Rfc3339).ok())
            .unwrap_or_default();
        let f = IndexedFile {
            project_id: project.id.clone(),
            abs_path: abs.to_path_buf(),
            rel_path: rel,
            title,
            size_bytes: meta.len(),
            modified_at,
        };
        store.upsert_file(&f, &content)?;
        Ok(Some(f))
    }

    /// Remove a file from the index by absolute path.
    pub fn remove_file(store: &SqliteStore, project: &Project, abs: &Path) -> Result<()> {
        let rel = rel_path_str(&project.root_path, abs);
        if !rel.is_empty() {
            store.delete_file(&project.id, &rel)?;
        }
        Ok(())
    }
}

/// Walk `root` recursively, returning absolute paths of markdown files.
/// Respects .gitignore (via WalkBuilder) and prunes `exclude` directory names.
pub fn scan_markdown_files(root: &Path, exclude: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let exclude: Vec<String> = exclude.to_vec();
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .parents(false)
        .filter_entry(move |e| {
            let name = e.file_name().to_string_lossy();
            !exclude.iter().any(|ex| name.as_ref() == ex.as_str())
        })
        .build();
    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_file() && is_markdown(path) {
            out.push(path.to_path_buf());
        }
    }
    out
}

fn is_markdown(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| MARKDOWN_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Path relative to root, joined with `/` for URL use.
pub fn rel_path_str(root: &Path, abs: &Path) -> String {
    match abs.strip_prefix(root) {
        Ok(rel) => rel
            .components()
            .filter_map(|c| match c {
                Component::Normal(s) => s.to_str(),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/"),
        Err(_) => String::new(),
    }
}

fn filename(p: &Path) -> String {
    p.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
        .to_string()
}

/// First `# H1` in the document, if any.
pub fn extract_title(content: &str) -> Option<String> {
    for line in content.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("# ") {
            let title = rest.trim();
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

/// Derive a URL-safe project id from a root path's directory name.
pub fn slug_from_root(root: &Path) -> String {
    let base = root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project");
    let mut out = String::new();
    for ch in base.chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
        } else if ch == ' ' || ch == '-' || ch == '_' || ch == '.' {
            out.push('-');
        }
    }
    let s = out.trim_matches('-').to_string();
    if s.is_empty() {
        "project".into()
    } else {
        s
    }
}

/// Now as an RFC3339 UTC string.
pub fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Project;

    fn write(dir: &Path, rel: &str, body: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    #[test]
    fn scans_recursively_and_indexes_with_titles() {
        let dir = std::env::temp_dir().join(format!("mdview-idx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, "README.md", "# Root Readme\nhello");
        write(&dir, "docs/guide.md", "# Guide\ncontent");
        write(&dir, "docs/nested/deep.md", "no heading here");
        write(&dir, "notes.txt", "not markdown");
        write(&dir, "node_modules/pkg/x.md", "# Should be excluded");

        let store = SqliteStore::open_in_memory().unwrap();
        let project = Project {
            id: slug_from_root(&dir),
            name: "T".into(),
            root_path: dir.clone(),
            created_at: now_rfc3339(),
            last_seen_at: now_rfc3339(),
        };
        let n = IndexService::index_project(&store, &project, &["node_modules".into()], 10_000_000)
            .unwrap();
        assert_eq!(
            n, 3,
            "should index 3 md files (excluding node_modules + txt)"
        );

        let deep = store
            .get_file(&project.id, "docs/nested/deep.md")
            .unwrap()
            .unwrap();
        assert_eq!(deep.title, "deep.md"); // fallback to filename
        let guide = store
            .get_file(&project.id, "docs/guide.md")
            .unwrap()
            .unwrap();
        assert_eq!(guide.title, "Guide");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn slug_generation() {
        assert_eq!(slug_from_root(Path::new("/home/x/My App")), "my-app");
        assert_eq!(slug_from_root(Path::new("/home/x/proj.v2")), "proj-v2");
    }
}
