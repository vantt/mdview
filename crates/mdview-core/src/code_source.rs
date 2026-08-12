//! Safe on-disk access to source files for the Code viewer section.
//!
//! This is the ONLY module allowed to touch the filesystem for that section.
//! Unlike `engine::asset_path` (an extension allowlist, fine for images), the
//! Code section serves arbitrary text files, so the guard here is about the
//! *identity* of a file, not its extension: an unconditional sensitive-name
//! denylist plus gitignore-awareness, checked on the canonicalized path.
//!
//! The daemon has no authentication and can bind wildcard on a LAN
//! (`runtime::build_display_urls` exists precisely because of that), so
//! nothing here may depend on `config.indexing.exclude_patterns` alone for
//! the things that must never be served — that list is user-editable via
//! `/settings`. The denylist below and the hard-coded git-directory rule are
//! independent of it.

use crate::error::{Error, Result};
use crate::indexer;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Cap on how much of a file is read into memory / served. Past this, the
/// content is cut at the last complete line and reported `truncated`.
const MAX_SOURCE_BYTES: u64 = 2 * 1024 * 1024;

/// One entry in a directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// One directory level, already filtered and sorted.
#[derive(Debug, Clone)]
pub struct DirListing {
    pub rel_path: String,
    pub entries: Vec<DirEntry>,
}

/// The result of reading an authorised source path.
#[derive(Debug, Clone)]
pub enum SourceContent {
    Text { text: String, truncated: bool },
    Binary { size: u64 },
}

/// Canonicalise and authorise `rel` under `root`. Every filesystem access the
/// Code section makes must go through this (directly, or via `list_dir`,
/// which calls it on the directory itself before listing).
pub fn resolve_source_path(root: &Path, rel: &str, exclude: &[String]) -> Result<PathBuf> {
    let joined = crate::link_resolver::normalize(&root.join(rel));
    let canonical = fs::canonicalize(&joined).unwrap_or(joined);
    if !canonical.starts_with(root) {
        return Err(Error::PathOutsideProject(canonical));
    }
    let parent = canonical.parent().unwrap_or(root);
    let gi = build_gitignore(root, parent);
    if is_denied(root, &canonical, exclude, &gi) {
        return Err(Error::PathOutsideProject(canonical));
    }
    Ok(canonical)
}

/// One directory level: entries filtered the same way `resolve_source_path`
/// would filter each of them individually (same `is_denied` call), sorted
/// directories-first then alphabetically, case-insensitive.
pub fn list_dir(root: &Path, rel: &str, exclude: &[String]) -> Result<DirListing> {
    let dir = resolve_source_path(root, rel, exclude)?;
    if !dir.is_dir() {
        return Err(Error::InvalidPath(format!("not a directory: {rel}")));
    }
    let gi = build_gitignore(root, &dir);
    let mut entries = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let child = entry.path();
        let canonical_child = fs::canonicalize(&child).unwrap_or_else(|_| child.clone());
        if !canonical_child.starts_with(root) {
            continue;
        }
        if is_denied(root, &canonical_child, exclude, &gi) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = canonical_child.is_dir();
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        entries.push(DirEntry { name, is_dir, size });
    }
    entries.sort_by(|a, b| {
        (!a.is_dir, a.name.to_lowercase()).cmp(&(!b.is_dir, b.name.to_lowercase()))
    });
    Ok(DirListing {
        rel_path: rel.to_string(),
        entries,
    })
}

/// Read an authorised path: binary-sniffed (NUL byte in the first 8 KiB, or
/// invalid UTF-8) and capped at `MAX_SOURCE_BYTES`, cut at the last complete
/// line when truncated. Never lossy-converts a genuinely binary file.
pub fn read_source(abs: &Path) -> Result<SourceContent> {
    let meta = fs::metadata(abs)?;
    let file = fs::File::open(abs)?;
    let mut buf = Vec::with_capacity((meta.len().min(MAX_SOURCE_BYTES)) as usize);
    file.take(MAX_SOURCE_BYTES).read_to_end(&mut buf)?;
    let truncated_by_cap = meta.len() > MAX_SOURCE_BYTES;

    let sniff_len = buf.len().min(8192);
    if buf[..sniff_len].contains(&0u8) {
        return Ok(SourceContent::Binary { size: meta.len() });
    }

    let mut text = match std::str::from_utf8(&buf) {
        Ok(s) => s.to_string(),
        Err(e) => {
            // A capped read can slice a multi-byte char at the very end.
            // Recover the valid prefix only when that's clearly what
            // happened (a handful of trailing bytes lost to the cut) —
            // anything else is a genuinely non-UTF-8 file.
            let valid_up_to = e.valid_up_to();
            if truncated_by_cap && buf.len() - valid_up_to <= 3 {
                std::str::from_utf8(&buf[..valid_up_to])
                    .expect("valid_up_to is a verified UTF-8 boundary")
                    .to_string()
            } else {
                return Ok(SourceContent::Binary { size: meta.len() });
            }
        }
    };

    if truncated_by_cap {
        if let Some(idx) = text.rfind('\n') {
            text.truncate(idx + 1);
        }
    }

    Ok(SourceContent::Text {
        text,
        truncated: truncated_by_cap,
    })
}

