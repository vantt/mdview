# mdserve Inventory Report

**Extraction focus:** Frontend UI, architecture, Claude Code integration, packaging.

---

## 1. Front-End Page (templates/main.html)

### Layout & Structure

**Single-file vs. multi-file modes:**
- `templates/main.html:121-264` — Conditional navigation layout via Jinja2 `{% if show_navigation %}` block
- **Multi-file mode (directory):** Fixed 250px sidebar (`:59` var `--sidebar-width`), collapsible to 48px (`:61` var `--sidebar-collapsed-width`)
- **Single-file mode:** Standard centered layout, max-width 900px (`:62`)

**Sidebar structure:**
- `templates/main.html:139-153` — Fixed sidebar, full viewport height (100vh), non-recursive watch
- `templates/main.html:162-164` — Sidebar header reserved for toggle button
- `templates/main.html:670-683` — File list renders via Jinja2 loop: `{% for file in files %} ... {% endfor %}`
- `templates/main.html:673-681` — Links to `href="/{{ file.name }}"` with active state highlighting via `class="active"` when `file.name == current_file`

**Content pane:**
- `templates/main.html:210-223` — Main content area (`#content`), centered with margin calculations, responsive to sidebar collapse state
- `templates/main.html:687-689` — Content injected via Jinja2 template variable: `{{ content }}`

### Live-Reload Wiring

**WebSocket implementation:**
- `templates/main.html:604-634` — `setupLiveReload()` function establishes WebSocket
- `templates/main.html:605-606` — Protocol detection: `wss:` for HTTPS, `ws:` for HTTP
- `templates/main.html:607` — Hardcoded endpoint: `${protocol}//${window.location.host}/ws`
- `templates/main.html:613-619` — Message handler: `message.type === 'Reload'` triggers `window.location.reload()`
- `templates/main.html:629-632` — Auto-reconnect on close: 3-second timeout, re-runs `setupLiveReload()`

**Initialization:**
- `templates/main.html:637-641` — Called on `DOMContentLoaded` event within `setupLiveReload()` invocation

### Mermaid Diagram Rendering

**Conditional inclusion:**
- `templates/main.html:454-456` — Mermaid library loaded via Jinja2 conditional: `{% if mermaid_enabled %}`
- `templates/main.html:455` — Script source: `/mermaid.min.js` (bundled, served by Axum router)

**Initialization and theme management:**
- `templates/main.html:528-554` — `initMermaid()` initializes mermaid with theme, transforms code blocks, renders diagrams
- `templates/main.html:530-546` — Config: `startOnLoad: false`, theme detection via `getMermaidTheme()`, font family system defaults, flowchart/sequence/gantt `useMaxWidth: true`
- `templates/main.html:556-577` — `transformMermaidCodeBlocks()` finds `code.language-mermaid`, extracts text, replaces `<pre><code>` with `<div class="mermaid">`
- `templates/main.html:514-526` — Theme mapping: dark themes (`dark`, `catppuccin-macchiato`, `catppuccin-mocha`) → `'dark'` theme; light themes → `'default'` theme

**Theme-aware re-rendering:**
- `templates/main.html:579-601` — `updateMermaidTheme()` re-initializes mermaid, strips `data-processed` attribute, restores original content, calls `mermaid.run()`

### Syntax Highlighting

**No explicit highlight.js or Prism:**
- `templates/main.html:405-420` — Only CSS styling for `<pre>` and `<code>` tags (background, padding, border-radius, monospace font)
- Syntax highlighting is **server-side only** — markdown-rs renderer produces pre-highlighted HTML; client receives already-rendered HTML
- `templates/main.html:411-416` — Inline code block styling (padding, background, font-family)

### Theme System

**Theme storage and initialization:**
- `templates/main.html:9-33` — Early synchronous script (before body render) applies theme from localStorage
- `templates/main.html:12` — Default theme: `'catppuccin-mocha'`
- `templates/main.html:13` — Sets `data-theme` attribute on root (`<html>`)
- `templates/main.html:18-31` — Sidebar collapse state also restored early via `localStorage.getItem('sidebar-collapsed')`

