# System Overview

Technology-agnostic description of what mdview does and how its areas fit
together. First read for anyone new to the repo. (Implementation: Rust; this
spec avoids code detail — see PRD.md for design and crates/ for code.)

## What it is

mdview is a local background server that makes a project's markdown viewable in
a browser with **working cross-folder links**, live reload, full-text search,
and a one-call agent integration over MCP. One daemon owns all state; browser
tabs (and, later, a desktop window) are clients of it.

## Core invariant

**At most one daemon** owns the registry (`~/.mdview/registry.db`). Every
launcher — CLI, MCP, future desktop — coordinates through `~/.mdview/daemon.lock`
(pid + port). No second server ever writes the same registry.

## Areas

- **Registry** — the set of registered projects (id, name, root path,
  timestamps). Projects are created explicitly (`register`) or **implicitly** the
  first time a file under a new root is viewed. Persisted; survives restart.
- **Indexer** — recursively scans a project root (respecting `.gitignore` and
  exclude patterns), recording each markdown file's relative path, title (first
  H1 or filename), size, and modified time, plus its full text for search.
  Steady state is **incremental** (per file-change event); a full re-scan
  reconciles drift.
- **Link resolution** — the defining feature. When rendering a file, every
  internal link is rewritten into the app's URL namespace by resolving it
  (including `../` across folders) against the project's index. Unresolved links
  are left as-is (broken); links to other projects are out of scope.
- **Renderer** — markdown → HTML: GFM, frontmatter stripped, code highlighted
  server-side with class-based styling (theme via CSS, no re-render), mermaid
  marked for client rendering, output sanitized so untrusted agent markdown is
  safe to view.
- **Web interface** — a project list, and per-file pages with a file tree,
  themed rendering, and live reload. Non-markdown assets (images referenced
  from a rendered file, or any other file inside a registered project) are
  served from disk only when the file's extension is on a fixed, short
  allowlist of media types (the same types the renderer already recognizes for
  content-type detection: image formats and PDF) and the file is not inside a
  directory excluded from indexing; anything else — including dotfiles,
  extensionless files, and files in an excluded directory — is refused. This
  is on top of the existing path-traversal guard (a request can never resolve
  outside the project root, symlinks included).
- **Live reload** — a filesystem watcher (debounced) updates the index on change
  and pushes a reload signal over WebSocket; the browser reloads the page.
- **Search** — full-text (keyword) across a project or all projects.
- **Agent integration (MCP)** — a single tool, `mdview_view_file(project_root,
  relative_path)`, that ensures the project exists, indexes the file, ensures the
  daemon is up, and returns a viewable URL.
- **CLI** — `serve` (daemon), plus `register / open / list / search / status /
  refresh / unregister / stop`, and `doctor`.
- **Settings** — view and change the server binding, renderer theme, indexing
  behavior, and MCP transport, from a web page or `serve` CLI overrides.
  Server/Indexing/MCP changes need a restart to take effect. An optional
  display hostname can stand in for the real host/IP in every URL handed to a
  person or an agent, without changing what address the server binds/is
  health-checked on (see the Settings spec, R1) — this is a cross-area link
  into Agent integration and CLI `open`, both of which build their returned
  URL through this substitution.
- **Doctor** — diagnoses and safely repairs setup: config presence, daemon
  health, Claude Code MCP registration, and an AGENTS.md/CLAUDE.md mention of
  mdview's agent tool (all merged idempotently, with a backup where content
  already existed).

## Boundaries (non-goals)

Not a static site generator, editor, or public host. No cross-project link
resolution, no semantic search, no authentication. Read-only: never writes user
files.

## Status

MVP implemented and verified end-to-end (link resolution in served HTML, live
reload, MCP handshake + view_file, doctor --fix). Planned: desktop shell (Tauri),
scoped live-reload, and UX polish (backlinks, TOC, command palette). See PRD.md
§8 and docs/distillery/porting-log.md.
