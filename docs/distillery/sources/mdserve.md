---
name: mdserve
type: git-repo
url: https://github.com/jfernandez/mdserve
local: upstreams/mdserve
last_analyzed_commit: f84ae3e
last_analyzed_date: 2026-07-15
domains_covered: [rendering, live-reload, http-serving, link-resolution, file-indexing, skills, config-packaging, tooling, safety, ux, docs-style, repo-layout, testing-evals]
---

# mdserve — Feature Index

Rust CLI markdown preview server for AI coding agents (single binary, Axum).
Serves one file or one directory (non-recursive), live-reloads over WebSocket,
renders GFM + Mermaid, ships a Claude Code plugin/skill. Directly named in
mdview's PRD landscape as the closest prior art. Snapshot: `f84ae3e`.

Line anchors live in prose, not `Where:` (checker verifies bare paths).

## skills

### claude-skill-render-heuristics
- **What:** A shipped Claude Code skill telling the agent WHEN to spin up a
  markdown preview vs. when to skip it (short/trivial → skip; long docs, tables,
  Mermaid, multi-file sets, "preview"/"render" requests → serve). Threshold
  ~40–60 lines or complex formatting or multiple review iterations.
- **Where:** `skills/mdserve/SKILL.md`
- **Notable:** The value is the *decision boundary*, not the tool — encoded as a
  reusable heuristic so the agent self-selects when to render. Workflow it
  prescribes: write file → run server `run_in_background` + `--open` → report
  URL → keep editing (auto-reload) → stop the background task when done.
- **Keywords:** preview, render, when to use, background task
- **Seen:** f84ae3e

## config-packaging

### claude-plugin-manifest
- **What:** Claude Code plugin packaging — `plugin.json` (name/version/desc/
  author/repo) + a local `marketplace.json` entry pointing at the same dir, so
  the tool is installable as a plugin bundling the skill.
- **Where:** `.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json`
- **Notable:** Minimal manifest pair is enough to publish a CLI+skill as a
  Claude Code plugin; kebab-case marketplace name was a fixed bug (v1.1.0).
- **Seen:** f84ae3e

### template-embed-at-build
- **What:** `build.rs` runs `minijinja_embed::embed_templates!("templates",
  &[".html"])` to bake all HTML templates into the binary at compile time; no
  runtime file I/O for templates. Template env built once via `OnceLock`
  (`src/app.rs:31`, `src/app.rs:38-43`).
- **Where:** `build.rs`, `src/app.rs`
- **Notable:** Single-binary distribution — template changes require rebuild,
  traded for zero runtime asset dependencies. Release profile: strip+lto+
  codegen-units=1+panic=abort for size (`Cargo.toml`).
- **Seen:** f84ae3e

### multi-channel-install
- **What:** Distribution across install.sh (curl|bash), Homebrew, cargo, Arch
  pacman, and a Nix flake. install.sh detects Linux x86_64/aarch64, resolves an
  install dir (env override → /usr/local/bin → ~/.local/bin → ~/.mdserve/bin),
  downloads the latest GitHub release, checks PATH.
- **Where:** `install.sh`, `flake.nix`
- **Notable:** Nix flake uses fenix + naersk. macOS is routed to Homebrew (native
  build dropped, v0.5.0). Layered install-dir fallback is a clean pattern.
- **Seen:** f84ae3e

## tooling

### cli-surface
- **What:** clap-derive CLI: positional `path` (file or dir), `-H/--hostname`
  (default 127.0.0.1), `-p/--port` (default 3000), `-o/--open` (launch browser).
  File-vs-dir mode chosen by `is_file()`/`is_dir()` on the path
  (`src/main.rs:9-52`).
- **Where:** `src/main.rs`
- **Notable:** Zero-config default (`mdserve file.md` just works). Default host
  is loopback (changed from 0.0.0.0 in v0.4.1 to avoid port conflicts / exposure).
- **Seen:** f84ae3e

