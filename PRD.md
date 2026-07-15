# PRD: MDView — Multi-Project Markdown Viewer for AI Agent Workflows

**Version:** 1.2  
**Date:** July 2026  
**Status:** Draft

> **Changelog**
> - **1.2** — Đơn giản hoá MCP còn **1 tool** (`mdview_view_file`); project registry chuyển sang **implicit** (auto-create khi register file); yêu cầu code theo **clean architecture** (ports & adapters, service/command/DTO). Xem §5.1, §5.5, §7.4.
> - **1.1** — Bổ sung **desktop viewer** (Linux/Windows/macOS) qua Tauri như lớp phủ trên cùng core, không đổi ngôn ngữ. Xem §7.1, §7.5, §9, NFR-04.

---

## 1. Tóm tắt (Executive Summary)

MDView là một ứng dụng chạy nền trên máy cục bộ, đóng vai trò là **markdown server đa-project**. Nó giải quyết vấn đề broken links khi navigate giữa các markdown files thuộc cùng một project nhưng nằm ở nhiều folder khác nhau — một vấn đề phổ biến trong workflow làm việc với AI agents (Claude Code, Cursor, Aider, v.v.) tạo ra cấu trúc docs phức tạp.

App cung cấp giao diện web đẹp, truy cập được từ bất kỳ browser nào trên cùng network, có thể giao tiếp với agents qua MCP server hoặc CLI, và tự động quản lý file index để mọi internal link trong project đều được resolve chính xác. Cùng một core còn chạy được như **desktop app native** (Linux/Windows/macOS) — cửa sổ Tauri nhìn vào chính daemon đó (§7.1, §7.5).

---

## 2. Bối cảnh và vấn đề

### 2.1 Landscape hiện tại

Các tool hiện có đều thiếu ít nhất một yếu tố quan trọng:

| Tool | Điểm mạnh | Khoảng trống |
|---|---|---|
| **Marky** (GRVYDEV) | Desktop app native, workspace/folder mode, live reload, fuzzy search | Không phải web server (không remote view được), không có project registry đa-root, không có MCP |
| **mdserve** (jfernandez) | Rust CLI, Claude Code plugin, live reload | Directory mode chỉ scan immediate folder (non-recursive), không có cross-folder link resolution, không có multi-project |
| **markdown-vault-mcp** | MCP + file watcher, semantic search | Chỉ là query tool, không có web viewer/renderer |
| **library-mcp** | MCP cho knowledge base querying | Không có web UI, không phải viewer |
| **Markserv / Grip** | Classic, live reload | Single root folder, không có agent integration, link resolution yếu |

**Kết luận:** Chưa có tool nào xử lý đồng thời: multi-project registry + cross-folder link resolution + MCP agent integration + web server remote viewing.

### 2.2 Pain point cụ thể

Xét project có cấu trúc:

```
/projects/my-app/
├── CLAUDE.md
├── docs/
│   ├── architecture.md        ← link đến ../src/api/README.md
│   └── deployment.md
├── src/
│   └── api/
│       └── README.md          ← link đến ../../docs/architecture.md
└── tasks/
    └── sprint-1.md            ← link đến ../docs/deployment.md
```

Khi tool hiện tại serve thư mục `docs/`, người dùng click link `../src/api/README.md` → **404**. Tool không biết file đó tồn tại vì nó nằm ngoài folder được serve.

---

## 3. Mục tiêu

### 3.1 Goals

- **G1.** Serve markdown files từ nhiều projects, mỗi project có thể trải rộng nhiều folder/subfolder tùy ý.
- **G2.** Tất cả internal links trong một project phải resolve chính xác — không broken link khi navigate.
- **G3.** Agents (Claude Code và các AI agent khác) có thể báo cho app biết file vừa được tạo/cập nhật thông qua MCP hoặc CLI.
- **G4.** Truy cập được từ browser trên cùng máy hoặc qua network (remote view).
- **G5.** Live reload: file thay đổi trên disk thì browser tự refresh, không cần reload thủ công.
- **G6.** Project registry bền vững qua các lần restart.

### 3.2 Non-goals

- Không phải static site generator (không build output ra HTML tĩnh).
- Không phải authoring tool hay WYSIWYG editor.
- Không phải tool để deploy/host public (chỉ dùng locally hoặc trong private network).
- Không cần authentication (để đơn giản; security tùy người dùng tự xử lý ở network level).
- Không sync hay backup files.
- Desktop app không phải app đứng riêng có registry riêng — chỉ là cửa sổ/tray native nhìn vào cùng một daemon (§7.5); vẫn read-only, không phải editor.
- **Cross-project link resolution** (resolve link sang project KHÁC trong registry) — out of scope: link chỉ resolve trong phạm vi 1 project, ngoài ra là broken-link.
- **Semantic search** — deferred (YAGNI): chỉ FTS5 keyword; bật lại nếu có nhu cầu thật.

