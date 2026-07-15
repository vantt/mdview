---
name: marky
type: git-repo
url: https://github.com/GRVYDEV/marky
local: upstreams/marky
last_analyzed_commit: 5d02237
last_analyzed_date: 2026-07-15
domains_covered: [rendering, live-reload, file-indexing, ux, tooling, safety, config-packaging, repo-layout, testing-evals, docs-style, http-serving, link-resolution, skills]
---

# marky — Feature Index

Marky = desktop markdown **viewer** (Tauri v2 + React + markdown-it), "open `.md`
từ terminal, render đẹp, live reload". Cùng category với mdview nhưng là native app
(không web server, không remote view, không MCP, không multi-project registry).
Đối trọng của mdserve trên hầu hết trục — điền cột marky trong comparison-matrix.

---

## rendering

### markdown-render-pipeline
- **What:** `markdown-it` (html+linkify+typographer) + 4 plugin (anchor, footnote, task-lists, front-matter) → DOMPurify sanitize là bước CUỐI → async Shiki highlight + async mermaid render trên client. Front-matter YAML strip parse-time bằng plugin callback rỗng (không regex).
- **Where:** `src/lib/markdown.ts`, `src/components/Viewer.tsx`
- **Notable:** Core rule tự chèn `data-source-map="start,end"` lên mọi block token → sống sót qua plugin + renderer + cả DOM replacement của Shiki/mermaid (nền tảng cho copy-as-markdown). Fence "mermaid" đánh dấu `<pre class="mermaid-pending">`, hoãn render sang client. Sanitize sau cùng nên plugin tự do chèn markup, chỉ HTML final bị lọc.
- **Keywords:** markdown-it, source-map attrs, DOMPurify, mermaid-pending, front-matter strip
- **Seen:** 5d02237

## live-reload

### live-reload-watcher
- **What:** File watch per-folder bằng `notify-debouncer-full`, debounce 200ms; mỗi event → re-walk TOÀN BỘ tree của folder rồi emit `folder://changed` + `file://changed` (kèm path list). Frontend listener cập nhật tab đang mở qua `UPDATE_TAB_SOURCE`.
- **Where:** `src-tauri/src/watcher.rs`, `src/App.tsx`
- **Notable:** Không incremental — full re-walk mỗi lần đổi (đơn giản, đủ nhanh cho tree markdown). Hai event tách folder-level vs file-level để UI chọn phản ứng. Đây chính là use-case "xem plan Claude đang viết live".
- **Keywords:** notify, debouncer, folder://changed, file://changed, watch recursive
- **Seen:** 5d02237

## file-indexing

### file-watcher-debounced
- **What:** `WatcherHandle` giữ debouncer sống; `Watchers` = `HashMap<folder_id, handle>` guarded Mutex, insert/remove theo add/remove folder. Watch recursive.
- **Where:** `src-tauri/src/watcher.rs`
- **Notable:** Đối chiếu mdserve (non-recursive, ignore-delete để sống qua rename-save): marky recursive + full re-walk, không xử lý riêng delete-race vì luôn re-walk từ đầu → khác cách nhưng cùng mục tiêu robustness.
- **Keywords:** notify-debouncer-full, 200ms, recursive
- **Seen:** 5d02237

---

## ux

### theme-system-follow
- **What:** Theme light/dark/system; "system" nghe `(prefers-color-scheme: dark)` qua matchMedia và cập nhật `resolved`. Toggle bằng class `dark` trên `<html>` (chuẩn shadcn); mọi màu đọc từ CSS variables → markdown.css theme-agnostic.
- **Where:** `src/lib/theme.tsx`, `src/styles/markdown.css`
- **Notable:** Shiki KHÔNG re-init khi đổi theme (2 theme bundled sẵn, chọn theo tham số per-call); mermaid thì `initialize()` lại mỗi lần render để bắt theme mới. `color-mix(in oklch, …)` cho màu pha (table zebra, nút mờ) — không cần SCSS.
- **Keywords:** prefers-color-scheme, matchMedia, dark class, CSS variables, color-mix
- **Seen:** 5d02237

