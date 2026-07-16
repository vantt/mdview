# atelier-css-2 — Rewrite app.css as Atelier-token app-side glue

**Status:** [DONE]

**Outcome:** Replaced the entire contents of `crates/mdview/assets/app.css`
with app-side glue CSS authored only from Atelier Tier-2/3 tokens, covering
the topbar layout, the `.layout` grid (sidebar/content/rightbar), the sidebar
file tree (`.chapter`/`.chap-file`/`.active`), the right TOC + backlinks
panel, and the breadcrumb. No raw hex/rgba/literal font-family remain; raw
px is limited to permitted structural geometry (fixed column widths, the
topbar sticky offset).

**Files touched:** `crates/mdview/assets/app.css`

**Commit:** `46129ea` — `style(atelier-css-2): rewrite app.css as Atelier-token app glue`

**Full trace / evidence:** `.bee/cells/atelier-css-2.json`

## Consults

None.

## Outstanding Questions

- `.jump-overlay`/`.jump-box`/`.jump-input`/`.jump-list`/`.jump-item` (the
  Cmd/Ctrl+K fuzzy-jump palette) and `.mermaid-controls` (pan/zoom buttons)
  are actively assigned by `app.js` but appear in no cell's scope or the
  plan's Test Matrix. After this rewrite they render fully unstyled. Logged
  as friction on the cell trace; needs a decision on whether a follow-up
  glue cell covers them before the feature's manual light/dark E2E pass.
