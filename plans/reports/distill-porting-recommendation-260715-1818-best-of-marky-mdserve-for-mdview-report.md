# Distill → Porting Recommendation: Best-of marky + mdserve cho mdview

**Ngày:** 2026-07-15 · **Nguồn:** marky (`5d02237`) + mdserve (`f84ae3e`) · **16 candidates đã score**
**Trạng thái:** đề xuất — quyết định adoption thuộc về người dùng (distill propose, human decide)

---

## Bottom line

- **mdview = server model của mdserve + engine đa-folder/search của marky + 3 thứ NEITHER nguồn nào có (phải tự build): cross-folder link resolution, multi-project registry, MCP.**
- Cả 2 nguồn là **Rust** → phần lớn candidate port gần thẳng, không phải viết lại.
- Nơi 2 nguồn phân kỳ, chọn theo cột *Best-in-class* của matrix, rồi **adapt theo scale + multi-project của mdview** (nguồn nhỏ hơn ta 1 bậc: single-folder/single-user).
- Ưu tiên: 7 candidate nền tảng (Tier 1) đủ cho Phase 1-2; phần còn lại là polish/desktop (Tier 2-3).

---

## Tier 1 — TAKE ngay (nền tảng, Rust near-direct, Phase 1-2)

| Candidate | Nguồn | Verdict | PRD | Ghi chú adapt |
|---|---|---|---|---|
| Recursive folder tree (`WalkBuilder`, tôn trọng .gitignore, lọc .md/.mdx, prune rỗng) | marky | **ADAPT** | G1, FR-06, FR-17 | Best-in-class vs mdserve flat. Đổi grouping: theo **project** (registry) thay vì git-repo-root. Đây là engine quét đa-folder — lõi của mdview. |
| Atomic + corrupt-resilient persistence (temp+rename, load hỏng→default, `data_dir()` cross-platform) | marky | **TAKE** | NFR-02, FR-02, §10 config | Trực tiếp cho registry write + `config.toml` của màn Settings. Bài học Rust, rẻ. |
| Canonicalize + prefix path-traversal guard (canonicalize → `starts_with(base)` → 403) | mdserve | **TAKE** | NFR-05 | mdview serve **cross-folder** → bề mặt traversal RỘNG hơn mdserve → càng bắt buộc. Áp cho mọi file+image serve. |
| Sanitize-before-serve (marky DOMPurify → **ammonia** Rust) | marky | **ADAPT** | Safety (untrusted agent md) | marky sanitize client cuối pipeline; mdview render **server-side** → dùng `ammonia` allowlist trước khi serve. Cross-cutting, rẻ, cao giá trị. |
| WebSocket reload-signal (broadcast channel → client reload, auto-reconnect 3s) | mdserve | **ADAPT** | G5, FR-19 | Reload-signal đơn giản/bền hơn push DOM patch. **Tension:** FR-19 muốn refresh riêng nội dung file, không reload cả trang → adapt thành scoped content refresh qua WS thay vì `location.reload()`. |
| File-watcher robustness (atomic-save: đừng 404 giữa lúc editor rename-save) | mdserve (+marky) | **ADAPT** | FR-08, FR-09 | mdserve né bằng ignore-delete; marky né bằng full re-walk. mdview: recursive watch + debounce 200ms + **incremental** index update (xem Tier-1 divergence dưới). |
| Pre-render-to-memory markdown pipeline (render 1 lần, cache HTML, strip frontmatter) | mdserve | **ADAPT** | NFR-01 (<100ms) | Cache HTML đã render (in-memory/SQLite). mdview thêm **1 pass rewrite link** (FR-11) vào pipeline trước khi cache. |

---

## Tier 2 — TAKE, rẻ / UX (Phase 2-3)

| Candidate | Nguồn | Verdict | PRD | Ghi chú |
|---|---|---|---|---|
| nucleo fuzzy file search (backend, `match_paths`, smart-case) | marky | **ADAPT** | FR-27 | nucleo cho **filename/path** fuzzy; kèm **SQLite FTS5** cho full-text content (PRD yêu cầu FTS). Hai loại search bổ sung nhau. mdserve KHÔNG có search → marky lấp trục này. |
| Theme system: no-flash head-script + nhiều theme + picker | mdserve | **TAKE** | FR-21 | mdserve best-in-class theming (5 theme Catppuccin, no-FOUC). Đọc theme trong `<head>` blocking script. Mermaid re-derive theme khi đổi. |
| Port auto-increment on bind conflict (thử 10 port kế) | mdserve | **TAKE** | §10, daemon lifecycle | Bỏ papercut "server cũ giữ 7700". Ăn khớp `daemon.lock` §7.5. |
| When-to-render agent skill heuristic (ngưỡng ~40-60 dòng / bảng / mermaid → render) | mdserve | **ADAPT** | G3, §5.7, §11 | Ranh giới "khi nào agent gọi `mdview_view_file`". Đưa vào AGENTS.md template + skill. Doc, rẻ. |
| Non-goal scope-guard doc (CLAUDE.md non-goals) | **marky + mdserve (hội tụ ≥2 nguồn)** | **TAKE** | §3.2 | Tín hiệu mạnh: cả 2 dùng CLAUDE.md non-goals giữ scope viewer. Viết cho mdview để chống scope-creep (đã có non-goals trong PRD → mở rộng thành rule doc). |

---

## Tier 3 — DEFER / có điều kiện (Phase 3-4 hoặc desktop)

