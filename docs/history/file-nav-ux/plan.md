---
artifact_contract: bee-plan/v1
artifact_readiness: implementation-ready
mode: small
---

# File-nav UX: settings link on every page + breadcrumb-zoom chapter sidebar

## Scope (locked, D-IDs 12d62831 / 99e8df73)

- **#1** — extract a shared `topbar()` helper (brand + center slot + Settings
  nav-link + theme toggle) and use it on every page, so the Settings link is
  everywhere, not just the project list.
- **#2** — replace the flat full-path file list with a **C2 breadcrumb-zoom
  chapter** sidebar: always show exactly one folder's contents (subfolders +
  files-by-title), a clickable breadcrumb to zoom out, subfolders to zoom in,
  default focus = the current file's folder. Files labelled by `title`
  (fallback basename). Current file highlighted.

## Mode gate

Flags: **existing covered behavior** (file-list + headers) + **multi-domain**
(Rust views + JS asset) = 2. UI-only, no data/API/auth/migration; design fully
locked (no gray areas); 3 files (`views.rs`, `app.js`, `app.css`). → `small`.

## Discovery: L0

All facts gathered from the repo this session:
- Headers are duplicated inline per page; only `project_list_page`
  (views.rs:58) carries the Settings `nav-link`.
- `file_tree` (views.rs:169-192) emits a flat `<ul>` of every file labelled by
  full `rel_path` — the clutter. `IndexedFile.title` (domain.rs:26) already
  exists and is unused in the tree.
- The page already receives the full `files: &[IndexedFile]` slice, so the
  chapter/zoom can be computed client-side with no new routes.
- `app.js` already owns theme toggle + live-reload; adding a small renderer fits.

## Approach

### #1 shared topbar (`views.rs`)
`fn topbar(center: &str) -> String` → `<header class="topbar"><a href="/"
class="home">mdview</a> {center} <a class="nav-link" href="/settings">Settings</a>
{theme_toggle}</header>`. Replace the inline headers in `project_list_page`,
`file_page`, `search_page`, `settings_page`, `error_page` with it (center =
each page's crumb, or empty).

### #2 chapter sidebar (C2)
- `views.rs`: replace `file_tree` with `chapter_sidebar(project, files, active)`
  emitting: the existing search form; a `<nav class="chapter" id="chapter"
  data-current data-pid data-root>`; and a `<script type="application/json"
  id="filelist">[{"p":rel,"t":title},…]</script>` (serialized via serde_json,
  `<` escaped to `<`). SSR a minimal current-folder file list inside
  `#chapter` as a no-JS fallback.
- `app.js`: on load, read `#filelist` + `#chapter` data attrs; render the
  interactive chapter view (breadcrumb from focus segments; immediate subfolders
  as zoom-in buttons; files-in-focus as title links, current highlighted; an
  "↑ up" affordance when focus≠root). Folder/breadcrumb clicks re-render
  client-side (change focus, no navigation); file clicks are normal links.
- `app.css`: `.chapter` styles (breadcrumb, folder row, file row, up row,
  active) reusing existing sidebar tokens.

Risk map:
- `views.rs` header refactor — LOW (pure HTML restructure; visual check).
- chapter JSON plumbing + JS render — MEDIUM (client logic). Proof: build a
  nested-folder project, view a deep file, confirm sidebar shows only that
  folder, breadcrumb zooms out, subfolder zooms in, file links navigate, title
  labels shown, Settings link present on every page.
- `app.css` — LOW.

## Verification

- `cargo build --workspace && cargo test --workspace` — clean.
- E2E in a scratch project with nested folders (e.g.
  `guide/setup/linux.md`, `guide/setup/macos.md`, `guide/intro.md`, `api/rest.md`,
  `README.md`): serve, fetch the file page HTML for the deep file, assert:
  the topbar contains `href="/settings"` (also on search + home pages);
  the sidebar ships the `#filelist` JSON with all files + titles and
  `data-current` = the viewed file. (Interactive zoom is JS — verified by
  inspecting the emitted data + a manual note; the JS logic is deterministic
  from that data.)

## Test matrix (small-lane depth)

- File at project root (dirname empty) → focus = root, breadcrumb = project name
  only, no "up" row.
- Deeply nested file → only its folder shown; breadcrumb has all ancestor
  segments, each zoomable.
- Folder with both subfolders and files → both rendered (folders first).
- Title empty/equal to filename → falls back to basename cleanly.
- A title containing `</script>` or `<` → JSON `<`-escaped, no markup break.
- Settings link present on file, search, settings, error, and home pages.

## Open questions

None — design locked (C2), UI-only.
