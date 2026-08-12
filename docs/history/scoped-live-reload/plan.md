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
- **`_search` bị loại trừ tường minh trong `currentFileIdentity()`**,
  không dựa vào bất biến phía server. *(Sửa lại lúc dựng — xem "Ghi chú
  lúc dựng" bên dưới: lý luận ban đầu ở đây là sai.)*

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

## Ghi chú lúc dựng

- **Lý luận ban đầu về `_search` là sai — bắt được bằng proof thật, không
  phải đọc lại.** Kế hoạch lúc đầu cho rằng không cần loại trừ `_search`
  ở client vì server không bao giờ phát `rel_path == "_search"` (do
  `is_markdown()` chỉ index `.md`/`.markdown`). Lý luận đó đúng cho phía
  **server phát cái gì**, nhưng nhầm sang cả phía **client parse thế
  nào** — hai việc khác nhau. `currentFileIdentity()` chỉ dựa vào
  `location.pathname`, không biết gì về `is_markdown()`; nếu có sự kiện
  `rel_path == "_search"` (dù hiện server không tạo ra), trang search vẫn
  parse nhầm thành đang xem file tên `_search` và reload sai — đúng loại
  lỗi D3 muốn xoá bỏ. Bắt được bằng cách chạy proof thật trên chính file
  `app.js` đã commit (Node `vm`, trích đúng khối `ws.onmessage` mới —
  không viết lại logic để test), không phải suy luận. Đã sửa:
  `currentFileIdentity()` loại trừ `_search` tường minh, không dựa vào
  bất biến ở nơi khác.
- **Vì sao proof chạy trên `vm` thay vì thêm test harness:** repo không
  có `package.json`/runner JS nào (đã xác nhận lúc lập plan). Script proof
  nằm ngoài repo (`/tmp` scratchpad), đọc trực tiếp file `app.js` thật
  bằng `readFileSync`, trích đúng khối `ws.onmessage` (khối này tự đứng
  được — chỉ dùng `location`/`WebSocket`/`JSON`/`setTimeout`, không đọc
  gì từ phần đầu file), rồi chạy trong `vm.createContext` với
  `location`/`WebSocket` giả lập tối thiểu. Đây là proof thật trên đúng
  byte đã commit, không phải bản viết lại — nhưng vẫn không phải test tự
  động trong CI, đúng như gap đã ghi nhận ở phần Bản đồ rủi ro.
- **10 kịch bản đã chứng minh bằng proof này:** khớp đúng file → reload;
  khác `rel_path` cùng project → không; cùng `rel_path` khác project →
  không; trang chủ/`/settings`/`/p/:id/_search` → không bao giờ; xoá file
  khớp đúng danh tính → vẫn reload (D4); message JSON hỏng → không crash,
  không reload; `rel_path` Unicode qua percent-encoding → decode và khớp
  đúng; nhiều event trong một batch → chỉ đúng event khớp mới kích reload.

- **Không lấy được proof OS-level filesystem-event đầy đủ trong sandbox
  này — đã xác minh đây là giới hạn môi trường, không phải lỗi code.**
  Thử chạy daemon thật, sửa file thật, đọc `/ws` bằng WebSocket client thô
  (Python, chỉ dùng stdlib) — không nhận được broadcast nào dù đợi 10
  giây. Trước khi kết luận đây là bug, cô lập biến số bằng cách gọi thẳng
  `inotify_add_watch` qua `ctypes` (bỏ qua hoàn toàn mdview): nhận
  **errno 28 (ENOSPC — "No space left on device")**, đúng nghĩa "đã chạm
  giới hạn số lượng inotify watch của hệ thống" (`man inotify`). Thử lại
  sau khi dừng daemon test của chính mình (để loại trừ khả năng do chính
  nó giữ tài nguyên) — vẫn ENOSPC, xác nhận nguyên nhân là các tiến trình
  khác trong sandbox dùng chung, ngoài tầm kiểm soát của item này.
  `spawn_watchers` (code cũ, không phải viết ở đây) đang nuốt lỗi
  `.watch(...)` bằng `.ok()` — đúng chỗ khiến silent failure này khó thấy;
  sửa cách xử lý lỗi đó là việc ngoài phạm vi D1–D4 đã khoá, không làm ở
  item này.
- **Proof thay thế cho phần không chạy được:** tách `broadcast_payload()`
  làm hàm thuần (không I/O) khỏi closure của debouncer, test trực tiếp
  hình dạng gói tin `{"events":[...]}` mà `app.js` parse, và xác nhận
  batch rỗng trả `None` (không gửi gì — đúng cơ chế sửa root cause 1).
  Cộng với test `reindex_paths` (gọi thẳng, không qua OS notify — không bị
  ảnh hưởng bởi giới hạn inotify) đã chứng minh đúng logic "đổi/không
  đổi/xoá". Phần duy nhất chưa chứng minh được trong sandbox này là chuỗi
  tích hợp thật `notify` → `notify_debouncer_full` → closure — bản thân
  các thư viện đó không đổi trong item này, chỉ closure gọi chúng đổi
  (rút gọn 8 dòng logic gửi thành gọi `broadcast_payload`), nên rủi ro
  còn lại là thấp.

## Outstanding questions

None
