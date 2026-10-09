//! Fuzzy file-jump matching over indexed relative paths, using nucleo-matcher.
//!
//! This complements FTS5 content search: FTS5 finds files by their *content*
//! (see `repository::SqliteStore::search`), while this ranks files by a fuzzy
//! match of the query against their *relative path* — the "jump quickly to a
//! file by name" affordance. Smart-case: an all-lowercase query matches
//! case-insensitively; any uppercase character makes the match case-sensitive.

use crate::domain::IndexedFile;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use serde::Serialize;
use std::time::SystemTime;

/// One ranked fuzzy file-jump hit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FuzzyHit {
    /// Path relative to the project root (the matched haystack).
    pub rel_path: String,
    /// File title (first H1, or filename), for display.
    pub title: String,
    /// Click-through URL, matching the FTS `SearchResult::url` convention.
    pub url: String,
    /// nucleo match score; higher is a better match.
    pub score: u32,
}

/// Rank `files` by a fuzzy match of `query` against each file's relative path
/// and title. See [`rank_items`].
pub fn rank_files(
    files: &[IndexedFile],
    project_id: &str,
    query: &str,
    limit: usize,
) -> Vec<FuzzyHit> {
    let items: Vec<(String, String, SystemTime)> = files
        .iter()
        .map(|f| (f.rel_path.clone(), f.title.clone(), SystemTime::UNIX_EPOCH))
        .collect();
    rank_items(&items, project_id, query, limit)
}

/// Rank `(rel_path, title, modified)` items by a fuzzy match of `query`.
///
/// An item's score is the better of its path score and its title score; equal
/// scores list the more recently modified item first. Returns at most `limit`
/// hits by descending score. A blank query yields an empty result.
/// `project_id` is used only to build the click-through URL.
pub fn rank_items(
    items: &[(String, String, SystemTime)],
    project_id: &str,
    query: &str,
    limit: usize,
) -> Vec<FuzzyHit> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }

    let mut path_matcher = Matcher::new(Config::DEFAULT.match_paths());
    let mut title_matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut buf = Vec::new();

    let mut scored: Vec<(u32, SystemTime, &(String, String, SystemTime))> = items
        .iter()
        .filter_map(|item| {
            let by_path = pattern.score(Utf32Str::new(&item.0, &mut buf), &mut path_matcher);
            let by_title = pattern.score(Utf32Str::new(&item.1, &mut buf), &mut title_matcher);
            by_path.max(by_title).map(|score| (score, item.2, item))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));

    scored
        .into_iter()
        .take(limit)
        .map(|(score, _, (rel_path, title, _))| FuzzyHit {
            rel_path: rel_path.clone(),
            title: title.clone(),
            url: crate::domain::file_url(project_id, rel_path),
            score,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(rel: &str, title: &str) -> IndexedFile {
        IndexedFile {
            project_id: "p1".into(),
            abs_path: std::path::PathBuf::from("/proj").join(rel),
            rel_path: rel.into(),
            title: title.into(),
            size_bytes: 0,
            modified_at: "1970-01-01T00:00:00Z".into(),
        }
    }

    fn corpus() -> Vec<IndexedFile> {
        vec![
            file("docs/architecture.md", "Architecture"),
            file("docs/api/auth.md", "Auth API"),
            file("README.md", "Readme"),
            file("src/server.md", "Server"),
        ]
    }

    #[test]
    fn ranks_by_path_match_and_builds_url() {
        let files = corpus();
        let hits = rank_files(&files, "p1", "arch", 10);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].rel_path, "docs/architecture.md");
        assert_eq!(hits[0].url, "/p/p1/docs/architecture.md");
        assert_eq!(hits[0].title, "Architecture");
    }

    #[test]
    fn matches_path_segments_not_just_filename() {
        let files = corpus();
        // "auth" lives in the path docs/api/auth.md — path haystack must find it.
        let hits = rank_files(&files, "p1", "auth", 10);
        assert_eq!(hits[0].rel_path, "docs/api/auth.md");
    }

    #[test]
    fn smart_case_lowercase_is_insensitive() {
        let files = corpus();
        // lowercase query matches the capitalized path component
        assert!(!rank_files(&files, "p1", "readme", 10).is_empty());
    }

    #[test]
    fn empty_query_returns_empty() {
        let files = corpus();
        assert!(rank_files(&files, "p1", "", 10).is_empty());
        assert!(rank_files(&files, "p1", "   ", 10).is_empty());
    }

    #[test]
    fn respects_limit() {
        let files = corpus();
        // "m" (in .md) matches many; cap at 2.
        let hits = rank_files(&files, "p1", "m", 2);
        assert!(hits.len() <= 2);
    }

    #[test]
    fn scores_are_descending() {
        let files = corpus();
        let hits = rank_files(&files, "p1", "doc", 10);
        for w in hits.windows(2) {
            assert!(w[0].score >= w[1].score);
        }
    }

    fn item(rel: &str, title: &str, secs: u64) -> (String, String, SystemTime) {
        (
            rel.into(),
            title.into(),
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs),
        )
    }

    #[test]
    fn title_only_match_is_found() {
        let items = vec![item("notes/a1.md", "Deployment runbook", 1)];
        let hits = rank_items(&items, "p1", "runbook", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].rel_path, "notes/a1.md");
    }

    #[test]
    fn newer_wins_a_tie() {
        let items = vec![
            item("old/guide.md", "Guide", 10),
            item("new/guide.md", "Guide", 99),
        ];
        let hits = rank_items(&items, "p1", "guide", 10);
        assert_eq!(hits[0].score, hits[1].score);
        assert_eq!(hits[0].rel_path, "new/guide.md");
    }

    #[test]
    fn rank_items_blank_query_is_empty() {
        let items = vec![item("a.md", "A", 1)];
        assert!(rank_items(&items, "p1", "  ", 10).is_empty());
    }
}
