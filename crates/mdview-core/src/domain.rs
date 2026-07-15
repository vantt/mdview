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
}

/// Rendered markdown page plus metadata for the viewer.
#[derive(Debug, Clone)]
pub struct RenderedPage {
    pub html: String,
    pub title: String,
    pub headings: Vec<Heading>,
    /// True if the page contains mermaid blocks (client must load mermaid.js).
    pub has_mermaid: bool,
}
