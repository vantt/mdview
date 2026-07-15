# Porting Log

Nguồn sự thật duy nhất về trạng thái porting. Tính năng bị từ chối vẫn ghi lại kèm lý do.

- Status: `candidate` → `planned` → `in-progress` → `ported` / `adapted` / `rejected`
- Score `R# E# F#` chấm một lần lúc tạo candidate; xếp hạng bằng `distill.mjs rank`.
- Local = tên trong project ta sau khi port (bắt buộc khi ported/adapted); tra hai chiều bằng `distill.mjs map`.

| Feature | Nguồn | Status | Score | Local | Đích (path) | Commit | Ghi chú / Lý do |
|---|---|---|---|---|---|---|---|
| file-watcher atomic-save handling (ignore delete) | mdserve:file-watcher-notify | planned | R2 E2 F1 | — | — | f84ae3e | Bài học robustness: watcher ngây thơ sẽ 404 giữa lúc editor rename-save. Bug thực (v0.5.1). Rẻ để áp dụng, high leverage. |
| WebSocket reload-signal live reload | mdserve:websocket-live-reload | planned | R2 E2 F2 | — | — | f84ae3e | Reload-signal + server re-render đơn giản & bền hơn push DOM patch; auto-reconnect 3s. Phủ G5 của mdview. |
| Pre-render-to-memory markdown pipeline | mdserve:markdown-render-pipeline | planned | R2 E2 F2 | — | — | f84ae3e | Render 1 lần lúc startup/change, cache HTML in-memory; server-side highlight; strip frontmatter. Stack mdview khác Rust → có thể adapt. |
| When-to-render agent skill heuristic | mdserve:claude-skill-render-heuristics | planned | R2 E1 F1 | — | — | f84ae3e | Ranh giới quyết định "khi nào bật preview" — tái dùng cho MCP/skill của mdview (G3). Doc/convention, rẻ. |
| Canonicalize+prefix path-traversal guard | mdserve:path-traversal-guard | planned | R2 E1 F1 | — | — | f84ae3e | mdview serve cross-folder → bề mặt traversal RỘNG hơn mdserve; guard này càng cần. |
| No-flash theme via blocking head script | mdserve:theme-system-no-flash | planned | R1 E1 F1 | — | — | f84ae3e | Đọc theme từ localStorage trong script chặn ở <head> để tránh FOUC. UX polish nhỏ. |
| Port auto-increment on bind conflict | mdserve:unified-http-router | planned | R1 E1 F1 | — | — | f84ae3e | Thử 10 port kế tiếp, báo port thực. Bỏ papercut server cũ giữ 3000. |
| Sidebar file-nav (unified single/dir template) | mdserve:sidebar-file-nav | rejected | R2 E1 F2 | — | — | f84ae3e | mdview cần nav nhưng ĐỆ QUY + hierarchical → adapt, không port thẳng (mdserve flat). **REJECTED:** thay bằng marky:folder-workspace-tree (recursive tree). |
| Recursive folder tree + git-repo grouping | marky:folder-workspace-tree | planned | R3 E2 F2 | — | — | 5d02237 | ignore::WalkBuilder (tôn trọng .gitignore) lọc .md/.markdown/.mdx, prune dir rỗng, group theo find_git_repo_root. Rust → mdview tái dùng gần thẳng. Đúng G1 (đa folder/subfolder). Best-in-class vs mdserve flat. |
| Atomic + corrupt-resilient settings persistence | marky:settings-persistence | planned | R2 E2 F1 | — | — | 5d02237 | temp+rename atomic write, load hỏng→default không crash, data_dir() cross-platform. PRD mdview yêu cầu atomic registry write (FR-02) → bài học trực tiếp, Rust, rẻ. |
| Sanitize-before-serve (DOMPurify → ammonia) | marky:dompurify-sanitize | planned | R3 E2 F1 | — | — | 5d02237 | marky sanitize là bước cuối render, allowlist attr, "safe untrusted markdown". mdview render server-side → adapt sang sanitize Rust (ammonia) trước khi serve. Cross-cutting safety, rẻ. |
| nucleo fuzzy file search (backend) | marky:nucleo-fuzzy-search | planned | R2 E2 F2 | — | — | 5d02237 | Config::DEFAULT.match_paths(), haystack folder/relative_path, smart-case, flat index rebuild. mdview là Rust → nucleo port gần thẳng. Lấp trục Search mà mdserve thiếu; PRD marky-column ghi có fuzzy. |
| Live reload recursive full re-walk (notify-debouncer-full) | marky:live-reload-watcher | rejected | R2 E2 F2 | — | — | 5d02237 | debounce 200ms, re-walk toàn tree, emit folder+file event tách biệt. **REJECTED:** full re-walk mỗi event quá nặng ở NFR-03 (100k files) → mdview đi incremental index + re-scan trigger (PRD FR-09b). Giữ ý debounce 200ms. |
| Copy-as-markdown qua source-map attrs | marky:source-map-copy-as-markdown | candidate | R2 E1 F2 | — | — | 5d02237 | data-source-map chèn parse-time map selection DOM → dòng markdown gốc; copy ra markdown không phải HTML. Differentiator hiếm; adapt sang client JS của mdview. |
| Cmd+K command palette (pluggable, backend fuzzy) | marky:command-palette | candidate | R2 E1 F2 | — | — | 5d02237 | cmdk + section cắm được (actions/jump-folder/files), fuzzy ở backend, debounce+cancel. UX nav mạnh; web mdview adapt. Ăn khớp với nucleo-fuzzy-search. |
| Non-goal rules doc (CLAUDE.md scope guard) | marky:claude-md-conventions + mdserve:claude-md-design-constraints | planned | R1 E3 F1 | — | — | 5d02237, f84ae3e | **Hội tụ độc lập ≥2 nguồn** → E1↑E3 (backfill mdserve 2026-07-15). Cả hai dùng CLAUDE.md non-goals giữ scope viewer (marky: No editor/No data ngoài app_data_dir/No remote; mdserve: non-recursive intentional/zero-config/agent-companion, không thành platform). Convention rẻ, mẫu mạnh cho mdview tự ràng buộc scope. |