**Theme picker UI:**
- `templates/main.html:266-286` — Fixed button (top-right), 🎨 emoji icon (`:686`), opens modal on click
- `templates/main.html:288-315` — Modal overlay with 5 theme cards in grid layout
- `templates/main.html:695-749` — Theme cards: Catppuccin Latte (☕), Catppuccin Macchiato (🥛), Catppuccin Mocha (🐱), Light (☀️), Dark (🌙)
- `templates/main.html:324-377` — Card styling: preview swatches, hover transform, selected border highlight

**Available themes (5 total):**
1. Light (`:728-737`)
2. Dark (`:739-748`)
3. Catppuccin Latte (`:695-704`)
4. Catppuccin Macchiato (`:706-715`)
5. Catppuccin Mocha (`:717-726`)

**CSS variables per theme:**
- `templates/main.html:51-109` — Theme-specific CSS custom properties: `--bg-color`, `--text-color`, `--border-color`, `--code-bg`, `--blockquote-color`, `--link-color`, `--table-header-bg`

**Theme selection handler:**
- `templates/main.html:479-485` — `selectTheme(theme)` sets `data-theme` attribute, persists to localStorage, updates UI, updates mermaid theme, closes modal

### Search UI

**No search UI present** — No search input, search button, or search functionality found in template.

### Keyboard Shortcuts

**Implemented shortcuts:**
- `templates/main.html:654-658` — ESC key closes theme modal when `modal.classList.contains('show')`
- No other keyboard shortcuts documented in template

### Table of Contents (TOC)

**No TOC implementation** — No table of contents sidebar, no heading jump navigation, no TOC generation found in template.

### File Tree Navigation

**File list rendering:**
- `templates/main.html:673-681` — Unordered list of file links, sorted alphabetically server-side (architecture doc `:148`)
- `templates/main.html:688-691` — Single-line links with file name only, no tree nesting or folder hierarchy
- Files are **flat, not hierarchical** — non-recursive directory watching (architecture doc `:141`)

---

## 2. Architecture Doc (docs/architecture.md)

### Overview & Core Principle

`docs/architecture.md:3-7` — mdserve is an HTTP server for markdown preview with live reload; supports single-file and directory modes; core principle: always work with base directory + list of tracked files.

### Components & Data Flow

**Architecture diagram (mermaid):**
`docs/architecture.md:9-18` — Data flow: File System → File Watcher → MarkdownState & WebSocket → Browser; HTTP Request → MarkdownState lookup → Template render → HTML response.

### Modes

**Single-File Mode:**
`docs/architecture.md:23-28` — Watches parent directory, tracks single file, no sidebar.
Example: `mdserve README.md`

**Directory Mode:**
`docs/architecture.md:31-36` — Watches specified directory, tracks all `.md` and `.markdown` files, shows sidebar.
Example: `mdserve ./docs/`

### State Management

**MarkdownState structure (class diagram, lines 49-64):**
```
class MarkdownState {
    +PathBuf base_dir
    +HashMap<String, TrackedFile> tracked_files
    +bool is_directory_mode
    +Sender<ServerMessage> change_tx
}

class TrackedFile {
    +PathBuf path
    +SystemTime last_modified
    +String html
}
```

**Mode determination:**
`docs/architecture.md:66-68` — Mode determined by user intent, not file count. `mdserve /docs/` with 1 file shows sidebar; `mdserve single.md` never shows sidebar.

**Example state (single-file):**
`docs/architecture.md:72-77` — base_dir, tracked_files (single entry "README.md"), is_directory_mode = false

**Example state (directory):**
`docs/architecture.md:82-90` — base_dir, tracked_files (multiple entries: api.md, guide.md, README.md), is_directory_mode = true

### Live Reload Mechanism