---

## 4. User Personas

**Primary: Power Developer + AI Agent User**
- Chạy nhiều projects song song trên một machine.
- Dùng Claude Code hoặc AI agent khác để generate documentation, task plans, architecture docs.
- Muốn review output từ browser mà không cần mở từng file bằng tay.
- Thường xuyên click-through giữa các linked documents.

**Secondary: Team Lead / Reviewer**
- Truy cập từ máy khác trong cùng network để review docs mà agent vừa viết.
- Không cần biết cấu trúc folder, chỉ cần navigate tự nhiên qua links.

---

## 5. Functional Requirements

### 5.1 Project Registry

**FR-01.** App duy trì một **project registry** — danh sách các projects đã được đăng ký, mỗi project có:
- `id`: unique identifier (slug, tự sinh hoặc do người dùng đặt)
- `name`: tên hiển thị
- `root_path`: đường dẫn tuyệt đối đến thư mục gốc của project
- `created_at`: thời điểm đăng ký
- `last_seen_at`: lần cuối có file activity

**FR-02.** Registry được persist xuống disk (SQLite database hoặc JSON file ở `~/.mdview/registry.json`) và tự động load lại khi app restart.

**FR-03.** Mỗi project có thể tùy chọn file marker để auto-detect root: `.mdview.json`, `CLAUDE.md`, hoặc `README.md` tại thư mục gốc.

**FR-04.** Đăng ký project theo hai đường:
- **Implicit (mặc định, qua MCP):** project được **tự tạo** ở lần đầu một file thuộc `project_root` mới được register qua `mdview_view_file` (§5.5) — agent KHÔNG cần bước đăng ký project riêng. Server sinh `id` từ tên thư mục gốc (hoặc marker FR-03), set `root_path = project_root`, rồi recursive scan nền (FR-06).
- **Explicit (qua CLI, tuỳ chọn):** `mdview register /path/to/project [--name "My Project"]` cho ai muốn đăng ký trước từ terminal.

**FR-05.** Hỗ trợ xóa project khỏi registry mà không xóa files:
- MCP: `mdview_unregister_project`
- CLI: `mdview unregister <project-id>`

### 5.2 File Indexing & Watching

**FR-06.** Khi một project được đăng ký, app sẽ **recursive scan toàn bộ cây thư mục** từ `root_path`, index tất cả files có extension `.md` và `.markdown`.

**FR-07.** Index mỗi file bao gồm:
- Absolute path trên disk
- Relative path từ project root (dùng làm URL)
- Title (H1 đầu tiên trong file, hoặc filename nếu không có H1)
- Danh sách tất cả internal links trong file
- Danh sách headings (dùng cho anchor navigation)
- File size + last modified timestamp
- Frontmatter metadata (nếu có): `title`, `tags`, `description`

**FR-08.** App sử dụng **filesystem watcher** (inotify trên Linux, FSEvents trên macOS, ReadDirectoryChangesW trên Windows) để detect:
- File mới được tạo → thêm vào index
- File thay đổi → cập nhật index entry
- File bị xóa → xóa khỏi index
- Thư mục mới → scan và thêm files vào index

**FR-09.** Debounce file events: đợi 200ms sau sự kiện cuối cùng trước khi update index và push live reload signal, để tránh spam khi agent đang viết file liên tục.

**FR-09b. Index tăng dần + re-scan reconcile.** Watcher cập nhật index **incremental** theo từng event (KHÔNG re-walk toàn tree — đáp ứng NFR-03 ở 100k files). Bổ sung một **full re-scan trigger** làm lưới an toàn chống drift: (a) **tự động** sau khi watcher lỗi rồi recover (NFR-02) để reconcile event bị miss, (b) **thủ công** qua CLI `mdview refresh [project-id]`. Incremental là steady-state; re-scan là đối chiếu.

**FR-10.** Khi agent register file qua MCP (`mdview_view_file`, §5.5), app ưu tiên index file đó ngay lập tức thay vì đợi filesystem event; nếu `project_root` chưa có trong registry thì tự tạo project trước (FR-04 implicit).

### 5.3 Link Resolution

**FR-11 (Core).** Khi render một markdown file, app phải **rewrite tất cả internal links** sang URL namespace của app trước khi trả về client. Thuật toán:

