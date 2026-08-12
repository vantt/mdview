# CONTEXT — Auth thật cho mdview daemon (token + Cloudflare Access)

Item: `tsk-1j4`.

## Ranh giới feature

**Trong phạm vi.** Thêm authentication thật cho toàn bộ HTTP daemon của
mdview, port theo đúng shape đã dùng ở `~/projects/herdr-gateway`
(Rust/Axum, cùng stack): đăng nhập bằng token → session cookie, cộng
Cloudflare Access JWT làm phương án thay thế khi đã cấu hình. Đây là đảo
ngược có chủ đích một quyết định đã ghi trong
`docs/history/review-2026-07-16-full-app/reports/findings-security.md`
("No auth" is an explicit documented choice — not reversing it) — người
dùng xác nhận trực tiếp muốn đảo ngược.

**Ngoài phạm vi.** Không đổi cách CLI (`mdview open`) hay MCP tool
(`mdview_view_file`) hoạt động — cả hai thao tác thẳng qua `SqliteStore`
chung, không gọi HTTP vào daemon, nên không cần biết về token. Không
thêm middleware/layer toàn cục — giữ đúng kiểu per-handler extractor như
bản gốc. Không tự chọn giá trị mặc định cho `cf_access_team_domain`/
`cf_access_aud` — đây là cấu hình vận hành thật, người dùng phải tự cấp.

## Locked decisions

Chốt trong phiên làm việc ngày 2026-08-12, có xác nhận trực tiếp của
người dùng qua AskUserQuestion cho từng mục có đánh dấu (*).

| ID | Quyết định | Lý do |
|----|-----------|-------|
| D1 | Port gần như nguyên trạng shape của herdr-gateway: `AuthSession` là một Axum extractor (`FromRequestParts`), kiểm session cookie `mdv_session` trước, CF Access JWT là fallback thay thế khi thiếu/hỏng cookie VÀ operator đã cấu hình `cf_access_team_domain`+`cf_access_aud`. | Đã scout xong source thật của `~/projects/herdr-gateway/src/web/auth.rs` và `cf_access.rs` — cùng stack Axum 0.7, port được gần như copy. |
| D2 | (*) Sai token ở `/login` → 404 trống, giống hệt triết lý "silence is the point" của bản gốc. | Người dùng chọn giữ nguyên UX gốc thay vì thông báo lỗi thân thiện. |
| D3 | (*) `web_secret` không bắt operator tự điền trước: nếu config chưa có, daemon tự sinh token ngẫu nhiên ở lần start đầu tiên, lưu vào `config.toml`, in ra stdout một lần. | Người dùng chọn UX kiểu Jupyter/code-server thay vì fail-closed hoàn toàn như bản gốc (bản gốc bắt operator tự cấp `HERDR_GO_WEB_SECRET` trước). |
| D4 | Route ở trạng thái mở, không cần auth: `/health`, `/login` (GET, trang form), `/static/*`, `/highlight.css`. Mọi route còn lại — kể cả `/ws` (cookie tự đính kèm theo same-origin WS handshake) — đều qua `AuthSession`. | `/login` phải mở để còn có cửa vào; static assets phải mở vì trang login cần chúng để render (nếu không sẽ vòng lặp không lối ra); health-check là thông tin thấp nhạy cảm và các công cụ vận hành (mdview status) cần gọi được. |
| D5 | CF Access chỉ bật khi CẢ HAI `cf_access_team_domain` và `cf_access_aud` được cấu hình trong config.toml/Settings — thiếu một trong hai thì tắt hoàn toàn, request có header `Cf-Access-Jwt-Assertion` cũng bị bỏ qua y hệt không có header. | Đúng nguyên bản D5-tương-đương của herdr-gateway (`state.cf_access.as_ref()` là `None` nếu chưa cấu hình đủ) — tránh half-configured state âm thầm bật một phần bảo vệ. |
| D6 | JWT chỉ chấp nhận RS256 tường minh (`Validation::new(Algorithm::RS256)`), ép `exp`/`iss`/`aud`/`nbf` là required claim (không chỉ optional-nếu-có), `iss` phải khớp `team_domain` đã chuẩn hoá (trim trailing slash), `aud` phải chứa đúng tag cấu hình. | Chặn hai lớp bypass kinh điển: `alg:none` và RS256→HS256 key-confusion (cả hai đã có test trong bản gốc, port nguyên cả bộ test này). |
| D7 | Session lưu server-side trong `Arc<Mutex<HashSet<String>>>` (in-memory, mất khi restart daemon — không phải thiết kế thiếu, là lựa chọn có chủ đích của bản gốc), id ngẫu nhiên 24-byte hex qua `rand::thread_rng`. Cookie `HttpOnly; SameSite=Strict; Path=/; Max-Age=604800`. | Đơn giản, không cần DB, khớp mô hình single-operator của cả hai app. Session mất khi restart là chấp nhận được — operator đăng nhập lại. |
| D8 | So token dùng constant-time compare tự viết (không early-return theo byte đầu tiên khác), không dùng crate ngoài cho việc này. | Tránh timing attack lộ dần từng byte đúng của token — port nguyên hàm `constant_time_eq` từ bản gốc, đã có test riêng. |
| D9 | Toàn bộ code Axum-specific (extractor, route, cookie, verifier) nằm trong crate `mdview` (binary), KHÔNG đặt trong `mdview-core`. | `crates/mdview-core/src/lib.rs` doc đã ghi rõ: "this crate never depends on Axum/Tauri" (PRD §7.4) — vi phạm sẽ phá kiến trúc ports & adapters hiện có. Field cấu hình (`web_secret`, `cf_access_team_domain`, `cf_access_aud`) là data thuần nên vẫn đặt trong `mdview-core::config::ServerConfig`, đúng chỗ `hostname`/`host` hiện có. |
| D10 | Không decompose thành nhiều item con. | Ba mảnh (verifier CF Access, auth module, wiring vào route) phụ thuộc TUẦN TỰ hoàn toàn — auth module cần type của verifier, wiring cần extractor của auth module — không có lợi ích chạy song song như item Code viewer trước đó, nên chia nhỏ chỉ thêm chi phí worktree/merge mà không được gì. |

