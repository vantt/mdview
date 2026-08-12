# CONTEXT — Chỉ reload đúng browser liên quan

Item: `tsk-2io`.

## Ranh giới feature

**Trong phạm vi.** Live-reload qua `/ws` hiện gửi cùng một tín hiệu tới
mọi browser đang mở, và gửi cả khi file bị "chạm" nhưng nội dung không
đổi. Item này thu hẹp lại: chỉ browser đang xem đúng file thay đổi mới
reload, và chỉ khi nội dung thật sự đổi (hoặc file bị xoá).

**Ngoài phạm vi.** Tần suất/độ trễ quét filesystem
(`indexing.debounce_ms`, `notify_debouncer_full`) giữ nguyên — không phải
nguyên nhân của vấn đề, "liên tục rescan" trong yêu cầu là mô tả hành vi
bình thường, không phải điểm cần sửa. Không thêm cơ chế patch DOM từng
phần (chỉ vẫn full-page reload, chỉ thu hẹp *ai* nhận tín hiệu).

## Vấn đề rõ (đã scout)

- `crates/mdview/src/watch.rs:48-72` (`reindex_paths`) — với mọi sự kiện
  filesystem trên file `.md`, nếu file tồn tại và reindex thành công thì
  `changed = true`, **không so sánh nội dung cũ/mới**. Một lần "chạm"
  không đổi nội dung (git checkout, editor auto-save rỗng) vẫn kích
  broadcast.
- `crates/mdview/src/server.rs:34,455-456` + `watch.rs:29` — `reload_tx`
  là `broadcast::Sender<String>` toàn cục, `handle_ws` forward y nguyên
  cho mọi socket. Không có khái niệm "socket này đang xem file nào" ở
  server.
- `crates/mdview/assets/app.js:704-715` — client chỉ so `ev.data ===
  "reload"` rồi `location.reload()` mù quáng; không mang theo danh tính
  file đang xem khi kết nối `/ws`.
- `crates/mdview/src/views.rs:8-36` (`layout`) — dùng chung cho **mọi**
  trang (`file_page`, `project_list_page`, `search_page`,
  `settings_page`), nên trang không gắn file cụ thể cũng mở `/ws` và
  cũng bị reload theo tín hiệu toàn cục hiện tại.
- Route file là `/p/:id/*path` (`server.rs:109`) — `location.pathname`
  của một trang file đã sẵn là `/p/<project_id>/<rel_path>`, không cần
  server truyền thêm gì để client tự biết danh tính của chính nó.

## Quyết định đã khoá

| D-ID | Quyết định | Vì sao tin được | Sai thì mất gì |
|------|-----------|-----------------|----------------|
| D1 | Reload chỉ gửi cho browser đang xem **đúng** `(project_id, rel_path)` của file vừa đổi, không phải cả project | Đây là nguyên nhân trực tiếp gây chớp; lọc client-side qua `location.pathname` không cần server giữ bảng "socket nào xem file nào" — không state mới | Sidebar của tab đang mở có thể hơi cũ nếu file khác trong cùng project được thêm/xoá — chấp nhận được, đổi lấy hết bị giật liên tục |
| D2 | Phát hiện "nội dung không đổi" bằng cột `content_hash` mới trên bảng `files`, qua cơ chế `MIGRATIONS` append-only đã có (`repository.rs`) — không tái dùng `files_fts.content` | `content_hash` tra theo primary key là O(1); `files_fts`'s `project_id`/`rel_path` là cột `UNINDEXED`, so `=` trên đó là quét tuyến tính — đúng loại bẫy `GLOB` ghép chuỗi đã bắt được ở `tsk-3sl` | Tái dùng FTS thì né được một migration nhưng ăn full-scan mỗi lần watcher chạy — đường nóng, chạy liên tục |
| D3 | Các trang không gắn với một file cụ thể (`/`, `/p/:id/_search`, `/settings`) không bao giờ reload do sự kiện thay đổi file | Hệ quả tự nhiên của D1 — các trang đó không có `(project_id, rel_path)` để so khớp nên tự động không match. Giải quyết luôn điểm khó chịu nhất: form `/settings` không còn bị reload giữa chừng khi đang gõ | Không có nhánh else — D1 tự loại các trang này, không cần cơ chế riêng |
| D4 | File bị xoá luôn phát tín hiệu reload cho đúng `(project_id, rel_path)` đang xem, **bỏ qua** so sánh `content_hash` | Không có nội dung mới để hash; tab đang xem file đã mất phải được báo để chuyển 404/điều hướng — đây không phải trường hợp cần lọc | Nếu lọc luôn cả xoá thì tab đó đứng yên với nội dung ma, không nhận ra file đã biến mất |

## Thuật ngữ đã ghim

- **"liên quan"** — trong toàn bộ item này nghĩa là: trang đang hiển thị
  đúng file vừa thay đổi (khớp cả `project_id` lẫn `rel_path`), theo D1.
  Không phải "cùng project", không phải "có link tới file đó".
- **`content_hash`** — hash nội dung file (không phải đường dẫn), cột
  mới trên `files`, cùng cơ chế thêm cột đã dùng cho `path_hash`
  (`tsk-3sl`). So sánh giá trị cũ/mới quyết định có phát tín hiệu
  "changed" hay không.

## Đường scout và bằng chứng

| Đường dẫn | Điều đã xác nhận |
|---|---|
| `crates/mdview/src/watch.rs:18-72` | Toàn bộ luồng watcher hiện tại: debounce → reindex → broadcast không điều kiện |
| `crates/mdview/src/server.rs:34,106,455-466` | `AppState.reload_tx`, route `/ws`, `handle_ws` forward nguyên văn |
| `crates/mdview/assets/app.js:704-715` | Client connect `/ws`, so `"reload"`, reload mù |
| `crates/mdview/src/views.rs:8-160,177-213` | `layout()` dùng chung mọi trang; `file_page` có sẵn `project.id` + `file.rel_path`; `right_panel` cho thấy sidebar/backlinks là project-scoped |
| `crates/mdview-core/src/indexer.rs:37-74` | `IndexService::index_file` đọc `content`, gọi `upsert_file` — điểm duy nhất có cả nội dung cũ (chưa ghi) lẫn mới trong cùng một lượt |
| `crates/mdview-core/src/repository.rs` (sau `tsk-3sl`) | `MIGRATIONS` append-only + `PRAGMA user_version`, tiền lệ trực tiếp cho `content_hash` |

**`impact-analysis: inactive`** — `fgos tool query --capability
impact-analysis --status present` trả `providers: []` trên máy này, như ở
item trước.

## Tham chiếu chuẩn

- `docs/history/short-link-for-file-urls/` — tiền lệ trực tiếp cho cơ chế
  migration (`MIGRATIONS`, `PRAGMA user_version`) và bẫy so sánh chuỗi
  UNINDEXED (D2 ở đây trực tiếp trích từ bài học đó).

## Outstanding questions

None