```
Input: file tại /projects/my-app/docs/architecture.md
Link trong file: ../src/api/README.md

Resolve:
1. Xác định absolute path: /projects/my-app/docs/../src/api/README.md
   → normalized: /projects/my-app/src/api/README.md
2. Tra cứu trong index của project my-app
3. Tìm thấy → rewrite link thành /projects/my-app/src/api/README.md (URL nội bộ của app)
4. Không tìm thấy trong project hiện tại → giữ nguyên link gốc + thêm CSS class "broken-link"
   (Cross-project resolution: OUT OF SCOPE — xem §3.2 non-goals)
```

**FR-12.** Hỗ trợ các dạng link:
- Relative links: `./other.md`, `../folder/file.md`
- Absolute links (tính từ project root): `/docs/guide.md`
- Links với anchor: `../api/README.md#installation`
- Links không có extension: `../api/README` → tự thêm `.md`

**FR-13.** External links (http/https) được giữ nguyên và mở trong tab mới.

**FR-14.** Image links (`.png`, `.jpg`, `.gif`, `.svg`, `.webp`) được resolve tương tự, serve trực tiếp từ disk.

### 5.4 Web Interface

**FR-15.** App expose một web server (mặc định `http://localhost:7700`), accessible từ browser bất kỳ. Port có thể config.

**FR-16.** Trang chủ (`/`) hiển thị danh sách tất cả projects đã đăng ký với:
- Tên project
- Root path
- Số lượng markdown files
- Thời gian last activity

**FR-17.** Trang project (`/p/{project-id}/`) hiển thị:
- File tree của project (sidebar trái)
- Danh sách files gần đây (mặc định view)
- Search box để tìm kiếm full-text trong project

**FR-18.** Trang file (`/p/{project-id}/{relative/path/to/file.md}`) hiển thị:
- Markdown được render đẹp (GitHub Flavored Markdown)
- Sidebar với file tree của project (có thể collapse)
- Breadcrumb navigation
- Table of Contents từ headings (sticky sidebar phải)
- Thông tin meta: đường dẫn thực, thời gian modified
- **Backlinks panel**: danh sách các files khác trong project link đến file này

**FR-19.** Live reload: app push WebSocket event (reload-signal) đến browser khi file thay đổi; browser reload trang đang xem, auto-reconnect nếu server restart. **Phase 1:** full-page reload (đơn giản, bền — theo mdserve). Scoped content-only refresh (thay riêng nội dung, không reload cả trang) **defer** sang Phase 3 nếu nhấp nháy gây khó chịu.

**FR-20.** Hỗ trợ Mermaid diagrams, syntax highlighting cho code blocks, LaTeX/KaTeX cho math equations.

**FR-21.** Light/dark theme, theo system preference, có nút toggle thủ công. Code block đổi màu theo theme qua CSS (`data-theme`) — highlight server-side class-based (§9), đổi theme không cần re-render.

**FR-22.** Responsive layout, dùng được trên màn hình nhỏ.

**FR-22b. Màn hình Settings.** App có trang cấu hình (`/settings`, có trong CẢ web lẫn desktop) để xem/chỉnh cấu hình hệ thống mà không cần sửa file tay:
- **Server:** port, bind host (`127.0.0.1` vs `0.0.0.0`), open-browser-on-start
- **Renderer:** theme (light/dark/system), syntax highlight theme
- **Indexing:** debounce ms, max file size, exclude patterns
- **MCP:** enabled, transport (stdio/http)

Thay đổi được validate và ghi **atomic** xuống `~/.mdview/config.toml` (§10). UI đánh dấu rõ mục nào áp dụng nóng vs mục nào cần restart (vd đổi bind host). Registry (danh sách projects) KHÔNG sửa ở đây — đó là §5.1.

### 5.5 MCP Server Interface

App chạy kèm một **MCP server** (stdio hoặc HTTP/SSE transport). Bề mặt MCP được giữ **tối thiểu — đúng 1 tool** cho workflow chính; mọi thứ khác server tự lo. Nguyên tắc thiết kế: agent KHÔNG quản project, KHÔNG quản index, KHÔNG gọi nhiều bước — chỉ báo "file này, ở project root này".

**FR-23 (Core — tool DUY NHẤT).** `mdview_view_file` — làm một file xem được và trả link. Gộp thay thế `register_project` + `notify_file` + `open_file`.
```
Input:
  - project_root:  string (absolute path đến thư mục gốc project)
  - relative_path: string (đường dẫn file .md/.markdown, tính từ project_root)
Output:
  - url:           string (URL browser để xem file)
  - project_id:    string (server tự tạo hoặc tái dùng)
```
Server tự xử lý (agent không cần biết):
1. Chuẩn hoá `project_root`, tra registry:
   - đã có project với `root_path` đó → tái dùng `project_id`
   - chưa có → **tự tạo project** (id sinh từ tên thư mục gốc hoặc marker FR-03), khởi động recursive scan nền (FR-06)
