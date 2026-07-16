# Atelier Design System — Handoff

A portable, **CSS-only** design system: one skeleton of markup, themed entirely
through CSS custom properties. This bundle ships **one theme — Atelier — in two
standard color schemes, Light and Dark.** Point an existing app's components at
these classes and tokens and it takes on the Atelier look without changing any
markup structure.

> **What these files are.** The CSS under `contract/` + `themes/` is the *real,
> production-ready* stylesheet — not a throwaway mock. The pages under `demo/`
> are reference galleries showing every component rendered in Atelier. Your job
> is to restyle the target app's existing UI using this system's classes/tokens
> (or map its components onto them), in the app's own framework — React, Vue,
> Svelte, plain HTML, whatever it already uses. You ship the CSS; you re-express
> the app's components with the `.fg-*` vocabulary.

**Fidelity: high.** Colors, typography, spacing, radius, elevation and motion
are all final and token-exact. Reproduce them via the tokens — never hard-code
the hex values.

---

## Quick start

```html
<link rel="stylesheet" href="styles.css">
<script src="contract/elements.js"></script>   <!-- optional Web Component companion -->
<html data-theme="atelier" data-scheme="light" class="fg-root">
```

- `data-theme="atelier"` is **required** and the only theme in this bundle.
- `data-scheme="light"` or `"dark"` picks the color scheme. **Light is native.**
- Put `class="fg-root"` on the top element that owns the app (usually `<html>`
  or `<body>`) — it sets the page background, base text color, body font and
  scrollbar colors from tokens.

Switching Light ⇄ Dark is a one-attribute change; **markup never changes.**

Framework integration: in React/Vue/etc., set the attribute on the root element
and toggle `data-scheme` from your theme state. The stylesheet is plain CSS —
import `styles.css` once in your entry (or copy `contract/` + `themes/` into your
assets pipeline).

---

## What's in the box

```
styles.css                  ← link THIS. @import barrel, correct load order.
contract/
  contract.css              Tier 2 (semantic) + Tier 3 (character) token interface + neutral defaults
  components.css            Tier 4 — the 50 core .fg-* roles (button, card, field, table, nav…)
  patterns.css              Tier 4 — barrel that pulls the five domain files
  patterns/
    editorial.css           long-form / markdown reading
    task.css                project / task management (kanban, gantt, lanes…)
    crm.css                 CRM (profile, activity, pipeline, rule builder…)
    financial.css           reports / charts / ledger / cohort
    media.css               audio-video playback (waveform, scrubber, transport, transcript)
  elements.js               optional Web Components (<fg-button>, <fg-chip>, …) — light DOM
  catalog.json              MACHINE role index — class · concept · aliases · variants · parts
  CATALOG.md                the same index, human-readable tables (125 roles)
themes/
  atelier.css               the theme: Tier-1 primitives → Tier 2/3 + Light (native) & Dark scheme
demo/
  core.html … media.html    live galleries per group (open in a browser)
  elements.html             Web Components vs raw markup, side by side
  theme-switcher.js         Atelier-locked switcher (Scheme · Accent · Density · Numbers · Typeface)
AGENTS.md                   ← READ FIRST if you are an LLM agent editing an app with this system
```

Open any `demo/*.html` in a browser to see the components; the palette button
(top-right) toggles Light/Dark and the other axes.

---

## The Atelier look

> "The Scholarly Workshop." A burnt-orange **ember** on **bone white**, warm
> slate neutrals, Manrope throughout. Soft & rounded — pill buttons/chips, 12px
> cards, gentle shadows, a warm ember focus ring. `action = brand = link` are
> deliberately unified: the tool speaks in one warm voice.

**Core palette (read via tokens, do not hard-code):**

| Token | Light | Dark |
|---|---|---|
| `--color-bg` (canvas) | `#fdfbf7` bone | `#1a110c` |
| `--color-surface` | `#ffffff` | `#231810` |
| `--color-surface-sunken` | `#f2eee6` | `#150d09` |
| `--color-text` | `#0f172a` | `#f5efe6` |
| `--color-text-muted` | `#334155` | `#c3b4a5` |
| `--color-border` | `#e2e8f0` | `#33251c` |
| `--color-action` / `-brand` / `-link` (ember) | `#e9590c` | `#e9590c` (link `#f2894f`) |
| `--color-success` | `#15803d` | `#4cb96a` |
| `--color-warning` | `#b45309` | `#d99a2e` |
| `--color-danger` | `#dc2626` | `#ef5350` |
| `--color-info` | `#0369a1` | `#3fa0e0` |

**Type:** Manrope (display + body); numbers ride a mono stack by default.
Display/title = Manrope 800, tight tracking; labels = 700 at `0.14em` (widest eyebrow).
**Character:** buttons/chips `radius-pill`; cards `radius-lg` (12px) + `elevation-sm`;
inputs/nav `radius-md` (8px); focus ring = 2px ember.

---

## Optional axes (all within Atelier)

These are extra `data-*` attributes on `<html>`; each is optional and orthogonal.

| Attribute | Values | Effect |
|---|---|---|
| `data-scheme` | `light` (default) · `dark` | The color scheme — **the headline feature.** |
| `data-accent` | _(Ember, default)_ · `clay` · `honey` | Re-points `--color-action` only. |
| `data-density` | _(default)_ · `compact` | Tighter rows, cards, inputs. |
| `data-num-font` | `mono` (default) · `sans` | Which family the figures use. |
| `data-typeface-set` | _(Manrope, default)_ · `grotesk` · `system` | Swaps the display face / falls back to system fonts. |

If you only need Light/Dark, ignore the rest — the defaults are the Atelier standard.

---

## How to map an existing app's components

Every reusable role has a stable class and a list of **aliases** in
`catalog.json` / `CATALOG.md`. To restyle a component you already have:

1. Search `CATALOG.md` for its name or an alias (e.g. you have an `AlertBanner`
   → it maps to `.fg-banner`; a `Combobox` → `.fg-select`; a `Snackbar` → `.fg-toast`).
2. The row's **Class** is what to author; **See** disambiguates look-alikes.
3. Open the matching `demo/*.html` gallery and the SPEC/CATALOG entry for the
   exact markup (parts, variants, character tokens it reads).
4. Re-express your component with those classes. It will theme automatically.

**Read `AGENTS.md` before making changes** — it is the binding operating
contract (what you may and may not touch, the token rules, how to add a role).