## Route được bảo vệ (áp dụng D4)

Mở (không cần `AuthSession`): `/health`, `GET /login`, `POST /api/login`,
`POST /api/logout`, `/static/app.css`, `/static/app.js`,
`/static/mermaid.min.js`, `/highlight.css`.

Đóng (bắt buộc `AuthSession`): `/`, `/api/status`, `/api/projects`,
`/api/projects/:id/unregister`, `/settings`, `GET+POST /api/config`,
`/ws`, `/s/:code`, mọi route `/p/:id/*` (Docs và Code section).

## Kế hoạch triển khai (một item, không chia con)

Ba module theo đúng ranh giới file của bản gốc, làm tuần tự trong cùng
một lần implement vì phụ thuộc chồng nhau:

1. `crates/mdview/src/cf_access.rs` — port gần như nguyên `cf_access.rs`
   của herdr-gateway (JWKS fetch/cache TTL 1h, verify RS256, test key
   material cho test offline). Thêm dependency `jsonwebtoken = "9"`,
   `reqwest = { version = "0.12", default-features = false, features =
   ["json", "rustls-tls"] }` vào workspace.
2. `crates/mdview/src/auth.rs` — port `auth.rs`: `AuthSession` extractor,
   `login`/`logout` handler, `session_cookie`/`constant_time_eq`/
   `new_session_id` helper. Thêm dependency `async-trait = "0.1"`,
   `rand = "0.8"`. `AppState` (`server.rs`) thêm field `web_secret:
   Arc<Option<String>>`, `sessions: Arc<Mutex<HashSet<String>>>`,
   `cf_access: Arc<Option<CfAccessVerifier>>` cộng builder
   `with_cf_access`. `Config::server` (`mdview-core/src/config.rs`) thêm
   `web_secret: Option<String>`, `cf_access_team_domain: Option<String>`,
   `cf_access_aud: Option<String>`. Logic tự sinh token lần đầu (D3) đặt
   trong `runtime::build_engine`-adjacent startup path của `serve()`.
3. Wiring: thêm `GET /login` (view mới, form đơn giản POST tới
   `/api/login`) và `POST /api/login`/`POST /api/logout` vào router;
   thêm `_auth: AuthSession` vào chữ ký mọi handler thuộc danh sách
   "Đóng" ở trên; `/settings` form thêm field `cf_access_team_domain`/
   `cf_access_aud` (không hiện `web_secret` ở dạng plain text trên UI —
   chỉ nút "regenerate token" nếu cần, tránh lộ qua DOM/history).

## Test bắt buộc

Port nguyên bộ test đã có ở bản gốc (đều áp dụng được cho mdview với
tên route đổi tương ứng):
- `constant_time_eq` đúng ngữ nghĩa (equal/mismatch/length-diff).
- Request không auth tới route đã bảo vệ → 404 trống.
- Token sai ở `/api/login` → 404 trống.
- Token đúng → set cookie `HttpOnly`+`SameSite=Strict`, cookie đó dùng
  lại được để vào route đã bảo vệ.
- CF Access chưa cấu hình: header `Cf-Access-Jwt-Assertion` bất kỳ bị
  bỏ qua hoàn toàn, vẫn 404 trống (không rò rỉ rằng có xử lý header đó).
- CF Access đã cấu hình + JWT hợp lệ ký đúng → vào được, không cần cookie.
- CF Access đã cấu hình + JWT giả/không ký được → 404 trống y hệt không
  có gì.
- CF Access: cookie vẫn hoạt động song song khi CF Access đã bật.
- CF Access verify: `alg:none` bị từ chối; RS256→HS256 key-confusion bị
  từ chối; thiếu `aud`/`iss` (dù ký hợp lệ) bị từ chối vì là required
  claim; token hết hạn/chưa hiệu lực (`exp`/`nbf`) bị từ chối; team
  domain có trailing slash vẫn khớp đúng `iss` đã chuẩn hoá.
- e2e thật (giống `tests/e2e_open.rs`): daemon thật, chưa login →
  `/` trả 404; login bằng token đúng → cookie dùng lại được để vào `/`
  và `/p/:id/_code/`.
- Token tự sinh lần đầu (D3): config chưa có `web_secret` → sau khi
  start, `config.toml` có `web_secret` không rỗng, và giá trị đó được in
  ra stdout đúng một lần.

## Rủi ro

- Session in-memory (D7) nghĩa là restart daemon (kể cả bản build mới)
  bắt tất cả browser đang mở phải đăng nhập lại — chấp nhận được, không
  phải regression cần né.
- `/ws` gate qua cookie same-origin: cần xác nhận trình duyệt thật sự gửi
  cookie trên WebSocket upgrade request (đúng theo spec, nhưng đáng test
  bằng e2e thật thay vì chỉ tin lý thuyết).
- CF Access JWKS fetch cần mạng ra ngoài (`{team_domain}/cdn-cgi/access/certs`)
  — nếu mdview chạy offline/air-gapped và operator lỡ bật CF Access, mọi
  request qua nhánh CF Access sẽ fail (nhưng cookie vẫn hoạt động bình
  thường, không phải outage toàn phần).
