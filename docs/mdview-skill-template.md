---
name: mdview
description: View a markdown or docs file in the local mdview browser viewer and return a shareable URL. Use when the user asks to preview/open/render a markdown file, or after writing docs that read better in a browser (long docs, tables, Mermaid diagrams, multi-file doc sets).
---

# mdview

Render a file in the local mdview viewer and hand the user a browser URL. mdview
runs a background daemon that indexes markdown across a whole project and
resolves cross-folder links, so click-through navigation never 404s.

## Input

`/mdview <relative-file-path>` — the file to view, relative to the project root
(or an absolute path). If no path is given, ask which file to open.

## How to produce the URL

Pick the best available method:

1. **MCP tool (preferred)** — if `mdview_view_file` is available, call it with:
   - `project_root`: absolute path to the project root
   - `relative_path`: the file relative to that root

   It returns a `url` (and a `urls` array when the daemon is bound to a wildcard
   host). It auto-registers the project and indexes the file on first use — no
   separate registration step.

2. **CLI fallback** — otherwise run:

   ```sh
   mdview open <absolute-path-to-file>
   ```

   It prints the browser URL(s), auto-starting the daemon if needed.

## Reporting the URL

Tell the user: "You can view this at: `<url>`".

When more than one URL comes back — the daemon is bound to `0.0.0.0` with no
configured `host_name`, so it lists every reachable IP — show all of them and
let the user pick whichever is reachable from their browser. The URL host is a
display value only; the daemon still binds and is health-checked on its real
address.
