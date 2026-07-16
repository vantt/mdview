# atelier-css-7 — Restyle the fuzzy-jump palette + mermaid zoom controls in app.css glue

**Status:** [DONE]

**Outcome:** Appended token-only glue CSS to `crates/mdview/assets/app.css`
for the two client-built widgets `atelier-css-2` missed: the Cmd/Ctrl+K
fuzzy file-jump palette (`.jump-overlay`/`.jump-box`/`.jump-input`/
`.jump-list`/`.jump-item` + `.active`/`.jump-title`/`.jump-path`) and the
mermaid zoom/pan/fullscreen toolbar (`.mermaid-controls` + buttons on
`pre.mermaid.zoomable`, shown on hover/focus/fullscreen). Every value reads
an Atelier Tier-2/3 token; no raw hex/rgba/literal font-family/weight/shadow
were added. `atelier-css-2`'s existing chrome glue is untouched.

**Files touched:** `crates/mdview/assets/app.css`

**Commit:** `d3171a6` — `style(atelier-css-7): restyle jump palette + mermaid zoom controls in app.css`

**Full trace / evidence:** `.bee/cells/atelier-css-7.json`

## Consults

None.

## Outstanding Questions

None.