### git-cliff-changelog
- **What:** Automated CHANGELOG from Conventional Commits via git-cliff:
  `conventional_commits = true`, feat/fix/doc/perf/refactor/style/test/build/ci/
  chore → grouped sections, merge & release commits skipped, `body.*security` →
  Security group.
- **Where:** `cliff.toml`
- **Notable:** CI enforces conventional commits on PRs (v1.1.0), so the changelog
  stays fully derivable from history.
- **Seen:** f84ae3e

## safety

### path-traversal-guard
- **What:** Image/static requests join the filename onto `base_dir`, canonicalize,
  then verify `canonical_path.starts_with(&base_dir)`; reject (403) if outside
  (`src/app.rs:585-596`). Route pattern also rejects filenames containing `/`.
  Only markdown + known image types are served; anything else 404s
  (`src/app.rs:629-646`).
- **Where:** `src/app.rs`
- **Notable:** Canonicalize-then-prefix-check is the correct traversal defense
  (resolves `..`/symlinks before comparison). Non-recursive design deliberately
  narrows the served surface (author cites security as a reason).
- **Seen:** f84ae3e

## ux

### theme-system-no-flash
- **What:** 5 client themes (Light, Dark, Catppuccin Latte/Macchiato/Mocha) via
  CSS custom properties per `data-theme` (`templates/main.html:51-109`). A 🎨
  modal picker; selection persisted to localStorage. An early synchronous
  `<head>` script applies the saved theme before body render to prevent theme
  flash (`templates/main.html:9-33`). ESC closes the modal.
- **Where:** `templates/main.html`
- **Notable:** No-flash-on-load = read theme from localStorage in a blocking head
  script (added v0.3.0). Mermaid theme is re-derived and diagrams re-rendered on
  theme change (`updateMermaidTheme`, `main.html:579-601`).
- **Keywords:** catppuccin, theme picker, data-theme, FOUC
- **Seen:** f84ae3e

### sidebar-file-nav
- **What:** Directory mode shows a fixed 250px sidebar listing tracked files
  (flat, alphabetical), each a link to `/<filename>`, active file highlighted;
  collapsible to 48px with state persisted to localStorage
  (`templates/main.html:121-264`, `templates/main.html:670-691`). Single-file
  mode: centered 900px, no sidebar. Mode is by user intent (`mdserve dir/` vs
  `mdserve file.md`), not file count (`docs/architecture.md:66-68`).
- **Where:** `templates/main.html`, `docs/architecture.md`
- **Notable:** One template + one code path serve both modes via `show_navigation`
  flag — "unified architecture" is the author's stated core decision. Flat only:
  no folder nesting, no tree hierarchy.
- **Seen:** f84ae3e

### mermaid-client-render
- **What:** Server marks pages needing Mermaid (`mermaid_enabled` when HTML
  contains `class="language-mermaid"`, `src/app.rs:487`); client loads bundled
  `/mermaid.min.js`, transforms `code.language-mermaid` blocks into
  `<div class="mermaid">`, renders with `startOnLoad:false` and a theme mapped
  from the active app theme (`templates/main.html:514-601`).
- **Where:** `templates/main.html`, `src/app.rs`
- **Notable:** Mermaid.js is vendored and embedded (`include_str!`), served with
  an ETag → 304 revalidation, so the ~2.8k-line lib isn't re-downloaded.
- **Seen:** f84ae3e

## rendering

### markdown-render-pipeline
- **What:** `markdown` crate (GFM fork) with `Options::gfm()`,
  `allow_dangerous_html = true`, and frontmatter parsing on
  (`parse.constructs.frontmatter`) so YAML/TOML frontmatter is stripped before
  render (`src/app.rs:161-170`). HTML is rendered ONCE at startup / on change and
  cached in memory per file (`src/app.rs:90-118`); requests never re-parse.
  Syntax highlighting is server-side (the renderer emits pre-highlighted HTML —
  no client Prism/highlight.js).
- **Where:** `src/app.rs`
- **Notable:** Pre-render-to-memory caching is the central performance decision.
  `allow_dangerous_html` is deliberate — agent markdown embeds raw HTML.