### command-palette
- **What:** Cmd+K palette (`cmdk` + shadcn command) với section cắm được: Actions (open/add-folder/split/theme/zoom), Jump-to-folder, File results. Query file gọi backend `search_files` debounce 25ms + cancellation token chống stale.
- **Where:** `src/components/CommandPalette.tsx`
- **Notable:** Fuzzy match chạy ở Rust (nucleo), không JS — UI chỉ render kết quả. Empty query = hiện actions + jump-to-folder; có query = file results grouped theo folder.
- **Keywords:** cmdk, Cmd+K, searchFiles, debounce, pluggable sections
- **Seen:** 5d02237

### in-document-search
- **What:** Cmd+F: tree-walk text node của article, bọc match trong `<mark class="doc-search-match">`, cycle Enter/Shift+Enter, counter "3/12", scroll-into-view smooth, Esc đóng.
- **Where:** `src/lib/docSearch.ts`, `src/components/DocSearch.tsx`
- **Notable:** Case-insensitive, bỏ qua SCRIPT/STYLE, `clear()` phục hồi text gốc. Re-highlight theo `contentNonce` (bump sau khi render xong) — đồng bộ với pipeline async.
- **Keywords:** Cmd+F, mark, tree walk, contentNonce, cycle
- **Seen:** 5d02237

### table-of-contents
- **What:** Sidebar phải (chỉ khi 1 pane) render outline từ `extractHeadings(source)` lọc level 1-4, anchor `href="#slug"`, indent theo level.
- **Where:** `src/components/TableOfContents.tsx`, `src/lib/markdown.ts`
- **Notable:** `extractHeadings` parse token markdown-it (không regex), tái tạo slug nếu thiếu — dùng chung slugify với markdown-it-anchor nên anchor khớp.
- **Keywords:** extractHeadings, TOC, heading outline, slug
- **Seen:** 5d02237

### split-panes-tabs
- **What:** Workspace = reducer thuần (`workspace.ts`) quản 1-2 pane, mỗi pane có tabs. Split V (Cmd+\) / H (Cmd+Shift+\); OPEN_FILE tái dùng tab nếu path đã mở; đóng tab cuối của pane tự collapse split.
- **Where:** `src/lib/workspace.ts`, `src/App.tsx`
- **Notable:** SPLIT tạo pane RỖNG (không clone tab active) — comment giải thích tránh renderer contention (2 pane cùng render nặng). `compact()` dọn pane rỗng. State thuần immutable, test kỹ.
- **Keywords:** useReducer, panes, tabs, split, compact
- **Seen:** 5d02237

### folder-workspace-tree
- **What:** Sidebar trái: nhiều folder "workspace" bền vững (kiểu Obsidian), mỗi folder render tree đệ quy, group theo git repo_root (hoặc flat), collapse per-folder/per-group, hover hiện nút xóa.
- **Where:** `src/components/FolderSidebar.tsx`, `src/components/FileTree.tsx`, `src-tauri/src/folder.rs`
- **Notable:** Tree build ở Rust bằng `ignore::WalkBuilder` (tôn trọng .gitignore) lọc chỉ `.md/.markdown/.mdx`, prune dir rỗng; grouping repo tính `find_git_repo_root` (đi lên tìm `.git`, dừng ở home). Đối chiếu mdserve flat non-recursive → marky recursive hierarchical (đúng khoảng trống mdview nhắm).
- **Keywords:** WalkBuilder, ignore, repo_root grouping, TreeNode, recursive
- **Seen:** 5d02237

### source-map-copy-as-markdown
- **What:** Copy trong article ghi MARKDOWN GỐC (không phải HTML): map selection DOM về dòng nguồn qua `data-source-map`, tìm min-start/max-end, cắt `source.split("\n").slice(...)`, ghi clipboard.
- **Where:** `src/lib/copyAsMarkdown.ts`, `src/components/Viewer.tsx`
- **Notable:** Differentiator hiếm — nhờ source-map attrs chèn parse-time (ổn định qua Shiki/mermaid replacement). Chuẩn hoá selection ngược. Bật/tắt theo preference; listener gắn trên article, không global.
- **Keywords:** copy, data-source-map, clipboardData, source range
- **Seen:** 5d02237