2. Index NGAY file `relative_path` (không đợi filesystem event) — kể cả khi scan nền chưa xong, url của file này dùng được liền
3. Resolve toàn bộ internal link của file (FR-11) để navigate không gãy
4. Trả `url = /p/{project_id}/{relative_path}`

Một lời gọi, hai tham số, không tiền-đăng-ký. Đây là 90% workflow: agent tạo/sửa file → có link → show cho user.

**FR-24.** Transport & bind: MCP mặc định `stdio`; khi bật HTTP/SSE thì bind `127.0.0.1` mặc định (NFR-05), phải config rõ ràng mới expose ra network.

#### 5.5.1 Deferred MCP tools (future — NGOÀI scope hiện tại)

Các tool dưới đây **cố tình để dành** (YAGNI): chỉ thêm lại khi có nhu cầu thật, không implement ở các phase đầu. Chức năng của chúng hoặc đã gộp vào `mdview_view_file`, hoặc đã có qua CLI (§5.6) cho người dùng ở terminal.

| Deferred tool | Vì sao hoãn / thay thế bằng |
|---|---|
| `mdview_register_project` | Gộp vào `mdview_view_file` (auto-create). Explicit vẫn có qua CLI `mdview register`. |
| `mdview_notify_file` | Gộp vào `mdview_view_file` (index ngay). Sự kiện delete/update do filesystem watcher lo (FR-08). |
| `mdview_open_file` | Url đã trả sẵn trong `mdview_view_file`. Side-effect auto-navigate browser để hoãn. |
| `mdview_list_projects` | Có qua CLI `mdview list` / web `/` / REST `/api/projects`. |
| `mdview_search` | Có qua CLI `mdview search` / web `/search`. |
| `mdview_status` | Có qua CLI `mdview status` / `/health`. Lỗi khi gọi `mdview_view_file` cũng đủ báo server sống/chết. |
| `mdview_unregister_project` | Có qua CLI `mdview unregister`. |

### 5.6 CLI Interface

**FR-29.** App có CLI với các commands:

```bash
# Khởi động server (daemon mode)
mdview serve [--port 7700] [--host 0.0.0.0]

# Đăng ký project
mdview register /path/to/project [--name "My App"] [--id my-app]

# Mở file trong browser
mdview open /path/to/file.md

# Liệt kê projects
mdview list

# Tìm kiếm
mdview search "query" [--project my-app]

# Xem status
mdview status

# Re-scan reconcile index (FR-09b)
mdview refresh [<project-id>]

# Chẩn đoán & tự fix integration (FR-33)
mdview doctor [--json] [--dry-run] [--fix]

# Xóa project
mdview unregister <project-id>

# Dừng server
mdview stop
```

**FR-30.** CLI output hỗ trợ `--json` flag để pipe vào các tool khác hoặc scripts.

### 5.7 Agent Integration Instruction

**FR-31.** App cung cấp sẵn một file `AGENTS.md` (mẫu) mô tả cách agents tích hợp với app, để người dùng copy vào project của mình hoặc đặt ở global CLAUDE.md.

Nội dung mẫu:
```markdown
## MDView Integration

After creating or updating any markdown file, make it viewable in ONE call —
no project registration step needed:

### Using MCP (preferred)
Call `mdview_view_file` with:
- project_root:  absolute path to the project root
- relative_path: the file path relative to that root
It returns a browser `url`. Show the user: "You can view this at: <url>".
The server auto-registers the project on first use and indexes the file
immediately.

### Using CLI fallback
Run: `mdview open <absolute-path>`
```

---

### 5.8 Installation & Doctor

Mục tiêu: end-user cài đặt **nhanh và dễ nhất có thể**, và sau khi cài tự tích hợp được vào Claude/Claude Code không cần chỉnh tay.

**FR-32. One-command install.** Script `install.sh` (`curl … | bash`) tự: phát hiện OS/arch (Linux x86_64/aarch64, macOS), tải binary release mới nhất từ GitHub, đặt vào PATH (env override → `/usr/local/bin` → `~/.local/bin` → `~/.mdview/bin`), cảnh báo nếu dir không nằm trong PATH, rồi gợi ý chạy `mdview doctor`. Kèm kênh khác: Homebrew tap, `cargo install mdview`, và Tauri bundle cho desktop (NFR-04). Tham khảo mdserve multi-channel-install.

