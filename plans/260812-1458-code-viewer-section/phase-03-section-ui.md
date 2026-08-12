# Phase 03 — Section UI

Routes, views and CSS for the Code section. Server-rendered throughout; **no new
JavaScript**.

## Routes

Registered in `create_router` (`server.rs:95-111`), before the `/p/:id/*path`
catch-all, same as `_search` / `_jump`:

```rust
.route("/p/:id/_code/",      get(code_root))
.route("/p/:id/_code/*path", get(code_path))
```

`code_path` behaviour:

| Target | Response |
|---|---|
| directory | `code_dir_page` — listing of that directory |
| text file | `code_page` — highlighted source |
| binary | `code_page` with a "binary, N bytes" notice instead of source |
| denied / missing | `not_found("file not found")` — identical message for both, so the response never discloses that a denied file exists |

Every filesystem touch goes through `code_source::resolve_source_path` /
`list_dir` from phase 01. The handler does no path arithmetic of its own.

## Views (`crates/mdview/src/views.rs`)

New:

- `code_page(project, rel_path, highlighted: &HighlightedSource, listing: &DirListing) -> String`
- `code_dir_page(project, listing: &DirListing) -> String`
- `code_tree(project, listing, active: Option<&str>) -> String` — the sidebar
- `section_switch(project, active: Section) -> String` — the Docs|Code control

`code_page` does **not** reuse `file_page` (`views.rs:74`): that function is
bound to `IndexedFile` / `RenderedPage` and carries TOC, backlinks and the
copy-as-markdown source blob, none of which exist here. `layout`,
`topbar_full`, `breadcrumb` and `theme_toggle` *are* reused as-is.

### Sidebar

Mirrors the md sidebar's "always show exactly one folder" model (`app.js:26`),
but rendered server-side from the phase-01 listing:

- a `..` entry when not at the root;
- directories (trailing `/`), then files;
- the currently open file marked `active`;
- each entry is a plain `<a href="/p/:id/_code/…">`.

Reuse the existing `.sidebar` / `.chapter` / `.chap-file` classes so the mobile
drawer JS (`app.js:351`, keyed on `.layout` / `.sidebar` / `#sidebar-toggle`)
works with no changes. Render `sidebar_toggle()` (`views.rs:304`) on code pages.

### Section switch

`topbar_full(lead, center, actions)` (`views.rs:324`) already has the slots
needed — put the switch in `center`, before the breadcrumb. Two links:

- Docs → `/p/:id/` · Code → `/p/:id/_code/`
- the active one gets `aria-current="page"`.

This is the **only** change to the existing markdown pages: `file_page` and
`project_home` gain the switch. No other md behaviour moves.

## Code view markup

```html
<div class="codeview">
  <div class="codeview__head"><span class="codeview__lang">Rust</span>
       <span class="codeview__meta">412 lines · 11 KB</span></div>
  <table class="codeview__table">
    <tr id="L1">
      <td class="codeview__num"><a href="#L1">1</a></td>
      <td class="codeview__line"><code>…</code></td>
    </tr>
  </table>
</div>
```

A table is used because it keeps the gutter and the code perfectly aligned
without fixed line-height maths. Give `.codeview__num` `user-select: none` so
selecting the code does not drag line numbers into the clipboard.

Banners rendered above the table when applicable: truncated-at-cap, binary.

## CSS (`crates/mdview/assets/app.css`)

Append a `.codeview*` block near the existing code styles. Requirements:

- `.codeview__line { overflow-x: auto }` — minified files must scroll in their
  own cell, never widen the page;
- `:target` row highlight so `#L42` is visible on arrival;
- colours come from `/highlight.css` and existing CSS variables — **no new
  palette**, so both themes work for free.

## Files

- modify `crates/mdview/src/server.rs` (2 routes + 2 handlers)
- modify `crates/mdview/src/views.rs` (4 new fns; `file_page` + `project_list_page`/`project_home` gain the switch)
- modify `crates/mdview/assets/app.css` (`.codeview*`, minor `.chapter` reuse)

## Tests

Extend `crates/mdview/tests/e2e_open.rs` (existing harness pattern):

1. `/p/:id/_code/` → 200, contains a known top-level directory name
2. `/p/:id/_code/crates/mdview-core/src/lib.rs` → 200, has `id="L1"` and a syntect token class
3. `/p/:id/_code/.git/config` → 404
4. `/p/:id/_code/../../etc/passwd` → 404
5. denied file and missing file return byte-identical bodies
6. an md file opened through `_code` → renders as highlighted source, not as
   rendered markdown (the two sections stay distinct)
7. Docs page still renders and now carries the section switch

## Risks

- A real project directory literally named `_code` at the root would be shadowed
  by the route. Same pre-existing hazard as `_search`; accept and note in the
  spec rather than adding escaping machinery.
- `breadcrumb` (`views.rs:83`) builds Docs URLs; the code section needs its own
  crumb targets — pass a base prefix rather than duplicating the function.
