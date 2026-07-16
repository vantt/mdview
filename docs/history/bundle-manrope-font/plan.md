---
artifact_contract: bee-plan/v1
artifact_readiness: implementation-ready
mode: small
---

# Bundle Manrope offline (PBI-09)

PBI-08 stripped the external Google-Fonts `@import` (offline constraint), leaving
Manrope to fall back to system-ui. This embeds it. 1 flag, 2 files → small.

## Approach

Fetch Manrope woff2 (weights 400/500/600/700/800) for the **latin + vietnamese**
subsets only (the app's UI languages; skip cyrillic/greek/latin-ext to stay
lean), base64-embed as `data:` URIs in a vendored `assets/atelier/fonts.css`, and
concat it first in `APP_CSS`. Result: ~226 KB of embedded font, no external
fetch, family resolves for the existing `--font-*` tokens.

## Cell

`bundle-manrope-font-1` — `assets/atelier/fonts.css` (new) + `views.rs` concat.

## Verification

`cargo test --workspace` green; served `/static/app.css` carries embedded
`@font-face` data-URIs and no external font URL (live-confirmed, 343 KB).

## Deferred

Cyrillic/Greek/extended-Latin coverage falls back to the system font — add more
subsets only if a real need appears.
