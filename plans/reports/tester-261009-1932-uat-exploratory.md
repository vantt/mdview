# UAT khám phá — mdview UAT build (http://100.121.46.35:7800)

Ngày: 2026-10-09 (Asia/Saigon). Công cụ: agent-browser 0.38.2, Chrome `--no-sandbox`. Dự án thật (chỉ đọc): `workshop-ecosystem` (2361 file .md được index) và `mdview`. Dự án thử (đã xoá): `uat-scratch`.
Ảnh chụp: `/tmp/claude-1000/-home-vantt-projects-mdview/5b62e0d2-df2d-4394-bcce-773b6c21271e/scratchpad/uat-shots/` (ký hiệu `shots/NN-...`).
Console/page errors của trình duyệt: không có lỗi nào trong toàn bộ phiên.

## Kết luận

Chức năng cốt lõi chạy tốt: mở file, link nội bộ, chặn file không phải markdown, live reload, editor và cảnh báo conflict, tìm nội dung (nhanh, tìm không dấu hoạt động). Có 1 lỗi đáng sửa nhất (tiêu đề sai ở sidebar/palette/kết quả tìm với file có BOM) và vài lỗi vừa/nhẹ về xếp hạng palette, sidebar thiếu thư mục, và tràn ngang khi cửa sổ hẹp.

## Kết quả theo mục checklist

| Mục | Kết quả | Ghi chú |
|---|---|---|
| 0 Chuẩn bị / đăng nhập | PASS | Đăng nhập bằng web_secret OK. |
| 1 Mở file và đọc | PASS | `open --json` trả về tức thì (~7 ms), JSON đủ `url, urls, path, code, project_id` (+`long_url(s)`). Có mục lục, nút Copy ở code block. `.env`, `.toml` trong `/p/...` trả 404; `.env` 404 cả ở tab Code. Link tới file md chưa mở bấm được, không 404. Không thấy editor khi chưa bấm Edit. Chưa thử Mermaid (không có file mẫu). |
| 2 Sidebar | PASS có lỗi (B2, B3) | File mới + thư mục ẩn trong dự án thử hiện ngay sau tải lại, kèm tiêu đề thật. Dự án lớn: thấy `.grok`, `.omp`, `.beads` nhưng thiếu `plans/` và (lúc đầu) `regression/`. |
| 3 Palette Ctrl+K | PASS có lỗi (B1, B4, B5) | Ô trống = file mới sửa gần đây (đúng thứ tự mtime). Không dấu "huong dan" khớp "Hướng dẫn sử dụng". Mũi tên/Enter/Esc OK. Dòng "Search content for …" ở đầu, Enter mở trang tìm. |
| 4 Tìm nội dung | PASS có lỗi (B6) | Xem số đo bên dưới. "tai lieu", "duoc", "danh gia", "huong dan", "ranh gioi" đều ra tài liệu tiếng Việt có dấu, từ khoá được tô vàng kể cả dạng có dấu. This folder / Whole project / Newest / Relevance hoạt động, giữ nguyên từ khoá. `.grok/`, `.omp/` tìm được; không thấy kết quả từ thư mục phụ thuộc, `target/`, `.git/`; `.claude/` bị loại vì `.gitignore` của dự án. Query lạ (`" OR * (`, HTML/XSS, `c++`) không lỗi, không chạy script. |
| 5 Live reload / sửa file | PASS có lỗi nhẹ (B7) | Reload sau 0.33 s; heading mới và tiêu đề đổi cập nhật cả sidebar. Sửa + Ctrl+S ghi đúng ra đĩa (kể cả tiếng Việt). Sửa ngoài khi đang Edit rồi Save → thông báo "This file changed on disk since you opened it…" với nút Cancel/Overwrite; đĩa không bị ghi đè. File agent tạo mới được tìm thấy ngay (`Synced 8 files (4 read)`). |
| 6 Agent (CLI-first) | PASS một phần | `doctor --dry-run` chạy, không sửa gì, báo 2 cảnh báo (permission, skill) và MCP bị bỏ qua đúng như checklist. Phần nhờ agent trong phiên Claude không thực hiện được trong bài kiểm tra này. |
| 7 Giao diện | PASS có lỗi (B8) | Light/dark đều đọc tốt, trang tìm kiếm không vỡ. 390 px: sidebar thành drawer (nút ☰), tìm kiếm dùng tốt. Nhưng trang tài liệu bị cuộn ngang. |
| 8 Tài nguyên | Ghi nhận | Xem bên dưới. |

