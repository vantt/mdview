# MDView Integration (template)

Copy this snippet into your project's `AGENTS.md` or global `~/.claude/CLAUDE.md`
so agents surface a viewable URL after writing markdown. mdview runs locally at
`http://localhost:7700`.

---

## Documentation Viewing (MDView)

After creating or updating any markdown file, make it viewable in ONE call —
no project registration step needed:

### Using MCP (preferred)

Call `mdview_view_file` with:

- `project_root`: absolute path to the project root
- `relative_path`: the file path relative to that root

It returns a browser `url`. Tell the user: "You can view this at: `<url>`".
The server auto-registers the project on first use and indexes the file
immediately.

### Using CLI fallback

```sh
mdview open <absolute-path-to-file.md>
```

### When to render

Spin up a preview for long docs, tables, Mermaid diagrams, multi-file document
sets, or when the user asks to "preview"/"render". Skip it for short, trivial
snippets.
