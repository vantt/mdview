//! Recursive scan (WalkBuilder, respects .gitignore) + indexing service.
//! Files are read once into an [`IndexedDoc`]; the store writes them in
//! batches (`SqliteStore::index_docs`) and project sync reconciles drift.

use crate::domain::{IndexedFile, Project};
use crate::error::Result;
use crate::link_resolver::ProjectFs;
use crate::render;
use crate::repository::SqliteStore;
use ignore::WalkBuilder;
use std::path::{Component, Path, PathBuf};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const MARKDOWN_EXTS: &[&str] = &["md", "markdown"];

/// A markdown file read once: its index row, its text and the internal links
/// it points at (project-relative), ready for `SqliteStore::index_docs`.
pub struct IndexedDoc {
    pub file: IndexedFile,
    pub content: String,
    pub links: Vec<String>,
}

pub struct IndexService;

impl IndexService {
    /// Read a markdown file for indexing. `None` when `confine` rejects it
    /// (not markdown, outside the canonical root, excluded), or it is too big
    /// or unreadable.
    pub fn read_file(
        project: &Project,
        abs: &Path,
        max_bytes: u64,
        exclude: &[String],
    ) -> Option<(IndexedFile, String)> {
        let canonical = confine(&project.root_path, abs, exclude)?;
        // Prefer the path the caller used so the row matches the URL it asked
        // for; fall back to the resolved one when the caller's path is not
        // lexically under the root (e.g. the root itself is a symlink).
        let mut rel = rel_path_str(&project.root_path, abs);
        let mut file_path = abs.to_path_buf();
        if rel.is_empty() {
            let canonical_root = std::fs::canonicalize(&project.root_path).ok()?;
            rel = rel_path_str(&canonical_root, &canonical);
            file_path = canonical;
        }
        if rel.is_empty() || is_excluded(&rel, exclude) {
            return None;
        }
        let meta = std::fs::metadata(&file_path).ok()?;
        if meta.len() > max_bytes {
            return None;
        }
        let content = std::fs::read_to_string(&file_path).ok()?;
        let title = extract_title(&content).unwrap_or_else(|| filename(&file_path));
        let modified_at = modified_rfc3339(&meta);
        let file = IndexedFile {
            project_id: project.id.clone(),
            abs_path: file_path,
            rel_path: rel,
            title,
            size_bytes: meta.len(),
            modified_at,
        };
        Some((file, content))
    }

