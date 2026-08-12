# plan.md — Shortlink cho URL file của mdview

Item: `tsk-3sl` · Quyết định khoá: [`CONTEXT.md`](./CONTEXT.md) ·
Thảo luận nguồn: [`DISCUSSION.md`](./DISCUSSION.md)

Mode: **standard**

**Đếm cờ: 3/10.** `data model` (thêm cột + migration trên DB bền đang có
dữ liệu thật) · `public contracts` (định dạng output của MCP
`mdview_view_file` đổi; thêm một route HTTP mới) · `existing covered
behavior` (đã có test quanh `mcp.rs`, `repository.rs`, `server.rs` —
không được hồi quy).

Không cờ hard-gate nào: migration là `ALTER TABLE ADD COLUMN` cộng
backfill — thuần bổ sung, không xoá và không viết đè hàng nào, nên không
phải cờ `data loss`.

**Vì sao không nhỏ hơn `standard`:** `small` nghĩa là vài file và không có
vùng xám. Item này chạm 5 file qua 2 crate, và mang theo bước migration
**đầu tiên** của repo trên một DB 228 MB đang có người dùng — đó là một
vùng cần chứng minh, không phải một sửa đổi thẳng tuột.

**`impact-analysis: inactive`** — `fgos tool query --capability
impact-analysis --status present` trả `providers: []` trên máy này. Mọi
điểm chứng minh dưới đây vì thế dựa vào test thật, không dựa vào bằng
chứng blast-radius.

## Không tách item

`DISCUSSION.md` §7 liệt kê 3 hạng mục ứng viên (T1/T2/T3). Sau khi cân
nhắc, **item này đi tiếp như một khối duy nhất**, ba hạng mục đó thành ba
pha bên trong plan này. Lý do:

1. **Footprint chồng nhau thật.** T1 và T2 cùng sửa
   `crates/mdview-core/src/repository.rs` (T1 thêm cột + migration, T2
   thêm hàm truy vấn theo prefix). Tách thành hai item song song là tự tạo
   ra đúng loại va chạm mà `footprintOverlapAmong` tồn tại để chặn.
2. **T2 và T3 vô nghĩa nếu thiếu T1.** Cả hai đọc cột `path_hash`. Chuỗi
   phụ thuộc là tuyến tính, không có nhánh song song thật để mua.
3. **Khối lượng nhỏ.** Ước tính ~200–300 dòng qua 5 file. Chi phí ba
   worktree, ba nhánh, ba lượt verify lớn hơn phần lợi.

`fgos graph --json` xác nhận không có gì để sắp xếp lại: `componentCount:
1`, `criticalPath.depth: 1`, `topUnblock` chỉ có chính item này với
`unblocks: 0`.

## Cách làm

### Đường đã chọn

Ba pha tuần tự trong một nhánh, mỗi pha để lại repo ở trạng thái xanh:

**Pha 1 — nền hash + migration** (honors D1, D7, D8)

- `crates/mdview-core/src/` — module mới cho FNV-1a 64-bit, hàm thuần
  `path_hash(project_id, rel_path) -> String` (hex 16 ký tự), kèm hằng số
  độ dài mã `SHORT_CODE_LEN = 12`.
- `crates/mdview-core/src/repository.rs` — thêm cột `path_hash` vào
  `SCHEMA`; thêm bước migration trong `from_conn` (`ALTER TABLE ADD
  COLUMN` khi cột chưa có, backfill trong một transaction, `CREATE INDEX`,
  rồi `PRAGMA user_version`); `upsert_file` tính và ghi cột này.

**Pha 2 — resolve** (honors D3, D6, D7, D10)

- `crates/mdview-core/src/repository.rs` — hàm truy vấn
  `find_by_hash_prefix(code) -> Option<(project_id, rel_path)>`, dùng
  `WHERE path_hash GLOB ?1` với `ORDER BY` ổn định và `LIMIT 1`, trong đó
  `?1` được bind bằng **một chuỗi `format!("{code}*")` dựng sẵn trong
  Rust**.

  **Không được ghép chuỗi trong SQL.** Dạng `GLOB ?1 || '*'` biến vế phải
  thành một biểu thức thay vì literal/tham số, và tối ưu hoá LIKE/GLOB
  của SQLite từ chối áp dụng — câu truy vấn vẫn trả **đúng** kết quả nên
  không test chức năng nào bắt được, nhưng âm thầm thành full scan, đúng
  thứ D7 chọn A3 để tránh. Đo thật trên 20.000 hàng có `ANALYZE`
  (SQLite 3.37.2):

  | Dạng | Query plan |
  |---|---|
  | `GLOB 'a3f9c1d20b74*'` (literal) | `SEARCH files USING INDEX idx_files_hash` |
  | `GLOB ?` (tham số đã ghép sẵn) | `SEARCH files USING INDEX idx_files_hash` |
  | `GLOB ? \|\| '*'` | **`SCAN files`** |
  | `>= ? AND < ?` | `SEARCH files USING INDEX idx_files_hash` |

  Điều này **sửa lại** dạng SQL viết trong `DISCUSSION.md` §6, vốn ghi
  `GLOB :code || '*'`. Bản trong plan này là bản đúng để dựng theo.
