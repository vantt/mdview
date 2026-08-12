# CONTEXT — Code viewer section

Item: `tsk-1hb`.

## Ranh giới feature

**Trong phạm vi.** Một section thứ hai trên giao diện mdview, nằm cạnh
section Docs, cho phép duyệt cây thư mục của project đã đăng ký và đọc
file nguồn có syntax highlight, gutter số dòng và anchor theo dòng. Đây
là một trình đọc file tiện dụng.

**Ngoài phạm vi.** Không phải một git web UI. Người dùng đã loại rõ ràng
khỏi v1: fuzzy jump cho file code, live-reload cho file code, endpoint
raw / tải binary, và diff view. Không mở rộng index: markdown vẫn là thứ
duy nhất được index và tìm kiếm.

## Locked decisions

Chốt trong phiên làm việc ngày 2026-08-12, có xác nhận trực tiếp của
người dùng cho từng mục.

| ID | Quyết định | Lý do |
|----|-----------|-------|
| D1 | Không index file code. Mọi file resolve on-demand từ đĩa. | Giữ index và FTS5 thuần markdown; index code làm phình DB trên repo lớn và kéo theo phát hiện binary, exclude semantics, giá trị tìm kiếm không rõ. |
| D2 | Section riêng dưới tiền tố `_code`, theo đúng quy ước tiền tố gạch dưới đã có cho search và jump. | Giữ project registry, theme, topbar; không đụng vào namespace đường dẫn file thật. |
| D3 | Bảo mật bằng gitignore cộng denylist tên nhạy cảm, không dùng allowlist đuôi file. | Daemon không có xác thực và có thể bind wildcard ra LAN. Allowlist đuôi file là bất khả thi cho mã nguồn; định danh file mới là thứ đáng chặn. Thư mục kho git bị chặn vô điều kiện, không phụ thuộc cấu hình exclude mà người dùng sửa được. |
| D4 | Liệt kê lazy theo từng thư mục, render phía server. | Chi phí có chặn trên với repo kích thước bất kỳ; bỏ được hoàn toàn client state, không thêm JavaScript, có sẵn fallback khi tắt JS. |
| D5 | Không làm palette nhảy file cho code ở v1. | Cơ chế nhảy hiện tại dựa vào index; bản cho code cần một lượt duyệt cây có cache — chi phí không tương xứng ở v1. |

## Hệ quả thiết kế đã xác minh trên cây mã

- Trình đọc phải dùng đường API sinh span theo từng dòng có giữ trạng
  thái xuyên dòng; cắt khối HTML một cục theo ký tự xuống dòng sẽ vỡ thẻ
  khi comment khối vắt qua nhiều dòng.
- Giữ nguyên kiểu class của bộ tô màu hiện tại để tệp CSS highlight sẵn
  có phủ luôn phần mới, không phát sinh bảng màu thứ hai.
- Đường dẫn bị từ chối và đường dẫn không tồn tại phải trả về nội dung
  giống hệt nhau: một thông báo khác biệt tự nó đã tiết lộ file tồn tại.
- Sidebar mới dùng lại đúng lớp CSS của sidebar hiện có, nhờ đó phần
  JavaScript mở ngăn kéo trên mobile chạy được mà không sửa dòng nào.

## Kế hoạch chi tiết

Ba phase, đặc tả đầy đủ kèm tham chiếu dòng đã xác minh, nằm trong thư
mục kế hoạch `plans/260812-1458-code-viewer-section/`:

- `plan.md` — trạng thái, phase, tiêu chí nghiệm thu, phần hoãn lại.
- `phase-01-safe-source-access.md` — mô hình đe doạ, denylist, liệt kê
  thư mục có tôn trọng gitignore, chính sách binary và giới hạn kích thước.
- `phase-02-per-line-highlight.md` — tô màu theo dòng, bẫy span vắt dòng.
- `phase-03-section-ui.md` — route, view, chuyển section, sidebar, CSS.
