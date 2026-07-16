# Adopt Atelier Design System — Context

**Feature slug:** adopt-atelier-design-system
**Date:** 2026-07-16
**Exploring session:** complete
**Scope:** Standard
**Domain types:** SEE

## Feature Boundary

Replace mdview's entire custom web UI design (the served `app.css`, the theme
logic in `app.js`, and the HTML markup in `views.rs`) with the **Atelier**
design system shipped in `docs/design/`, re-expressing the app's components with
Atelier's `.fg-*` role vocabulary and tokens. Ends when every rendered page
(project list, file view + markdown article, search, settings, the shared
`error_page`, and the desktop connecting splash) is themed by Atelier and a
Light/Dark toggle works. All server-rendered pages share the `layout()` wrapper,
so theming `layout()` + the components covers `error_page` (`views.rs:394`)
automatically. No new product features, no server/API/routing changes beyond
serving the new stylesheet.

## Locked Decisions

These are fixed. Planning must implement them exactly — cited, never reinterpreted.

| ID | Decision | Rationale |
|----|----------|-----------|
| D1 | Adopt the real Atelier CSS bundle wholesale: copy `docs/design/contract/` (contract.css, components.css, patterns.css + `patterns/*`) and `docs/design/themes/atelier.css` into the app's asset pipeline and serve them as the app stylesheet, replacing the custom `app.css` design entirely. | AGENTS.md states the `contract/`+`themes/` CSS is production-ready; we ship it, not a re-derivation. |
| D2 | Re-express markup with `.fg-*` roles, not a token re-skin of the old classes. Rewrite `views.rs` chrome (topbar→nav, project cards, breadcrumb, search form, settings form, buttons/inputs) onto Atelier core roles; the rendered markdown `<article>` uses the **editorial** pattern. App-specific chrome that has no core role (sidebar file tree, right TOC/backlinks panel) gets minimal app-side glue CSS written **only** from Atelier Tier-2/3 tokens — never raw hex/px/family/weight/radius/shadow. | `docs/design/AGENTS.md` binds: "re-express your components with the `.fg-*` vocabulary." A re-skin would not deliver the look and would violate the token contract. |
| D3 | Preserve the current toggle behavior on Atelier's scheme layer: initial paint follows OS (`prefers-color-scheme`, no-flash inline script), the header button toggles light↔dark, the choice persists in `localStorage`. Drive it via `data-theme="atelier"` (fixed) + `data-scheme="light\|dark"` on the root, plus `class="fg-root"`. | Exactly satisfies the "bật tắt Dark/Light" requirement and keeps existing UX; the current toggle is already effectively light↔dark with OS as the initial default (`app.js:6`). |
| D4 | Expose only the Light/Dark scheme axis. Keep Atelier defaults for every other axis (accent, density, num-font, typeface); no UI is added for them. | User scoped the requirement to Dark/Light only. |
| D5 | Keep syntect class-based code highlighting (served `/highlight.css`, switches without re-render) but coordinate it with the scheme so code blocks read correctly in both Light and Dark. The syntect CSS is a separate stylesheet from the Atelier bundle; markdown code-block containers still read Atelier surface/border tokens. **The syntect-generated `/highlight.css` is machine-generated from a highlighting theme and is exempt from the Atelier no-raw-hex authoring rule** — that rule governs authored components + app-side glue, not generated highlight themes. | Already works and is decoupled from render (`views.rs:3`); only the light/dark coordination is new. |
| D6 | The desktop app is in scope only via its 20-line pre-load splash (`crates/mdview-desktop/ui/index.html`): restyle it so there is no visual jar before the WebView loads the real (now-Atelier) server UI. **This file is a standalone Tauri asset that cannot load the Atelier token bundle, so it is an explicit, narrowly-scoped exception to the no-raw-hex rule:** its inline `<style>` may use literal hex, but those literals must equal Atelier's `--color-bg`/`--color-text` values, and it must follow the OS scheme via a `prefers-color-scheme` media query (Light = bone `#fdfbf7` on `#0f172a`, Dark = `#1a110c` on `#f5efe6`) so its initial paint matches whichever scheme the real page starts in (per D3). No token variables — they would not resolve in this file. The desktop window renders the served web pages, so it inherits Atelier automatically otherwise. | "Replace all our design"; the splash is the only desktop-owned visual surface, and it structurally cannot participate in the token layer. |