- **Keywords:** comrak-alternative, markdown-rs, GFM, frontmatter, pre-render
- **Seen:** f84ae3e

## live-reload

### websocket-live-reload
- **What:** Browser opens a WebSocket to `/ws`; a Tokio `broadcast` channel
  (cap 16) fans file-change events to all clients; server sends a JSON
  `ServerMessage::Reload`; client JS calls `window.location.reload()`. Client
  auto-reconnects 3s after close (`src/app.rs:648-687`,
  `templates/main.html:604-641`). Reload SIGNAL only — full page reload, not
  content diffing (switched from content-push to reload-signal in v0.3.0).
- **Where:** `src/app.rs`, `templates/main.html`
- **Notable:** Reload-signal + server-side re-render is simpler and more robust
  than pushing DOM patches; the reconnect loop survives server restarts during a
  dev session.
- **Keywords:** ws, hot reload, broadcast channel, reconnect
- **Seen:** f84ae3e

## file-indexing

### file-watcher-notify
- **What:** `notify` `RecommendedWatcher` watches `base_dir` with
  `RecursiveMode::NonRecursive` (`src/app.rs:281-297`). Modify/create/rename on a
  tracked/new markdown file → refresh (or add, in dir mode); image change →
  reload. Deletions are explicitly IGNORED to survive editor rename-save cycles
  (neovim writes backup, renames, recreates) (`src/app.rs:200-263`). Debouncing
  relies on notify's event coalescence.
- **Where:** `src/app.rs`
- **Notable:** Ignoring delete events (v0.5.1 fix) is the non-obvious robustness
  lesson — naive watchers 404 mid-save. Non-recursive is a deliberate constraint
  (simpler state + security), and is precisely the gap mdview's PRD calls out.
- **Keywords:** notify, fs watch, atomic save, rename, debounce
- **Seen:** f84ae3e

## http-serving

### unified-http-router
- **What:** One Axum router serves both modes: `GET /` (first file
  alphabetically), `GET /*filename` (markdown or image), `GET /ws` (live reload),
  `GET /mermaid.min.js` (embedded lib) (`src/app.rs:265-308`). Shared
  `Arc<Mutex<MarkdownState>>` state. `CorsLayer::permissive()`. On bind, if the
  port is taken it tries up to 10 sequential ports and reports the one it got
  (`src/app.rs:310-349`).
- **Where:** `src/app.rs`
- **Notable:** Port auto-increment (v1.1.0) removes a common agent-workflow
  papercut (stale server holding 3000). Single unified router for both modes
  mirrors the unified-template decision.
- **Keywords:** axum, router, port retry, CORS
- **Seen:** f84ae3e

### static-asset-and-etag-cache
- **What:** Images served from `base_dir` with extension-guessed Content-Type
  (png/jpg/gif/svg/webp/bmp/ico). Mermaid.js embedded in the binary and served
  with a fixed ETag + `Cache-Control: public, no-cache`; `If-None-Match` yields
  304 Not Modified (`src/app.rs:548-577`, `src/app.rs:579-646`).
- **Where:** `src/app.rs`
- **Notable:** no-cache + ETag = always revalidate but transfer only on change —
  right choice for a bundled asset that changes only across releases.
- **Seen:** f84ae3e

## link-resolution

### flat-filename-url-routing
- **What:** URLs are literal filenames, not hierarchical: `/docs/api.md` on disk
  is NOT reachable — only immediate-dir files at `/<filename>`
  (`src/app.rs:453-471`, `src/app.rs:500-510`). Markdown links are served AS-IS
  with NO rewriting; the renderer emits `<a href>` with original hrefs untouched.
- **Where:** `src/app.rs`
- **Notable:** This is the *deliberate absence* mdview exists to fix: no
  cross-folder link resolution, no link rewriting, no recursive tree. Recording
  it as the baseline mdview must exceed (PRD G2).
- **Keywords:** link resolution, href rewrite, relative links, 404
- **Seen:** f84ae3e

## docs-style

