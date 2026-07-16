---
area: appearance
updated: 2026-07-16
sources: [adopt-atelier-design-system]
decisions: [D1, D2, D3, D4, D5, D6]
coverage: full
---

# Spec: Appearance and color scheme

How the viewer looks: one cohesive visual style applied to every page, and the
Light/Dark color scheme the operator can switch between. This spec covers the
presentation layer and the scheme control — not what any individual page shows
(navigation is the Web interface spec; document rendering is the Renderer).

## Entry Points & Triggers

- Loading any page → the whole interface is presented in one consistent visual
  style, in the operator's current color scheme.
- First load with no remembered choice → the scheme follows the operating
  system / browser preference, applied before the page paints (no flash of the
  wrong scheme).
- Clicking the scheme toggle in the top bar → flips between Light and Dark.
- Returning later (same browser) → the last chosen scheme is restored.
- Launching the desktop app → its brief "connecting" loading screen is shown in
  the scheme that matches the operating-system preference.

## Data Dictionary

| # | Element | Meaning | Values |
|---|---|---|---|
| 1 | Color scheme | The active color palette for the whole interface | `light` (native) · `dark` |
| 2 | Scheme source (first load) | What decides the scheme when nothing is remembered | the operating-system / browser light-or-dark preference |
| 3 | Remembered scheme | The operator's explicit choice, kept for next time | `light` · `dark` · unset (follow the OS preference) |
| 4 | Visual style | The single design language every page shares | one fixed style — a warm, editorial look: rounded controls, soft elevation, one accent color used for actions, links, and focus alike |
| 5 | Reading style | The presentation of a rendered markdown document | a long-form "editorial" reading style (measured column, styled headings, lists, tables, quotes, callouts) |
| 6 | Code panel | The background a fenced code block sits on | a fixed dark panel, identical in both schemes |

## Behaviors & Operations

### One consistent style across every page

- **What it shows:** the project list, file pages, search, settings, and error
  pages all draw from the same visual style (per R1) — the same colors, spacing,
  type, corner rounding, elevation, and focus treatment. No page invents its own
  look.
- **Afterwards:** the interface reads as one product; a component looks the same
  wherever it appears.

### Switching Light ⇄ Dark

- **Triggers:** clicking the scheme toggle.
- **What changes:** only the color palette swaps — Light ⇄ Dark (per R2). Layout,
  type, spacing, corner rounding, and elevation are identical in both schemes;
  the interface keeps its personality across the switch.
- **First-load default:** with nothing remembered, the scheme matches the OS
  preference and is applied before the page is visible, so there is no flash of
  the wrong scheme (per R3).
- **Persistence:** an explicit toggle is remembered in the browser and restored
  on the operator's next visit (per R3).
- **Side effects:** any diagram on the page and the code-highlight colors follow
  the active scheme immediately.
- **Afterwards:** the operator reads in their preferred scheme; the choice sticks
  until they change it.

### Code blocks always on a dark panel

- **Triggers:** viewing a rendered file that contains fenced code.
- **What it shows:** code blocks sit on a fixed dark panel with a code-color
  palette designed for a dark background, in **both** Light and Dark schemes
  (per R4) — a deliberate signature of the visual style, not a bug.
- **Afterwards:** code is legible in either scheme; inline code (not a block)
  still follows the scheme normally.

### Offline appearance

- **What it guarantees:** the interface renders fully without any network access
  — no fonts, styles, or assets are fetched from the internet (per R5). The
  preferred display typeface ships **embedded** in the stylesheet (Latin +
  Vietnamese coverage, all weights), so the intended typography renders offline
  and identically on every machine, no local install required. Scripts outside
  the embedded coverage fall back to the system sans-serif.

### Desktop loading screen

- **Triggers:** launching the desktop app before its window has loaded the web
  interface.
- **What it shows:** a minimal "connecting to the local server" screen whose
  background and text match the operating-system color scheme (per R6), so there
  is no color jump when the real interface finishes loading.

## Actors & Access

A single local operator in a browser or the desktop window; no roles, no
authentication. The scheme choice is per-browser (local), not shared or synced.

## Business Rules

- **R1 (per D1, D2).** Every page shares one design system; its colors, type,
  spacing, rounding, elevation, and focus come from one shared token vocabulary,
  never hard-coded per page.
- **R2 (per D3).** Switching scheme changes only the color layer; all other
  visual character (type, spacing, rounding, elevation) is identical in Light and
  Dark.
- **R3 (per D3).** With no remembered choice the scheme follows the OS preference,
  applied before first paint; an explicit toggle is persisted per browser and
  restored on return.
- **R4 (per D5).** Fenced code blocks render on a fixed dark panel in both
  schemes, with a dark-background code-color palette; this is intentional.
- **R5 (per D1).** The interface is fully self-contained: it makes no external
  network requests for fonts or other appearance assets. A preferred display
  typeface that is absent locally degrades gracefully to a system font.
- **R6 (per D6).** The desktop loading screen follows the OS color preference so
  its scheme matches the interface that loads after it.
- **R7 (per D4).** Only the Light/Dark scheme is exposed to the operator; no other
  appearance axis (accent, density, typeface) is offered as a control.

## Edge Cases Settled

- Preferred display typeface not installed → text uses the system default
  sans-serif; the rest of the look is unchanged (per R5).
- A page starting in OS-dark → both diagrams and code highlighting start dark on
  first paint, matching the page (no light-then-dark flash) (per R3).
- Inline code inside prose (not a fenced block) → follows the scheme, unlike
  fenced blocks which are always dark (per R4).
- A rendered diagram (mermaid) is not code: it renders on a normal
  scheme-appropriate surface, never on the fixed dark code panel, and keeps its
  zoom/pan/fullscreen controls.

## Open Gaps

- The Settings page still exposes a separate "Theme" value (system/light/dark) as
  a saved configuration field; the live per-browser scheme toggle is the actual
  control the pages honor. Whether the saved Settings value should seed the
  first-load scheme (instead of only the OS preference) is not settled — the two
  are currently independent.
- The preferred display typeface is embedded for Latin + Vietnamese; other
  scripts (Cyrillic, Greek, extended Latin) fall back to the system font.
- No settled appearance snapshots are captured yet; Light and Dark reference
  screenshots under `docs/specs/visuals/appearance/` are an open item.

## Pointers (implementation)

- `crates/mdview/assets/atelier/` — vendored Atelier design system (contract
  tokens, core roles, editorial pattern, theme with Light + Dark schemes).
- `crates/mdview/assets/app.css` — app-side glue authored from Atelier tokens.
- `crates/mdview/src/views.rs` — `layout()` sets `data-theme="atelier"` +
  `class="fg-root"` and the no-flash scheme script; `APP_CSS` concatenates the
  bundle + glue; `.fg-*` roles across the page builders.
- `crates/mdview/assets/app.js` — scheme toggle + mermaid re-init, keyed off
  `data-scheme`.
- `crates/mdview/src/server.rs` — `build_highlight_css` emits the dark syntect
  palette (`base16-ocean.dark`) scoped by `data-scheme`.
- `crates/mdview-desktop/ui/index.html` — desktop splash, `prefers-color-scheme`.