**FR-33. `mdview doctor` — tự chẩn đoán & fix integration.** Sau khi cài, `mdview doctor` kiểm tra và **tự sửa an toàn** các điểm tích hợp, mỗi mục trả `OK | FIXED | MANUAL` (kèm lệnh gợi ý):
- Binary `mdview` có trong PATH.
- `~/.mdview/config.toml` hợp lệ (tạo mặc định nếu thiếu).
- Server sống (`/health`) + `~/.mdview/daemon.lock` hợp lệ; port cấu hình rảnh.
- **MCP registration:** phát hiện & ghi entry `mdview` vào file cấu hình MCP của Claude Code (`~/.claude.json`, project `.mcp.json`, hoặc `claude_desktop_config.json` tuỳ client) — **merge idempotent, backup trước khi sửa**, không phá config sẵn có.
- **Agent instruction:** kiểm tra AGENTS.md/CLAUDE.md có snippet MDView (§5.7); offer chèn.

Flags: `--json` (machine output), `--dry-run` (chỉ báo, không sửa), `--fix` (tự sửa; mặc định hỏi/không sửa mục ghi đè). Idempotent — chạy nhiều lần an toàn.

## 6. Non-Functional Requirements

**NFR-01. Performance**
- App startup: < 2 giây.
- Initial scan của một project 10,000 files: < 10 giây.
- File index update sau filesystem event: < 500ms.
- Page render (markdown → HTML): < 100ms cho file 1MB.
- Search query: < 200ms cho 50,000 files.

**NFR-02. Reliability**
- App không crash khi project root bị xóa — chỉ log warning và mark project as "offline".
- App tự recover khi filesystem watcher bị lỗi (retry với exponential backoff).
- Tất cả project registry data được write atomically để tránh corruption khi crash.

**NFR-03. Resource consumption**
- RAM: < 200MB cho 100,000 files indexed.
- CPU khi idle: < 1%.
- File index được lưu xuống disk (SQLite) để giảm memory footprint — không load toàn bộ content vào RAM.

**NFR-04. Cross-platform**
- Hỗ trợ macOS, Linux, Windows — cả bản CLI/daemon lẫn desktop app.
- Packaging:
  - **CLI/daemon:** single binary (không cần runtime cài thêm), Homebrew tap, `.deb`/`.rpm` cho Linux.
  - **Desktop:** Tauri bundle — `.dmg` (macOS), `.deb`/`.AppImage` (Linux), `.exe`/MSI (Windows). Native webview, KHÔNG nhúng Chromium (không Electron).

**NFR-05. Security**
- App chỉ serve files từ các project roots đã được đăng ký. Không serve file ngoài registry (prevent path traversal).
- MCP server chỉ bind localhost by default. Cần config rõ ràng để expose ra network.
- Web server có thể config bind address (localhost vs 0.0.0.0).

**NFR-06. Observability**
- Structured logging với level control (DEBUG/INFO/WARN/ERROR).
- Log file tại `~/.mdview/mdview.log` với rotation.
- `/health` endpoint trả về JSON status (dùng cho agent `mdview_status`).

---

## 7. Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                     MDView App                          │
│                                                         │
│  ┌─────────────┐    ┌──────────────┐   ┌────────────┐  │
│  │  MCP Server │    │  HTTP/WS     │   │   CLI      │  │
│  │  (stdio /   │    │  Web Server  │   │  Handler   │  │
│  │   HTTP SSE) │    │  :7700       │   │            │  │
│  └──────┬──────┘    └──────┬───────┘   └─────┬──────┘  │
│         │                  │                  │         │
│         └──────────────────┼──────────────────┘         │
│                            ▼                            │
│               ┌────────────────────────┐               │
│               │    Core Engine         │               │
│               │                        │               │
│               │  - Project Registry    │               │
│               │  - File Indexer        │               │
│               │  - Link Resolver       │               │
│               │  - Search Engine       │               │
│               │  - Live Reload (WS)    │               │
│               └────────────┬───────────┘               │
│                            │                            │
│         ┌──────────────────┼──────────────────┐        │
│         ▼                  ▼                  ▼        │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  SQLite DB  │  │  FS Watcher  │  │  Markdown    │  │
│  │  (registry  │  │  (per-project│  │  Renderer    │  │
│  │   + index)  │  │   watchers)  │  │  (pulldown   │  │
│  └─────────────┘  └──────────────┘  │  /comrak)    │  │
│                                     └──────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### 7.1 Deployment Topology — Một Daemon, Nhiều Client

MDView chạy như **một server (daemon) duy nhất**; browser tab và cửa sổ desktop chỉ là **client** nhìn vào nó. Bất biến bắt buộc: **không bao giờ có 2 daemon cùng ghi một registry SQLite.**

