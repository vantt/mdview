//! Cross-folder link resolution (PRD §7.3 / FR-11) — mdview's core differentiator.
//!
//! Rewrites an internal markdown link into the app URL namespace, resolving
//! `../` across folders within a single project. Cross-project lookup is
//! intentionally OUT OF SCOPE (PRD §3.2 non-goal).

use crate::domain::ResolvedLink;
use std::path::{Component, Path, PathBuf};

/// Abstraction over "is this absolute path an indexed file?" so the resolver
/// stays pure and testable without a database.
pub trait IndexLookup {
    fn contains(&self, abs: &Path) -> bool;
}

impl IndexLookup for std::collections::HashSet<PathBuf> {
    fn contains(&self, abs: &Path) -> bool {
        std::collections::HashSet::contains(self, abs)
    }
}

/// True for links we leave untouched (external / in-page / protocol).
pub fn is_external(href: &str) -> bool {
    let h = href.trim();
    h.is_empty()
        || h.starts_with('#')
        || h.starts_with("http://")
        || h.starts_with("https://")
        || h.starts_with("mailto:")
        || h.starts_with("tel:")
        || h.starts_with("//")
        || h.starts_with("data:")
}

/// Lexically normalize a path (resolve `.` and `..`) without touching disk.
/// Leading `..` that would escape the base are dropped (clamped at root).
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    // escaped above the anchor; ignore (clamp)
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Resolve an internal link to the target's project-relative path (no anchor,
/// no URL), or None if external/unresolvable. Used for backlinks and to detect
/// broken links.
pub fn resolve_to_rel(
    source_abs: &Path,
    href: &str,
    project_root: &Path,
    index: &dyn IndexLookup,
) -> Option<String> {
    if is_external(href) {
        return None;
    }
    let path_part = href.split('#').next().unwrap_or(href);
    let path_part = path_part.split('?').next().unwrap_or(path_part);
    if path_part.is_empty() {
        return None;
    }
    let abs = if let Some(rest) = path_part.strip_prefix('/') {
        normalize(&project_root.join(rest))
    } else {
        let source_dir = source_abs.parent().unwrap_or(project_root);
        normalize(&source_dir.join(path_part))
    };
    let candidates = [
        abs.clone(),
        with_md_extension(&abs),
        abs.join("README.md"),
        abs.join("index.md"),
    ];
    for cand in candidates.iter() {
        if index.contains(cand) {
            if let Ok(rel) = cand.strip_prefix(project_root) {
                return Some(to_url_path(rel));
            }
        }
    }
    None
}

/// Resolve one link. Returns the rewritten in-app URL, or a broken marker.
///
/// * `source_abs` — absolute path of the file the link lives in.
/// * `href` — the raw link target from the markdown.
/// * `project_id` / `project_root` — the owning project.
/// * `index` — lookup for indexed absolute paths.
pub fn resolve_link(
    source_abs: &Path,
    href: &str,
    project_id: &str,
    project_root: &Path,
    index: &dyn IndexLookup,
) -> ResolvedLink {
    if is_external(href) {
        return ResolvedLink { url: None, broken: false };
    }

    // Split off anchor.
    let (path_part, anchor) = match href.split_once('#') {
        Some((p, a)) => (p, Some(a)),
        None => (href, None),
    };
    // Strip any query string (rare in local md, but be safe).
    let path_part = path_part.split('?').next().unwrap_or(path_part);

    if path_part.is_empty() {
        // Pure anchor like `#section` already handled by is_external, but guard.
        return ResolvedLink { url: None, broken: false };
    }

    // Resolve to an absolute path.
    let abs = if let Some(rest) = path_part.strip_prefix('/') {
        // Absolute from project root.
        normalize(&project_root.join(rest))
    } else {
        let source_dir = source_abs.parent().unwrap_or(project_root);
        normalize(&source_dir.join(path_part))
    };

    // Candidate resolution: exact, +.md, and README.md inside a dir link.
    let candidates = [
        abs.clone(),
        with_md_extension(&abs),
        abs.join("README.md"),
        abs.join("index.md"),
    ];

    for cand in candidates.iter() {
        if index.contains(cand) {
            if let Ok(rel) = cand.strip_prefix(project_root) {
                let rel_url = to_url_path(rel);
                let mut url = format!("/p/{project_id}/{rel_url}");
                if let Some(a) = anchor {
                    url.push('#');
                    url.push_str(a);
                }
                return ResolvedLink { url: Some(url), broken: false };
            }
        }
    }

    ResolvedLink { url: None, broken: true }
}

