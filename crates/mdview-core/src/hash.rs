//! The one FNV-1a primitive shared by every hash-derived value in this crate
//! (`short_link::path_hash`, `indexer::content_hash`).
//!
//! Kept in its own module rather than duplicated in each caller: both existing
//! callers depend on this producing byte-identical output forever (a short link
//! or a change-detection hash that silently changes shape breaks every value
//! already handed out), so there must be exactly one place the algorithm lives.
//!
//! Hand-written rather than pulled from a crate because `std`'s `DefaultHasher`
//! documents that its algorithm may change between Rust releases, which would
//! silently invalidate every hash already computed. Nothing here needs to resist
//! an adversary — the server has no authentication in the first place.

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a 64-bit over `bytes`, formatted as 16 lowercase hex characters.
pub(crate) fn fnv1a64_hex(bytes: &[u8]) -> String {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_is_16_hex_chars() {
        let h = fnv1a64_hex(b"anything");
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn empty_input_still_produces_a_real_hash() {
        // Never the empty string -- that value is reserved as the
        // not-yet-computed sentinel on the `path_hash`/`content_hash` columns.
        let h = fnv1a64_hex(b"");
        assert_eq!(h.len(), 16);
    }

    #[test]
    fn different_bytes_differ() {
        assert_ne!(fnv1a64_hex(b"a"), fnv1a64_hex(b"b"));
    }
}