`docs/architecture.md:92-106` — Uses [notify](https://github.com/notify-rs/notify) crate, non-recursive (immediate directory only):
- Create/modify: Refresh file, add if new (directory mode only)
- Delete: Remove from tracking
- Rename: Remove old, add new
- All changes trigger WebSocket `ServerMessage::Reload` broadcast
- Clients receive message and execute `window.location.reload()`

### Routing

`docs/architecture.md:108-117` — Unified router handles both modes:
- `GET /` → First file alphabetically
- `GET /:filename.md` → Specific markdown file
- `GET /:filename.<ext>` → Images from base directory
- `GET /ws` → WebSocket connection
- `GET /mermaid.min.js` → Bundled Mermaid library
- `:filename` pattern rejects paths with `/` (prevents directory traversal)

### Rendering

`docs/architecture.md:119-133` — MiniJinja (Jinja2 syntax) with templates embedded at compile time via minijinja_embed (template changes require rebuild).

**Conditional rendering:**
- Directory mode: Sidebar + active file highlighting
- Single-file mode: Content only
- Both use same pre-rendered HTML from state

**Template variables:**
- `content` — Pre-rendered markdown HTML
- `mermaid_enabled` — Boolean, conditionally includes Mermaid.js
- `show_navigation` — Controls sidebar visibility
- `files` — List of tracked files (directory mode)
- `current_file` — Active file name (directory mode)

### Design Decisions

`docs/architecture.md:135-143`:
- **Unified architecture:** Single code path handles both modes
- **Pre-rendered caching:** All tracked files rendered to HTML in memory on startup and change; serving always from memory
- **Non-recursive watching:** Only immediate directory, simplifies security and state management
- **Server-side logic:** Most logic server-side (rendering, tracking, navigation, active highlighting, live reload); minimal client JS (theme management, reload execution)

### Constraints

`docs/architecture.md:145-149`:
- Non-recursive (flat directories only)
- Alphabetical file ordering only
- All files pre-rendered in memory

---

## 3. Claude Code Integration

### Plugin Metadata

**plugin.json:**
- `plugin.json:2` — Name: `"mdserve"`
- `plugin.json:3` — Version: `"1.1.0"`
- `plugin.json:4` — Description: "Markdown preview server for AI coding agents. Renders markdown live in the browser with instant reload, Mermaid diagrams, and GFM support."
- `plugin.json:5-8` — Author: Jose Fernandez, email me@jrfernandez.com, MIT License
- `plugin.json:10-11` — GitHub repository and homepage

**marketplace.json:**
- `marketplace.json:1-4` — Owner: Jose Fernandez; owner name only, no email/URL
- `marketplace.json:6-9` — Plugin source: local (same directory)

### Claude Code Skill (skills/mdserve/SKILL.md)

**Purpose (frontmatter):**
`SKILL.md:1-8` — Instructs Claude Code agent when to use mdserve:
- Serve markdown when content is long or likely to be iterated (tables, diagrams, multi-section docs)
- Skip preview for short markdown easy to read in terminal

**When to use (lines 15-29):**
- Plans, proposals, architecture/design documents
- Reports, comparisons, summaries with tables
- Content with Mermaid diagrams
- Multi-file documentation sets
- User requests to "preview" or "render"
- **Threshold:** ~40-60 lines, complex formatting, multiple edit/review iterations

**When NOT to use:**
- Short conversational answers
- Single code snippets
- Trivial one-paragraph responses
- Markdown fitting comfortably in terminal

**Workflow (lines 35-47):**
1. Write markdown file
2. Start mdserve with Bash tool, `run_in_background: true` and `--open` flag
3. Report URL (default: http://127.0.0.1:3000)
4. Continue editing; changes reload automatically
5. Stop background task with TaskStop when finished

**Port conflict handling (lines 49-53):**
mdserve auto-finds available port if 3000 in use; always report URL that mdserve outputs

**Directory mode (lines 55-66):**
Serve parent directory for multiple related files; user gets sidebar for navigation; non-recursive (immediate directory only)

**Mermaid diagrams (lines 68-78):**
Use Mermaid for flowcharts, sequence diagrams, ER diagrams, state diagrams when they improve clarity over plain text

**Installation instructions (lines 80-91):**
Four options if mdserve not found:
1. Install script: `curl -sSfL https://raw.githubusercontent.com/jfernandez/mdserve/main/install.sh | bash`
2. Homebrew: `brew install mdserve`
3. Cargo: `cargo install mdserve`
4. Arch Linux: `sudo pacman -S mdserve`

---

## 4. Packaging & Distribution

### Install Script (install.sh)

**Entry point (line 5):**
`curl -sSfL https://raw.githubusercontent.com/jfernandez/mdserve/main/install.sh | bash`

**Platform detection (lines 87-114):**
- OS detection: Linux only (macOS routed to Homebrew, Windows unsupported)
- Architecture: x86_64 and aarch64 supported; maps to `x86_64-unknown-linux-musl`

**Installation directory resolution (lines 116-149):**
1. `MDSERVE_INSTALL_DIR` env var override
2. `/usr/local/bin` (if writable or root)
3. `$HOME/.local/bin` or `$HOME/bin` (if writable)
4. Create `$HOME/.local/bin` (XDG standard)
5. Fallback: `$HOME/.mdserve/bin`

**Binary download:**
- `lines 161-189` — Fetches latest GitHub release tag via API
- Downloads to temp file via curl or wget
- Copies to install directory with execute permission

**Verification (lines 218-226):**
- Checks binary is executable
- Runs `--version` check (warns if it fails but continues)

**PATH checking (lines 230-239):**
- Warns if install directory not in PATH
- Suggests adding export to shell profile or using full path

### Nix Flake (flake.nix)

**Inputs (lines 2-6):**
- nixpkgs (unstable)
- flake-utils
- fenix (Rust toolchain)
- naersk (Nix Rust build)

**Rust toolchain (lines 21-32):**
- Stable Rust with rustc, cargo, rustfmt, clippy, rust-src, rust-analyzer

**Package build (line 36):**
- `defaultPackage = naersk-lib.buildPackage ./` — Builds from Cargo.toml in repo root

**Dev shell (lines 37-43):**
- Provides Rust toolchain for development

### Changelog Generation (cliff.toml)

**Tool:** [git-cliff](https://git-cliff.org/)

**Conventional commits parsing (line 34):**
- `conventional_commits = true` — Parses commits per [Conventional Commits spec](https://www.conventionalcommits.org/)

**Commit type → group mapping (lines 40-58):**
- `feat` → Features
- `fix` → Bug Fixes
- `doc` → Documentation
- `perf` → Performance
- `refactor` → Refactoring
- `style` → Styling
- `test` → Testing
- `build` → Build
- `ci` → CI
- `chore` → Miscellaneous Tasks
- `body.*security` → Security

**Skip rules (lines 41-42, 52-54):**
- Skip merge commits
- Skip chore(release) and Release commits

**Grouping order (line 12):**
Features, Bug Fixes, Performance, Refactoring, Documentation, Styling, Testing, Build, CI, Miscellaneous Tasks, Security

---

## 5. README Feature List

`README.md:10-26` — Enumerated features:

1. **Zero config** — `mdserve file.md` just works, no config files/flags/setup
2. **Single binary** — Statically compiled executable, install and forget, no runtime dependencies
3. **Instant live reload** — File changes appear immediately via WebSocket (core interaction: agent writes, human reads)
4. **Ephemeral sessions** — Start during coding session, kill when done; not long-running
5. **Agent-friendly content** — Full GFM support (tables, task lists, code blocks), Mermaid diagrams, directory mode with sidebar navigation (content AI agents produce)

**Explicitly NOT:**
- Documentation site generator (use mdBook, Docusaurus, MkDocs)
- Static site server or production deployment target
- General-purpose markdown authoring tool with heavy customization

---

## 6. CHANGELOG by Version

### v1.1.0 (2026-03-07)

**Features:**
- Auto-increment port when requested port in use

**Bug Fixes:**
- Use kebab-case for marketplace name
- Use native background tasks in mdserve skill
- Allow serving images from subdirectories

**Documentation:**
- Add Claude Code plugin installation instructions to README

**CI:**
- Enforce conventional commits on PRs

### v1.0.0 (2026-02-07)

**Features:**
- Add `--open` flag to launch browser
- Add Claude Code plugin metadata and mdserve skill

**Refactoring:**
- Convert to binary-only crate

**Documentation:**
- Rewrite README for AI agent companion focus
- Add CLAUDE.md for AI agent contributors
- Add changelog workflow to CLAUDE.md

**Build:**
- Add crates.io publishing to workflow

**Miscellaneous:**
- Update crate description to match new focus
- Skip release commits in git-cliff output

### v0.5.1 (2025-10-28)

**Bug Fixes:**
- Handle temp-file-rename edits in file watcher

### v0.5.0 (2025-10-23)

**Features:**
- Add directory mode for serving multiple markdown files
- Add YAML and TOML frontmatter support

**Bug Fixes:**
- Center content in folder mode with sidebar collapsed
- Prevent 404 race during neovim saves

**Refactoring:**
- Simplify server startup output messages
- Migrate to minijinja template engine

**Documentation:**
- Update Cargo install, Arch Linux install instructions
- Add changelog and improve git-cliff config
- Fix changelog duplicate 0.4.1 entry

**Build:**
- Downgrade to edition 2021, set MSRV to 1.82.0
- Add git-cliff configuration

**CI:**
- Run on aarch64-linux

**Miscellaneous:**
- Add package metadata for cargo publish
- Remove macOS support, direct to Homebrew

### v0.4.1 (2025-10-04)

**Bug Fixes:**
- Change default hostname to 127.0.0.1 to prevent port conflicts

**Documentation:**
- Update Homebrew install instructions

### v0.4.0 (2025-10-03)

**Features:**
- Add ETag support for mermaid.min.js

**Documentation:**
- Add Arch Linux install instructions

**Build:**
- Optimize and reduce release binary size
- Add Nix flake packaging
- Update min Rust version to 1.85+ (2024)
- Bundle mermaid.min.js
- Remove cargo install instructions, add naming conflict warning
- Add `-H|--hostname` flag for non-localhost listening

### v0.3.0 (2025-09-27)

- Prevent theme flash on page load
- Replace WebSocket content updates with reload signals
- Add Mermaid diagram support

### v0.2.0 (2025-09-24)

- Add install script and update README
- Add macOS install instructions
- Add image support
- Add screenshot of mdserve serving README.md
- Enable HTML tag rendering in markdown files

### v0.1.0 (2025-09-22)

- (Initial release, details not provided)

---

## Summary of Key Findings

### Frontend
- **Layout:** Flexbox-based, sidebar (250px) + content pane, collapsible sidebar (48px)
- **Live-reload:** WebSocket at `/ws`, JSON message type `Reload`, auto-reconnect every 3s
- **Mermaid:** Bundled library, dynamically transformed from code blocks, theme-aware re-rendering
- **Syntax highlighting:** Server-side only (markdown-rs renderer)
- **Themes:** 5 CSS-based themes (Light, Dark, Catppuccin Latte/Macchiato/Mocha), localStorage persistence
- **Search, TOC:** Not implemented
- **Shortcuts:** ESC to close theme modal only
- **File tree:** Flat, alphabetical, no nesting

### Architecture
- **Modes:** Single-file (no sidebar) or directory (with sidebar); mode by user intent, not file count
- **State:** HashMap of tracked files, pre-rendered HTML, WebSocket broadcast channel
- **File watcher:** notify crate, non-recursive (immediate directory only)
- **Routing:** Unified router for both modes, image support, WebSocket endpoint, Mermaid asset

### Claude Code Integration
- **Plugin:** v1.1.0, metadata in plugin.json and marketplace.json
- **Skill:** Teaches agent when to render (long docs, tables, diagrams) vs. skip (short responses)
- **Workflow:** Write file → `mdserve --open` (background) → auto-reload on edit → TaskStop when done
- **Port conflict:** Auto-finds available port if 3000 in use

### Packaging
- **install.sh:** Platform detection (Linux x86_64/aarch64), GitHub release download, PATH setup
- **Nix flake:** Uses fenix + naersk, Rust stable toolchain
- **Changelog:** git-cliff with conventional commits parsing, 11 type categories