/// Append `.md` unless the path already ends in a markdown extension.
fn with_md_extension(p: &Path) -> PathBuf {
    match p.extension().and_then(|e| e.to_str()) {
        Some("md") | Some("markdown") => p.to_path_buf(),
        _ => {
            let mut s = p.as_os_str().to_os_string();
            s.push(".md");
            PathBuf::from(s)
        }
    }
}

/// Join path components with `/` for use in a URL (cross-platform).
fn to_url_path(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn index(root: &str, paths: &[&str]) -> HashSet<PathBuf> {
        paths.iter().map(|p| PathBuf::from(root).join(p)).collect()
    }

    #[test]
    fn resolves_parent_dir_cross_folder() {
        let root = "/proj";
        let idx = index(root, &["src/api/README.md", "docs/architecture.md"]);
        let src = PathBuf::from("/proj/docs/architecture.md");
        let r = resolve_link(&src, "../src/api/README.md", "p1", Path::new(root), &idx);
        assert_eq!(r.url.as_deref(), Some("/p/p1/src/api/README.md"));
        assert!(!r.broken);
    }

    #[test]
    fn resolves_absolute_from_project_root() {
        let root = "/proj";
        let idx = index(root, &["docs/guide.md"]);
        let src = PathBuf::from("/proj/src/api/README.md");
        let r = resolve_link(&src, "/docs/guide.md", "p1", Path::new(root), &idx);
        assert_eq!(r.url.as_deref(), Some("/p/p1/docs/guide.md"));
    }

    #[test]
    fn resolves_link_without_extension() {
        let root = "/proj";
        let idx = index(root, &["api/README.md"]);
        let src = PathBuf::from("/proj/docs/x.md");
        let r = resolve_link(&src, "../api/README", "p1", Path::new(root), &idx);
        assert_eq!(r.url.as_deref(), Some("/p/p1/api/README.md"));
    }

    #[test]
    fn preserves_anchor() {
        let root = "/proj";
        let idx = index(root, &["api/README.md"]);
        let src = PathBuf::from("/proj/docs/x.md");
        let r = resolve_link(&src, "../api/README.md#installation", "p1", Path::new(root), &idx);
        assert_eq!(r.url.as_deref(), Some("/p/p1/api/README.md#installation"));
    }

    #[test]
    fn dir_link_resolves_to_readme() {
        let root = "/proj";
        let idx = index(root, &["api/README.md"]);
        let src = PathBuf::from("/proj/docs/x.md");
        let r = resolve_link(&src, "../api", "p1", Path::new(root), &idx);
        assert_eq!(r.url.as_deref(), Some("/p/p1/api/README.md"));
    }

    #[test]
    fn external_links_untouched() {
        let idx: HashSet<PathBuf> = HashSet::new();
        let src = PathBuf::from("/proj/x.md");
        for h in ["https://a.com", "http://a.com", "mailto:a@b.c", "#top"] {
            let r = resolve_link(&src, h, "p1", Path::new("/proj"), &idx);
            assert_eq!(r.url, None);
            assert!(!r.broken, "{h} should not be broken");
        }
    }

    #[test]
    fn unresolvable_is_broken() {
        let idx: HashSet<PathBuf> = HashSet::new();
        let src = PathBuf::from("/proj/docs/x.md");
        let r = resolve_link(&src, "./missing.md", "p1", Path::new("/proj"), &idx);
        assert_eq!(r.url, None);
        assert!(r.broken);
    }

    #[test]
    fn does_not_escape_project_root() {
        let root = "/proj";
        let idx = index(root, &["a.md"]);
        let src = PathBuf::from("/proj/a.md");
        // `../../etc/passwd` clamps and won't match an indexed file → broken, never escapes.
        let r = resolve_link(&src, "../../../etc/passwd", "p1", Path::new(root), &idx);
        assert!(r.broken);
        assert_eq!(r.url, None);
    }
}
