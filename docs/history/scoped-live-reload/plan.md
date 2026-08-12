# plan.md — Chỉ reload đúng browser liên quan

Item: `tsk-2io` · Quyết định khoá: [`CONTEXT.md`](./CONTEXT.md)

Mode: **standard**

**Đếm cờ: 2/10.** `data model` (thêm cột `content_hash` + migration thứ
hai trên DB bền) · `weak proof around the area` (`watch.rs` hiện có **0
test** — đúng chỗ đang sửa, và đúng chỗ mà một lỗi lọc sai sẽ im lặng,
giống hệt bản chất của bug đang sửa).

Không cờ hard-gate: không đụng auth/data-loss/external-provider. Xếp
`standard` chứ không phải `small` vì kéo theo migration thứ hai của repo
và sửa đúng vùng chưa có test nào bảo vệ.

**`impact-analysis: inactive`** — như `CONTEXT.md` đã ghi, `providers: []`
trên máy này. Không có bằng chứng blast-radius; dựa hoàn toàn vào test
thật.

## Không tách item

Cùng lý do đã áp dụng ở `tsk-3sl`: Pha 1 và Pha 2 cùng chạm
`crates/mdview-core/src/repository.rs` / `indexer.rs` (Pha 1 thêm cột và
đổi chữ ký `upsert_file`, Pha 2 tiêu thụ giá trị trả về mới đó qua
`index_file_incremental`) — tách ra là tự tạo va chạm footprint. Pha 3
(client JS) không đứng được một mình vì không có gì để test nếu server
chưa phát đúng định dạng. `fgos graph --json` xác nhận không có gì để sắp
xếp lại (component đơn, không có nhánh song song thật).

## Cách làm

### Đường đã chọn

**Pha 1 — `content_hash` + đổi `upsert_file`** (honors D2)

- Module mới `crates/mdview-core/src/hash.rs`: `pub(crate) fn
  fnv1a64_hex(bytes: &[u8]) -> String` — nơi duy nhất chứa vòng lặp
  FNV-1a. Tách ra khỏi `short_link.rs` vì hash nội dung file không phải
  mối quan tâm của "short codes for file URLs"; cả hai module gọi chung
  primitive này để không có hai bản thuật toán có thể lệch nhau theo
  thời gian.
- `short_link::path_hash` đổi sang gọi `hash::fnv1a64_hex` — **hành vi
  giữ nguyên byte-for-byte**, test vector đã ghim ở `tsk-3sl` phải tiếp
  tục khớp không đổi.
- `indexer::content_hash(content: &str) -> String` — gọi
  `hash::fnv1a64_hex(content.as_bytes())`. Đặt ở `indexer.rs` vì đó là
  nơi ngữ nghĩa "nội dung file này có đổi không" thực sự sống.
- `repository.rs`: `SCHEMA` thêm cột `content_hash TEXT NOT NULL DEFAULT
  ''`. `MIGRATIONS` thêm entry thứ hai (append-only, không sửa
  `migration_1_path_hash`):
  ```rust
  const MIGRATIONS: &[(i64, fn(&Connection) -> Result<()>)] = &[
      (1, migration_1_path_hash),
      (2, migration_2_content_hash),
  ];
  pub const SCHEMA_VERSION: i64 = 2;
  ```
  `migration_2_content_hash` thêm cột nếu thiếu, rồi backfill bằng
  `LEFT JOIN files_fts` (đọc `content` đã lưu sẵn trong FTS, **không**
  đọc lại từ đĩa — nhất quán với cách `migration_1` backfill mà không
  cần filesystem).
- `upsert_file` đổi chữ ký `Result<()>` → `Result<bool>`: `SELECT
  content_hash` trước khi ghi, so với hash mới tính từ `content` (tham
  số đã có sẵn, không đọc thêm), trả `true` nếu khác (hoặc hàng chưa từng
  tồn tại).

**Pha 2 — `watch.rs` phát đúng sự kiện** (honors D1, D3, D4)

- `IndexService::index_file` (`indexer.rs`) đổi trả về
  `Result<Option<(IndexedFile, bool)>>` — `bool` là giá trị `upsert_file`
  trả lại, xuyên suốt.