### conventional-commit-conventions
- **What:** Contributor-facing commit discipline documented in two places:
  `CONTRIBUTING.md` (full Conventional Commits type list + subject rules —
  imperative, lowercase, no trailing period, ≤72 chars) and `CLAUDE.md`'s
  "Commits" section (`type: lowercase description`, no scopes, no emojis). The
  CHANGELOG is then git-cliff-generated from those commits (`git cliff -o
  CHANGELOG.md`), and CI enforces the format.
- **Where:** `CONTRIBUTING.md`, `CLAUDE.md`, `.github/workflows/commitlint.yml`
- **Notable:** The doc convention and its enforcement are a closed loop:
  documented rule → commitlint CI gate → git-cliff derivation. Same commit-driven
  changelog family as marky's `keep-a-changelog` but mdserve *generates* it from
  history rather than hand-maintaining sections. Cross-links tooling entry
  `git-cliff-changelog`.
- **Keywords:** conventional commits, commitlint, CONTRIBUTING, changelog generation
- **Seen:** f84ae3e

## repo-layout

### compact-single-file-server
- **What:** Binary-only crate, deliberately compact: `src/main.rs` (CLI parse +
  entry, ~66 lines) and `src/app.rs` (~1711 lines holding the ENTIRE server —
  Axum router, all handlers, `MarkdownState`, markdown rendering, the file
  watcher, AND the inline test module) + `build.rs` (template embed). No `src/
  lib.rs`, no `tests/` dir at HEAD.
- **Where:** `src/main.rs`, `src/app.rs`, `build.rs`
- **Notable:** Opposite of marky's per-concern module split — mdserve keeps one
  fat `app.rs`, matching its "minimal, agent-companion, not a platform" scope
  (converted lib→binary-only in v1.0.0). **Doc drift caveat:** `CLAUDE.md`'s
  "Project structure" still lists `src/lib.rs` (rendering) and
  `tests/integration_test.rs` — neither exists at `f84ae3e` (rendering lives in
  `app.rs:161-170`; tests are inline in `app.rs`). Verified, not inferred.
- **Keywords:** binary crate, single app.rs, thin main, doc drift
- **Seen:** f84ae3e

### claude-md-design-constraints
- **What:** `CLAUDE.md` encodes the project's scope as explicit design
  constraints / non-goals: agent-companion scope (features toward a docs
  platform / configurable server / deployment target are OUT), zero-config,
  non-recursive ("intentional"), pre-rendered-in-memory, minimal client-side JS.
- **Where:** `CLAUDE.md`
- **Notable:** Same scope-guarding pattern as marky's `claude-md-conventions`
  (negative rules keep a viewer from bloating) — **independent convergence across
  both references**, a strong signal for mdview to adopt a written non-goals
  charter. Also documents the canonical build/test commands.
- **Keywords:** non-goals, design constraints, agent-companion scope, CLAUDE.md
- **Seen:** f84ae3e

## testing-evals

### inline-axum-integration-tests
- **What:** 37 tests inline in `src/app.rs` under `mod tests` (`app.rs:690`),
  driving the real router via `axum-test`'s `TestServer` (`TestServer::builder()`
  with the ws feature for WebSocket tests), `tempfile` for on-disk fixtures, and
  `tokio-test`. Coverage includes render output, non-recursive directory scan
  (`app.rs:790-804`), route serving, websocket reload, and path-traversal
  rejection. Dev-deps: `axum-test` (ws), `tempfile`, `tokio-test`.
- **Where:** `src/app.rs`, `Cargo.toml`
- **Notable:** Full HTTP+WS **integration** testing without a separate `tests/`
  crate — the whole server is exercised end-to-end from inside the module.
  Contrast marky (`vitest-happy-dom` for frontend logic + inline Rust `cli.rs`
  unit tests): both test the substance, not UI chrome, but mdserve tests at the
  request/response boundary while marky tests pure functions.
- **Keywords:** axum-test, TestServer, integration test, tempfile, inline mod tests
- **Seen:** f84ae3e
</content>
