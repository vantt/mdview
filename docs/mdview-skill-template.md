---
name: mdview
description: View a markdown or docs file in the local mdview browser viewer and return a shareable URL. Use when the user asks to preview/open/render a markdown file, or after writing docs that read better in a browser (long docs, tables, Mermaid diagrams, multi-file doc sets).
---

# mdview

Render a file in the local mdview viewer and hand the user a browser URL. Links
between markdown files resolve across folders, so click-through navigation
never 404s.

## Input

`/mdview <relative-file-path>` — the file to view, relative to the project root
(or an absolute path). If no path is given, ask which file to open.

## How to produce the URL

Pick the best available method:

1. **CLI (preferred)** — run:

   ```sh
   mdview open --json <absolute-path-to-file>
   ```

   It prints JSON whose `url` is the link to share (plus a `urls` array when
   several hosts are reachable), registering the project and starting the
   daemon if needed.

2. **MCP tool (if you have no shell)** — call `mdview_view_file` with:
   - `project_root`: absolute path to the project root
   - `relative_path`: the file relative to that root

   It returns the same fields.

## Reporting the URL

Tell the user: "You can view this at: `<url>`".

When more than one URL comes back — the daemon is bound to `0.0.0.0` with no
configured `hostname`, so it lists every reachable IP — show all of them and
let the user pick whichever is reachable from their browser. The URL host is a
display value only; the daemon still binds and is health-checked on its real
address.