### scroll-memory
- **What:** Map keyed theo filePath lưu vị trí scroll; khôi phục khi mở lại file/remount Viewer.
- **Where:** `src/components/Viewer.tsx`
- **Notable:** State module-level ngoài React (Map), không persist disk — chỉ trong phiên. Rẻ, cải thiện đọc file dài nhiều lần.
- **Keywords:** scroll position, per-file, Map
- **Seen:** 5d02237

### zoom-text-size
- **What:** Zoom app-wide (Cmd+ +/-/0) đặt `document.documentElement.style.fontSize = 16*zoom` px → mọi text rem-based scale cùng lúc (sidebar, toolbar, palette, content). Range 0.7-1.6x.
- **Where:** `src/lib/preferences.tsx`, `src/components/Toolbar.tsx`
- **Notable:** Một biến root fontSize thay vì scale từng component — đơn giản, nhất quán. Persist qua preferences.
- **Keywords:** zoom, root font-size, rem, Cmd+Plus
- **Seen:** 5d02237

### resizable-sidebars
- **What:** `ResizeHandle` kéo bằng pointer đổi width sidebar trái/phải; double-click reset default. Hit area vô hình 1.5px, hover/drag đổi màu.
- **Where:** `src/components/ResizeHandle.tsx`, `src/lib/preferences.tsx`
- **Notable:** Width persist (localStorage + backend), clamp (trái 180-400, phải 160-360). Pointer capture chuẩn, delta trái = +delta, phải = -delta.
- **Keywords:** ResizeHandle, pointer capture, sidebar width, clamp
- **Seen:** 5d02237

### code-copy-overlay
- **What:** Gắn nút "Copy" trực tiếp DOM lên mọi `<pre>` (trừ mermaid), idempotent (skip nếu đã có). Click copy text, đổi "Copied!" 1200ms.
- **Where:** `src/components/CodeCopyOverlay.tsx`, `src/styles/markdown.css`
- **Notable:** Direct DOM (không rebuild React node) để không đụng Shiki spans; nút absolute top-right, opacity 0 → hiện on `pre:hover`.
- **Keywords:** copy button, direct DOM, idempotent, pre hover
- **Seen:** 5d02237

---

## tooling

### cli-first-invocation
- **What:** `marky FILE` mở file, `marky FOLDER` mở workspace, `marky` (no arg) khôi phục phiên trước. Resolve args ở Rust: skip cờ `--*`, canonicalize path đầu, `classify()` → enum `InitialTarget {File|Folder|None}` truyền lên frontend qua event + state.
- **Where:** `src-tauri/src/cli.rs`, `src-tauri/src/lib.rs`
- **Notable:** Canonicalize (resolve symlink + relative → absolute) fail gracefully nếu path không tồn tại. macOS "Open With" đi qua cùng `handle_target()`.
- **Keywords:** InitialTarget, canonicalize, classify, argv
- **Seen:** 5d02237

### single-instance-forwarding
- **What:** `tauri-plugin-single-instance`: `marky` lần 2 KHÔNG mở cửa sổ mới mà forward argv cho instance đang chạy → `handle_target()` add folder / mở file / focus window.
- **Where:** `src-tauri/src/lib.rs`, `scripts/install-cli.sh`
- **Notable:** Wrapper CLI phải exec binary trực tiếp (không `open -a`) để plugin thấy file-lock ở `$TMPDIR` và forward; wrapper canonicalize path lúc invoke (không lúc gen) rồi `nohup`+disown.
- **Keywords:** single-instance, forward argv, file lock, handle_target
- **Seen:** 5d02237

### nucleo-fuzzy-search
- **What:** Fuzzy search file bằng crate `nucleo` (`Config::DEFAULT.match_paths()`), haystack = `folder_name/relative_path`, smart-case, sort desc score, truncate limit (default 50). Empty query = fast path trả n file đầu score 0.
- **Where:** `src-tauri/src/search.rs`, `src-tauri/src/registry.rs`
- **Notable:** Index phẳng (`Vec<IndexedFile>`) rebuild từ trees mỗi khi folder đổi; nucleo tối ưu riêng cho path. Search ở backend, không JS. mdserve KHÔNG có search → marky lấp trục này.
- **Keywords:** nucleo, match_paths, smart case, IndexedFile, flat index
- **Seen:** 5d02237

---

