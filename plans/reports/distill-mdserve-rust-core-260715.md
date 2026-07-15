# mdserve Rust Core Inventory

## 1. CLI Argument Parsing

**Framework:** clap 4.5.45 (derive macro) — src/main.rs:9-28

- `path` (positional): Path to markdown file or directory to serve
- `-H, --hostname` (string, default "127.0.0.1"): Hostname or IP address
- `-p, --port` (u16, default 3000): Port to serve on
- `-o, --open` (bool flag): Open preview in default browser

Parsed via `Args::parse()` at src/main.rs:32. Entry flow determines file vs. directory mode via `is_file()` and `is_dir()` checks (src/main.rs:35-52). Single-file mode derives `base_dir` from parent; directory mode scans for markdown files via `scan_markdown_files()`.

## 2. HTTP Server Framework and Routes

**Framework:** Axum 0.7.9 with WebSocket support — src/app.rs:2-11, 299-307

Routes registered in `new_router()` (src/app.rs:265-308):

| Route | Handler | Purpose |
|-------|---------|---------|
| `/` | `serve_html_root()` | Returns first (root) markdown file rendered as HTML |
| `/*filename` | `serve_file()` | Routes markdown files (`.md`/`.markdown`) and images |
| `/ws` | `websocket_handler()` | WebSocket upgrade for live reload |
| `/mermaid.min.js` | `serve_mermaid_js()` | Embedded mermaid.js library (included via `include_str!()` at src/app.rs:32) |

CORS enabled globally via `CorsLayer::permissive()` at src/app.rs:304. Router uses shared state `Arc<Mutex<MarkdownState>>` for thread-safe file tracking (src/app.rs:36, 272-276).

## 3. Markdown Rendering

**Framework:** markdown crate 1.0 (GFM fork) — src/app.rs:161-170

`MarkdownState::markdown_to_html()` at src/app.rs:161-170:
- Constructs `markdown::Options::gfm()` (GitHub Flavored Markdown)
- Sets `allow_dangerous_html = true` to render raw HTML tags (src/app.rs:163)
- Enables frontmatter parsing: `parse.constructs.frontmatter = true` (src/app.rs:164) — supports YAML and TOML frontmatter, stripped before rendering
- Compiles to HTML via `markdown::to_html_with_options()` (src/app.rs:166)
- Returns HTML string or fallback "Error parsing markdown" on failure (src/app.rs:167)

Rendered HTML is pre-cached in-memory in `MarkdownState::tracked_files` HashMap — no re-parsing on request.

Mermaid detection: at src/app.rs:487, checks if HTML contains `class="language-mermaid"` to conditionally inject mermaid.min.js script.

## 4. File Serving Mode

**Single-file vs. directory mode:**

- **Single-file:** src/main.rs:35-42. Parent directory is `base_dir`; only one file tracked.
- **Directory:** src/main.rs:43-49. Full directory is `base_dir`; all markdown files in immediate directory are tracked. Non-recursive: src/app.rs:52-67 and src/app.rs:790-804 (test confirms subdirectories are ignored).

**File discovery:** `scan_markdown_files()` at src/app.rs:52-67 reads directory, filters by extension (`.md` or `.markdown`, case-insensitive via `is_markdown_file()` at src/app.rs:69-74), sorts alphabetically, returns `Vec<PathBuf>`.

**Index/tree structure:** Files stored in `HashMap<filename, TrackedFile>` where key is filename (not full path), built at startup in `MarkdownState::new()` (src/app.rs:90-118). Each `TrackedFile` holds: `path` (full PathBuf), `last_modified` (SystemTime), `html` (pre-rendered HTML string).

**Path resolution:** `serve_file()` at src/app.rs:453-471 checks if filename exists in tracked files. For images, resolves via `full_path = base_dir.join(&filename)` (src/app.rs:585), canonicalizes, and validates against path traversal via prefix check: `canonical_path.starts_with(&base_dir)` (src/app.rs:589) — access denied if outside base_dir.

## 5. Live Reload Mechanism

**Protocol:** WebSocket (not SSE or long-poll) — src/app.rs:648-687

**File watcher:** notify 8.2.0, `RecommendedWatcher` at src/app.rs:281-290. Spawned in background task at src/app.rs:292-297. Watches `base_dir` with `RecursiveMode::NonRecursive` (only immediate directory, no subdirs).

**Event handling:** `handle_file_event()` at src/app.rs:200-263:
- Listens to modify, create, and rename events
- Markdown files: refresh if already tracked (src/app.rs:189) or add if new in directory mode (src/app.rs:192-196)
- Image files: trigger reload on any change (src/app.rs:249-258)
- Deletion: explicitly ignored to avoid 404 during editor save sequences (src/app.rs:241-245 comment: "Editors like neovim save by renaming to backup, then creating a new one")
- Debouncing: relies on notify's event coalescence; no explicit debounce logic observed

**Change broadcast:** `broadcast::channel()` at src/app.rs:91 (capacity 16 messages). State holds `change_tx` sender; WebSocket handler subscribes via `change_rx = state.change_tx.subscribe()` (src/app.rs:660).

**Browser reload:** `ServerMessage::Reload` enum (src/app.rs:48-50) serialized to JSON and sent over WebSocket (src/app.rs:675-680). Client-side JS detects message and reloads page (implied by test assertions at src/app.rs:1159, 1202).

## 6. Link Handling and URL Routing

**Markdown link rewriting:** Not explicitly performed. Markdown links remain as-is; markdown crate produces `<a href="...">` with original hrefs. Mermaid diagrams in code blocks are detected but not transformed — rendered as-is by client-side mermaid.js.