## Số đo

- Lần tìm đầu trên workshop-ecosystem: `Synced 2361 files (2340 read) in 0.57 s` (wall-clock từ Enter đến khi có kết quả ≈ 0.62 s).
- Tìm lại ngay: `Index reused (synced moments ago)` (~60 ms). Sau >10 s: `Synced 2361 files (0 read) in 0.04 s`. Mỗi truy vấn tiếp theo 25–70 ms.
- `registry-v4.db` = 8,712,192 B (8.4 MB) + WAL 5.0 MB (`registry-v4.db-wal` 5,162,392 B).
- RSS daemon UAT (`~/.mdview-uat/bin/mdview serve`) = 37,752 KB (~37 MB). (Bản 7700 đang dùng: 42,876 KB, không đụng tới.)
- Cuối phiên: không còn process do bài test tạo ra.

## Lỗi

### B1 — Trung bình: tiêu đề sai ở sidebar, palette và kết quả tìm cho file bắt đầu bằng BOM
- Repro: mở `/p/workshop-ecosystem/docs/deployment.md`, xem sidebar; hoặc Ctrl+K gõ "system architecture".
- Thấy: `docs/system-architecture.md` hiện tiêu đề `.github/workflows/deploy.yml`; `docs/setup-and-operations-guide.md` hiện `Google Drive Proxy URL`; một file khác hiện `Test production build before commit`. Cũng sai ở tiêu đề kết quả tìm kiếm. Trang file thật vẫn hiện đúng h1 (`System Architecture - Workshop Media Player`).
- Nguyên nhân quan sát được: các file này bắt đầu bằng BOM (`EF BB BF`) nên dòng `# Tiêu đề` đầu bị bỏ qua, sau đó bộ trích tiêu đề lấy dòng `# ...` nằm trong code fence (comment shell/yaml). Trong cây git của workshop có ít nhất 30 file có BOM. Có thể kiểm tra riêng: tiêu đề không nên lấy dòng bên trong code fence.
- Ảnh: `shots/05-ws-subfolders.png`, `shots/31-narrow-drawer-light.png` (thấy `.github/workflows/deploy.yml` làm tiêu đề trong danh sách).

### B2 — Trung bình: sidebar gốc thiếu thư mục `plans/` (và lúc đầu `regression/`) dù có trong index
- Repro: mở `/p/workshop-ecosystem/README.md`, bấm SUBFOLDERS. Thấy 7 thư mục (`.beads .grok .omp dhamma-player docs packages proxy`); không có `plans`, dù Ctrl+K và tìm nội dung đều ra file trong `plans/` (vd `plans/261009-1632-proxy-synclite-boundary-rollout/phase-01-b0-baseline.md`, mở được qua URL trực tiếp).
- `regression/` (thư mục mới, chưa commit) cũng chưa có trong sidebar lúc đầu dù `regression/checklist.md` đã có trong palette; sau vài phút (sau khi chạy tìm kiếm) nó mới xuất hiện (SUBFOLDERS 8). `plans/` vẫn không xuất hiện. `plans/**` bị `.gitignore` của workshop liệt kê (nhưng một phần được git theo dõi) nên có thể là sự khác biệt giữa quy tắc ignore của sidebar và của index. Cần xác nhận hành vi mong muốn.
- URL thư mục `/p/workshop-ecosystem/plans/`, `/docs/` trả 404 (chỉ truy cập thư mục qua sidebar).
- Ảnh: `shots/05-ws-subfolders.png`, `shots/22-palette-dark.png` (đã có 8 thư mục).