| Candidate | Nguồn | Verdict | Lý do |
|---|---|---|---|
| Single-instance CLI forwarding (`tauri-plugin-single-instance`, forward argv) | marky | **TAKE khi desktop (Phase 4)** | Map trực tiếp vào `mdview-desktop` + `daemon.lock` §7.5. Không phải defer — chỉ đúng phase. |
| Multi-channel install (curl/brew/cargo/pacman/nix) | mdserve | **TAKE khi packaging (Phase 4)** | CLI binary distribution. Kèm Tauri bundle của marky cho desktop. NFR-04. |
| Copy-as-markdown qua source-map attrs | marky | **DEFER** | Differentiator hiếm nhưng YAGNI cho MVP. Phase 3+ nếu có nhu cầu. |
| Cmd+K command palette (pluggable, backend fuzzy) | marky | **DEFER** | UX polish. Ăn khớp nucleo nếu làm. Phase 3+. |
| Sidebar file-nav flat unified template | mdserve | **SKIP** | Thay bằng recursive tree của marky (Tier 1). mdserve flat non-recursive là đúng cái mdview vượt qua. |
| Split panes / tabs | marky | **SKIP (web)** | Desktop-app concept; web mdview không cần. Xét lại nếu desktop muốn. |

---

## Cross-source winners (nơi 2 nguồn phân kỳ)

| Trục | Chọn | Vì sao |
|---|---|---|
| Directory / multi-file | **marky** (recursive tree, hierarchical) | Đúng G1; mdserve flat 1-level là điểm yếu mdview vá. |
| File watching | **marky** (recursive) nhưng **incremental**, KHÔNG full re-walk | marky re-walk toàn tree mỗi event — ổn cho folder nhỏ, quá nặng cho NFR-03 (100k files). mdview giữ recursive+debounce nhưng update index tăng dần qua SQLite. **Đây là chỗ mdview cố tình lệch cả 2 nguồn do scale lớn hơn.** |
| Remote / network view | **mdserve** (server, `-H`) | marky desktop không remote được — đúng khoảng trống PRD. mdview = server model. |
| Distribution | **cả hai** | mdserve multi-channel cho CLI binary + marky Tauri bundle cho desktop. PRD đã ghi cả 2. |
| Module layout | **marky** (per-concern split) | Khớp quyết định clean-arch §7.4 (workspace core+adapters). mdserve 1 fat `app.rs` chỉ hợp scope nhỏ. |
| Changelog | **mdserve** (git-cliff auto từ conventional commits) | Tự động hoá, khớp commit convention. |
| Syntax highlight | **server-side (syntect), KHÔNG client Shiki** — synthesis | mdview đã chọn pre-render cache (mdserve) + sanitize-before-serve (ammonia). Server-side highlight (syntect trong Rust) khớp pipeline "render 1 lần → highlight → sanitize → cache → serve", không cần client JS. **Lệch PRD §9 (đang ghi Shiki/highlight.js client)** — cân nhắc đổi. Shiki chỉ hơn nếu cần đổi theme code-block client-side runtime. |

---

## BUILD ORIGINAL — không nguồn nào có (differentiators lõi của mdview)

| Cần | Trạng thái ở 2 nguồn | Ghi chú |
|---|---|---|
| **Cross-folder link resolution (G2, FR-11)** | ✗ cả hai (mdserve literal filename no-rewrite; marky relative không rewrite) | Đây là lý do mdview tồn tại. Thuật toán §7.3 là nguyên bản — không port được, phải tự viết + test kỹ. |
| **Multi-project registry (FR-01)** | ✗ (marky có folder-list persist nhưng không phải registry đa-root có id/url) | Có thể mượn pattern persist atomic của marky làm nền lưu trữ. |
| **MCP server (`mdview_view_file`)** | ✗ cả hai (mdserve chỉ có plugin/skill, không MCP) | Nguyên bản. Mượn "when-to-render heuristic" của mdserve cho phần agent-facing. |

---

## Khuyến nghị promote sang `planned` (chờ bạn chốt)

**Nhóm A — nền tảng, promote ngay (7):** recursive-tree (adapt), atomic-persistence, traversal-guard, sanitize-ammonia (adapt), ws-reload-signal (adapt), watcher-robustness (adapt), pre-render-cache (adapt).

**Nhóm B — Phase 2-3, promote sau A (5):** nucleo+FTS5 (adapt), theme-no-flash, port-auto-increment, when-to-render-heuristic (adapt), non-goal-doc.

**Nhóm C — giữ `candidate`, xét theo phase:** single-instance (Phase 4), multi-channel-install (Phase 4), copy-as-markdown (defer), command-palette (defer). **SKIP:** sidebar-flat, split-panes.

---

## Unresolved questions

1. **Syntax highlight: server-side syntect hay client Shiki?** Synthesis nghiêng server-side (khớp pre-render+ammonia); PRD §9 đang ghi Shiki client. Cần bạn chốt → sẽ update PRD §9.
2. **Live reload granularity:** full-page reload (mdserve, đơn giản) hay scoped content refresh (FR-19 yêu cầu)? Adapt cần thêm công.
3. **File-watch ở scale lớn:** xác nhận mdview đi incremental-index (lệch marky full-re-walk) — đúng NFR-03 nhưng phức tạp hơn.
4. Bạn muốn tôi **cập nhật `porting-log.md`** (đổi status Nhóm A/B sang `planned`) sau khi chốt, và **map candidate ↔ PRD phase** không?
