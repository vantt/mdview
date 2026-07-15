# mdview

Multi-project markdown viewer for AI agent workflows. A local background server
that indexes markdown across a whole project (any folder depth), **resolves
cross-folder links so nothing 404s**, live-reloads on change, and integrates
with Claude Code over MCP.

Built in Rust (single binary). See [PRD.md](PRD.md) for the full design.

## Why

Tools that serve a single folder break the moment a doc links to
`../src/api/README.md`. mdview indexes the entire project and rewrites every
internal link into its own URL namespace, so click-through navigation just
works — exactly the docs structure AI agents tend to generate.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/vantt/mdview/main/install.sh | sh
mdview doctor --fix     # wire up Claude Code MCP integration
```

Or from source:

```sh
cargo install --git https://github.com/vantt/mdview mdview
```

## Use

```sh
mdview register /path/to/project     # recursive scan + index
mdview serve                         # http://localhost:7700
mdview open docs/architecture.md     # print the browser URL
mdview search "deployment"           # full-text search (FTS5)
mdview status                        # is the daemon up?
mdview doctor                        # diagnose integration, --fix to repair
```

Open <http://localhost:7700> to browse projects; click through links across
folders without broken links. Edits on disk live-reload the page.

**On a remote server over SSH?** Forward the port and browse locally:
```sh
ssh -L 7700:localhost:7700 user@host   # then open http://localhost:7700
```

See the **[full usage guide](docs/usage.md)** for SSH workflows, MCP setup,
settings, and the desktop app.

## Agent integration (MCP)

`mdview doctor --fix` registers the MCP server with Claude Code. The single tool
is:

- **`mdview_view_file(project_root, relative_path)`** → returns a browser `url`.
  Auto-registers the project on first use and indexes the file immediately — no
  separate registration step.

Drop the snippet in [`docs/mdview-agents-template.md`](docs/mdview-agents-template.md)
into your project's `AGENTS.md` / `CLAUDE.md` so agents surface a viewable URL
after writing docs.

## How it works

One daemon owns the registry (`~/.mdview/registry.db`); browser tabs are just
clients. On `view_file` the server auto-creates the project, scans it
recursively, indexes the target file immediately, resolves its links, and
returns the URL. A filesystem watcher keeps the index current and pushes a
reload signal over WebSocket.

- **Rendering:** comrak (GFM) → server-side syntect (class-based, theme via CSS)
  → ammonia sanitize. Mermaid renders client-side.
- **Search:** SQLite FTS5.
- **Safety:** only registered project roots are served; path-traversal guarded.

## Status

MVP (Phase 1 + MCP + CLI + doctor), verified end-to-end. Desktop shell (Tauri)
and some UX polish are planned — see [PRD.md](PRD.md) §8 and
[docs/distillery/porting-log.md](docs/distillery/porting-log.md).

## License

MIT