### B3 — Thấp: tên trong sidebar không có tiêu đề cho file chưa index
- Chấp nhận theo checklist (hiện tên file). Ghi nhận thêm: trong dự án lớn, nhiều file cùng tên `SKILL.md`/`README.md` trong palette chỉ phân biệt được bằng đường dẫn nằm cạnh.

### B4 — Trung bình: xếp hạng palette — file ở thư mục gốc bị chìm dưới `.grok/`, `.omp/` khi trùng tên
- Repro: mở bất kỳ file workshop, Ctrl+K, gõ `readme` hoặc `readme.md`: 20 kết quả đầu đều là `.grok/skills/**/README.md` và `.omp/skills/**/README.md`, README gốc của dự án không có trong 20 dòng đầu. `task` → file `task.md` gốc xếp sau `.grok/skills/...task-operations.md`. `agents` → `AGENTS.md` gốc đứng thứ 6, sau "Engineering Prose — the reasoning protocol", "Agents Workflow" (.grok/.omp).
- Mong đợi: khi khớp tên file như nhau, ưu tiên đường dẫn ngắn/ở gốc (hoặc ngoài thư mục gói skill).
- Trường hợp tốt: `claude.md` → `CLAUDE.md` gốc đứng đầu; trong dự án mdview `prd`, `phase 02`, `uat`, `viewd scope` đều xếp hợp lý.
- Ảnh: `shots/22-palette-dark.png`, `shots/07-palette-q.png`.

### B5 — Thấp/Trung bình: palette hiện kết quả nhiễu khi không có kết quả thật
- Repro: Ctrl+K, gõ `synclite` hoặc `kehoach` hoặc `danh gia` (trong palette).
- Thấy: một danh sách file không liên quan (`headline-templates.md`, `tailwind-utilities.md`, `seo-geo-ax-checklist.md`...) vì khớp rải rác từng ký tự; không có trạng thái "không có kết quả". Người dùng dễ nghĩ đó là kết quả tốt. Gợi ý: đặt ngưỡng điểm tối thiểu hoặc yêu cầu khớp liền.

### B6 — Trung bình: xếp hạng tìm nội dung ưu tiên khớp một phần từ hơn khớp cả cụm
- Repro: `/p/workshop-ecosystem/_search?q=danh%20gia` (Relevance).
- Thấy: kết quả 2 (`docs/prompts/plan-stepthrough-kickoff.md`) chỉ chứa "giải" (khớp tiền tố `gia*`), kết quả 3 chỉ chứa "giao"; trong khi các file chứa đúng cụm "được đánh giá" (phase-07, phase-02 của plan) xếp thấp hơn. Có vẻ truy vấn là OR + khớp tiền tố, nên từ ngắn (`gia`) kéo nhiễu. Ảnh: `shots/09-search-danhgia.png`.
- Ghi chú: truy vấn `tai lieu` (ảnh `shots/08-search-first.png`) cho kết quả tốt.

### B7 — Thấp: sau khi Cancel khi conflict, trang giữ nội dung cũ
- Repro: mở file, Edit, gõ vài ký tự; sửa file đó từ terminal; bấm Save → thấy cảnh báo conflict; bấm Cancel.
- Thấy: trang render vẫn là bản cũ (không có dòng vừa sửa ngoài) cho tới khi tải lại thủ công, dù live reload bình thường hoạt động. Thông báo có nói "Cancel and reload", nên chỉ là bất tiện. Khi đang ở trạng thái conflict, phần cuối nội dung render còn lộ một dòng bị cắt dưới header (xem `shots/15-conflict.png`, dòng "EDITED-IN-BROWSER tiếng Việt" bị che một nửa phía trên thông báo).

