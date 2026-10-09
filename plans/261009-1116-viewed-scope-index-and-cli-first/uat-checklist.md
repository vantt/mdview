# UAT — mdview bản mới (index theo file đang xem, tìm kiếm mới, CLI-first)

Bản UAT chạy **song song** với mdview đang dùng, không đụng vào nhau:

| | Bản đang dùng | Bản UAT (build mới) |
|---|---|---|
| Lệnh | `mdview` | `~/.mdview-uat/mdview-uat` |
| Địa chỉ | `http://100.121.46.35:7700` | `http://100.121.46.35:7800` |
| Dữ liệu | `~/.mdview` | `~/.mdview-uat/home/.mdview` |

Bản UAT chỉ nghe trên IP Tailscale, nên chỉ máy trong tailnet (laptop `lap-vantt`) mở được.

## 0. Chuẩn bị (1 phút)

1. Trên laptop Windows, mở Chrome/Edge tới `http://100.121.46.35:7800`.
   Nếu không vào được: kiểm tra Tailscale trên laptop đang **Connected**.
2. Lấy token đăng nhập (chạy trong terminal SSH):
   ```sh
   grep web_secret ~/.mdview-uat/home/.mdview/config.toml
   ```
   Dán giá trị vào trang Sign in.
3. Mở một file thật của bạn bằng bản UAT, ví dụ:
   ```sh
   ~/.mdview-uat/mdview-uat open --json ~/projects/workshop-ecosystem/README.md
   ```
   Lệnh in JSON; mở `url` (link ngắn `/s/…`) trên laptop.

> Mẹo: dùng thêm một project nhỏ và một project lớn (`workshop-ecosystem` có ~2.300 file `.md`, nhiều trong `.grok/`, `.omp/`).

## 1. Mở file và đọc (index theo file đang xem)

- [ ] `mdview-uat open --json <file>` trả về ngay (< 1 s), JSON có `url`, `urls`, `path`, `code`, `project_id`.
- [ ] Mở link: trang hiện đúng, có mục lục, code block có nút copy, Mermaid (nếu có) zoom được.
- [ ] Link sang file `.md` khác trong cùng project bấm được, **không 404**, kể cả file chưa từng mở.
- [ ] Link tới file không phải markdown (`.env`, `.toml`…) **không** hiện nội dung file đó.
- [ ] Không thấy khung editor khi chưa bấm **Edit**.

## 2. Sidebar

- [ ] Sidebar liệt kê **mọi** file `.md` của project (kể cả file chưa mở, thư mục ẩn như `.claude/`), không chỉ file đã xem.
- [ ] File đã xem/đã index hiện **tiêu đề thật**; file chưa index hiện tên file — chấp nhận được.
- [ ] Tạo một file `.md` mới trong project → trong vài giây (tải lại trang) nó xuất hiện ở sidebar.

## 3. Palette nhảy file (Ctrl+K)

- [ ] Ctrl+K khi ô trống: danh sách **file mới sửa gần đây**.
- [ ] Gõ một phần tên/đường dẫn → file đúng lên đầu.
- [ ] Gõ tiêu đề **không dấu** (vd "huong dan") → khớp tiêu đề có dấu.
- [ ] Mũi tên + Enter mở file; Esc đóng.
- [ ] Dòng đầu "Search content for …" → Enter mở trang tìm nội dung.

## 4. Tìm nội dung

- [ ] Lần tìm đầu trên project lớn: dòng trạng thái "Synced N files (M read) in X s" — ghi lại X: ______ s.
- [ ] Tìm lại ngay: "Index reused"; sau > 10 s: "… (0 read)".
- [ ] Kết quả xếp hợp lý, đoạn trích có tô vàng từ khoá.
- [ ] Tìm **không dấu** ("duoc", "tai lieu", "danh gia") ra tài liệu tiếng Việt có dấu.
- [ ] Nút **This folder** thu hẹp vào thư mục của file đang xem; **Whole project** mở rộng lại.
- [ ] Nút **Newest** xếp theo thời gian sửa; **Relevance** theo độ liên quan; đổi nút không mất từ khoá.
- [ ] Tìm thấy nội dung trong thư mục ẩn (`.grok/`, `.claude/`…), không thấy gì trong `node_modules/`, `target/`, `.git/`.

## 5. Live reload và sửa file

- [ ] Mở một file, rồi sửa file đó trên đĩa (agent hoặc `echo "test" >> file.md`) → trang tự cập nhật trong ~1–2 s.
- [ ] Bấm **Edit**, sửa, Ctrl+S → lưu được, trang hiển thị nội dung mới.
- [ ] Trong lúc đang Edit, sửa file đó từ terminal rồi bấm Save → báo **conflict**, không ghi đè.
- [ ] File mới agent vừa tạo trong thư mục đang xem → tìm kiếm thấy được.

## 6. Agent (CLI-first)

- [ ] Trong một phiên Claude dùng thử, nhờ agent: "tạo `docs/demo.md` rồi mở bằng `~/.mdview-uat/mdview-uat open --json`" → agent đưa link, mở được.
- [ ] `~/.mdview-uat/mdview-uat doctor --dry-run` chạy được, **không** sửa gì; đọc các dòng báo cáo (permission, schema, MCP bị bỏ qua vì chưa `--mcp`).
  - Lưu ý: **đừng** chạy `--fix` của bản UAT — nó sẽ sửa `~/.mdview-uat/home/.claude/settings.json` (thư mục giả), vô hại nhưng không có ý nghĩa.

## 7. Giao diện / thiết bị

- [ ] Light/dark mode đều đọc tốt, trang tìm kiếm không vỡ layout.
- [ ] Cửa sổ hẹp (hoặc mở trên điện thoại trong tailnet): sidebar thành drawer, tìm kiếm dùng được, không cuộn ngang.

## 8. Tài nguyên

- [ ] Sau khi dùng một lúc: `ls -la ~/.mdview-uat/home/.mdview/` — ghi kích thước `registry-v4.db`: ______
- [ ] `ps -o rss,cmd -C mdview` — RSS của daemon UAT: ______

## Ghi lỗi

Ghi lại: bước nào, làm gì, thấy gì, mong đợi gì (ảnh chụp nếu có). Gửi lại cho Claude để sửa.

## Kết thúc UAT

```sh
~/.mdview-uat/mdview-uat stop      # dừng daemon UAT
rm -rf ~/.mdview-uat               # xoá toàn bộ bản UAT khi không cần nữa
```
