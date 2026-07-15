# Product Backlog

PBI rows cho mdview. Status: `proposed` → `in-flight` → `done`.
Chất liệu cho các mục port: [docs/distillery/porting-log.md](distillery/porting-log.md).

| ID | Title | Status | Source | Detail |
|---|---|---|---|---|
| PBI-01 | nucleo fuzzy file search (jump-to-file theo tên/đường dẫn) | proposed | marky:nucleo-fuzzy-search (porting-log) | Wire `nucleo-matcher` (đã có trong Cargo.toml, hiện 0 usage) thành fuzzy search theo tên/đường dẫn file, song song với FTS5 (FTS5 = tìm nội dung; nucleo = nhảy nhanh tới file). Smart-case; haystack = relative path đã index; rebuild index phẳng. Đích khả dĩ: `crates/mdview-core` (matcher) + `crates/mdview/src/server.rs` (endpoint). Port Rust→Rust gần thẳng từ marky. |
| PBI-02 | Copy-as-markdown qua source-map attrs | proposed | marky:source-map-copy-as-markdown (porting-log) | Chèn `data-source-map` lúc parse để map vùng chọn DOM → dòng markdown gốc; copy ra **markdown thô**, không phải HTML đã render. Cần: attrs lúc parse (`crates/mdview-core/src/render.rs`) + client JS xử lý selection→copy (`crates/mdview/src/views.rs` / static app.js). Differentiator hiếm, hợp workflow agent/dev. |
| PBI-03 | Zoom mermaid diagram (kể cả fullscreen) | proposed | user request 2026-07-16 | Tìm & đánh giá thư viện cho phép **zoom/pan diagram mermaid, lý tưởng có chế độ fullscreen**. Mermaid hiện render client-side ra SVG (xem "How it works" README). Ứng viên khả dĩ: `svg-pan-zoom`, `@panzoom/panzoom` (anvaka), `d3-zoom`. Việc = research chọn lib + wrap SVG mermaid với controls zoom/pan + nút fullscreen. Đích: client JS + `crates/mdview/src/views.rs` (khối `<script type="module">` mermaid). |

<!-- Các PBI do người dùng chốt adopt, xếp hàng cho session đang chạy nhặt khi xong việc hiện tại. Không thực thi ngay. -->
