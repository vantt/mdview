<h1 align="center">mdview</h1>

<p align="center">
  <strong>Click, read and fix the docs your AI agent writes — even on a remote server.</strong>
</p>

<p align="center">
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
  <img alt="Built with Rust" src="https://img.shields.io/badge/built%20with-Rust-orange?logo=rust&logoColor=white">
  <img alt="Single binary" src="https://img.shields.io/badge/single-binary-brightgreen">
  <img alt="Works with Claude Code" src="https://img.shields.io/badge/works%20with-Claude%20Code%20(MCP)-8A63D2">
</p>

<p align="center">
  Your agent finishes with <code>Wrote docs/plans/payment-retry.md</code>, a path you can't click.
  mdview turns it into a link: click it and the doc opens in your browser, rendered for
  comfortable reading, with an Edit button when something's wrong.
  No IDE, no hunting through folders, no extra tool.
</p>

<!-- ▶ HERO DEMO — add docs/assets/hero-demo.mp4 (or .gif), then uncomment:
<p align="center">
  <img src="docs/assets/hero-demo.gif" alt="An agent on a remote server hands back a link; one click opens the doc in the browser, where it is read and edited" width="820">
</p>
-->

---

## Why mdview?

Coding agents increasingly run where the code runs: a dev server over SSH, a cloud box,
a VM, WSL. They write plans, specs and reports there, then hand back a relative path.
To read one, you connect an IDE, dig through the tree, and squint at raw markdown —
tables out of line, Mermaid as code.

mdview runs next to your agent. The agent hands you a URL instead of a path, you click it
from your laptop or phone, and the whole project is there: every cross-folder link
resolved, diagrams drawn, search across all docs, and live reload while the agent keeps writing.

| | |
|---|---|
| 🤖 **A link, not a path** | `mdview open --json <file>` (or the `mdview_view_file` MCP tool when there is no shell) hands your agent a clickable URL the moment it writes a doc. |
| 📱 **Read anywhere** | Responsive layout, mobile sidebar drawer, light & dark. Browse from your laptop or phone over the LAN or an SSH tunnel, behind a login token. |
| ✏️ **Edit in place** | Hit Edit on any doc, fix it in a CodeMirror editor with markdown highlighting, Ctrl+S saves straight to disk and re-renders. Refuses to clobber a file your agent changed meanwhile. |
| 📊 **Diagrams that move** | Mermaid renders client-side with pan / zoom / fullscreen — and pinch-to-zoom on mobile. |
| 📋 **Copy-ready code** | Syntax-highlighted code blocks with a one-tap copy button. |
| 🔗 **Nothing 404s** | Every internal link across every folder is resolved into one URL namespace. Click straight through `../`, `./sub/`, anchors — no dead ends. |
| ⚡ **Live reload** | A filesystem watcher pushes changes over WebSocket. Save on disk, the page updates itself. |
| 🔍 **Find anything** | Full-text search across the entire project (SQLite FTS5) plus fuzzy file-jump. |
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
mdview doctor --fix     # let agents run `mdview open` and install the /mdview skill
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/vantt/mdview/main/install.ps1 | iex
mdview doctor --fix     # let agents run `mdview open` and install the /mdview skill
```

Upgrade later with `mdview update` (`--check` to only look; see [usage](docs/usage.md#upgrading)).
Remove it with `mdview uninstall` (`--purge` also deletes config and index; see [usage](docs/usage.md#uninstalling)).

Or from source (needs Rust):

```sh
cargo install --git https://github.com/vantt/mdview mdview
```

---

## Agent integration (CLI first)

```
mdview doctor --fix
```

Agents call the CLI: `mdview open --json <absolute-path-to-file.md>` prints a JSON object
with `url`, `urls`, `long_url`, `long_urls`, `path`, `code` and `project_id`, auto-registering
the project and starting the daemon. `doctor --fix` adds the Claude Code permission
`Bash(mdview open:*)` to `~/.claude/settings.json` (after a timestamped backup; a settings
file that is not valid JSON is never touched), installs the `/mdview` skill, and syncs the
agent instruction block. It no longer registers the MCP server by default.

MCP is only needed for clients with no shell, chiefly **Claude Desktop**:

```
mdview doctor --fix --mcp
```

This registers a small MCP server (`mdview mcp`, tool **`mdview_view_file(project_root,
relative_path)`**) with Claude Desktop, and with Claude Code, Codex and Antigravity when
detected. It is a thin proxy that runs `mdview` per call, so an `mdview update` takes effect
without reconnecting. With mdview inside WSL, Claude Desktop on Windows is registered
through `wsl.exe`.

Drop the snippet from [`docs/mdview-agents-template.md`](docs/mdview-agents-template.md)
into your project's `AGENTS.md` / `CLAUDE.md`, and your agent will surface a viewable URL
the moment it finishes writing docs.

**Indexing is on demand.** Nothing scans the whole repo in the background: a file is
indexed when its URL is opened (together with its link targets and sibling markdown files),
and a project is fully indexed on its first content search. `mdview refresh` forces a full
sync. Projects idle for 14 days are cleaned up automatically.

**Upgrading.** The registry is rebuilt as `~/.mdview/registry-v4.db` (it is a disposable
cache); `doctor --fix` removes the old `registry.db`. A running daemon from an older build is
restarted automatically the next time the CLI needs it; older builds keep using
`registry.db`.

<!-- ▶ OPTIONAL VIDEO — agent → view_file → browser. See "Media checklist". -->

---

## Use in 30 seconds

```sh
mdview open docs/architecture.md
```

That's it. The daemon **auto-starts**, indexes the file and its neighbours, resolves the links, and prints
a browser URL. Open <http://localhost:7700> to browse every project; edits on disk
live-reload the page.

**From inside an AI agent (Claude Code, Codex, …):** say your agent just wrote
`docs/spec/prd.md`. Two ways to see it:

- **Skill:** run `/mdview docs/spec/prd.md` in the agent's terminal — it replies with a URL to open.
- **Agent wired up (`doctor --fix`)?** No need to ask — the agent runs `mdview open --json`
  itself and hands you the URL right after it
  finishes writing the file.

**Reading from a remote server over SSH?** Forward the port and browse locally:

```sh
ssh -L 7700:localhost:7700 user@host   # then open http://localhost:7700
```

> mdview can also bind your LAN (`mdview serve --host 0.0.0.0`) to read from a phone or
> another machine. Sign-in is required (a login token, auto-generated on first start —
> or Cloudflare Access, if you configure it) — details in the [usage guide](docs/usage.md).

---


## CLI

```sh
mdview open <file.md>                # print the browser URL (auto-starts the daemon)
mdview register <dir> [--name ...]   # register a project (indexed on view/search)
mdview search "query"                # full-text search (FTS5)
mdview status                        # is the daemon up?
mdview config edit                   # edit ~/.mdview/config.toml in $EDITOR
mdview restart                       # restart the daemon (apply config changes)
mdview doctor [--fix] [--mcp]        # diagnose & repair the integration
mdview serve [--host H] [--port P]   # optional: pre-start / bind a custom address
```

Most commands accept `--json` for scripting. Full reference, SSH workflows, settings, and
the desktop app live in the **[usage guide](docs/usage.md)**.

---

## How it works

One daemon owns the registry (`~/.mdview/registry-v4.db`); browser tabs are just clients. On an
`open` / `view_file` call the server auto-creates the project, indexes the target file (plus its
link targets and sibling markdown files), and returns the URL; the whole project is indexed on
its first content search. A filesystem watcher keeps the index current
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
