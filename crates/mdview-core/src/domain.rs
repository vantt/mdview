//! Domain types. Pure data — no dependency on Axum/Tauri/SQLite.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A registered project: a root directory whose markdown tree is indexed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root_path: PathBuf,
    /// RFC3339 timestamps.
    pub created_at: String,
    pub last_seen_at: String,
}

/// One indexed markdown file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedFile {
    pub project_id: String,
    /// Absolute path on disk.
    pub abs_path: PathBuf,
    /// Path relative to project root — used as the URL segment.
    pub rel_path: String,
    /// First H1, or filename if none.
    pub title: String,
    pub size_bytes: u64,
    /// RFC3339 modified timestamp.
    pub modified_at: String,
}

/// A heading extracted from a file (for TOC / anchor navigation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub slug: String,
}

/// A resolved internal link, ready to become an `<a href>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLink {
    /// The rewritten in-app URL, or None if the link is broken/unresolvable.
    pub url: Option<String>,
    /// True when the target could not be resolved within the project.
    pub broken: bool,
}

/// Result of a search query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub project_id: String,
    pub rel_path: String,
    pub title: String,
    pub excerpt: String,
    pub url: String,
    pub score: f64,
    /// RFC3339 modified timestamp of the file.
    pub modified_at: String,
}

/// How content-search results are ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchSort {
    #[default]
    Relevance,
    Recent,
}

/// What one project sync did.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SyncStats {
    pub files_seen: usize,
    pub files_read: usize,
    pub files_removed: usize,
    pub elapsed_ms: u128,
    /// True when the previous sync finished moments ago and this call did nothing.
    pub skipped_recent: bool,
}

/// A content search plus the sync that preceded it.
#[derive(Debug, Clone, Default)]
pub struct SearchOutcome {
    pub results: Vec<SearchResult>,
    pub sync: SyncStats,
    /// Set when the sync failed; the query still ran over what was indexed.
    pub sync_error: Option<String>,
}

/// Rendered markdown page plus metadata for the viewer.
#[derive(Debug, Clone)]
pub struct RenderedPage {
    pub html: String,
    pub title: String,
    pub headings: Vec<Heading>,
    /// True if the page contains mermaid blocks (client must load mermaid.js).
    pub has_mermaid: bool,
    /// The raw markdown source, carried so the viewer can map a DOM selection
    /// back to source lines (copy-as-markdown) via the `data-sourcepos` attrs.
    pub source: String,
    /// Project-relative internal link targets that resolved, sorted and
    /// deduplicated.
    pub links: Vec<String>,
}

/// Viewer URL for a file: `/p/<project_id>/<rel_path>` with every path segment
/// percent-encoded (the `/` separators are kept), so names such as `C#.md` or
/// `a?b.md` survive as path rather than turning into a fragment or query.
pub fn file_url(project_id: &str, rel_path: &str) -> String {
    let mut out = format!("/p/{project_id}/");
    for (i, segment) in rel_path.split('/').enumerate() {
        if i > 0 {
            out.push('/');
        }
        for b in segment.bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod url_tests {
    use super::file_url;

    #[test]
    fn segments_are_encoded_and_separators_kept() {
        assert_eq!(file_url("p1", "docs/a.md"), "/p/p1/docs/a.md");
        assert_eq!(file_url("p1", "C#.md"), "/p/p1/C%23.md");
        assert_eq!(file_url("p1", "x/a?b c.md"), "/p/p1/x/a%3Fb%20c.md");
        assert_eq!(file_url("p1", "tài.md"), "/p/p1/t%C3%A0i.md");
    }
}
