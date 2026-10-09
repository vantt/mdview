# Xác minh lại 8 lỗi UAT sau bản sửa (UAT daemon http://100.121.46.35:7800)

Ngày: 2026-10-09 (Asia/Saigon). agent-browser 0.38.2, Chrome `--no-sandbox`. Ảnh: `/tmp/claude-1000/-home-vantt-projects-mdview/5b62e0d2-df2d-4394-bcce-773b6c21271e/scratchpad/uat-shots2/`.
Kết quả: 6 FIXED, 1 FIXED một phần (B8, còn tràn ngang ở các trang `main.fg-page`), 1 không đổi theo thiết kế (B3). Không có lỗi console/page error.

## Từng lỗi

| Lỗi | Kết quả | Bằng chứng |
|---|---|---|
| B1 BOM/tiêu đề | FIXED | Sau khi mở `docs/system-architecture.md` và `docs/setup-and-operations-guide.md`, sidebar hiện "System Architecture - Workshop Media Player" và "Setup and Operations Guide - Workshop Player Ecosystem"; palette "system architecture" cho dòng đầu đúng tiêu đề (`b1-palette.png`); kết quả tìm nội dung cũng đúng tiêu đề. `deployment.md` hiện "Deployment Guide". |
| B2 sidebar thiếu thư mục | FIXED | Sidebar gốc workshop-ecosystem: SUBFOLDERS 9, gồm `plans` và `regression` (trước 7/8). `plans/` xuất hiện sau khi các file trong đó được index (qua xem/tìm). Lưu ý: lần quét nội dung đầu vẫn ra 2361 file và không có `plans/**` (gitignored) cho tới khi file được mở. |
| B3 file chưa index hiện tên file | Không đổi (theo thiết kế) | Chưa index → tên file; palette hiện kèm đường dẫn. Không hồi quy. |
| B4 xếp hạng palette | FIXED | `readme` → README.md gốc đứng đầu, rồi `proxy/README.md`, `dhamma-player/README.md`...; `.grok/.omp` không còn trong 12 dòng đầu. `task` → `task.md` gốc đầu. `agents` → `AGENTS.md` gốc đầu, các `.grok/agents/*` sau. |
| B5 palette nhiễu | FIXED | `synclite` chỉ ra tài liệu về synclitedb/plans synclite (liên quan thật), không còn file rải rác. `heatmap` → không có kết quả (chỉ có trong nội dung, không trong tiêu đề/đường dẫn: đúng). `sync`, `auth`, `huong dan`, `hoi quy`, `deploy`, `regression`, `phase 02` đều trả kết quả hợp lý. `ke hoach` trong palette không có kết quả vì không file nào có "kế hoạch" ở tiêu đề/đường dẫn; tìm nội dung `ke hoach` vẫn ra đúng 2 tài liệu. |
| B6 xếp hạng "danh gia" | FIXED | Đối chiếu từng kết quả với nội dung file: 7 kết quả đầu đều chứa đúng cụm "đánh giá" (project-brief, 4 file plans, 2 CTI Expert); các file chỉ chứa "giải/giao" (plan-stepthrough-kickoff ở hạng 18...) nằm sau. |
| B7 Cancel sau conflict | FIXED | Sửa file ngoài khi đang Edit → Save → hiện Cancel/Overwrite → Cancel (có hộp xác nhận "Discard your unsaved changes?", chấp nhận) → trang hiện dòng sửa ngoài `EXTERNAL-EDIT-LATEST`, không có chữ gõ trong trình duyệt, không cần tải lại tay. |
| B8 tràn ngang 390px | FIXED một phần | Trang tài liệu: `docs/deployment.md`, `README.md`, `docs/system-architecture.md`, scratch alpha đều có `scrollWidth` = 390; bảng rộng 358 px, lề trái 16 px, `overflow-x:auto`; code block cuộn riêng (`b8-docs-deployment-md.png`). Desktop 1440: `scrollWidth` 1440, bố cục không đổi (`b8-desktop.png`). **Còn lỗi**: trang tìm kiếm, trang chủ (danh sách dự án) và Settings ở 390px có `scrollWidth` = 422 (xem dưới). |

## Lỗi mới / còn lại

N1 (trung bình, B8 chưa hết): ở 390 px các trang dùng `<main class="fg-page">` tràn ngang 32 px (`documentElement.scrollWidth` 422 > 390, nội dung bị cắt mép phải: `b8-search-narrow.png`).
- Repro: viewport 390x844, mở `/p/workshop-ecosystem/_search?q=danh%20gia`, `/` hoặc `/settings`, chạy `document.documentElement.scrollWidth`.
- Nguyên nhân quan sát được: `main` có `width:390px` (từ `.fg-page{width:100%}` trong `crates/mdview/assets/atelier/components.css:221`), `box-sizing: content-box`, cộng `padding` 16+16 từ rule mới trong `app.css` (~dòng 725) → 422 px, `margin` tự động thành `0 -32px 0 0`. Cách sửa gợi ý: `box-sizing: border-box` cho `main.fg-page` trong media query này.
- Trước bản sửa trang tìm kiếm không tràn (chỉ thiếu lề); đây là hồi quy do thêm padding.

Quan sát nhỏ: nút Cancel khi conflict mở hộp `confirm` "Discard your unsaved changes?" (chấp nhận được, chỉ ghi nhận).

## Hồi quy nhanh

- Đăng nhập bằng web_secret, mở file qua CLI `open --json`, link nội bộ (`to beta` → `sub/beta.md`), live reload (nối dòng ngoài → hiện ≤1.5 s), Mermaid (render SVG), lưu bằng Ctrl+S ghi đúng ra đĩa, tìm nội dung (Relevance/Newest, This folder/Whole project đổi kết quả đúng), `errors`/`console` trống: đều đạt.
- Dự án thử `uat-scratch2` đã xoá thư mục; mục dự án có thể còn trong registry của daemon UAT (không đụng tới). Không sửa mã nguồn; không đụng cổng 7700 hay ~/.mdview.

Status: DONE_WITH_CONCERNS
Summary: 6 FIXED (B1, B2, B4, B5, B6, B7), B8 FIXED một phần (trang tài liệu đạt, 3 trang `fg-page` còn tràn 32 px), B3 không đổi theo thiết kế; 0 lỗi console.
Concerns:
- B8/N1 NOT FIXED một phần: ở 390 px, mở `/p/workshop-ecosystem/_search?q=x`, `/` hoặc `/settings` → `scrollWidth` 422 (cần `box-sizing:border-box` cho `main.fg-page`).
