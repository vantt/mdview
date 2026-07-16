# atelier-css-3 — Re-express views.rs markup with .fg-* roles + concat Atelier bundle into APP_CSS

**Status:** [DONE]

**Outcome:** Re-expressed every HTML builder in `views.rs` (layout,
project list, file page, search, settings, error) onto Atelier `.fg-*`
roles: `.fg-btn`, `.fg-chip`, `.fg-banner`, `.fg-input`, `.fg-select`,
`.fg-field`, `.fg-check`, `.fg-card`, `.fg-prose`, `.fg-reading`,
`.fg-mark`, `.fg-page`/`.fg-pagehead__title`, `.fg-empty`. `<html>` now
carries `data-theme="atelier"` and `class="fg-root"`; both the no-flash
head script and the mermaid init script were retargeted from
`data-theme` to `data-scheme` (light|dark) as the only scheme signal.
`APP_CSS` is now `concat!(contract.css, components.css, editorial.css,
atelier.css, app.css)` in load order.

**Files touched:** `crates/mdview/src/views.rs`

**Commit:** `618c0ae` — `style(atelier-css-3): re-express views.rs markup with Atelier .fg-* roles`

**Full trace / evidence:** `.bee/cells/atelier-css-3.json`

## Consults

None.

## Outstanding Questions

None blocking. Three implementation judgment calls are recorded as
`deliberate_exceptions` in the cell trace (see full trace for detail):

- `topbar()` keeps the app-glue classes shipped by `atelier-css-2`
  (`.topbar`/`.home`/`.crumb`/`.nav-link`/`.theme-toggle`) rather than
  `.fg-nav` — the already-capped `app.css` documents that `.fg-nav` is a
  vertical rail and would break the horizontal topbar layout.
- The two real settings checkboxes use `.fg-check` + `.fg-check__text`
  with the native `<input type="checkbox">` left visible; `.fg-check__box`
  was omitted because it has no `:checked`-bound CSS or JS binding in
  this cell's scope and would otherwise show a permanently-unchecked
  decorative box.
- `.fg-article-title` was not added to `file_page`: `render.rs` keeps the
  document's first H1 inside `page.html`, so a separate title element
  would duplicate it.