- `crates/mdview/src/server.rs` — route `/s/:code` trong `router()`,
  handler trả `Redirect::to("/p/{id}/{rel}")` (302) hoặc `not_found`.

**Pha 3 — phát link** (honors D2, D5, D9, D10)

- `crates/mdview-core/src/engine.rs` — `ViewFile` mang thêm `code` = 12 ký
  tự đầu của `path_hash`; `view_file()` điền nó.
- `crates/mdview/src/mcp.rs` — đổi phần dựng text: mỗi dòng thành
  `<rel_path> → <base>/s/<code>`; giữ `structuredContent.url`/`urls` trỏ
  link **ngắn**, thêm `long_url` cho ai còn cần đường dẫn đầy đủ.

  **Tách một hàm thuần trước khi sửa.** `mcp.rs` hiện có **0 test**, và
  phần dựng text nằm inline trong `handle_tool_call`, chung một hàm với
  lời gọi `runtime::ensure_daemon_bases()` — thứ spawn daemon thật. Nên
  điểm chứng minh của pha này không chạy được nếu giữ nguyên hình dạng đó.
  Tách `fn viewable_text(bases: &[String], rel_path: &str, code: &str) ->
  String` thuần, không I/O, rồi `handle_tool_call` chỉ gọi nó. Đây là điều
  kiện để proof point tồn tại, không phải dọn dẹp tuỳ hứng.
- `crates/mdview/src/cli.rs` — nhánh `open` in cùng định dạng.

**Pha 4 — nâng cấp hệ thống đang chạy** (honors D11, thêm giữa chừng theo
yêu cầu người dùng)

Migration tự chạy khi `SqliteStore::open` mở DB, nên bản thân việc nâng
cấp là tự động và thuần bổ sung. Cái **không** tự lo được là một daemon
bản cũ vẫn đang sống: nâng binary xong, CLI mới migrate DB và phát link
`/s/...`, trong khi daemon cũ trong bộ nhớ không có route đó — người dùng
bấm vào và nhận 404, không hiểu vì sao.

- `crates/mdview-core/src/daemon.rs` — `DaemonInfo` mang thêm
  `version: Option<String>` (Option để lock file cũ vẫn đọc được, không
  phá tương thích); thêm `daemon_version(host, port)` đọc `/health` bằng
  đúng client HTTP thô `health_check` đã dùng, không thêm dependency.
- `crates/mdview/src/doctor.rs` — check `daemon` so version của daemon
  đang chạy với `env!("CARGO_PKG_VERSION")`, lệch thì `Warn` kèm hướng dẫn
  `mdview restart`; thêm check mới `index schema` báo `user_version` và
  liệu `path_hash` đã backfill xong chưa.

### Các hướng đã loại (và vì sao)

| Hướng | Vì sao loại |
|---|---|
| Bảng `shortlinks` riêng + TTL + job dọn | D1: đẻ ra vòng đời thứ hai phải quản lý, đúng thứ đề bài muốn tránh |
| Cache `HashMap` trong daemon | D7: phải cắm hook vào mọi đường ghi (`upsert_file`, `remove_file`, `delete_project`); sót một đường là link 404 oan |
| Quét toàn bảng mỗi request | D7: ~10ms/lần trên 15.480 hàng, và không có lý do gì khi index rẻ như vậy |
| Prefix co giãn kiểu git | D10: kéo theo hàm prefix-ngắn-nhất, query kiểm tra duy nhất, và một nhánh nhập nhằng phải thiết kế |
| `std::DefaultHasher` | D8: không cam kết ổn định giữa các bản Rust ⇒ link cũ chết sau khi nâng toolchain |
| base36 thay hex | Rút được ~2 ký tự, đổi lấy code encode và nhầm lẫn `0`/`o`, `1`/`l` khi đọc |

### Bản đồ rủi ro

| Thành phần | Mức | Điều gì chứng minh được |
|---|---|---|
| **Migration trên DB đang có dữ liệu** | **Cao** | Test mở một DB dựng theo schema **cũ** (không có cột), chạy `from_conn`, khẳng định mọi hàng có `path_hash` đúng 16 hex; chạy `from_conn` lần hai khẳng định không đổi gì (idempotent) và `user_version` không tăng tiếp |
| Truy vấn prefix có dùng index không | Trung bình | **Đã chứng minh** trong Pha 2 ở trên: chỉ dạng bind-một-tham-số mới dùng index; `GLOB ? \|\| '*'` thành `SCAN files`. Test phải khẳng định `EXPLAIN QUERY PLAN` chứa `USING INDEX idx_files_hash` — một test chỉ kiểm kết quả trả về sẽ xanh ở cả hai dạng và không bảo vệ được gì |
| Tính ổn định của hash | Trung bình | Test vector cố định: `path_hash("mdview", "docs/a.md")` phải bằng đúng một hằng số hex viết thẳng trong test — bắt được mọi thay đổi thuật toán sau này |
| Đổi định dạng output MCP | Trung bình | `mcp.rs` hiện **0 test** và phần dựng text dính với `ensure_daemon_bases()` (spawn daemon). Phải tách `viewable_text(...)` thuần trước (xem Pha 3), rồi test trên hàm đó: 1 dòng khi có `hostname`, nhiều dòng khi bind wildcard không hostname, mỗi dòng chứa mã đúng 12 ký tự hex |
| **Daemon cũ còn sống sau khi nâng binary** | **Cao** | Không phải rủi ro dữ liệu mà là rủi ro UX: link ngắn 404 im lặng. Chứng minh bằng test cho `daemon_version` (lock file cũ không có trường `version` vẫn parse được, trả `None`) và cho nhánh so version của `doctor` |
| Route mới đụng route cũ | Thấp | `/s/` không giao với bất kỳ pattern nào trong `router()` (`server.rs:92-111`) — đã xác nhận khi scout |