/// Sensitive file/directory names, checked case-insensitively against every
/// path component — a directory match (e.g. a git directory, `.ssh`) denies
/// everything beneath it. Independent of `exclude_patterns` and of whether
/// the project has a `.gitignore` at all.
fn denylist_matches(name_lower: &str) -> bool {
    const EXACT: &[&str] = &[
        ".git",
        ".ssh",
        ".aws",
        ".gnupg",
        ".env",
        "id_rsa",
        "id_dsa",
        "id_ecdsa",
        "id_ed25519",
        ".netrc",
        ".npmrc",
        ".pypirc",
        ".git-credentials",
        "credentials",
        "secrets",
    ];
    if EXACT.contains(&name_lower) {
        return true;
    }
    const PREFIXES: &[&str] = &[".env.", "credentials.", "secrets."];
    if PREFIXES.iter().any(|p| name_lower.starts_with(p)) {
        return true;
    }
    const EXTENSIONS: &[&str] = &["pem", "key", "p12", "pfx", "keystore", "jks"];
    if let Some(ext) = Path::new(name_lower).extension().and_then(|e| e.to_str()) {
        if EXTENSIONS.contains(&ext) {
            return true;
        }
    }
    false
}

/// True if any path component (by exact name equality, mirroring
/// `engine::is_excluded_path`) is denylisted or matches a configured exclude
/// pattern.
fn is_denied_component(rel: &str, exclude_patterns: &[String]) -> bool {
    Path::new(rel)
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .any(|name| {
            denylist_matches(&name.to_lowercase()) || exclude_patterns.iter().any(|ex| ex == name)
        })
}

/// Combines the component-name gate with the gitignore matcher. Used for
/// both a single-path resolve and per-entry filtering in `list_dir`, so the
/// two agree by construction rather than by parallel logic.
fn is_denied(root: &Path, canonical: &Path, exclude_patterns: &[String], gi: &Gitignore) -> bool {
    let rel = indexer::rel_path_str(root, canonical);
    if rel.is_empty() {
        return false; // root itself
    }
    if is_denied_component(&rel, exclude_patterns) {
        return true;
    }
    gi.matched(canonical, canonical.is_dir()).is_ignore()
}

