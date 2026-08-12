# CONTEXT — Shortlink cho URL file của mdview

Item: `tsk-3sl` · Nguồn quyết định: [`DISCUSSION.md`](./DISCUSSION.md)
(5 vòng thảo luận, D1–D10 đã ghi vào `fgos decision`).

## Ranh giới feature

**Trong phạm vi.** Một route mới `/s/:code` trên daemon mdview, một cột
hash mới trên bảng `files`, và đổi định dạng output ở hai nơi phát link:
MCP `mdview_view_file` và CLI `mdview open`.

**Ngoài phạm vi.** URL dài `/p/<id>/<rel-path>` giữ nguyên và vẫn hoạt
động — link ngắn là lối vào thêm, không phải thay thế route. Không đụng
tới trang project home, search, jump, hay bất kỳ route nào khác.

**Đã đánh dấu hoãn (scope creep, không làm ở item này).** Link ngắn cho
*project* (`/p/<id>/` → dạng ngắn) không được yêu cầu và không nằm trong
item này.

## Quyết định đã khoá

| D-ID | Quyết định | Vì sao tin được | Sai thì mất gì |
|------|-----------|-----------------|----------------|
| D1 | Mã ngắn **dẫn xuất** từ `(project_id, rel_path)` đã có trong index; không bảng shortlink riêng | Vòng đời mã trùng vòng đời hàng `files`; indexer đã xoá hàng khi file biến mất (`engine.rs:146`, `IndexService::remove_file`) | Nếu sai hướng thì phải thêm bảng + TTL + job dọn, đúng thứ đề bài muốn tránh |
| D2 | Output **thay** URL dài bằng link ngắn, kèm `rel_path` dạng text thường cùng dòng | Vấn đề gốc là rớt dòng terminal; giữ tên file dạng text vẫn đọc-là-biết-file-nào mà không kéo dài dòng | Mất khả năng đọc lại lịch sử chat (mã đục), hoặc quay lại rớt dòng |
| D3 | `/s/<code>` trả **302** về `/p/<id>/<rel>`, không serve nội dung trực tiếp | Tái dùng nguyên `project_path` (`server.rs:109`), không phải xử lý lại resolve link tương đối trong trang | Nếu serve trực tiếp thì mọi link tương đối trong trang phải rewrite lại — rủi ro hồi quy cao |
| D4 | ~~Prefix co giãn kiểu git~~ — **bị D10 thay thế** | — | — |
| D5 | Có `hostname` override ⇒ đúng 1 link; không có ⇒ giữ danh sách IP | `runtime::display_urls_for` (`runtime.rs:162`) đã phân biệt sẵn hai trường hợp này | Khi bind wildcard mà chỉ in 1 IP, người dùng có thể nhận IP không routable |
| D6 | Mã không resolve được ⇒ **404** | File đã rời index thì không có gì đúng để hiển thị | Đoán bừa sẽ mở nhầm file |
| D7 | Biến thể **A3**: cột `path_hash` + index trên bảng `files`; không cache RAM, không quét toàn bảng | Chỉ một đường ghi (`upsert_file`, `repository.rs:94-109`) ⇒ không có bản sao nào lệch được. Đo thực tế: 15.480 file / 12 project, DB 228 MB | Cache RAM sẽ lệch nếu sót một đường ghi (`remove_file`, `delete_project`); quét toàn bảng tốn ~10ms mỗi request |
| D8 | **FNV-1a 64-bit tự viết** trong `mdview-core`, không thêm dependency | `std::DefaultHasher` không cam kết ổn định giữa các bản Rust ⇒ mã chết sau khi nâng toolchain. Không có yêu cầu chống đối thủ: server không xác thực (`server.rs:157-160` đã ghi rõ) | Dùng `DefaultHasher` thì mọi link cũ chết âm thầm sau một lần `cargo update`/nâng toolchain |
| D9 | CLI `mdview open` in link ngắn **cùng định dạng** với MCP | Cùng một vấn đề rớt dòng ở cả hai lối vào; dùng chung `Engine::view_file` (`engine.rs:97`) | Hai lối vào lệch định dạng, người dùng phải nhớ hai kiểu |
| D10 | Mã **cố định 12 hex** (thay thế D4) | Xoá bỏ hàm prefix-ngắn-nhất, nhánh nhập nhằng, và câu hỏi xử lý nhập nhằng. Ở 12 hex: ~1,8×10⁻⁵ cặp đụng kỳ vọng tại quy mô 100.000 file | Ngắn hơn (8 hex) thì đụng gần như chắc chắn ở 100k file; dài hơn thì không thêm giá trị |

## Thuật ngữ đã ghim

- **`path_hash`** — FNV-1a 64-bit của `project_id + "\0" + rel_path`, lưu
  hex **16 ký tự đầy đủ** trong một cột mới của bảng `files`.
- **mã ngắn (`code`)** — **12 ký tự đầu** của `path_hash`. Cột lưu 16, chỉ
  phát 12: đổi độ dài sau này là sửa một hằng số, không migrate lại DB.
- **link ngắn** — `http://<host>:<port>/s/<code>`, 37 ký tự với host
  `design-lap:7700`, so với 81 ký tự của URL dài tương ứng.

## Đường scout và bằng chứng

| Đường dẫn | Điều đã xác nhận |
|---|---|
| `crates/mdview-core/src/engine.rs:97-111` | `view_file()` sinh `url = /p/{id}/{rel}` — điểm phát link duy nhất, dùng chung cho MCP lẫn CLI |
| `crates/mdview/src/mcp.rs:106-122` | MCP nối `base + vf.url` cho mỗi base của `ensure_daemon_bases()`, cộng dòng `project_id:` |
| `crates/mdview/src/server.rs:92-111` | Bảng route hiện tại; namespace `/s/` còn trống |
| `crates/mdview-core/src/repository.rs:256-287` | Schema: `files` PK `(project_id, rel_path)`, `files_fts`, `links` |
| `crates/mdview-core/src/repository.rs:94-109` | `upsert_file` dùng `ON CONFLICT DO UPDATE` — đường ghi duy nhất của một hàng file |
| `crates/mdview-core/src/repository.rs:30-37` | **Không có cơ chế migration**: chỉ `execute_batch(SCHEMA)` với `CREATE TABLE IF NOT EXISTS`, không `PRAGMA user_version` |
| `crates/mdview-core/src/config.rs:114-126` | DB bền tại `~/.mdview/registry.db`, WAL |
| `crates/mdview-core/Cargo.toml` | Không có crate hash nào (blake3/sha2/xxhash đều vắng) |
| Đo thực tế trên `~/.mdview/registry.db` | 15.480 file, 12 project, `rel_path` dài nhất 151 ký tự, DB 228 MB |

**Trạng thái năng lực impact-analysis: `inactive`** — `fgos tool query
--capability impact-analysis --status present` trả về `providers: []` trên
máy này. Ghi lại để người đọc `CONTEXT.md` sau không phải tự dò.

## Tham chiếu chuẩn

- [`DISCUSSION.md`](./DISCUSSION.md) — 5 vòng thảo luận đầy đủ; §6 là bản
  tổng hợp thiết kế, §7 là 3 hạng mục ứng viên (`#task-hash-column-migration`,
  `#task-short-route-resolve`, `#task-emit-short-link`).
- `docs/backlog.md` — không có mục nào trùng với feature này.

## Outstanding questions

None