**URL routing:** Routes are literal filenames. File `/path/to/test.md` served at `/test.md` (not hierarchical). Directory navigation via sidebar links at src/app.rs:500-510 — template renders each filename as a link to `/<filename>`.

## 7. Theming

**Theme selection:** Client-side theme picker JavaScript (src/app.rs:965-968 test assertions reference `openThemeModal`, `theme-toggle`). Theme stored as `data-theme="dark"` attribute on root element (src/app.rs:968). CSS variables (src/app.rs:967: `--bg-color`) suggest theme colors defined in stylesheet.

**Theme source:** Embedded in MiniJinja template `main.html` (name at src/app.rs:30). Template is loaded at src/app.rs:38-43 via `minijinja_embed::load_templates!()` at src/app.rs:41.

**Catppuccin:** Not explicitly referenced in code. User notes mention "Catppuccin" in CLAUDE.md but no hardcoded theme names in Rust. Themes are likely defined in client CSS/JS within template.

## 8. Notable Server Behaviors

**Caching:**
- Markdown HTML: pre-rendered at startup and on file change, served from memory (no per-request compilation).
- Mermaid.js: cached with ETag `"<version>"` (src/app.rs:33, MERMAID_ETAG). Cache-Control: `public, no-cache` forces revalidation (src/app.rs:570). If-None-Match header triggers 304 Not Modified (src/app.rs:548-561, `is_etag_match()`).

**Concurrency:** All state guarded by `Arc<Mutex<...>>`. File watcher runs in spawned task; handlers lock state only when needed (e.g., src/app.rs:438, 658).

**Static asset serving:**
- Mermaid.js: embedded in binary via `include_str!()` at src/app.rs:32 (included from `../static/js/mermaid.min.js`).
- Images: served from `base_dir` via `serve_static_file_inner()` at src/app.rs:579-623. Content-Type guessed by extension (src/app.rs:629-646). Supported: PNG, JPG, GIF, SVG, WebP, BMP, ICO.

**Error handling:**
- File not found: 404 HTML response (src/app.rs:461, 490).
- Template rendering errors: 500 with error message (src/app.rs:478-480, 522-525, 537-540).
- Markdown parse errors: fallback to "Error parsing markdown" string (src/app.rs:167).
- Path traversal: 403 Forbidden if canonicalized path doesn't start with base_dir (src/app.rs:589-595).

**Security:**
- Path traversal guard at src/app.rs:587-596: canonicalizes path and validates against base_dir prefix.
- Only markdown and image files served; other file types return 404 (src/app.rs:469).
- Dangerous HTML allowed in markdown (`allow_dangerous_html = true`) — used for agent-produced markdown with embedded HTML.

**Port retry:** Binds to requested port; if in use, tries up to 10 sequential ports (src/app.rs:310-331, MAX_PORT_ATTEMPTS=10). User is notified if port changes (src/app.rs:349).

**Browser open:** Platform-specific via `open` (macOS) or `xdg-open` (Linux) commands. Not supported on other platforms (src/app.rs:409-435).

## 9. Dependencies (Cargo.toml)

| Crate | Version | Purpose |
|-------|---------|---------|
| axum | 0.7.9 (ws feature) | HTTP server framework |
| tokio | 1.0 (rt-multi-thread, macros, net, fs, time) | Async runtime & utilities |
| markdown | 1.0 | Markdown-to-HTML compiler (GFM) |
| clap | 4.5.45 (derive) | CLI argument parsing |
| tower | 0.5.2 | Middleware & service tower |
| tower-http | 0.6.6 (fs, cors) | HTTP utilities (file serving, CORS) |
| notify | 8.2.0 | File system watcher |
| futures-util | 0.3 | Async stream utilities |
| serde | 1.0 (derive) | Serialization framework |
| serde_json | 1.0 | JSON serialization |
| anyhow | 1.0 | Error handling (Result/Context) |
| minijinja | 2.12.0 | Jinja2-like templating engine |
| minijinja-embed | 2.12.0 (build-dep & dev) | Compile-time template embedding |

## 10. Build Script (build.rs)

At src/app.rs and build.rs:1-4:

```rust
fn main() {
    minijinja_embed::embed_templates!("templates", &[".html"]);
}
```

Macro invoked at compile time. Scans `templates/` directory, embeds all `.html` files as static strings in the binary. Template environment is initialized once via `OnceLock::new()` at src/app.rs:31 and populated at src/app.rs:38-43. This ensures `main.html` template is baked into the executable — no external file I/O at runtime.

Release profile optimizations: strip=true, lto=true, codegen-units=1, panic=abort (Cargo.toml:31-35) for minimal binary size.

---

## Summary of Key Facts

- **Entry:** Clap CLI (port, hostname, path, --open flag)
- **Server:** Axum WebSocket-capable router with 4 routes (/, /*, /ws, /mermaid.min.js)
- **Markdown:** markdown crate (GFM), pre-rendered to memory, GFM+HTML+frontmatter support
- **File modes:** Single file (serve parent dir) or directory (serve all .md files, non-recursive)
- **Live reload:** notify file watcher → broadcast channel → WebSocket → client JS page reload
- **Theming:** Client-side JS theme picker, data-theme attribute, CSS variables
- **Caching:** In-memory HTML + ETag on mermaid.js (304 Not Modified)
- **Security:** Path traversal guard, markdown/image only, dangerous HTML allowed
- **Assets:** Mermaid.js embedded; templates compiled at build time via minijinja-embed