/// Build a gitignore matcher covering every `.gitignore` from `root` down to
/// `dir` (inclusive), so a subdirectory's own `.gitignore` is honoured the
/// same way git honours it. Works with no `.git` present at all — this reads
/// literal `.gitignore` files, not git repository state; a project with none
/// yields an always-empty (never-ignore) matcher, and the denylist above is
/// what still protects it.
fn build_gitignore(root: &Path, dir: &Path) -> Gitignore {
    let mut ancestors = Vec::new();
    let mut cur = dir.to_path_buf();
    loop {
        ancestors.push(cur.clone());
        if cur == root {
            break;
        }
        match cur.parent() {
            Some(p) if cur.starts_with(root) => cur = p.to_path_buf(),
            _ => break,
        }
    }
    let mut builder = GitignoreBuilder::new(root);
    for candidate in ancestors.into_iter().rev() {
        let gi_path = candidate.join(".gitignore");
        if gi_path.is_file() {
            let _ = builder.add(&gi_path);
        }
    }
    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, body: &str) {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    fn tmp(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mdview-code-src-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn traversal_escape_is_denied() {
        let dir = tmp("trav");
        write(&dir, "readme.md", "# root");
        let err = resolve_source_path(&dir, "../../../../../../../etc/passwd", &[]);
        assert!(err.is_err());
        fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escaping_root_is_denied() {
        let dir = tmp("symlink-out");
        write(&dir, "readme.md", "# root");
        let outside = std::env::temp_dir().join(format!(
            "mdview-code-src-outside-{}.txt",
            std::process::id()
        ));
        fs::write(&outside, "secret-outside").unwrap();
        let link = dir.join("escape.txt");
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        assert!(resolve_source_path(&dir, "escape.txt", &[]).is_err());

        fs::remove_file(&outside).ok();
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn git_directory_denied_even_with_empty_exclude_patterns() {
        let dir = tmp("gitdir");
        write(&dir, ".git/config", "[core]\n");
        // exclude_patterns deliberately empty: the git-directory guard must
        // not depend on it — a user can clear that config via /settings.
        assert!(resolve_source_path(&dir, ".git/config", &[]).is_err());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn env_file_denied_when_gitignored() {
        let dir = tmp("env-ignored");
        write(&dir, ".gitignore", ".env\n");
        write(&dir, ".env", "SECRET=1");

        let listing = list_dir(&dir, "", &[]).unwrap();
        assert!(!listing.entries.iter().any(|e| e.name == ".env"));
        assert!(resolve_source_path(&dir, ".env", &[]).is_err());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn env_file_denied_with_no_gitignore_at_all() {
        let dir = tmp("env-no-gitignore");
        write(&dir, ".env", "SECRET=1");
        // No .gitignore anywhere in this project: the denylist alone must
        // still catch it.
        assert!(resolve_source_path(&dir, ".env", &[]).is_err());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dotdir_outside_denylist_is_browsable() {
        let dir = tmp("dotdir-ok");
        write(&dir, ".github/workflows/ci.yml", "name: ci");

        let root_listing = list_dir(&dir, "", &[]).unwrap();
        assert!(root_listing
            .entries
            .iter()
            .any(|e| e.name == ".github" && e.is_dir));
        assert!(resolve_source_path(&dir, ".github/workflows/ci.yml", &[]).is_ok());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn configured_exclude_pattern_is_not_listed() {
        let dir = tmp("exclude-cfg");
        write(&dir, "node_modules/pkg/index.js", "module.exports = {}");
        write(&dir, "src/lib.rs", "pub fn x() {}");

        let listing = list_dir(&dir, "", &["node_modules".to_string()]).unwrap();
        assert!(!listing.entries.iter().any(|e| e.name == "node_modules"));
        assert!(listing.entries.iter().any(|e| e.name == "src"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn listing_is_sorted_dirs_first_then_alphabetical_case_insensitive() {
        let dir = tmp("sort");
        write(&dir, "zeta.rs", "");
        write(&dir, "Alpha.rs", "");
        write(&dir, "beta/mod.rs", "");
        write(&dir, "Aardvark/mod.rs", "");

        let listing = list_dir(&dir, "", &[]).unwrap();
        let names: Vec<_> = listing.entries.iter().map(|e| e.name.clone()).collect();
        assert_eq!(names, vec!["Aardvark", "beta", "Alpha.rs", "zeta.rs"]);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn binary_file_is_reported_as_binary() {
        let dir = tmp("binary");
        let p = dir.join("blob.bin");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&p, [0x00u8, 0x01, 0x02, 0xff, 0xfe]).unwrap();

        match read_source(&p).unwrap() {
            SourceContent::Binary { size } => assert_eq!(size, 5),
            SourceContent::Text { .. } => panic!("expected Binary"),
        }
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn oversized_file_is_truncated_at_a_line_boundary() {
        let dir = tmp("oversize");
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("big.txt");
        // One line just over the cap, so the file has no newline within the
        // cap window until the very first line ends past it — build instead
        // from many short lines so a clean cut point exists inside the cap.
        let line = "x".repeat(100);
        let mut content = String::new();
        while (content.len() as u64) <= MAX_SOURCE_BYTES + 1000 {
            content.push_str(&line);
            content.push('\n');
        }
        fs::write(&p, &content).unwrap();

        match read_source(&p).unwrap() {
            SourceContent::Text { text, truncated } => {
                assert!(truncated);
                assert!(text.len() as u64 <= MAX_SOURCE_BYTES);
                assert!(text.ends_with('\n'), "must cut at a line boundary");
            }
            SourceContent::Binary { .. } => panic!("expected Text"),
        }
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn project_root_without_git_still_enforces_denylist() {
        let dir = tmp("no-git-repo");
        // No .git, no .gitignore anywhere — a plain directory tree.
        write(&dir, "id_rsa", "-----BEGIN OPENSSH PRIVATE KEY-----");
        write(&dir, "readme.md", "# hi");

        let listing = list_dir(&dir, "", &[]).unwrap();
        assert!(!listing.entries.iter().any(|e| e.name == "id_rsa"));
        assert!(listing.entries.iter().any(|e| e.name == "readme.md"));
        assert!(resolve_source_path(&dir, "id_rsa", &[]).is_err());
        fs::remove_dir_all(&dir).ok();
    }
}