## safety

### dompurify-sanitize
- **What:** `DOMPurify.sanitize()` là bước cuối của renderMarkdown; allowlist attr (`target,class,id,aria-hidden,data-source-map`) + add tag `<section>`, trước khi `dangerouslySetInnerHTML`.
- **Where:** `src/lib/markdown.ts`, `src/components/Viewer.tsx`
- **Notable:** "Safe to view untrusted markdown" — sanitize SAU khi plugin/renderer chạy nên chỉ HTML final bị lọc, không double-sanitize. Đối chiếu mdserve dùng path-traversal-guard (bề mặt khác); marky là XSS-surface guard.
- **Keywords:** DOMPurify, sanitize, allowlist, dangerouslySetInnerHTML, XSS
- **Seen:** 5d02237

### capability-allowlist
- **What:** Tauri capability tối thiểu: core + opener (link ngoài) + dialog + event listen/unlisten. KHÔNG cấp `fs:read/write` blanket, không `shell:execute`.
- **Where:** `src-tauri/capabilities/default.json`, `src-tauri/tauri.conf.json`
- **Notable:** File chỉ đọc qua command Rust có validate, không để webview truy cập fs tuỳ tiện — least-privilege. (`csp: null` là điểm nới lỏng, chấp nhận vì app local.)
- **Keywords:** Tauri capabilities, least privilege, no blanket fs, opener
- **Seen:** 5d02237

### read-only-viewer
- **What:** Marky KHÔNG BAO GIỜ ghi vào file/folder user; mọi state (folder list, prefs, recent) ở `app_data_dir/settings.json`. Không editor (read-only by design).
- **Where:** `CLAUDE.md`, `src-tauri/src/settings.rs`
- **Notable:** Rule kiến trúc rõ trong CLAUDE.md: "No user data outside app_data_dir", "no remote fetch runtime (trừ ảnh trong markdown)". Folder chỉ là pointer.
- **Keywords:** read-only, app_data_dir, pointer, no editor
- **Seen:** 5d02237

### file-size-utf8-guard
- **What:** `read_text` từ chối file > 25MB, validate UTF-8, lỗi → `AppError::Invalid`.
- **Where:** `src-tauri/src/fs.rs`, `src-tauri/src/error.rs`
- **Notable:** No streaming (đọc cả file vào RAM) — chấp nhận với ngưỡng 25MB. Guard rẻ chống mở nhầm binary/file khổng lồ.
- **Keywords:** MAX_FILE_BYTES, 25MB, UTF-8, AppError
- **Seen:** 5d02237

---

## config-packaging

### settings-persistence
- **What:** Prefs 2 lớp: localStorage (nhanh) + Tauri backend (bền), reconcile lúc mount, `persistToBackend` debounce 300ms. Backend ghi `data_dir()/marky/settings.json` atomic (temp + rename), load resilient (JSON hỏng → default, không crash).
- **Where:** `src-tauri/src/settings.rs`, `src/lib/preferences.tsx`
- **Notable:** `data_dir()` cross-platform (`dirs`): macOS Application Support, Linux `~/.local/share`, Windows `%APPDATA%`. `folders` field có `alias="vaults"` để tương thích ngược. recent_files dedup, cap 20. Atomic write chống corruption khi crash (mdview PRD cũng yêu cầu atomic registry write).
- **Keywords:** settings.json, atomic write, data_dir, localStorage, debounce, resilient load
- **Seen:** 5d02237

### install-cli-script
- **What:** `install-cli.sh` tạo wrapper `~/.local/bin/marky`: tìm binary (app bundle / release / debug), gen script convert arg tương đối → tuyệt đối lúc chạy, `nohup` + disown, cảnh báo nếu `~/.local/bin` không trong PATH.
- **Where:** `scripts/install-cli.sh`
- **Notable:** Path resolve lúc INVOKE (không lúc gen) nên đúng dù đổi cwd; cần cho single-instance thấy path tuyệt đối. Không dùng `open -a` để plugin single-instance hoạt động.
- **Keywords:** wrapper script, ~/.local/bin, nohup, absolute path, PATH warning
- **Seen:** 5d02237

