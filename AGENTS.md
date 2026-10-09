# mdview

<!-- [unknown] one-line project description - replace me -->

- README.md

<!-- mdview:START -->
## Documentation Viewing (MDView)

After creating or updating any markdown file, make it viewable in ONE call —
no project registration step needed:

### Using the CLI (preferred)

```sh
mdview open --json <absolute-path-to-file.md>
```

It prints JSON with `url` (the short link to share), `urls` (one per reachable
IP when the daemon binds a wildcard host), `long_url`, `long_urls`, `path`,
`code` and `project_id`. Tell the user: "You can view this at: `<url>`".
The project is auto-registered on first use and the daemon is started if needed.
The file is indexed when its URL is opened; the rest of the project is indexed
on the first content search.

### If you have no shell

Call the MCP tool `mdview_view_file` with `project_root` (absolute path to the
project root) and `relative_path` (the file path relative to that root). It
returns the same fields.

### When to render

Spin up a preview for long docs, tables, Mermaid diagrams, multi-file document
sets, or when the user asks to "preview"/"render". Skip it for short, trivial
snippets.
<!-- mdview:END -->
