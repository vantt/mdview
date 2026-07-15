# Feature Comparison Matrix

So sánh tính năng giữa các learning sources. Mỗi domain một bảng; ô có ✓ link về entry trong `sources/<name>.md#<slug>`. Ký hiệu: ✓ có | ~ một phần | ✗ không | ? chưa khảo sát. Matrix là curated view — chỉ hàng có đối chiếu đáng giá, không exhaustive.

Các trục dưới đây chọn theo differentiators trong PRD của mdview.

## Core markdown-server axes

| Axis | mdserve | marky | Best-in-class | Ghi chú |
|---|---|---|---|---|
| Live reload | ✓ WebSocket reload-signal [→](sources/mdserve.md#websocket-live-reload) | ✓ notify events → UPDATE_TAB_SOURCE [→](sources/marky.md#live-reload-watcher) | tie | mdserve: web push reload-signal + re-render; marky: desktop event → cập nhật tab. Cùng UX "xem plan Claude live" |
| File watching | ✓ notify, non-recursive, ignore-delete [→](sources/mdserve.md#file-watcher-notify) | ✓ debouncer-full 200ms, recursive, full re-walk [→](sources/marky.md#file-watcher-debounced) | marky (recursive) | mdserve né delete-race bằng ignore-delete; marky bằng full re-walk. Marky recursive |
| Directory / multi-file | ~ immediate dir only, flat [→](sources/mdserve.md#sidebar-file-nav) | ✓ recursive tree, repo-grouped, đa folder [→](sources/marky.md#folder-workspace-tree) | marky | Marky đệ quy + hierarchical + group theo git repo — đúng hướng G1 mdview |
| Cross-folder link resolution | ✗ literal filename, no rewrite [→](sources/mdserve.md#flat-filename-url-routing) | ✗ relative link không rewrite (chỉ http → target=_blank) | — | Cả hai thiếu — G2 là differentiator riêng của mdview |
| Markdown rendering | ✓ markdown-rs GFM, pre-render cache [→](sources/mdserve.md#markdown-render-pipeline) | ✓ markdown-it client, DOMPurify, source-map attrs [→](sources/marky.md#markdown-render-pipeline) | tie | mdserve server-side pre-render; marky client-side + source-map copy-as-markdown |
| Mermaid | ✓ bundled, client transform, theme-aware [→](sources/mdserve.md#mermaid-client-render) | ✓ mermaid-pending → client render, re-init theme [→](sources/marky.md#markdown-render-pipeline) | tie | Cùng client-side, theme-aware. marky re-init mỗi render để bắt theme |
| Syntax highlight | ✓ server-side [→](sources/mdserve.md#markdown-render-pipeline) | ✓ Shiki lazy singleton, per-call theme [→](sources/marky.md#theme-system-follow) | marky (Shiki) | Shiki 2 theme bundled, dynamic lang load, VS Code grammar |
| Theming | ✓ 5 themes, picker, no-flash [→](sources/mdserve.md#theme-system-no-flash) | ✓ light/dark/system, matchMedia, CSS vars [→](sources/marky.md#theme-system-follow) | mdserve (nhiều theme) | mdserve 5 theme Catppuccin; marky theo system, ít theme hơn |
| Search | ✗ | ✓ nucleo fuzzy (Cmd+K) + Cmd+F in-doc [→](sources/marky.md#nucleo-fuzzy-search) | marky | mdserve không có; marky fuzzy file ở Rust + in-doc DOM search |
| Copy-as-markdown | ✗ | ✓ source-map DOM→dòng nguồn [→](sources/marky.md#source-map-copy-as-markdown) | marky | Differentiator hiếm — copy ra markdown gốc không phải HTML |
| Split panes / tabs | ✗ | ✓ reducer 1-2 pane tabbed [→](sources/marky.md#split-panes-tabs) | marky | Desktop app; web mdview có thể không cần |
| Remote/network view | ~ `-H/--hostname` opt-in [→](sources/mdserve.md#cli-surface) | ✗ desktop app, không server | mdserve | marky không remote được — đúng điểm yếu PRD nêu |
| Multi-project registry | ✗ | ~ folder list bền, không đa-root "project" | — | marky có folder workspace persist nhưng không phải registry đa-root — differentiator lõi mdview |

## Agent integration

| Axis | mdserve | marky | Best-in-class | Ghi chú |
|---|---|---|---|---|
| Claude Code plugin/skill | ✓ plugin + when-to-render skill [→](sources/mdserve.md#claude-skill-render-heuristics) | ✗ (chỉ có CLAUDE.md convention doc) | mdserve | marky không tích hợp agent; chỉ dùng Claude Code để dev |
| MCP server | ✗ (CLI/skill only) | ✗ | — | mdview G3 muốn MCP; cả hai reference đều chưa |
| CLI invocation | ✓ `mdserve <path>` server [→](sources/mdserve.md#cli-surface) | ✓ `marky FILE/FOLDER/none` + single-instance [→](sources/marky.md#cli-first-invocation) | marky (single-instance) | marky forward argv cho instance đang chạy thay vì mở cửa sổ mới |
| Distribution | ✓ curl/brew/cargo/pacman/nix [→](sources/mdserve.md#multi-channel-install) | ✓ brew cask / .deb / AppImage [→](sources/marky.md#tauri-bundle-config) | mdserve (nhiều kênh) | mdserve single static binary nhiều kênh hơn; marky app bundle |

## Project conventions

| Axis | mdserve | marky | Best-in-class | Ghi chú |
|---|---|---|---|---|
| Non-goals / scope-guard doc | ✓ CLAUDE.md design-constraints [→](sources/mdserve.md#claude-md-design-constraints) | ✓ CLAUDE.md non-goal rules [→](sources/marky.md#claude-md-conventions) | tie (hội tụ) | **Hội tụ độc lập** — cả hai dùng CLAUDE.md non-goals giữ scope viewer. Tín hiệu mạnh cho mdview. |
| Changelog style | ✓ git-cliff generated from commits [→](sources/mdserve.md#conventional-commit-conventions) | ✓ hand-kept Keep-a-Changelog [→](sources/marky.md#keep-a-changelog) | mdserve (auto) | Cùng họ commit-driven; mdserve sinh tự động + commitlint CI, marky tay |
| Module layout | ~ 1 fat app.rs (binary-only) [→](sources/mdserve.md#compact-single-file-server) | ✓ per-concern split src vs src-tauri [→](sources/marky.md#module-boundaries) | marky (scale) / mdserve (nhỏ gọn) | mdserve dồn cả server vào app.rs hợp scope nhỏ; marky tách module rõ hợp app lớn |
| Test strategy | ✓ inline axum-test integration (37) [→](sources/mdserve.md#inline-axum-integration-tests) | ✓ vitest+happy-dom + inline rust cli [→](sources/marky.md#vitest-happy-dom) | tie (khác tầng) | Cả hai test phần lõi, không test chrome; mdserve ở tầng request/response, marky ở pure logic |
| Sanitization / safety | ✓ path-traversal-guard [→](sources/mdserve.md#path-traversal-guard) | ✓ DOMPurify + capability allowlist [→](sources/marky.md#dompurify-sanitize) | tie (khác bề mặt) | mdserve chống traversal (server); marky chống XSS + least-privilege fs (client) |
</content>