### B8 — Trung bình: tràn ngang ở cửa sổ hẹp (390 px)
- Repro: viewport 390 px, mở `/p/workshop-ecosystem/docs/deployment.md` (scrollWidth 490 > 390) hoặc `/p/workshop-ecosystem/README.md` (418 > 390).
- Thấy: trang cuộn ngang. Code block (`pre`) tự cuộn đúng, nhưng bảng (`table`, phải tới 406 px) và `code` inline dài vượt khung. Checklist yêu cầu "không cuộn ngang". Ảnh: `shots/34-narrow-doc-light.png`, `shots/30-narrow-doc-dark.png`.
- Trang tìm kiếm ở 390 px không tràn ngang nhưng ô nhập và các thẻ kết quả sát mép trái (không có lề, xem `shots/33-narrow-search-light.png`) — nhẹ.

## Gợi ý UX (không phải lỗi)

- Tiêu đề kết quả tìm và palette hiện nguyên dấu backtick (`` `proxy` ``, `` `/player` ``); nên bỏ định dạng markdown trong tiêu đề.
- Đoạn trích có thể là mã Mermaid/bảng thô (vd tìm `cloudflare` → đoạn `C1["..."]`); nên ưu tiên đoạn văn thường hoặc rút gọn.
- Tìm đa từ nên có lựa chọn "cả cụm" (hoặc ưu tiên cụm liền) để tránh nhiễu như B6.
- Cancel sau conflict nên tự tải lại hoặc có nút "Reload" cạnh Overwrite.
- Trang tìm kiếm mặc định "Whole project" kể cả khi URL có `dir=docs` (nút này chỉ áp dụng khi chọn "This folder"); hành vi đúng nhưng URL `...&dir=docs` mà kết quả không bị thu hẹp dễ gây hiểu lầm khi chia sẻ link.
- Tab Code mở được `cfg.toml` (nội dung hiện); `.env` thì 404. Có vẻ cố ý (trình xem mã), chỉ lưu ý khi đánh giá yêu cầu "không hiện nội dung file không phải markdown".
- `mdview-uat open` được test qua JSON; trường `long_url(s)` ngoài checklist, không gây hại.

## Câu hỏi còn mở

- `plans/` bị `.gitignore` nhưng vẫn được theo dõi một phần: thiết kế mong muốn là sidebar bỏ qua, còn index/tìm kiếm thì gồm không? (B2)
- Mermaid zoom và mục 6 (nhờ agent trong phiên Claude) chưa được kiểm tra bằng trình duyệt.

Status: DONE_WITH_CONCERNS
Summary: Đã chạy hết các mục checklist có thể làm không sửa file thật; tìm kiếm, live reload, editor và conflict đều chạy đúng, first-search 0.57 s, DB 8.4 MB, RSS 37 MB. Có 8 lỗi/điểm cần xem, đã ghi chi tiết trong báo cáo; dự án thử đã xoá và trình duyệt đã đóng.
Concerns/Blockers:
- B1 (trung bình): file bắt đầu bằng BOM bị lấy tiêu đề từ dòng `#` trong code fence — mở `docs/deployment.md` ở workshop-ecosystem, xem sidebar.
- B2 (trung bình): sidebar gốc thiếu `plans/` (và `regression/` lúc đầu) dù palette/tìm kiếm có — mở README của workshop-ecosystem, xem SUBFOLDERS.
- B4 (trung bình): Ctrl+K `readme`/`task`/`agents` đẩy file gốc xuống dưới các bản trong `.grok/`, `.omp/`.
- B6 (trung bình): tìm `danh gia` xếp file chỉ khớp tiền tố `gia*` (giải/giao) trên file có đúng cụm.
- B8 (trung bình): ở 390 px trang tài liệu cuộn ngang do bảng và `code` inline — mở `docs/deployment.md`.
- B5 (thấp/trung bình): palette hiện kết quả rải ký tự nhiễu khi gõ `synclite`.
- B7 (thấp): sau Cancel khi conflict, trang giữ bản cũ tới khi tải lại.
- B3 (thấp): nhiều file trùng tên (SKILL.md/README.md) trong palette khó phân biệt khi chưa index.
