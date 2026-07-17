<h1 align="center">mdview</h1>

<p align="center">
  <strong>The markdown viewer built for the docs your AI agent actually writes.</strong>
</p>

<p align="center">
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
  <img alt="Built with Rust" src="https://img.shields.io/badge/built%20with-Rust-orange?logo=rust&logoColor=white">
  <img alt="Single binary" src="https://img.shields.io/badge/single-binary-brightgreen">
  <img alt="Works with Claude Code" src="https://img.shields.io/badge/works%20with-Claude%20Code%20(MCP)-8A63D2">
</p>

<p align="center">
  One command turns a sprawling, multi-folder pile of markdown into a fast, linked,
  live-reloading site in your browser — <strong>cross-folder links that never 404</strong>,
  full-text search, Mermaid diagrams you can zoom, and a one-call hook so your AI agent
  can open any doc it just wrote.
</p>

<!-- ▶ HERO DEMO — add docs/assets/hero-demo.gif (or .mp4), then uncomment:
<p align="center">
  <img src="docs/assets/hero-demo.gif" alt="mdview turning a project's markdown into a linked, live-reloading site" width="820">
</p>
-->

---

## Why mdview?

AI coding agents generate docs like a firehose: nested folders, `../src/api/README.md`
links, Mermaid diagrams, tables, long code blocks. Open that in a typical single-folder
viewer and half the links 404, there's no search, and every edit means a manual refresh.

mdview indexes the **whole project** — at any folder depth — rewrites every internal link
into its own URL namespace, and live-reloads on save. The docs your agent generates just… work.

| | |
|---|---|
| 🔗 **Nothing 404s** | Every internal link across every folder is resolved into one URL namespace. Click straight through `../`, `./sub/`, anchors — no dead ends. |
| ⚡ **Live reload** | A filesystem watcher pushes changes over WebSocket. Save on disk, the page updates itself. |
| 🔍 **Find anything** | Full-text search across the entire project (SQLite FTS5) plus fuzzy file-jump. |
| 📊 **Diagrams that move** | Mermaid renders client-side with pan / zoom / fullscreen — and pinch-to-zoom on mobile. |
| 📋 **Copy-ready code** | Syntax-highlighted code blocks with a one-tap copy button. |
| 🤖 **Agent-native** | A single MCP tool, `mdview_view_file`, hands your agent a clickable URL the moment it writes a doc. |
| 📱 **Read anywhere** | Responsive layout, mobile sidebar drawer, light & dark. Browse from your phone over the LAN or an SSH tunnel. |
| 🦀 **One binary** | Written in Rust. No runtime, no Node, no Docker. Install and go. |

---

## See it

<!-- ▶ SCREENSHOTS — see "Media checklist" at the bottom for exact shots/sizes. -->
<table>
  <tr>
    <td width="50%"><img src="docs/assets/shot-reading.png" alt="Reading view: sidebar file tree, rendered doc, on-this-page TOC"><br><em align="center">Reading view — file tree, rendered doc, live TOC</em></td>
    <td width="50%"><img src="docs/assets/shot-search.png" alt="Full-text search results across a project"><br><em>Project-wide full-text search</em></td>
  </tr>
  <tr>
    <td><img src="docs/assets/shot-mermaid.png" alt="Mermaid diagram with zoom controls"><br><em>Mermaid with pan / zoom / fullscreen</em></td>
    <td><img src="docs/assets/shot-mobile.png" alt="mdview on a phone with the sidebar drawer open"><br><em>Mobile — sidebar drawer, pinch-zoom diagrams</em></td>
  </tr>
</table>

---

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/vantt/mdview/main/install.sh | sh
mdview doctor --fix     # wire up Claude Code MCP integration
```

Or from source (needs Rust):

```sh
cargo install --git https://github.com/vantt/mdview mdview
```

---

## Use in 30 seconds

```sh
mdview open docs/architecture.md
```

That's it. The daemon **auto-starts**, indexes the project, resolves the links, and prints
a browser URL. Open <http://localhost:7700> to browse every project; edits on disk
live-reload the page.

**Reading from a remote server over SSH?** Forward the port and browse locally:

```sh
ssh -L 7700:localhost:7700 user@host   # then open http://localhost:7700
```

> mdview can also bind your LAN (`mdview serve --host 0.0.0.0`) to read from a phone or
> another machine. It has **no authentication**, so only expose it on networks you trust —
> details in the [usage guide](docs/usage.md).

---

## Agent integration (MCP)

`mdview doctor --fix` registers an MCP server with Claude Code that exposes a single tool:

- **`mdview_view_file(project_root, relative_path)`** → returns a clickable `url` to the
  rendered markdown, **auto-registering** the project and indexing the file on first use.

Drop the snippet from [`docs/mdview-agents-template.md`](docs/mdview-agents-template.md)
into your project's `AGENTS.md` / `CLAUDE.md`, and your agent will surface a viewable URL
the moment it finishes writing docs.

<!-- ▶ OPTIONAL VIDEO — agent → view_file → browser. See "Media checklist". -->

---

## CLI

```sh
mdview open <file.md>                # print the browser URL (auto-starts the daemon)
mdview register <dir> [--name ...]   # recursive scan + index a project
mdview search "query"                # full-text search (FTS5)
mdview status                        # is the daemon up?
mdview config edit                   # edit ~/.mdview/config.toml in $EDITOR
mdview restart                       # restart the daemon (apply config changes)
mdview doctor [--fix]                # diagnose & repair the integration
mdview serve [--host H] [--port P]   # optional: pre-start / bind a custom address
```

Most commands accept `--json` for scripting. Full reference, SSH workflows, settings, and
the desktop app live in the **[usage guide](docs/usage.md)**.

---

## How it works

One daemon owns the registry (`~/.mdview/registry.db`); browser tabs are just clients. On a
`view_file` call the server auto-creates the project, scans it recursively, indexes the target
file, resolves its links, and returns the URL. A filesystem watcher keeps the index current
and pushes a reload signal over WebSocket.

- **Rendering:** comrak (GFM) → server-side syntect highlight → ammonia sanitize. Mermaid renders client-side.
- **Search:** SQLite FTS5.
- **Safety:** only registered project roots are served; path traversal is guarded and project HTML is sanitized before it's sent.

---

## Status

Actively developed. Core viewer, project-wide search, MCP + CLI + `doctor`, and the mobile
UX are working end-to-end; a native desktop shell (Tauri) is experimental. See
[PRD.md](PRD.md) for the full design.

---

## Credits

mdview is an independent project, but its design leans on ideas and hard-won lessons from two
prior open-source markdown servers. Grateful thanks to both:

- **[mdserve](https://github.com/jfernandez/mdserve)** — Jose Fernandez, MIT. Watcher
  robustness across atomic editor saves, WebSocket reload-signal live reload, the
  pre-render-to-memory pipeline, path-traversal guarding, and port auto-increment on bind conflict.
- **[marky](https://github.com/GRVYDEV/marky)** — GRVYDEV, Apache-2.0. Recursive folder tree
  that respects `.gitignore`, atomic corrupt-resilient settings persistence,
  sanitize-before-serve, and nucleo-backed fuzzy search.

## License

MIT — see [LICENSE](LICENSE).
