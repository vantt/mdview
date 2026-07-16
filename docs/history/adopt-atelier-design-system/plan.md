---
artifact_contract: bee-plan/v1
artifact_readiness: implementation-ready
mode: standard
---

# Adopt Atelier Design System — Plan

Source of truth: `CONTEXT.md` (D1–D6). This plan implements those decisions; it
does not reinterpret them.

## Mode Gate

**Flags counted (3): cross-platform · existing-covered-behavior · weak-proof.**
- cross-platform — web UI + Tauri desktop splash (D6).
- existing-covered-behavior — the Light/Dark toggle + no-flash script have real
  behavior (`app.js:1-20`, `views.rs:16-24`) that must be preserved (D3).
- weak-proof — the frontend has no automated visual tests; correctness is proven
  by build/test + manual light/dark E2E.

No hard-gate flag (no auth/data-loss/security/external-provider once the external
font fetch is removed — see Discovery). → **standard** (not high-risk). Smaller
modes rejected: >3 files, a real toggle contract to preserve, and multi-file
markup re-expression exceed `tiny`/`small`.

## Discovery (L1 — pattern is in-repo)

The design system is fully specified in `docs/design/`; no external research.
Verified this session:
- `@import` load order is `contract.css → components.css → patterns.css →
  atelier.css` (`docs/design/styles.css`). We include only the **editorial**
  pattern (YAGNI — task/crm/financial/media patterns are irrelevant to a
  markdown viewer).
- `build_highlight_css` (`server.rs:380`) already scopes light/dark syntect
  themes by attribute selector — D5 is just swapping `data-theme` → `data-scheme`.
- `css_asset` serves `views::APP_CSS` (`server.rs:232`); `APP_CSS` is an
  `include_str!` const → the bundle can be concatenated at compile time via
  `concat!(include_str!(…), …)`, no route change.
- **Finding (external fetch):** `atelier.css:17-18` `@import url('https://fonts.googleapis.com/…')`
  pulls Manrope + Space Grotesk from Google Fonts. This violates the CONTEXT.md
  discretion constraint "single link works offline, no external fetches." → strip
  both `@import` lines when vendoring; Manrope falls back to the `system-ui`
  stack already declared in the token (`--font-body: 'Manrope', system-ui, …`).
  Bundling Manrope locally for exact fidelity is a **deferred idea** (see below).

## Approach

Adopt the real Atelier CSS bundle as the served stylesheet (D1), re-express
`views.rs` markup with `.fg-*` roles + the editorial pattern (D2), retarget the
existing toggle onto `data-scheme` (D3), keep only the scheme axis (D4),
coordinate syntect highlighting (D5), and restyle the desktop splash (D6).

**Serving mechanism (Agent's Discretion, resolved):** vendor the four needed CSS
files into `crates/mdview/assets/atelier/`, and make `APP_CSS` a compile-time
concatenation in load order: `contract.css + components.css + editorial.css +
atelier.css + app.css` (app-side glue last). One `/static/app.css` response, no
extra routes, works offline. `app.css` is repurposed from the old design to
**app-side glue only** (Tier-2/3 tokens, no raw hex/px) for chrome with no core
role: the layout grid, sidebar file tree (`.chapter`/`.chap-file`), right panel
(`.rightbar`/`.toc`/`.backlinks`), breadcrumb, topbar layout.

**Component mapping (execution reads `CATALOG.md` + demos for exact classes):**
project cards → `.fg-card`; markdown `<article>` → editorial prose role; search
box + settings inputs/selects/textarea → `.fg-field`/`.fg-input`/`.fg-select` +
`.fg-button`; topbar → nav role; status tags (`restart`) → chip/tag role;
`banner` → `.fg-banner`. App-specific chrome → glue CSS.

### Risk map

| Component | Risk | Proof |
|---|---|---|
| Atelier vendoring + concat (`APP_CSS`) | LOW | `cargo build` resolves `include_str!`; grep vendored `atelier.css` has no `https:` |
| App-side glue CSS (authored) | MEDIUM | self-check: no raw hex/`px` in font-size/border/radius (AGENTS.md); manual light+dark render |
| `views.rs` `.fg-*` re-expression | MEDIUM | `cargo test --workspace` (escaping tests must still pass); manual render of every page |
| Toggle retarget (`app.js` + no-flash) | MEDIUM | manual: OS default on first load, button toggles + persists, no flash |
| Syntect scheme swap (`server.rs`) | LOW | `cargo test --workspace` (scope_css/highlight tests) |
| Desktop splash (D6) | LOW | `cargo build -p mdview-desktop`; manual: no color jar before webview |

MEDIUM items are visual — proven by manual light/dark E2E during validating, not
by unit tests. No HIGH unknowns; no spike needed.

## Current Slice — the whole design swap (atomic)

Partial states render broken (new markup on old CSS, or vice-versa), so the swap
is one slice. Cells are split by **file ownership** so each compiles independently;
visual correctness is verified together at the end (validating + manual E2E).

| Cell | Owns | Depends on |
|---|---|---|
| `vendor-atelier-css` | `assets/atelier/{contract,components,editorial,atelier}.css` (new) | — |
| `app-glue-css` | `assets/app.css` (rewrite → glue) | — |
| `views-fg-markup` | `src/views.rs` (markup + `APP_CSS` concat + layout head) | `vendor-atelier-css` |
| `toggle-js-scheme` | `assets/app.js` (toggle + mermaid → `data-scheme`) | — |
| `highlight-scheme-prefix` | `src/server.rs` (`build_highlight_css` → `data-scheme`) | — |
| `desktop-splash` | `crates/mdview-desktop/ui/index.html` (D6) | — |
| `atelier-css-7` (added during execution) | `assets/app.css` — restyle `.jump-*` palette + `.mermaid-controls` (dynamically built by app.js, missed by the views.rs-based mapping; css-2 rewrite dropped them) | `atelier-css-2` |

`views-fg-markup` and `highlight-scheme-prefix` both compile the `mdview` crate
but own different files — no write conflict.

## Test Matrix (edge dimensions, visual lane)

- **Scheme:** light, dark, first-load-with-OS-dark, first-load-with-OS-light,
  toggle+reload persistence.
- **Pages:** project list, file view (with + without mermaid), search (empty /
  no-match / results), settings, `error_page` (404).
- **Content:** long markdown (editorial), tables, code blocks (light+dark
  syntect), mermaid diagram (light+dark), backlinks + TOC present/absent.
- **No-JS fallback:** file tree fallback list still renders.
- **Desktop:** splash matches scheme, no flash before webview loads.

## Verification

- Per-cell: `cargo test --workspace` (baseline green this session) + `cargo build`.
- Feature acceptance (validating / verify skill): run the server, load pages in
  light and dark, confirm the Atelier look + working toggle + readable code
  blocks + mermaid in both schemes.

## Deferred Ideas

- **Bundle Manrope locally** (woff2 + `@font-face` served as an asset) for exact
  typeface fidelity offline — deferred to keep this slice bounded and offline-clean;
  system-ui fallback ships now. Appended to `docs/backlog.md` as proposed.
- Exposing the other Atelier axes (accent/density/num-font/typeface) — per D4.

## Open Questions For Validating

- Confirm the editorial pattern's exact prose class name + required wrapper parts
  from `docs/design/contract/patterns/editorial.css` + `demo/editorial.html`.
- Confirm no glue rule needs a new token (if one does, add to `contract.css`
  first per AGENTS.md contract-first — but prefer an existing token).
