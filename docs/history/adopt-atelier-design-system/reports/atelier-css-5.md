# atelier-css-5

**Status:** DONE

`build_highlight_css` in `crates/mdview/src/server.rs` now emits a single dark
syntect theme (`base16-ocean.dark`) scoped to both `:root[data-scheme="light"]`
and `:root[data-scheme="dark"]`, per D5. Dropped the `InspiredGitHub` light
branch and the stale `data-theme` attribute (Atelier's code panel is always
dark, so pairing a light theme with the light scheme made code unreadable
there). Added a regression test confirming no bare `:root[data-scheme=...] {`
rule leaks a page-wide background.

**Files touched:** `crates/mdview/src/server.rs`

**Full trace/evidence:** `.bee/cells/atelier-css-5.json`
