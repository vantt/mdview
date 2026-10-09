# Project Rules

<!-- mdview:START -->
## Documentation Viewing (MDView)

After creating or updating any markdown file, make it viewable in ONE call —
no project registration step needed:

### Using the CLI (preferred)

```sh
mdview open --json <absolute-path-to-file.md>
```

Tell the user: "You can view this at: `<url>`", using the `url` field of the
JSON it prints. When `urls` holds more than one link, list them all so the user
can pick the one reachable from their browser. The project is registered and
the daemon started automatically.

### If you have no shell

Call the MCP tool `mdview_view_file` with `project_root` (absolute path to the
project root) and `relative_path` (the file path relative to that root). It
returns the same fields.

### When to render

Spin up a preview for long docs, tables, Mermaid diagrams, multi-file document
sets, or when the user asks to "preview"/"render". Skip it for short, trivial
snippets.
<!-- mdview:END -->