    /// [`Self::read_file`] plus the file's internal links, resolved against the
    /// filesystem so a link to a not-yet-indexed file still counts.
    pub fn build_doc(
        project: &Project,
        abs: &Path,
        max_bytes: u64,
        exclude: &[String],
    ) -> Option<IndexedDoc> {
        let (file, content) = Self::read_file(project, abs, max_bytes, exclude)?;
        let fs = ProjectFs {
            root: &project.root_path,
            exclude,
        };
        let links =
            render::extract_internal_links(&content, &file.abs_path, &project.root_path, &fs);
        Some(IndexedDoc {
            file,
            content,
            links,
        })
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

/// A file's modified time as RFC3339, the exact string stored on its row —
/// sync compares it against the stored value to skip unchanged files.
pub(crate) fn modified_rfc3339(meta: &std::fs::Metadata) -> String {
    meta.modified()
        .ok()
        .and_then(|t| OffsetDateTime::from(t).format(&Rfc3339).ok())
        .unwrap_or_default()
}

/// True when any `/`-separated component of the root-relative path `rel` equals
/// an exclude pattern (exact name equality, not glob/substring).
pub fn is_excluded(rel: &str, exclude: &[String]) -> bool {
    Path::new(rel)
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .any(|name| exclude.iter().any(|ex| ex == name))
}

/// Canonicalize `abs`; `Some(canonical)` only if it is a markdown file inside
/// the canonical `root` and no component of its root-relative path is excluded.
///
/// Every path that may enter the index, a link target list or a rendered page
/// goes through this: markdown-only keeps `.env` or `Cargo.toml` out, and
/// checking the *resolved* path keeps a symlink from smuggling in a file that
/// lives outside the project.
pub fn confine(root: &Path, abs: &Path, exclude: &[String]) -> Option<PathBuf> {
    let root = std::fs::canonicalize(root).ok()?;
    let canonical = std::fs::canonicalize(abs).ok()?;
    if !canonical.is_file() || !is_markdown(&canonical) {
        return None;
    }
    let rel = canonical.strip_prefix(&root).ok()?;
    if is_excluded(&rel.to_string_lossy(), exclude) {
        return None;
    }
    Some(canonical)
}

/// Walk `root` recursively, returning absolute paths of markdown files.
/// Respects .gitignore (via WalkBuilder) and prunes `exclude` directory names.
pub fn scan_markdown_files(root: &Path, exclude: &[String]) -> Vec<PathBuf> {
    scan_markdown_files_with(root, exclude, true)
}

/// Same walk as [`scan_markdown_files`], but never skips a file any ignore
/// mechanism (`.gitignore`, `.git/info/exclude`, a generic `.ignore` file)
/// would otherwise hide.
///
/// Used to locate one already-known-viewable file (short-link resolution),
/// where none of those must hide a file that the long URL
/// (`Engine::ensure_indexed`, which consults none of them) would happily
/// index — a project's `.git/info/exclude` commonly excludes local-only
/// paths (e.g. `.claude/worktrees/`) that still hold real, viewable files.
/// `exclude`'s named directories (`.git`, `node_modules`, `target`, …) still
/// get pruned, so this stays cheap even over a large ignored tree.
pub fn scan_markdown_files_ignoring_gitignore(root: &Path, exclude: &[String]) -> Vec<PathBuf> {
    scan_markdown_files_with(root, exclude, false)
}

fn scan_markdown_files_with(
    root: &Path,
    exclude: &[String],
    respect_gitignore: bool,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let exclude: Vec<String> = exclude.to_vec();
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .ignore(respect_gitignore)
        .git_ignore(respect_gitignore)
        .git_global(false)
        .git_exclude(respect_gitignore)
        .parents(false)
        .filter_entry(move |e| !is_excluded(&e.file_name().to_string_lossy(), &exclude))
        .build();
    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_file() && is_markdown(path) {
            out.push(path.to_path_buf());
        }
    }
    out
}

/// `.md` / `.markdown`, case-insensitive.
pub fn is_markdown(p: &Path) -> bool {
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

pub(crate) fn filename(p: &Path) -> String {
    p.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
        .to_string()
}

/// Hash of a file's content, used to tell a real edit apart from a touch that
/// left the bytes unchanged (git checkout, an editor's no-op autosave). Stored
/// on the `files` row and compared against on every reindex — see
/// `SqliteStore::index_docs`.
pub fn content_hash(content: &str) -> String {
    crate::hash::fnv1a64_hex(content.as_bytes())
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

/// RFC3339 UTC timestamp `secs` seconds before now — the cutoff a cleanup
/// sweep compares stored `last_seen_at` values against.
pub fn cutoff_rfc3339(secs: i64) -> String {
    (OffsetDateTime::now_utc() - time::Duration::seconds(secs))
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

    fn tempdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mdview-idx-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::canonicalize(&dir).unwrap()
    }

    fn project_for(dir: &Path) -> Project {
        Project {
            id: slug_from_root(dir),
            name: "T".into(),
            root_path: dir.to_path_buf(),
            created_at: now_rfc3339(),
            last_seen_at: now_rfc3339(),
        }
    }

    #[test]
    fn cutoff_rfc3339_is_earlier_than_now_by_roughly_the_requested_span() {
        let now = now_rfc3339();
        let one_week_ago = cutoff_rfc3339(7 * 24 * 60 * 60);
        // RFC3339's fixed-width fields sort lexicographically like the
        // timestamps they represent — this is the same comparison
        // `cleanup_stale` relies on.
        assert!(one_week_ago < now);
    }

    #[test]
    fn scans_recursively_and_builds_docs_with_titles() {
        let dir = tempdir("scan");
        write(&dir, "README.md", "# Root Readme\nhello");
        write(&dir, "docs/guide.md", "# Guide\ncontent");
        write(&dir, "docs/nested/deep.md", "no heading here");
        write(&dir, "notes.txt", "not markdown");
        write(&dir, "node_modules/pkg/x.md", "# Should be excluded");

        let project = project_for(&dir);
        let exclude = vec!["node_modules".to_string()];
        let files = scan_markdown_files(&project.root_path, &exclude);
        let docs: Vec<IndexedDoc> = files
            .iter()
            .filter_map(|p| IndexService::build_doc(&project, p, 10_000_000, &exclude))
            .collect();
        assert_eq!(
            docs.len(),
            3,
            "should read 3 md files (excluding the excluded dir + txt)"
        );

        let store = SqliteStore::open_in_memory().unwrap();
        store.index_docs(&docs).unwrap();
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
    fn confine_accepts_only_markdown_inside_the_root() {
        let dir = tempdir("confine");
        write(&dir, "a.md", "# A");
        write(&dir, "docs/b.MARKDOWN", "# B");
        write(&dir, ".env", "SECRET=1");
        write(&dir, "Cargo.toml", "[package]");
        write(&dir, "node_modules/x.md", "# X");
        let exclude = vec!["node_modules".to_string()];

        assert!(confine(&dir, &dir.join("a.md"), &exclude).is_some());
        assert!(confine(&dir, &dir.join("docs/b.MARKDOWN"), &exclude).is_some());
        assert!(confine(&dir, &dir.join(".env"), &exclude).is_none());
        assert!(confine(&dir, &dir.join("Cargo.toml"), &exclude).is_none());
        assert!(confine(&dir, &dir.join("node_modules/x.md"), &exclude).is_none());
        assert!(confine(&dir, &dir.join("docs/../../escape.md"), &exclude).is_none());
        assert!(confine(&dir, &dir.join("missing.md"), &exclude).is_none());
        assert!(confine(&dir, &dir.join("docs"), &exclude).is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn confine_rejects_symlinks_that_resolve_outside_the_root() {
        let outside = tempdir("confine-outside");
        write(&outside, "secret.md", "# Secret");
        let dir = tempdir("confine-link");
        write(&dir, "a.md", "# A");
        std::os::unix::fs::symlink(&outside, dir.join("linked")).unwrap();
        std::os::unix::fs::symlink(outside.join("secret.md"), dir.join("file-link.md")).unwrap();

        assert!(confine(&dir, &dir.join("linked/secret.md"), &[]).is_none());
        assert!(confine(&dir, &dir.join("file-link.md"), &[]).is_none());
        let project = project_for(&dir);
        assert!(
            IndexService::read_file(&project, &dir.join("linked/secret.md"), 1 << 20, &[])
                .is_none()
        );

        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(&outside).ok();
    }

    #[test]
    fn build_doc_links_to_unindexed_files_but_never_to_non_markdown() {
        let dir = tempdir("links");
        write(&dir, "a.md", "[b](b.md) [env](.env) [cargo](Cargo.toml)");
        write(&dir, "b.md", "# B");
        write(&dir, ".env", "SECRET=1");
        write(&dir, "Cargo.toml", "[package]");

        let project = project_for(&dir);
        let doc = IndexService::build_doc(&project, &dir.join("a.md"), 1 << 20, &[]).unwrap();
        assert_eq!(doc.links, vec!["b.md".to_string()]);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn slug_generation() {
        assert_eq!(slug_from_root(Path::new("/home/x/My App")), "my-app");
        assert_eq!(slug_from_root(Path::new("/home/x/proj.v2")), "proj-v2");
    }
}