### Agent's Discretion

Delegated to planning/implementation, constrained by the decisions above and the
Atelier AGENTS.md hard rules:

- Exact asset-embedding mechanism (concatenate the bundle server-side into one
  `/static/app.css` response vs. serve `styles.css` + parts; keep `@import`
  order from `docs/design/styles.css`). Constraint: single link works offline,
  no external fetches.
- Which specific `.fg-*` core roles and editorial-pattern parts each existing
  component maps to (use `docs/design/CATALOG.md` aliases).
- Whether any new app-side glue token is genuinely needed; if so it is added to
  `contract.css` first with a neutral default (Atelier contract-first rule),
  never hard-coded in a component.
- Mermaid theme coordination (already reads `data-theme`; update to read the
  resolved scheme).

## Terms

| Term | Meaning in this feature |
|------|-------------------------|
| Scheme | The color layer only — `data-scheme="light\|dark"`. Type/radius/weight/elevation are identical across schemes (Atelier: scheme is color-only). |
| Character token (Tier 3) | Atelier's `--btn-radius`, `--card-elevation`, `--focus-color`, etc. — the app's personality; identical in Light and Dark. |
| App-side glue CSS | Small app-specific rules (file tree, right panel) authored only from Atelier Tier-2/3 tokens, for chrome with no matching `.fg-*` core role. |

## Specific Ideas And References

- `docs/design/demo/*.html` — the target look is fully pinned here; `editorial.html`
  is the reference for the markdown reading view, `core.html` for nav/cards/inputs.
- `docs/design/CATALOG.md` / `catalog.json` — role index with aliases for mapping
  existing components to `.fg-*` roles.
- `docs/design/AGENTS.md` — binding authoring contract (4-tier token rules, no
  raw hex/px, contract-first, reuse-first).

## Existing Code Context

From the quick scout only.

### Reusable Assets

- `crates/mdview/assets/app.css` (247L) — current design; its structure/selectors
  are replaced, not extended.
- `crates/mdview/assets/app.js` (386L) — theme toggle (`app.js:1-20`), WebSocket
  live reload, search, mermaid. Toggle logic is retargeted to `data-scheme`;
  reload/search/mermaid logic is preserved.
- `crates/mdview/src/views.rs` (447L) — all page HTML builders; markup is
  re-expressed with `.fg-*` roles.

### Established Patterns

- No-flash theme script inlined in `<head>` (`views.rs:16-32`) — keep the
  pattern, retarget to `data-scheme`.
- Class-based syntect highlighting served at `/highlight.css`, built in
  `server.rs:build_highlight_css` — keep, coordinate with scheme.

### Integration Points

- `crates/mdview/src/server.rs:89-91` — asset routes (`/static/app.css`,
  `/static/app.js`, `/highlight.css`) that serve the stylesheet(s).
- `crates/mdview/src/views.rs:394` `error_page` — user-visible surface rendered
  through the shared `layout()`; themed automatically once `layout()` is Atelier.
- `crates/mdview-desktop/ui/index.html` — pre-load splash (D6).

## Canonical References

- `docs/design/README.md` — quick start, token table, file map.
- `docs/design/AGENTS.md` — authoring contract (must be honored by all new CSS).

## Outstanding Questions

### Resolve Before Planning

None — decisions locked.

### Deferred To Planning

- [ ] Exact per-component `.fg-*` role mapping — answered by reading
  `CATALOG.md` + the matching `demo/*.html` during planning.
- [ ] Whether the syntect light/dark themes need regenerating or just scheme-scoped
  CSS selectors — answered by inspecting `build_highlight_css` (D5 fixes the
  no-hex-rule question; this remaining item is mechanism-only).

## Deferred Ideas

- Exposing the other Atelier axes (accent / density / num-font / typeface) as
  user settings — out of scope per D4; could be a later enhancement.

## Handoff Note

CONTEXT.md is the source of truth. Decision IDs are stable. Planning reads locked
decisions, code context, canonical references, and deferred-to-planning questions.
Validating and reviewing use locked decisions for coverage and UAT.
