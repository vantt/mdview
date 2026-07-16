---
date: 2026-07-16
feature: adopt-atelier-design-system
categories: [process, frontend, validation]
severity: medium
tags: [css-rewrite, client-rendered-dom, plan-checker, design-system, theming]
---

# Adopting the Atelier design system — learnings

## What Happened

Replaced mdview's custom web-UI CSS with the vendored Atelier design system:
re-expressed `views.rs` markup with `.fg-*` roles, retargeted the Light/Dark
toggle onto `data-scheme`, and served the bundle + app glue as one concatenated
`/static/app.css`. Executed as 7 cells; the app was driven end-to-end (server up,
pages curled) to confirm the wiring serves correctly.

Two classes of problem were caught **before** they shipped:

1. **A silent visual regression from a markup-only component mapping.** The plan
   derived which components to restyle from the server-rendered markup in
   `views.rs`. But two live widgets — the Cmd/Ctrl+K fuzzy-jump palette
   (`.jump-*`) and the mermaid zoom/pan/fullscreen controls (`.mermaid-controls`)
   — are built **dynamically by `app.js`** (`el.className = "jump-overlay"`,
   `controls.className = "mermaid-controls"`), so they never appear in `views.rs`.
   The `app.css` rewrite dropped their styling entirely, and no cell covered them.
   The glue worker noticed the orphaned selectors mid-execution and flagged it;
   it was fixed by adding cell `atelier-css-7`.

2. **Three real blockers from the adversarial plan-checker**, all confirmed:
   (a) the mermaid init script read `data-theme`, which becomes the fixed literal
   `"atelier"` after the swap → diagrams stuck light in OS-dark; (b) a light
   syntect palette scoped to the light scheme would paint dark tokens on Atelier's
   fixed-dark code panel → unreadable code in Light mode; (c) `cargo test
   --workspace`-only verifies asserted none of the actual change (an assertion is
   not evidence). All three were repaired in the cells before any code was written.

## Root Cause

- The component inventory was built from one of two DOM sources. In this repo the
  rendered DOM comes from **both** `views.rs` (server markup) **and** `app.js`
  (client-injected elements). A mapping that reads only the server side is
  structurally blind to client-built widgets.
- Default verify commands (`cargo test --workspace`) exercise Rust logic, but the
  behavior changed here is in strings/attributes (CSS classes, `data-scheme`,
  concatenation) that no unit test asserts — so a green suite proved nothing about
  the actual change.

## Recommendation

- **When rewriting or replacing app CSS in this repo, inventory class names from
  BOTH `crates/mdview/src/views.rs` AND `crates/mdview/assets/app.js`** (grep
  `className`, `classList.add`, `class="`), not just the server markup. Any class
  the JS assigns is a live surface the CSS must still cover.
- **For a change whose observable effect is a string/attribute (a CSS class, an
  HTML attribute, a concatenation), add a grep-based verify that asserts the exact
  change** (`grep -q 'data-scheme'`, `! grep -q "getAttribute('data-theme')"`),
  alongside the build/test command — never rely on `cargo test --workspace` alone
  to prove a non-logic edit.
- **Keep spending an adversarial plan-checker on visual/frontend features.** Its
  three catches here were all real and all invisible to the unit suite; the manual
  light/dark E2E pass alone would likely have missed the light-mode code-block one.