- `Engine::index_file_incremental` (`engine.rs`) đổi trả `Result<bool>`.
  Caller hiện có (`view_file`) chỉ dùng `?` ở vị trí statement — không
  cần sửa gì thêm ở đó.
- `watch.rs`: `reindex_paths` đổi từ trả `bool` sang trả `Vec<ReloadEvent>`
  (struct nhỏ `{project_id, rel_path, kind}` với `kind: Changed |
  Removed`). File còn tồn tại → gọi `index_file_incremental`, chỉ đẩy
  `Changed` nếu trả `true`. File không còn tồn tại → luôn đẩy `Removed`
  (D4 — không so hash, không có gì để so).
- `spawn_watchers`: nếu `events` không rỗng, gói thành một message JSON
  `{"events":[{"kind":"changed","project_id":"..","rel_path":".."}, ...]}`
  và gửi **một lần** qua `reload_tx` (giữ nguyên kiểu
  `broadcast::Sender<String>` — không đổi `AppState`, không đổi
  `ws_handler`/`handle_ws`, đúng điểm mạnh của D1: lọc ở client, server
  không giữ thêm state). Batch rỗng → không gửi gì (đây chính là chỗ sửa
  root cause 1 — "chạm không đổi nội dung" giờ không tạo message nào cả).

**Pha 3 — `app.js` tự lọc theo danh tính đang xem** (honors D1, D3)

- Thay khối `ws.onmessage` (`app.js:707-713`): parse
  `location.pathname` bằng `^\/p\/([^/]+)\/(.+)$` để lấy
  `(projectId, relPath)` của chính trang đang mở; không khớp (trang chủ,
  `/settings`) → không bao giờ reload, đúng D3 mà không cần thêm nhánh gì
  (các trang đó tự nhiên không match).
- Parse message thành `{events: [...]}`; nếu bất kỳ event nào khớp cả
  `projectId` lẫn `relPath` của trang hiện tại → `location.reload()`.
  Không quan tâm `kind` — cả `changed` lẫn `removed` đều reload khi khớp
  danh tính (D4).
- **Không cần loại trừ `/p/:id/_search` bằng tay.** `is_markdown()`
  (`indexer.rs`) chỉ index file `.md`/`.markdown`; chuỗi `"_search"`
  không có phần mở rộng đó nên không bao giờ là `rel_path` thật của một
  sự kiện server phát ra. Route `/p/:id/_search` (`server.rs:107`) là
  static route, axum khớp nó trước wildcard `/p/:id/*path` nên
  `search_page` không bao giờ nhận nhầm là file — bất biến này được giữ
  bởi bản chất của `is_markdown`, không phải bởi code mới viết ở đây.

### Các hướng đã loại (và vì sao)

| Hướng | Vì sao loại |
|---|---|
| Server giữ bảng "socket nào xem file nào", lọc ở `handle_ws` | D1: thêm state phải đồng bộ đúng lúc connect/disconnect/navigate — đúng loại phức tạp D1 chọn né bằng cách để client tự lọc |
| Tái dùng `files_fts.content` để so sánh mỗi lần watcher chạy | D2: cột UNINDEXED, so `=` là quét tuyến tính trên đường nóng — đúng bẫy đã bắt ở `tsk-3sl` |
| Đổi `broadcast::Sender<String>` thành kiểu có cấu trúc (`Sender<ReloadBatch>`) | Không cần: `handle_ws` chỉ forward nguyên văn, việc serialize/deserialize JSON ở biên (gửi) và biên (nhận JS) là đủ; đổi kiểu kênh không mua thêm gì, chỉ đổi chữ ký không cần thiết |
| Gửi một message WS riêng cho mỗi file đổi (thay vì gộp theo batch debounce) | Một lần rescan có thể đổi nhiều file cùng lúc (vd. git checkout) — gộp thành một message giữ đúng tinh thần "một tick debounce, một lượt thông báo" đã có sẵn |

### Bản đồ rủi ro

