# MDView Integration (template)

Copy this snippet into your project's `AGENTS.md` or global `~/.claude/CLAUDE.md`
so agents surface a viewable URL after writing markdown. mdview runs locally at
`http://localhost:7700`.

---

## Documentation Viewing (MDView)

After writing a long markdown doc (tables, Mermaid, multi-file sets), or when
asked to preview one, run `mdview open --json <absolute-path-to-file.md>` and
give the user the `url` from the JSON it prints (all of `urls` when several).
