# mdview

<!-- BEE:BACKLOG-BADGES:START -->
![backlog done](https://img.shields.io/badge/backlog%20done-10-brightgreen) ![backlog in-flight](https://img.shields.io/badge/backlog%20in--flight-0-blue) ![backlog proposed](https://img.shields.io/badge/backlog%20proposed-6-lightgrey)
<!-- BEE:BACKLOG-BADGES:END -->

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
mdview open docs/architecture.md     # print the browser URL (starts the daemon if needed)
mdview register /path/to/project     # recursive scan + index
mdview search "deployment"           # full-text search (FTS5)
mdview status                        # is the daemon up?
mdview doctor                        # diagnose integration, --fix to repair
mdview serve                         # optional: pre-start the daemon (or set host/port)
```

The daemon **auto-starts** on the first `open` or MCP call — you don't need to
run `mdview serve` first. Run `serve` only to pre-start it or to bind a custom
host/port (`mdview serve --host 0.0.0.0 --port 7700`).

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

## Credits

mdview is an independent project, but its design leans on ideas and hard-won
lessons from two prior open-source markdown servers. Grateful thanks to both:

- **[mdserve](https://github.com/jfernandez/mdserve)** — Jose Fernandez, MIT.
  Watcher robustness across atomic editor saves, WebSocket reload-signal live
  reload, the pre-render-to-memory pipeline, path-traversal guarding, and port
  auto-increment on bind conflict.
- **[marky](https://github.com/GRVYDEV/marky)** — GRVYDEV, Apache-2.0.
  Recursive folder tree that respects `.gitignore`, atomic corrupt-resilient
  settings persistence, sanitize-before-serve, and nucleo-backed fuzzy search.

What mdview took are these design ideas, adapted into its own Rust
implementation; the per-feature record lives in
[docs/distillery/porting-log.md](docs/distillery/porting-log.md).

## License

MIT — see [LICENSE](LICENSE). The upstream projects credited above are MIT
(mdserve) and Apache-2.0 (marky), both compatible with mdview's MIT license.