### version-bump-script
- **What:** `bump-version.sh <semver>` sửa version đồng bộ ở package.json, tauri.conf.json, Cargo.toml (dòng 3), chạy `cargo check` regen lock, commit 4 file, tag `v$VERSION`, push.
- **Where:** `scripts/bump-version.sh`
- **Notable:** Một nguồn version phải khớp 3 nơi (Tauri constraint) — script hoá tránh drift. Dùng `sed -i ''` (BSD) — portability Linux cần chỉnh.
- **Keywords:** bump version, semver, tag, cargo check, atomic bump
- **Seen:** 5d02237

### tauri-bundle-config
- **What:** `tauri.conf.json`: 1 window 1200×800 (min 600×400), drag-drop on, `csp: null`, bundle active (.dmg/.deb/.AppImage), identifier `dev.marky.app`. Release profile Cargo: `opt-level="s"`, lto, `codegen-units=1`, `panic="abort"`, strip → binary < 15MB.
- **Where:** `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`
- **Notable:** "Small & fast, no Electron" — .dmg < 15MB nhờ webview native + profile size-optimized. Linux .deb + AppImage amd64/arm64.
- **Keywords:** tauri.conf, opt-level s, lto, panic abort, dmg deb AppImage
- **Seen:** 5d02237

---

## repo-layout

### module-boundaries
- **What:** `src/` (React) vs `src-tauri/src/` (Rust) tách rõ; Rust chia module theo concern (cli, commands, folder, fs, registry, search, settings, watcher, error), `main.rs` mỏng → `lib.rs::run()`. Mọi `invoke()` đi qua wrapper `src/lib/tauri.ts`.
- **Where:** `src-tauri/src/lib.rs`, `src/lib/tauri.ts`, `CLAUDE.md`
- **Notable:** Folder state gom 1 chỗ `Arc<RwLock<FolderRegistry>>`; command tập trung `commands.rs` đăng ký ở `lib.rs`. Ranh giới sạch, dễ soi cho project cùng loại.
- **Keywords:** src vs src-tauri, thin main, tauri.ts wrapper, module split
- **Seen:** 5d02237

### claude-md-conventions
- **What:** `CLAUDE.md` là doc "authoritative stack + rules": liệt kê stack, layout, convention Rust/frontend, folder model, security, và loạt rule "No …" (No Electron, No editor, No data ngoài app_data_dir, No remote fetch, Don't over-abstract).
- **Where:** `CLAUDE.md`
- **Notable:** Rule dạng phủ định (non-goals) giữ scope viewer chặt — mẫu tốt cho mdview tự ràng buộc scope.
- **Keywords:** CLAUDE.md, conventions, non-goals, stack authoritative
- **Seen:** 5d02237

---

## testing-evals

### vitest-happy-dom
- **What:** Test frontend bằng Vitest + happy-dom: markdown.test (14 ca: anchor, table, task-list, code, mermaid-flag, external link, sanitize, front-matter, hr, mid-doc YAML, heading extract), workspace.test (reducer), theme.test, docSearch.test.
- **Where:** `src/lib/markdown.test.ts`, `src/lib/workspace.test.ts`, `src/lib/docSearch.test.ts`
- **Notable:** Test tập trung pure logic (render correctness, reducer invariants, DOM search) — chỗ đáng test nhất, không test UI chrome. happy-dom nhẹ hơn jsdom.
- **Keywords:** vitest, happy-dom, reducer test, render test
- **Seen:** 5d02237

### rust-cli-tests
- **What:** Test `cli.rs`: no-arg→None, missing path→None, file→File, dir→Folder, cờ `--*` bị skip.
- **Where:** `src-tauri/src/cli.rs`
- **Notable:** Unit test biên CLI classification ngay cạnh code (Rust inline `#[cfg(test)]`).
- **Keywords:** cli test, classify, tempfile
- **Seen:** 5d02237

---

## docs-style

### keep-a-changelog
- **What:** `CHANGELOG.md` theo chuẩn Keep a Changelog (Unreleased / [x.y.z] - date, Added/Fixed).
- **Where:** `CHANGELOG.md`
- **Notable:** Đi kèm `bump-version.sh` + tag — release flow gọn cho app desktop.
- **Keywords:** keep a changelog, unreleased, semver
- **Seen:** 5d02237
