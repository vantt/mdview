# CONTEXT — Sửa quadratic join trong backfill `content_hash`

Item: `tsk-155` (bug, khẩn cấp — phát hiện giữa bước release của `tsk-2io`).

## Ranh giới

**Trong phạm vi.** Đúng một hàm: `backfill_content_hash` trong
`crates/mdview-core/src/repository.rs` (migration v2, thêm ở `tsk-2io`).

**Ngoài phạm vi.** Không đụng `backfill_path_hash` (migration v1, từ
`tsk-3sl`) — hàm đó chỉ đọc từ `files`, không JOIN sang bảng nào khác, nên
không mắc cùng lớp lỗi.

## Vấn đề

`backfill_content_hash` dùng `LEFT JOIN files_fts` trên `project_id`/
`rel_path` — hai cột này là `UNINDEXED` trong FTS5, nên SQLite không có
index để join qua, rơi về nested-loop scan O(n×m).

**Tái hiện có kiểm soát:** 3.000 hàng synthetic mất 2.58s (đo bằng
`sqlite3` Python, script scratchpad). Trên DB sản xuất thật (228MB,
15.480 hàng), chạy `mdview doctor` (kích hoạt migration) treo hơn 3 phút
**không tiến triển** — xác nhận bằng cách theo dõi kích thước file
`registry.db-wal` không đổi suốt cửa sổ quan sát, nghĩa là còn chưa tới
được vòng lặp UPDATE, vẫn đang kẹt ở chính câu SELECT.

**Mức độ nghiêm trọng:** tag `v0.7.0` mang bug này đã push lên GitHub
trước khi phát hiện — bất kỳ ai nâng cấp lên bản đó và có vài nghìn file
trở lên sẽ gặp treo im lặng khi mở `mdview doctor`/daemon lần đầu sau khi
migrate.

## Quyết định

| D-ID | Quyết định | Vì sao tin được |
|------|-----------|-----------------|
| D1 | Chuyển "join" sang `HashMap` trong Rust — một lượt `SELECT` từ `files` lấy hàng cần backfill, một lượt `SELECT` toàn bộ `files_fts` dựng map, tra map trong vòng lặp UPDATE | Đo trên **đúng bản sao DB sản xuất thật** (15.584 hàng, không phải synthetic): **0.988 giây**, hoàn tất migration `v1 → v2`. Đối chiếu 5 hàng thật: `content_hash` tính lại từ nội dung khớp 100% với giá trị đã ghi |

## Bằng chứng

- Script tái hiện: đo JOIN cũ trên 3.000 hàng synthetic → 2.58s.
- Script kiểm chứng bản sửa: HashMap trên 16.000 hàng synthetic →
  0.027 giây tổng cộng (select + scan + tra cứu).
- Dry-run thật: copy `~/.mdview/registry.db` (228MB, chưa qua migration
  v2) sang HOME cô lập, chạy binary đã sửa qua `cargo run` → `real 0.988s`,
  `index schema: v2, every file has a short-link code`, 0 hàng còn thiếu
  `content_hash`, 0 hàng còn thiếu `path_hash` (không bị ảnh hưởng).
- Đối chiếu 5 hàng thật: `content_hash` lưu khớp tuyệt đối với FNV-1a
  tính lại từ `files_fts.content`.
- Test hồi quy mới (`backfilling_thousands_of_rows_stays_fast`,
  `repository.rs`): 4.000 hàng, ngưỡng chặn cứng < 2 giây — bắt được nếu
  ai đó vô tình quay lại dạng JOIN, vì test chức năng thuần (so kết quả)
  sẽ pass ở cả hai dạng, chỉ thời gian mới phân biệt được.

## Outstanding questions

None