```
   agent (MCP) ─┐        ┌─────────────────────────┐
   CLI ─────────┼──────► │  mdview daemon           │ ──► ~/.mdview/registry.db
                         │  Axum :7700 + MCP + WS   │      (nguồn sự thật duy nhất)
                         └───────────┬─────────────┘
                                     │ HTTP / WebSocket
                    ┌────────────────┼────────────────┐
                    ▼                ▼                 ▼
              browser tab      Tauri window      browser máy khác
              (localhost)      (native + tray)   (remote view LAN)
```

- **Web (phần lớn thời gian):** agent gọi `mdview_view_file` → daemon trả url → user click → xem trong browser. Desktop không cần bật.
- **Desktop (thỉnh thoảng):** bật app ở terminal khác / icon Windows. App đọc `~/.mdview/daemon.lock`: có daemon sống → cửa sổ chỉ attach (webview → :7700); chưa có → app tự spawn daemon rồi mới hiện cửa sổ. Vòng đời chi tiết: §7.5.

Hệ quả DRY: chỉ **một** web UI — xem qua browser hay qua Tauri webview đều cùng một code path render; live reload / registry / MCP share tự động vì cùng một daemon.

### 7.2 URL Namespace

```
/                                 → Project list
/p/{project-id}/                  → Project home + file tree
/p/{project-id}/{path/to/file.md} → Render file
/p/{project-id}/_search           → Search trong project
/api/projects                     → REST API (cho UI)
/api/projects/{id}/files          → File list
/settings                         → Trang cấu hình hệ thống (FR-22b)
/api/config                       → GET/PUT cấu hình (cho Settings UI)
/api/status                       → Health check
/ws                               → WebSocket endpoint (live reload)
```

### 7.3 Link Resolution Algorithm

```python
def resolve_link(source_file_abs_path, link_href, project) -> Optional[str]:
    """
    Returns the MDView URL for a link, or None if external/unresolvable.
    """
    # 1. Skip external links
    if link_href.startswith(("http://", "https://", "mailto:", "#")):
        return None  # keep as-is
    
    # 2. Split anchor
    anchor = ""
    if "#" in link_href:
        link_href, anchor = link_href.rsplit("#", 1)
    
    # 3. Resolve to absolute path
    source_dir = os.path.dirname(source_file_abs_path)
    if link_href.startswith("/"):
        # Absolute from project root
        abs_path = os.path.join(project.root_path, link_href.lstrip("/"))
    else:
        # Relative from source file
        abs_path = os.path.normpath(os.path.join(source_dir, link_href))
    
    # 4. Try with and without .md extension
    candidates = [abs_path, abs_path + ".md", abs_path + "/README.md"]
    for candidate in candidates:
        if project.index.contains(candidate):
            rel = os.path.relpath(candidate, project.root_path)
            url = f"/p/{project.id}/{rel}"
            return url + (f"#{anchor}" if anchor else "")
    
    # 5. Cross-project lookup: OUT OF SCOPE (non-goal) — KHÔNG resolve sang project khác
    
    # 6. Not found → return None (render as broken link)
    return None
```

---

### 7.4 Code Organization — Clean Architecture (Ports & Adapters)

Yêu cầu chất lượng: code tổ chức clean-code, tách **domain** khỏi **hạ tầng** theo **ports & adapters (hexagonal)** ở mức *đủ dùng* — hexagonal khi nó thật sự giảm coupling, không phải nghi thức (YAGNI/KISS).

**Workspace Rust** (một core, nhiều adapter):

```
mdview-core/    (lib)  DOMAIN + APPLICATION: registry, indexer, link resolver, search,
                       render (comrak). Định nghĩa PORTS (trait): FileStore, Watcher,
                       Clock, ProjectRepository. KHÔNG phụ thuộc Axum / Tauri / SQLite.
mdview/         (bin)  Adapter CLI + daemon: HTTP/WS (Axum), MCP server, clap CLI.
mdview-desktop/ (bin)  Adapter Tauri: cửa sổ native + tray, attach/spawn daemon.
adapters/              SQLite (rusqlite) impl ProjectRepository; notify impl Watcher; ...
```

**Patterns:**
- **Service** — logic nghiệp vụ nằm trong service của application layer (vd `ViewFileService`, `IndexService`, `LinkResolverService`), nhận port qua constructor (dependency inversion) → test bằng fake adapter.
- **Command / Query** — mỗi thao tác vào là một command/query object (vd `ViewFileCommand { project_root, relative_path }`); một handler một việc; ánh xạ 1-1 với MCP tool và CLI subcommand.
- **DTO** — biên ngoài (MCP input/output, REST/JSON, CLI `--json`) dùng DTO riêng, **không lộ struct domain ra ngoài**; map DTO ↔ domain ở rìa adapter.
- **Dependency rule** — phụ thuộc chỉ hướng vào trong (adapter → application → domain). Domain không `use` Axum/Tauri/rusqlite.