Ba mục Cao/Trung bình ở trên là điểm chứng minh mang sang
`fgos-coding-validating`, không phải phỏng đoán chốt ở đây.

### Thứ tự

Pha 1 → Pha 2 → Pha 3, tuyến tính. Pha 2 và 3 đều đọc cột do Pha 1 tạo,
nên không có thứ tự nào khác hợp lệ. `fgos graph` không đề xuất gì khác:
item đứng một mình trong đồ thị.

## Các trường hợp đáng chứng minh

- **DB cũ, chưa có cột** — đường migration chính. Phải backfill đủ.
- **DB mới tinh** — `CREATE TABLE` đã có cột; migration phải nhận ra và
  không chạy `ALTER TABLE` lần nữa.
- **Chạy `from_conn` hai lần** — idempotent, `user_version` không tăng
  tiếp.
- **Câu truy vấn prefix dùng index** — `EXPLAIN QUERY PLAN` phải chứa
  `USING INDEX idx_files_hash`. Test hồi quy bắt buộc: nếu ai đó sau này
  đổi sang ghép chuỗi trong SQL, mọi test chức năng vẫn xanh còn hiệu năng
  âm thầm sập.
- **Mã không tồn tại** — 404, không panic.
- **Mã ngắn hơn 12 ký tự** — vẫn là prefix hợp lệ về mặt SQL; hành vi phải
  xác định (khớp hàng đầu theo `ORDER BY` ổn định), không phải ngẫu nhiên.
- **File bị xoá rồi index lại** — `path_hash` phải giữ nguyên vì
  `(project_id, rel_path)` không đổi.
- **`rel_path` có ký tự Unicode** — hash theo byte của UTF-8, không phụ
  thuộc locale.
- **Hành vi cũ không hồi quy** — `/p/<id>/<rel>` vẫn phục vụ như trước;
  test hiện có của `server.rs`/`mcp.rs` vẫn xanh.

## Giả định

- **A1** — Không có tiến trình nào khác đang mở `~/.mdview/registry.db`
  giữa chừng migration. Cơ sở: `runtime.rs` tuần tự hoá việc spawn daemon
  qua `daemon.lock`, và WAL cho phép nhiều reader. Chưa chứng minh; nếu
  sai thì `ALTER TABLE` có thể gặp `SQLITE_BUSY` và cần retry.
- **A2** — 12 ký tự hex là định dạng mã cuối cùng, không cần cấu hình
  được. Cơ sở: D10 ghi rõ đổi độ dài là sửa một hằng số. Nếu sai thì thêm
  một trường config, không phải migrate lại.

## Lệnh chứng minh

```
cargo test --workspace
```

Thật và chạy được (`cargo 1.96.1` có trên máy này). Mọi test nêu trong
bản đồ rủi ro và danh sách trường hợp ở trên đều nằm dưới lệnh này.

## Ghi chú lúc dựng

- **Redirect là 303, không phải 302.** `axum::response::Redirect::to` phát
  303 See Other, và đó cũng đúng thứ `project_home` (`server.rs`) đang
  dùng sẵn. Ý của D3 — redirect thay vì serve nội dung trực tiếp — giữ
  nguyên; chỉ con số ghi trong D3 và §6 là sai. Bám theo hành vi có sẵn
  của repo thay vì ép 302 cho khớp chữ.
- **Index `idx_files_hash` thuộc về migration, không thuộc `SCHEMA`.**
  Test đường legacy bắt được: `SCHEMA` chạy trước `migrate`, mà
  `CREATE TABLE IF NOT EXISTS files` là no-op trên DB cũ — nên tạo index
  trên `path_hash` trong `SCHEMA` sẽ nổ vì cột chưa tồn tại.
- **`MIGRATIONS` là danh sách append-only, đóng dấu `user_version` sau
  từng bước.** Đây là pattern `PRAGMA user_version` chuẩn của SQLite, cũng
  là thứ `rusqlite_migration` cài đặt bên dưới. Với đúng một bước,
  dependency đó chỉ bọc lại danh sách này nên chưa đáng thêm; hình dạng để
  sẵn đúng như nó mong đợi, nên đổi sang dùng nó về sau là thay cơ học.

## Outstanding questions

None
