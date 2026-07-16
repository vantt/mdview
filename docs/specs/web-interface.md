---
area: web-interface
updated: 2026-07-15
sources: [file-nav-ux]
decisions: [12d62831, 99e8df73]
coverage: partial
---

# Spec: Web interface navigation

The browser chrome shared across every page of the viewer: the top bar that is
always present, and the per-file sidebar used to move between files in a
project. This spec covers navigation and orientation, not the rendered document
content itself.

## Entry Points & Triggers

- Any page (project list, a rendered file, search results, settings, an error
  page) → shows the shared top bar.
- Opening a file's page → shows the chapter sidebar focused on that file's
  folder.
- Clicking "Settings" in the top bar → the settings page.
- Clicking the brand ("mdview") → the project list.

## Data Dictionary

| # | Element | Meaning | Values |
|---|---|---|---|
| 1 | Brand | Always-present link back to the project list | "mdview" |
| 2 | Center slot | Page-specific orientation text in the top bar | a file's `project / path`, "· search", "Settings", or empty |
| 3 | Settings link | Always-present link to the settings page | — |
| 4 | Theme toggle | Always-present light/dark switch (behavior in the Appearance spec) | — |
| 5 | Chapter focus (file pages) | Which single folder the sidebar is currently showing | a folder within the project; starts at the viewed file's folder |
| 6 | Chapter breadcrumb | The ancestor path of the focused folder, each segment selectable | project root → … → focused folder |
| 7 | File label | How a file is named in the sidebar | its title (first H1); the file name when it has no title |

## Behaviors & Operations

### Top bar (all pages)

- **What it shows:** the brand, a page-specific center slot, the Settings link,
  and the theme toggle — on every page without exception (per R1).
- **Afterwards:** from anywhere, the operator can reach Settings and the project
  list in one click.

### Chapter sidebar (file pages) — breadcrumb zoom

- **Triggers:** viewing any file.
- **What it shows:** exactly **one** folder's contents at a time (per R2) — the
  focused folder's immediate subfolders (each selectable to go into it), and the
  files directly in it, each labelled by title. The currently-viewed file is
  highlighted. When the focus is below the project root, an "up one level"
  affordance is shown.
- **Default focus:** the folder containing the file being viewed.
- **Zoom out:** selecting any breadcrumb segment refocuses the sidebar on that
  ancestor folder.
- **Zoom in:** selecting a subfolder refocuses on it.
- **What changes:** refocusing changes only what the sidebar lists — it does not
  navigate or reload. Selecting a *file* opens that file's page normally.
- **Afterwards:** the operator sees a short, folder-scoped list instead of the
  project's entire file list, and can move up or down the folder hierarchy
  without ever seeing the whole tree at once.

### Fuzzy file-jump palette (file pages)

- **Triggers:** pressing the jump shortcut (Cmd+K on macOS, Ctrl+K elsewhere)
  on any file page opens a centered overlay with a single text input; pressing
  it again, or Escape, or clicking outside the box, closes it.
- **What it does:** as the operator types, the project's files are ranked by a
  fuzzy match of the query against each file's **name and path** (not its
  content) and the top matches are listed live, each showing its title and its
  path. This is distinct from full-text search, which matches file *content*.
- **Navigation:** Arrow keys move the highlighted match; Enter opens the
  highlighted file; clicking a match opens it. An empty query shows no matches.
- **Afterwards:** the operator jumps directly to a file by approximate name
  without browsing the sidebar or running a content search.

### Copy as markdown (file pages)

- **Triggers:** selecting text inside a rendered file and copying it (the normal
  copy gesture).
- **What it does:** instead of the rendered HTML/plain text, the clipboard
  receives the **raw markdown** of the source lines the selection spans. The
  granularity is whole source lines of the blocks the selection touches — a
  partial selection inside a block still yields that block's full source lines.
- **Fallback:** copying from outside the rendered article, or from a region that
  maps to no source, behaves as an ordinary copy.
- **Afterwards:** the operator (often an agent) pastes back authorable markdown,
  not rendered output — round-tripping documentation without de-rendering by hand.

### Mermaid diagram zoom / pan / fullscreen (file pages)

- **Triggers:** a rendered file containing a Mermaid diagram; the diagram is
  drawn client-side, then gains interactive controls.
- **What it offers:** hovering a diagram reveals a small toolbar — zoom in, zoom
  out, reset, and fullscreen. The mouse wheel zooms toward the cursor; dragging
  pans; reset restores the original view; fullscreen expands the diagram to fill
  the screen (Escape/toggle exits).
- **Afterwards:** the operator can read a large or dense diagram that would
  otherwise overflow its box, without leaving the page.

## Actors & Access

Not applicable in the role sense — a single local operator in a browser; no
authentication, no distinct roles. A file page's sidebar data is the project's
file list (paths + titles); no other actor consumes it.

## Business Rules

- **R1 (per D 12d62831).** The Settings link (and the theme toggle) appear on
  every page via one shared top bar; no page renders its own divergent header.
- **R2 (per D 99e8df73).** The file-page sidebar shows exactly one folder at a
  time (breadcrumb-zoom), never the project's full flat file list; files are
  labelled by title, and moving between folders is done by zooming the
  breadcrumb in and out, not by scrolling one long list.
- **R3.** The fuzzy file-jump palette ranks files by name/path, never by
  content; it is the "jump to a file I can half-name" affordance and is kept
  distinct from full-text (content) search, which stays a separate results page.
- **R4.** Copying a selection from a rendered file yields the raw markdown of the
  spanned source lines, not the rendered output; the mapping is by source line
  range (block granularity), and a selection that maps to nothing copies normally.

## Edge Cases Settled

- A file at the project root → the sidebar focus is the root; the breadcrumb is
  just the project name and there is no "up" affordance.
- A file whose title is empty or the same as its file name → the file name is
  used as the label.
- A folder containing both subfolders and files → subfolders are listed first,
  then files.
- Without client scripting, the file page still shows the current folder's files
  by title (a reduced, non-zoomable fallback), so navigation is never blank.

## Open Gaps

- The interactive zoom (breadcrumb/subfolder selection) is delivered by client
  scripting; its behavior with scripting disabled is limited to the static
  current-folder fallback above — full parity is not a goal.
- Sort order of files within a folder (currently by label) and of subfolders is
  not a settled product rule, just current behavior.
- Whether search results and the project list should also adopt any of this
  folder-scoped navigation is not decided.

## Visuals

No settled screenshot captured yet — the top bar and chapter sidebar are new
this session; a snapshot under `docs/specs/visuals/web-interface/` is an open
item.

## Pointers (implementation)

- `crates/mdview/src/views.rs` — `topbar()` (shared header), `file_tree`
  (chapter sidebar: ships the file list as JSON + focus data), page functions.
- `crates/mdview/assets/app.js` — chapter renderer (breadcrumb zoom in/out,
  files by title).
- `crates/mdview/assets/app.css` — `.chapter` / `.chap-*` styles.