Nhờ đó: thêm adapter mới (HTTP SSE cho MCP, desktop, DB khác) không đụng domain; test domain chạy không cần server/DB thật.

### 7.5 Desktop Shell & Daemon Lifecycle

Desktop = binary `mdview-desktop` (Tauri v2), mỏng, tái dùng pattern từ marky (single-instance, capability allowlist least-privilege, atomic settings, bundle config).

**Khởi động desktop:**
1. Đọc `~/.mdview/daemon.lock` (port + pid).
2. Daemon sống (lock hợp lệ, `/health` trả lời) → cửa sổ Tauri **attach**: webview trỏ `http://127.0.0.1:{port}`.
3. Không có daemon → app **tự spawn daemon** (bind `127.0.0.1` mặc định, NFR-05), ghi lock, rồi hiện cửa sổ. → Windows: double-click icon là chạy, không cần terminal.
4. Bật lần 2 → `tauri-plugin-single-instance` focus cửa sổ cũ, không mở thêm.

**Đóng cửa sổ:** thu vào **system tray**, daemon **vẫn chạy** để agent tiếp tục push file; click tray mở lại cửa sổ. Quit hẳn từ tray mới dừng daemon (nếu daemon do chính app này spawn).

**Bất biến single-daemon:** mọi launcher (CLI `mdview serve` hay desktop) đều đi qua `daemon.lock` — sống thì attach/reuse, chết thì lên làm daemon. Không bao giờ 2 server cùng registry.

**Read-only:** desktop không ghi vào file/folder user; state riêng (window, prefs) ở app-data-dir cross-platform (macOS Application Support, Linux `~/.local/share`, Windows `%APPDATA%`).

## 8. Implementation Plan (Suggested Phases)

### Phase 1 — MVP (4-6 tuần)
- [ ] HTTP server cơ bản + markdown render
- [ ] Project registration (CLI only)
- [ ] Recursive file scan + SQLite index
- [ ] Filesystem watcher
- [ ] Link resolution (FR-11, FR-12)
- [ ] Live reload qua WebSocket
- [ ] Basic web UI: project list + file tree + file viewer

### Phase 2 — Agent Integration (2-3 tuần)
- [ ] MCP server (stdio transport)
- [ ] **Tool duy nhất `mdview_view_file`** (§5.5) + implicit project auto-create (FR-04)
- [ ] AGENTS.md template (mẫu 1-tool, §5.7)
- [ ] CLI hoàn chỉnh (tất cả commands FR-29) — status/list/search sống ở CLI, không ở MCP

### Phase 3 — UX Polish (2-3 tuần)
- [x] Backlinks panel
- [x] Full-text search (FTS5 + `/p/{id}/_search` web UI + CLI)
- [x] Table of Contents (sticky right sidebar)
- [x] Breadcrumb navigation
- [x] Broken link highlighting
- [x] Light/dark theme (no-flash)
- [x] Màn hình Settings (`/settings`, FR-22b) + `/api/config` (GET/POST)

### Phase 4 — Production Hardiness + Desktop (2-3 tuần)
- [ ] **Desktop shell `mdview-desktop` (Tauri)**: attach/spawn daemon, tray, single-instance (§7.5)
- [ ] Binary packaging: single CLI binary + Tauri bundle (.dmg/.deb/.AppImage/.exe), Homebrew tap
- [ ] Logging + log rotation
- [ ] Performance tuning cho large projects
- [ ] HTTP SSE transport cho MCP (bên cạnh stdio)

---

## 9. Tech Stack Recommendations

