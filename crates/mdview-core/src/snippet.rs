//! Search excerpt building (built from the file on disk for the top results,
//! using the same [`crate::fold::fold`] as the index).
//!
//! Matches are wrapped in private-use sentinels rather than HTML: the excerpt
//! is raw document text, so any markup in it (including a literal `<mark>`)
//! must stay inert until the view escapes it and swaps the sentinels for tags.

use crate::fold::fold;

/// Opens a highlighted match.
pub const MARK_OPEN: char = '\u{E000}';
/// Closes a highlighted match.
pub const MARK_CLOSE: char = '\u{E001}';

/// The query split the way the index splits it (non-alphanumeric boundaries),
/// each term folded.
pub fn query_terms(query: &str) -> Vec<String> {
    fold(query)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// A word matches when any term is a prefix of it (mirrors the prefix query).
fn word_matches(word: &str, terms: &[String]) -> bool {
    let folded = fold(word);
    let core = folded.trim_start_matches(|c: char| !c.is_alphanumeric());
    terms.iter().any(|t| core.starts_with(t.as_str()))
}

/// A `max_words`-word window of `content` with the most matching words,
/// matches wrapped in [`MARK_OPEN`]/[`MARK_CLOSE`], `…` where text was cut.
/// No match falls back to the leading words. Sentinel characters already in
/// the content are dropped so they cannot forge a highlight.
pub fn excerpt(content: &str, terms: &[String], max_words: usize) -> String {
    let clean: String = content
        .chars()
        .filter(|c| *c != MARK_OPEN && *c != MARK_CLOSE)
        .collect();
    let words: Vec<&str> = clean.split_whitespace().collect();
    if words.is_empty() || max_words == 0 {
        return String::new();
    }
    let hits: Vec<bool> = words.iter().map(|w| word_matches(w, terms)).collect();
    let window = max_words.min(words.len());
    let last_start = words.len() - window;
    let count = |s: usize| hits[s..s + window].iter().filter(|h| **h).count();

    let (mut best_start, mut best) = (0, count(0));
    for s in 1..=last_start {
        let c = count(s);
        if c > best {
            best_start = s;
            best = c;
        }
    }
    // Pull in leading context while the window keeps every match it had.
    if best > 0 {
        for back in (1..=window / 4).rev() {
            if let Some(s) = best_start.checked_sub(back) {
                if count(s) == best {
                    best_start = s;
                    break;
                }
            }
        }
    }

    let mut out = String::new();
    if best_start > 0 {
        out.push_str("… ");
    }
    for i in best_start..best_start + window {
        if i > best_start {
            out.push(' ');
        }
        if hits[i] {
            out.push(MARK_OPEN);
            out.push_str(words[i]);
            out.push(MARK_CLOSE);
        } else {
            out.push_str(words[i]);
        }
    }
    if best_start + window < words.len() {
        out.push_str(" …");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marked(s: &str) -> String {
        s.replace(MARK_OPEN, "[").replace(MARK_CLOSE, "]")
    }

    fn run(content: &str, query: &str, n: usize) -> String {
        marked(&excerpt(content, &query_terms(query), n))
    }

    #[test]
    fn query_terms_fold_and_split() {
        assert_eq!(query_terms("Tài-liệu, Đường"), vec!["tai", "lieu", "duong"]);
    }

    #[test]
    fn unaccented_query_marks_accented_words() {
        assert_eq!(
            run("Đây là tài liệu mới", "tai lieu", 24),
            "Đây là [tài] [liệu] mới"
        );
        assert_eq!(run("Nó được dùng", "duoc", 24), "Nó [được] dùng");
    }

    #[test]
    fn prefix_matches() {
        assert_eq!(
            run("we are indexing files", "index", 24),
            "we are [indexing] files"
        );
    }

    #[test]
    fn window_covers_a_late_match_with_context_and_ellipses() {
        let words: Vec<String> = (0..100).map(|i| format!("w{i}")).collect();
        let mut text = words.clone();
        text[60] = "needle".into();
        let out = run(&text.join(" "), "needle", 10);
        assert!(out.starts_with("… "), "{out}");
        assert!(out.ends_with(" …"), "{out}");
        assert!(out.contains("[needle]"), "{out}");
        assert!(!out.contains("w0 "), "{out}");
        // Not pinned to the window's first word.
        assert!(
            !out.trim_start_matches("… ").starts_with("[needle]"),
            "{out}"
        );
    }

    #[test]
    fn no_match_returns_leading_words() {
        assert_eq!(run("a b c d e f", "zzz", 3), "a b c …");
        assert_eq!(run("a b", "zzz", 3), "a b");
    }

    #[test]
    fn multibyte_words_survive_intact() {
        assert_eq!(
            run("日本語 のテキスト 🎉 emoji", "emoji", 24),
            "日本語 のテキスト 🎉 [emoji]"
        );
    }

    #[test]
    fn markup_and_sentinels_in_content_are_neutralised() {
        let out = excerpt(
            "<mark>hi</mark> \u{E000}forged\u{E001} word",
            &query_terms("word"),
            24,
        );
        assert_eq!(out.matches(MARK_OPEN).count(), 1);
        assert_eq!(out.matches(MARK_CLOSE).count(), 1);
        assert!(out.contains("<mark>hi</mark>"));
        assert!(out.contains("forged"));
    }

    #[test]
    fn empty_content_is_empty() {
        assert_eq!(excerpt("  \n ", &query_terms("x"), 24), "");
    }
}
