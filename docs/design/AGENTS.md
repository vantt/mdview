# AGENTS.md — operating contract for this design system

You are an agent (Claude Code or similar) restyling an existing application with
the **Atelier design system**. This file is binding. Read it fully before editing.

The system is *portable by construction*: **one skeleton of markup, themed
entirely through CSS custom properties.** Your leverage comes from honoring that
— never from working around it.

---

## The 4-tier architecture (know which layer you are in)

```
Tier 1  themes/atelier.css   PRIVATE primitives (--_ember, --_slate-700…). Never read these from a component.
Tier 2  contract/contract.css  SEMANTIC tokens (--color-*, --type-*, --space-*, --radius-*, --elevation-*, --motion-*)
Tier 3  contract/contract.css  CHARACTER tokens (--btn-radius, --card-elevation, --chip-radius, --focus-color…)
Tier 4  contract/components.css + patterns/*.css   the .fg-* classes — read ONLY Tier 2 + Tier 3
```

- **Components (Tier 4) and your app code read ONLY Tier 2 + Tier 3 tokens.**
- **Never read a theme's private `--_*` primitives**, and never hard-code a value
  a theme owns (color, radius, weight, font-family, elevation, border-width,
  letter-spacing, tracking).
- The theme (`themes/atelier.css`) is the single place that maps primitives →
  semantic/character tokens and defines the Light (native) + Dark schemes.

`data-scheme` swaps **only the color layer.** Type, radius, weight, elevation,
motion and every Tier-3 character token are identical in Light and Dark — the
theme keeps its personality across both schemes. Do not fork character per scheme.

---

## Hard rules (STRICT authoring constraints)

When writing ANY CSS in this system — including app-side glue and specimen cards —
never write a raw value where a token exists:

| Never write a raw… | Use instead |
|---|---|
| color hex / `rgba()` | a `--color-*` token (derive tints with `color-mix(in srgb, var(--color-*) N%, …)`) |
| `font-size` in px/em | a `--type-<role>-size` (there is a rung for every size) |
| `line-height` / `letter-spacing` on text | the role's `--type-<role>-leading` / `-tracking` |
| `font-family` literal | `--font-body/-display/-mono/-accent` or `--type-<role>-font` / `--num-font` |
| `font-weight` for character | the role's `--type-<role>-weight` (explicit weight only for local emphasis) |
| `border-radius` in px | a `--radius-*` value or the component's Tier-3 `--*-radius` |
| `border-width` 1px/2px | `--border-width-hairline` / `--border-width-strong` |
| `box-shadow` literal | `--elevation-*` or a Tier-3 character token |
| gap / padding between elements | a `--space-*` step (0,4,8,12,16,24,36,56,80) |

**Permitted raw values:** structural geometry not on the scale — fixed component
dimensions (avatar `56px`, kanban column `280px`, chart height `140px`) and
hairline positioning offsets. Prefer a `--space-*` token when the number equals one.

**Prefer the type helper classes** (`.t-display`, `.t-title`, `.t-body`, `.t-label`,
`.t-ui`, `.t-tag`, `.t-figure-lg`, …) — each applies a role's five properties at once.

**Self-check before you finish any CSS file:** grep it for a bare `px` in
`font-size`/`letter-spacing`/`border-*-width`/`border-radius`, and for any `#`
hex or `rgba(`. Each hit must be on the permitted list or converted to a token.

---

## Reuse-first — before writing any new element

Answer in order:

1. Does an existing **core role** (`components.css`) already cover it? → compose it.
   Check `CATALOG.md` / `catalog.json` by concept or alias first (e.g. `AlertBanner`
   → `.fg-banner`, `Combobox` → `.fg-select`, `Snackbar` → `.fg-toast`).
2. Is the concept reusable across ≥2 domains? → promote it to a **core role**, then use it.
3. Genuinely domain-specific only → add it to the right `patterns/<domain>.css`.

Domain patterns **compose** core roles; they do not re-invent buttons/inputs/cards.

---

## Contract-first — the order of every change

If a change introduces any decision a theme could own (a color, radius, weight,
font, elevation…), that decision becomes a **token in `contract.css` first** (with
a neutral default), then it is consumed by Tier-4 CSS and overridden in the theme.
Never write the literal in a component and "extract a token later."

Definition of done for a new role/token, in order:
1. **Contract** — add the token(s) to `contract.css` with a neutral default.
2. **Theme** — set the value in `themes/atelier.css` (both schemes if it is a color).
3. **Tier 4** — add the `.fg-*` skeleton in `components.css` (core) or `patterns/<domain>.css`, reading only Tier 2+3.
4. **Index + demo** — add a `catalog.json` + `CATALOG.md` row and a section to the matching `demo/*.html`.

A role shipped without its catalog row or demo section is **not done**.

---

## The token vocabulary you have (Tier 2 + Tier 3)

- **Surfaces:** `--color-bg` · `-surface` · `-surface-raised` · `-surface-sunken` · `-surface-hover` · `-scrim`.
- **Text:** `--color-text` · `-text-muted` · `-text-subtle` · `-text-disabled` · `-on-action` · `-on-brand` · `-on-dark`.
- **Lines:** `--color-border` · `-border-strong` · `-border-interactive` · `--border-width-hairline/-strong`.
- **Interactive (kept separate — Atelier co-points them at ember):** `--color-action/-hover/-press` · `--color-brand/-tint` · `--color-link/-hover/-visited`.
- **Status (text passes AA on its tint):** `--color-success/-warning/-danger/-info` each with `-tint`.
- **Value direction (numbers):** `--color-positive/-negative` each with `-tint`.
- **Accent-alt set:** `--color-accent-alt-1…-5` (tags, lanes, chart keys).
- **Type roles** (`.t-<role>`): display · title · heading · subheading · heading-sm · body · body-sm · label · caption · micro · ui · tag · figure-sm/-md/-lg. Families: `--font-display/-body/-mono/-accent`. Weight scale: `--weight-regular/-medium/-semibold/-bold`. Digits: `--num-font` + `--num-variant`.
- **Space:** `--space-0…-8`. **Radius:** `--radius-none/-xs/-sm/-md/-lg/-pill`. **Elevation:** `--elevation-none/-sm/-md/-lg`. **Motion:** `--motion-fast/-settle/-open` + eases. **Z-index:** `--z-nav/-popover/-modal/-toast`.
- **Character (Tier 3):** `--btn-radius/-weight/-pad-*` · `--card-radius/-elevation/-border-width/-pad` · `--chip-radius/-weight` · `--input-radius/-height` · `--nav-item-radius` · `--media-radius` · `--status-glow` · `--status-lane-fill/-text` · `--focus-color/-width/-offset` · `--num-font`.

Full details and per-role markup: `CATALOG.md` + the `demo/*.html` galleries.

---

## Do / Don't

**Do**
- Set `data-theme="atelier"` and `class="fg-root"` on the root; toggle `data-scheme` for Light/Dark.
- Reuse `.fg-*` roles; compose domain patterns from core roles.
- Reach for the nearest token; if none exists for a real decision, add it to `contract.css` first.
- Keep `data-comment-anchor` / semantic hooks intact when restructuring.

**Don't**
- Read `--_*` primitives or hard-code hex/px/family/weight/radius/shadow in a component.
- Fork character (radius/weight/type) between Light and Dark — scheme is color-only.
- Invent a new class when an existing role (or an alias in the catalog) already covers it.
- Add a second theme here — this bundle is intentionally Atelier-only.
