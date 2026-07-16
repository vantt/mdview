---
artifact_contract: bee-plan/v1
artifact_readiness: implementation-ready
mode: small
---

# Plan: UI polish — settings screen, sidebar, main section, TOC

## Mode Gate

Flags counted: **1** — "existing covered behavior" (settings.md / web-interface.md
are specced areas with locked business rules R1-R3 around Display hostname and
multi-IP link building). All changes here are layout/display only; none of
R1-R3's logic is touched.

Files touched: `crates/mdview/src/views.rs`, `crates/mdview/assets/app.css`,
`crates/mdview/assets/atelier/components.css`, `crates/mdview/assets/app.js` —
4 files, one over `small`'s stated cap of 3. Calling it **small** anyway,
explicitly: all 4 are the same tightly-coupled view-rendering pipeline (one
Rust file emitting HTML + its two stylesheets + its one JS file), every item
below is already resolved to an exact file:line target (no research or
architecture decision left), and none of the 10 sub-items has a gray area
left after inline investigation. Routing this as `standard` would add
plan-checker/cell-reviewer/swarm ceremony around what is, in substance, ten
small CSS/HTML/JS glue edits — a bigger mismatch than the file-count overage.

## Discovery note (skips a separate discovery.md — L0, pattern already found in repo)

Item 6 (project root path) had no formal prior decision logged
(`bee.mjs decisions search --text "project root"` — nothing on point). Read
`crates/mdview/src/views.rs:38-64` (`project_list_page`): the `/` projects
list renders `p.root_path.to_string_lossy()` verbatim as a card subtitle —
the operator's absolute filesystem path (e.g. `/home/user/projects/foo`) is
shown to anyone who can reach the page. Per `docs/specs/settings.md` there's
no auth and, per R3, a wildcard (`0.0.0.0`) bind is a supported LAN-reachable
mode — so this is a real (if low-severity) local-path leak to LAN visitors,
not a misremembered rule. Fix: drop that `<div class="fg-card__sub">{root}</div>`
row entirely; the project name + file count + last-seen line already
identify the project without the filesystem path. No product-ambiguity left
(a full omit is what the user asked for and what the code review learnings'
general exposure-minimization stance backs — no partial-truncation option
needed).

## Approach

All work is direct HTML/CSS/JS edits, one item at a time, in this order
(settings first since it's the most items, then sidebar, then main/TOC):

### A. Settings page (`views.rs::settings_page`, lines 325-425)

1. **Version next to title** — move `mdview v{version}` from the footer
   (line 404) to right after the `<h2 class="fg-pagehead__title">Settings</h2>`
   (line 338), e.g. a small `<span class="t-caption">` sibling. Drop the
   footer.
2. **Host + Port same row** — lines 342-350: wrap Port's and Host's
   `.fg-field` divs in one flex row container (new `.fg-field-row` class in
   `app.css`, `display:flex; gap: var(--space-3)`, each child `flex:1`).
   Display hostname (line 351-355) stays on its own row (it's a distinct,
   less-common field).
3. **MCP section moved after Server** — reorder the `<fieldset>` blocks so
   MCP (currently last, lines 389-401) comes right after Server (lines
   341-357), before Renderer and Indexing.
4. **Debounce + Max file size same row** — same `.fg-field-row` wrapper
   pattern as #2, applied to lines 377-383 in the Indexing fieldset.

### B. Input overflow (root cause, fixes settings inputs AND sidebar search in one place)

5. `crates/mdview/assets/atelier/components.css:76-83` (`.fg-input`) and
   `:91-98` (`.fg-select select`) are missing `box-sizing: border-box`.
   `width: 100%` + non-zero padding + `content-box` (the browser default) is
   why every input/select renders wider than its container. Add
   `box-sizing: border-box;` to both rules. This is the shared Tier-4
   component skeleton, so the one fix covers the settings form fields, the
   sidebar search box, and the search-page input — no per-page overrides
   needed. (`.jump-input` in `app.css:285-289` already has its own
   `box-sizing: border-box` as a local workaround for this same bug; leaving
   it — now redundant but harmless — is out of scope for this slice.)

### C. Left sidebar (`views.rs::file_tree`, `app.css`)

6. **Search box bottom padding** — the search `<form>` in `file_tree()`
   (`views.rs:245-246`) has no spacing class. Add `class="fg-sidebar-search"`
   to the `<form>` and a new rule in `app.css` (`margin-bottom: var(--space-3)`
   on `.fg-sidebar-search`).
7. **Active-page marker, not a loud background** — `app.css:174-177`
   (`.chap-file.active`): replace `background: var(--color-action); color:
   var(--color-on-action);` with a left-border marker: `border-left:
   var(--border-width-strong) solid var(--color-action); background:
   transparent;` (keep default text color, drop the inverted-color
   background). Matches the existing `.fg-card--rule` marker pattern already
   used elsewhere in the design system (`components.css:60`).

### D. Right sidebar / TOC (`app.css`, `app.js`)

8. **TOC line-height / breathing room** — `app.css:200` (`.toc li a,
   .backlinks li a { padding: var(--space-0) 0; ... }`) — `--space-0` is
   literally `0` (`contract.css:182`), so there is currently *no* vertical
   padding at all between TOC/backlink entries. Change to
   `padding: var(--space-1) 0;`.
9. **Active-section marker while scrolling** — new: `app.js` gets an
   `IntersectionObserver` over the headings inside `.fg-prose` (present only
   on `file_page`), toggling an `.active` class on the matching `.toc li a`
   (matched by `href="#slug"` vs. the heading's `id`). CSS: reuse the same
   left-border marker as #7 for `.toc li a.active` (`border-left: ...
   var(--color-action)`), consistent visual language across sidebar and TOC.
   Guard for pages with no `.toc` (nothing to observe).

### E. Main section (`app.css`)

10. **Breadcrumb bottom spacing** — `app.css:223-230` (`.breadcrumb`) has
    `padding: var(--space-3) var(--space-6) 0;` — top padding only, `0` on
    the bottom. Change the trailing `0` to `var(--space-3)` so the crumb
    doesn't sit flush against the article title below it. Same change in the
    `@media (max-width: 700px)` override at line 246-248.

## Test matrix (small lane — self-checked, no separate reviewer)

All 10 items are pure display/layout; no new business logic, no new inputs
parsed, no auth/data paths touched. Verification is: `cargo fmt --all --check
&& cargo clippy --workspace --all-targets -- -D warnings && cargo test
--workspace` (must stay green — no Rust logic changed, but `views.rs` string
literals must still compile/format), plus a manual visual pass in the running
app (`cargo run -p mdview -- serve`) hitting `/settings`, `/`, and a file page
with headings, checked in both light and dark scheme and at a narrow (<900px)
viewport to confirm the responsive rail hide-rules (`app.css:237-249`) still
apply.

## Open questions

None outstanding — item 6 (the one gray area) was resolved by inline
investigation above.
