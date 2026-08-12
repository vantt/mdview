//! Short codes for file URLs.
//!
//! A file's full URL (`/p/<project>/<deep/rel/path.md>`) runs past 80 characters
//! for anything nested, which wraps in a terminal and stops being clickable. The
//! short form `/s/<code>` stays at a fixed length no matter how deep the file is.
//!
//! The code is *derived* from `(project_id, rel_path)` rather than allocated, so
//! it has no lifetime of its own: it exists exactly as long as the file's index
//! row does, and the indexer already deletes that row when a file disappears.
//! That is why there is no shortlink table, no TTL, and nothing to garbage
//! collect.

/// How many leading hex characters of the hash a link actually carries.
///
/// The column stores all 16, so widening this is a one-line change that needs no
/// migration. At 12 characters (2.8e14 values) two files colliding is ~1.8e-5
/// even at 100k indexed files.
pub const SHORT_CODE_LEN: usize = 12;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Full 16-character hex hash identifying one file within one project.
///
/// FNV-1a is hand-written rather than pulled from a crate because the value has
/// to stay byte-identical across Rust releases: `std`'s `DefaultHasher` documents
/// that its algorithm may change, which would silently kill every link already
/// handed out. Nothing here needs to resist an adversary — the server has no
/// authentication in the first place.
pub fn path_hash(project_id: &str, rel_path: &str) -> String {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in project_id
        .as_bytes()
        .iter()
        .chain(std::iter::once(&0u8))
        .chain(rel_path.as_bytes())
    {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

/// The code that goes in a link: the leading [`SHORT_CODE_LEN`] characters.
pub fn short_code(path_hash: &str) -> String {
    path_hash.chars().take(SHORT_CODE_LEN).collect()
}

/// GLOB pattern matching every hash that starts with `code`.
///
/// The `*` is appended here, in Rust, and bound as a single parameter. Building
/// it in SQL instead (`path_hash GLOB ?1 || '*'`) makes the right-hand side an
/// expression rather than a literal, which disables SQLite's LIKE/GLOB index
/// optimisation: the query still returns the right rows, so no functional test
/// notices, but it degrades to a full table scan.
pub fn hash_prefix_pattern(code: &str) -> String {
    format!("{code}*")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pinned vectors. These must never change: every link already handed out
    /// depends on the algorithm producing exactly these bytes.
    #[test]
    fn path_hash_is_stable() {
        assert_eq!(path_hash("mdview", "docs/a.md"), "ea3387e1bec96ab7");
        assert_eq!(path_hash("mdview", "README.md"), "0eb211fd487e7bb0");
    }

    #[test]
    fn path_hash_is_16_hex_chars() {
        let h = path_hash("p", "a.md");
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// The separator is what keeps ("ab", "c") from colliding with ("a", "bc").
    #[test]
    fn project_and_path_do_not_run_together() {
        assert_ne!(path_hash("ab", "c.md"), path_hash("a", "bc.md"));
    }

    #[test]
    fn same_path_in_two_projects_differs() {
        assert_ne!(path_hash("one", "docs/a.md"), path_hash("two", "docs/a.md"));
    }

    #[test]
    fn short_code_takes_the_leading_characters() {
        let h = path_hash("mdview", "docs/a.md");
        let code = short_code(&h);
        assert_eq!(code.len(), SHORT_CODE_LEN);
        assert!(h.starts_with(&code));
    }

    #[test]
    fn prefix_pattern_appends_the_wildcard() {
        assert_eq!(hash_prefix_pattern("a3f9c1d20b74"), "a3f9c1d20b74*");
    }

    #[test]
    fn unicode_paths_hash_by_utf8_bytes() {
        let h = path_hash("p", "tài-liệu/hướng-dẫn.md");
        assert_eq!(h.len(), 16);
        assert_ne!(h, path_hash("p", "tai-lieu/huong-dan.md"));
    }
}
