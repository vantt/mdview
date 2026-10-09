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

**At most one daemon** owns the registry (`~/.mdview/registry-v4.db`). Every
launcher — CLI, MCP, future desktop — coordinates through `~/.mdview/daemon.lock`
(pid + port). No second server ever writes the same registry.

## Areas

- **Registry** — the set of registered projects (id, name, root path,
  timestamps). Projects are created explicitly (`register`) or **implicitly** the
  first time a file under a new root is viewed. Persisted; survives restart.
  The registry file is a disposable cache: its name carries the schema
  generation, and when the schema version differs it is rebuilt from scratch
  (no migrations) while older `registry.db*` files are left to `doctor --fix`.
  A project nobody has used for 14 days is dropped from the registry (files on
  disk untouched), and the database is compacted after large deletes; there is
  no per-file expiry.
- **Indexer** — records each markdown file's relative path, title (first H1 or
  filename), size, and modified time, plus its full text for search. Indexing is
  **viewed-scope**: registering a project or handing out a URL (`open`,
  `view_file`) scans nothing. Opening a page reads and indexes that file once
  and, off the request path, indexes the markdown files it links to and its
  sibling markdown files. A **content search** lazily syncs the whole project
  first (hidden directories included; `.gitignore` and exclude patterns
  respected), one sync at a time per project and skipped when the last one
  finished under 10 seconds ago; unchanged files are not re-read.
  `mdview refresh` is the explicit full sync. Only markdown files inside the
  canonical project root and outside excluded directories are ever indexed or
  rendered — a symlink out of the root is refused.
- **Link resolution** — the defining feature. When rendering a file, every
  internal link is rewritten into the app's URL namespace by resolving it
  (including `../` across folders) against the project's index. Unresolved links
  are left as-is (broken); links to other projects are out of scope.
- **Renderer** — markdown → HTML: GFM, frontmatter stripped, code highlighted
  server-side with class-based styling (theme via CSS, no re-render), mermaid
  marked for client rendering, output sanitized so untrusted agent markdown is
  safe to view.
- **In-place editing** — a file page can be switched into a CodeMirror editor
  over its raw markdown and saved back to disk through the daemon, which
  re-indexes the file in the same step and refuses to overwrite a file that
  changed underneath the editor (see web-interface.md, "Edit in place").
- **Appearance** — one cohesive visual style applied to every page, with a
  Light/Dark color scheme the operator can toggle (OS-default on first load,
  remembered per browser). Scheme swaps only the color layer; the interface is
  fully self-contained (no external appearance assets). See the Appearance spec.
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
- **Live reload** — a filesystem watcher (debounced) watches the directories
  that hold indexed files (non-recursively, picking up new ones as pages are
  indexed), updates the index on change, and pushes a reload signal over
  WebSocket only when the file's content hash actually changed.
- **Search** — full-text (keyword) over the indexed content of a project,
  diacritic-insensitive (including `đ` matching `d`), with excerpts taken from
  the file on disk. The CLI `search --project` syncs first like the web page;
  without `--project` it searches only what is already indexed. File lists for
  the sidebar and the jump palette come from a short-lived cached filesystem
  listing, not the index, so they show every markdown file whether or not it
  has been indexed.
- **Agent integration (MCP)** — a single tool, `mdview_view_file(project_root,
  relative_path)`, that ensures the project exists, ensures the daemon is up, and
  returns a viewable URL; no scan runs, and the file is indexed the moment its
  URL is opened. For agents with a shell the **CLI is the primary path**:
  `mdview open --json <file>` auto-registers the project, auto-starts the
  daemon, and prints the same fields as the MCP tool (including `path`). MCP
  registration is opt-in (`doctor --mcp`).
- **CLI** — `serve` (daemon), plus `register / open / list / search / status /
  refresh / unregister / stop`, `doctor`, and `version` (prints the single-source
  app version, same as `--version`).
- **Installation** — the install script resolves which released version it is
  about to install (a specific requested version, or the latest release) and
  echoes that resolved version to the operator before/while installing, so the
  operator always knows which version they ended up with — the same
  single-source version reported everywhere else (CLI, settings page,
  `/health`).
- **Settings** — view and change the server binding, renderer theme, indexing
  behavior, and MCP transport, from a web page or `serve` CLI overrides.
  Server/Indexing/MCP changes need a restart to take effect. An optional
  display hostname can stand in for the real host/IP in every URL handed to a
  person or an agent, without changing what address the server binds/is
  health-checked on (see the Settings spec, R1) — this is a cross-area link
  into Agent integration and CLI `open`, both of which build their returned
  URL through this substitution.
- **Doctor** — diagnoses and safely repairs setup: config presence, daemon
  health and version, a read-only index-schema check (reporting and, with
  `--fix`, removing legacy registry files), the Claude Code permission
  `Bash(mdview open:*)`, an AGENTS.md/CLAUDE.md mention of mdview's agent tool,
  and the skill (all merged idempotently, with a backup where content already
  existed). MCP registration is checked only with `--mcp`.

## Boundaries (non-goals)

Not a static site generator, editor, or public host. No cross-project link
resolution, no semantic search, no authentication. Read-only: never writes user
files.

## Status

MVP implemented and verified end-to-end (link resolution in served HTML, live
reload, MCP handshake + view_file, doctor --fix). Planned: desktop shell (Tauri),
scoped live-reload, and UX polish (backlinks, TOC, command palette). See PRD.md
§8 and docs/distillery/porting-log.md.
