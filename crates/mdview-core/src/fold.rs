//! The single text fold shared by FTS insert, FTS query and excerpt building.
//!
//! FTS5's `remove_diacritics 2` strips combining marks but does not map `đ`
//! (a distinct letter, not a base letter plus mark), so "được" would never
//! match a query "duoc". Folding in mdview, identically on both sides, closes
//! that gap and keeps the index and the query from drifting apart.

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// NFD, drop combining marks, map `đ`/`Đ` to `d`, lowercase.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.nfd().filter(|c| !is_combining_mark(*c)) {
        match c {
            'đ' | 'Đ' => out.push('d'),
            _ => out.extend(c.to_lowercase()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_vietnamese_including_d_stroke() {
        assert_eq!(fold("Đường đi được"), "duong di duoc");
        assert_eq!(fold("Tài liệu"), "tai lieu");
    }

    #[test]
    fn ascii_is_only_lowercased() {
        assert_eq!(fold("Hello, World_42"), "hello, world_42");
    }

    #[test]
    fn is_idempotent() {
        let once = fold("Đường đi được — Café");
        assert_eq!(fold(&once), once);
    }
}