| Thành phần | Mức | Điều gì chứng minh được |
|---|---|---|
| **`content_hash` migration trên DB đang có dữ liệu** | **Cao** | Cùng khuôn test đã dùng cho `path_hash`: DB dựng theo schema thiếu cột → migrate → mọi hàng có `content_hash` đúng 16 hex, backfill từ `files_fts.content` khớp giá trị tính trực tiếp từ nội dung gốc, chạy hai lần idempotent |
| `upsert_file` phân biệt đúng "đổi"/"không đổi"/"mới" | Trung bình | Test: upsert lần 1 (file mới) → `true`; upsert lần 2 cùng nội dung → `false`; upsert lần 3 nội dung khác → `true` |
| `path_hash` không đổi sau khi tách `hash.rs` | Cao (hồi quy im lặng nếu sai) | Test vector cũ ở `short_link.rs` (`tsk-3sl`) phải tiếp tục pass y nguyên — không sửa giá trị mong đợi, chỉ sửa nơi hàm sống |
| `reindex_paths` phát đúng sự kiện | Trung bình | Test: file đổi nội dung → `Changed` đúng `project_id`/`rel_path`; file "chạm" không đổi byte → **không có event nào** cho path đó; file bị xoá → `Removed` dù không còn nội dung để hash |
| Client-side scoping (`app.js`) | **Không có test tự động** | Repo không có JS test harness (không `package.json`, không runner) — thêm một cái cho một hàm ~10 dòng là bất cân xứng. Logic giữ tối giản (regex + so chuỗi, không nhánh ẩn) và xác minh bằng e2e thật (mở kết nối WS thứ hai giả lập browser không liên quan, xác nhận không nhận reload). Đây là gap được ghi nhận, không phải bị bỏ sót — cùng mức độ (0 test) mà đoạn `ws.onmessage` 6 dòng hiện tại vốn đã chạy production |

## Các trường hợp đáng chứng minh

- **File thay đổi nội dung thật** — đúng 1 event `changed`, đúng
  `project_id`/`rel_path`.
- **File bị "chạm" không đổi byte** — 0 event cho path đó (không phải
  event với cờ nào đó, mà là hoàn toàn không xuất hiện trong batch).
- **File bị xoá** — 1 event `removed`, dù không đọc được nội dung mới.
- **Nhiều file đổi trong cùng một tick debounce** — một message duy nhất
  mang mảng nhiều event.
- **File ở project khác với file đang xem** — client không match, không
  reload (bằng chứng chính cho D1).
- **Cùng file, đúng project** — client match, reload.
- **Trang `/`, `/settings`, `/p/:id/_search`** — `location.pathname`
  không parse ra `(projectId, relPath)` hợp lệ → không bao giờ reload.
- **DB cũ chỉ có `path_hash` (đã qua migration 1, chưa qua migration 2)**
  — `migrate()` chỉ chạy bước 2, không chạy lại bước 1 (đúng vai trò của
  vòng lặp `MIGRATIONS` đã dựng ở `tsk-3sl`).

## Giả định

- **A1** — `files_fts.content` luôn phản ánh đúng nội dung file tại thời
  điểm index gần nhất (không có đường ghi nào cập nhật `files` mà bỏ qua
  `files_fts`). Cơ sở: `upsert_file` luôn ghi cả hai trong cùng một lệnh
  gọi (`repository.rs:96-122`). Nếu sai, backfill `content_hash` sẽ tính
  sai cho những hàng bị lệch — nhưng đây không phải trạng thái mới, mọi
  đường đọc `files_fts` khác (search) đã ngầm giả định điều này từ trước.
- **A2** — Không có browser nào giữ hai tab mở cùng lúc trên hai
  `rel_path` khác nhau nhưng chia sẻ một kết nối WS. Cơ sở: mỗi tab mở
  một `WebSocket` riêng (`app.js:707`, gọi trong closure `connect()` mỗi
  lần trang tải). Nếu sai (một số trình duyệt dùng chung service worker
  gộp kết nối) thì một tab có thể nhận reload thay cho tab khác — chưa
  gặp trường hợp này trong codebase, không có cơ chế nào tạo ra nó.

## Lệnh chứng minh

```
cargo test --workspace
```

## Outstanding questions

None