| Layer | Recommendation | Rationale |
|---|---|---|
| Core language | **Rust** | Single binary, fast, native FS watcher, low memory. Tham khảo mdserve. |
| HTTP/WS server | **Axum** | Async, lightweight, excellent WebSocket support |
| Markdown renderer | **comrak** (Rust) | CommonMark + GFM extensions, fast, battle-tested |
| Database | **SQLite via rusqlite** | Embedded, zero-config, ACID, full-text search qua FTS5 |
| Filesystem watcher | **notify** crate | Cross-platform, battle-tested, used by Cargo |
| Frontend | **Vanilla JS + HTMX** hoặc **Preact** | Nhẹ, không over-engineer cho tool nội bộ |
| Code highlighting | **syntect** (server-side, class-based) | Highlight 1 lần lúc pre-render, cache kèm HTML, sanitize rồi serve. Output **class-based** (`ClassStyle::Spaced`) → màu ở CSS, đổi theme code-block tức thì qua `data-theme` (không re-render, không client JS). `css_for_theme_with_class_style` sinh CSS mỗi theme. |
| Diagrams | **Mermaid.js** (CDN) | Standard trong AI-generated docs |
| MCP SDK | **rmcp** hoặc **mcp-rs** | Rust MCP server implementation |
| Desktop shell | **Tauri v2** | Cửa sổ native + tray, tái dùng core Rust + web UI, binary nhỏ (<15MB). Tham khảo marky. Không Electron, không Go. |
| Desktop frontend | Web UI hiện có (native webview) | DRY: cùng UI với browser (WebView2/WKWebView/WebKitGTK), không render lại |
| Kiến trúc code | **Ports & adapters (hexagonal)** mức đủ dùng | Core (`mdview-core`) tách khỏi Axum/Tauri/SQLite; service/command/DTO; test domain không cần server/DB (§7.4) |

**Alternative:** Nếu muốn prototype nhanh hơn, có thể dùng **Node.js + Fastify** cho Phase 1-2, sau đó rewrite Rust ở Phase 3-4 nếu cần.

---

## 10. Configuration

File config tại `~/.mdview/config.toml`:

```toml
[server]
port = 7700
host = "127.0.0.1"      # Đổi thành "0.0.0.0" để cho phép remote access
open_browser_on_start = false

[mcp]
enabled = true
transport = "stdio"     # hoặc "http" (port 7701)

[indexing]
debounce_ms = 200
max_file_size_mb = 10   # Bỏ qua files lớn hơn mức này
exclude_patterns = [".git", "node_modules", ".venv", "target", "dist"]

[renderer]
theme = "system"        # "light" | "dark" | "system"
syntax_highlight_theme = "github-dark"

[search]
enable_fts = true
enable_semantic = false  # Deferred (YAGNI) — FTS5 keyword đủ; bật lại nếu có nhu cầu thật
```

---

## 11. Agent Instruction Mẫu

File này có thể đặt trong project's `CLAUDE.md` hoặc global `~/.claude/CLAUDE.md`:

```markdown
## Documentation Viewing (MDView)

MDView is running locally at http://localhost:7700 as a multi-project 
markdown server. Use it to let the user view generated docs in browser.

### After creating/updating markdown files:
1. Call `mdview_notify_file` with the absolute file path
2. Call `mdview_open_file` to get the browser URL
3. Tell the user: "You can view this at: <url>"

### Registering a new project (first time only):
Call `mdview_register_project` with:
- root_path: absolute path to project root
- name: human-readable project name

### MCP tools available:
- mdview_status — check if MDView is running
- mdview_register_project — register a project
- mdview_notify_file — notify of file changes
- mdview_open_file — get browser URL for a file
- mdview_search — search across project files
- mdview_list_projects — list registered projects
```

---

## 12. Success Metrics

| Metric | Target |
|---|---|
| Zero broken internal links sau khi project đăng ký | 100% |
| Thời gian từ agent tạo file → có thể xem trong browser | < 1 giây |
| Initial project scan (1000 files) | < 3 giây |
| Memory usage với 3 projects đăng ký (tổng 5000 files) | < 100MB |
| CLI commands thành công lần đầu (zero config) | 100% |
| Compatibility với Claude Code MCP integration | ✅ |

---

## Appendix A: Comparison với Tools Hiện Tại

| Feature | MDView (proposed) | Marky | mdserve | markdown-vault-mcp |
|---|---|---|---|---|
| Web server (remote view) | ✅ | ❌ (desktop app) | ✅ | ❌ |
| Multi-project registry | ✅ | ❌ | ❌ | Partial |
| Cross-folder link resolution | ✅ | ❌ | ❌ | ❌ |
| Recursive scan toàn project | ✅ | ✅ (workspace) | ❌ (1 level) | ✅ |
| MCP server interface | ✅ | ❌ | ❌ | ✅ (query only) |
| CLI agent integration | ✅ | Partial | ✅ (launch only) | ❌ |
| Live reload | ✅ | ✅ | ✅ | ❌ |
| Backlinks | ✅ | ❌ | ❌ | ❌ |
| Full-text search | ✅ | ✅ (fuzzy filename) | ❌ | ✅ |
| Persistent project registry | ✅ | ✅ (workspace) | ❌ | ❌ |
| Single binary | ✅ | ✅ | ✅ | ❌ |

---

*PRD này được soạn dựa trên research về landscape hiện tại (tháng 7/2026) và enriched với best practices từ mdserve, Marky, và markdown-vault-mcp.*