# Nhật ký: README mdview sẵn sàng launch

## 2026-10-27, vòng 1: đánh giá và ảnh nháp

**Binary cài trên máy cũ hơn code.** Binary trên PATH là `0.8.8`, release mới nhất là `0.8.9`, còn main
có thêm bản sửa CSS chưa phát hành. Nguyên nhân: cài bằng tay rồi không `mdview update`. Cách giải:
build `target/release/mdview` từ main, chạy sandbox với `MDVIEW_BIN=` trỏ vào đó. Bài học: trước khi
chụp, xem ba con số (binary đang chạy, release, HEAD) và chọn rõ là chụp theo con số nào.

**Thử cài trên máy sạch mà không đụng máy thật.** Chạy với `env -i HOME=<scratch> PATH=/usr/bin:/bin`.
`-i` bỏ cả `~/.local/bin` khỏi PATH, nhờ vậy mới thấy lỗi `mdview: not found` mà người mới sẽ gặp.
Trước khi chạy `doctor --fix --mcp` phải đọc code: trong WSL, lệnh này ghi vào config Claude Desktop
thật ở `/mnt/c/Users/...`, HOME tạm không che được. Vì vậy chỉ chạy `doctor --mcp` (kiểm tra, không sửa).

**`doctor --fix` ghi file vào cwd.** Chạy ở thư mục trống thì `AGENTS.md` và `CLAUDE.md` mọc ra.
Chỉ thấy được khi chạy ở thư mục riêng rồi `ls`. Bài học: thử lệnh "cài đặt" ở một thư mục trống, xem
nó để lại gì.

**Hai daemon cùng lúc.** Daemon thử (HOME sạch) và sandbox tự lấy port 7701/7711, vì 7700/7710 đã có
daemon chạy. Lệnh `mdview search` mới đầu tưởng hỏng do trỏ nhầm daemon. Đọc `cli.rs` thì thấy không
phải: `search` không có `--project` chỉ tìm trong phần đã index nội dung, nên đây là lỗi thật. Bài học:
đọc code trước khi đổ cho môi trường thử.

**Project bị đăng ký thành `docs`.** Sandbox ở scratchpad không có `.git`, nên `open` lấy `docs/` làm
root, và breadcrumb hiện "docs". Cách giải: `unregister docs` rồi `register <dir> --name acme-payments`.

**Fixture quá mỏng để chụp.** Fixture hero chỉ có một plan thật, các file khác dài 1 đến 3 dòng, nên
search và bảng rộng không có gì để xem. Cách giải: bổ sung ADR, runbook, bảng mã decline 8 cột ngay
trong bản sao ở scratchpad. Nội dung là giả, nhưng phần render là của mdview thật.

**Ảnh desktop không đọc được ở lưới 2 cột của README.** Ảnh 1440px bị thu còn khoảng 420px trên GitHub.
Chỉ phát hiện ra khi dựng một trang giả README rộng 880px rồi chụp lại. Bài học: luôn xem ảnh ở đúng
kích thước nó sẽ hiện, đừng chỉ xem ảnh gốc.

**Zoom Mermaid trước khi chụp làm cắt mất đầu diagram.** Bỏ zoom, cuộn tới heading "Flow" rồi hover
để thanh điều khiển hiện ra.

## 2026-10-27, vòng 2: sửa README

**Quyết định:** anh giữ token mặc định `demo`, không đổi code. README ghi rõ token là `demo` và thêm
một khung `[!WARNING]`: daemon nghe trên mọi interface, ai biết token thì đọc và sửa được, nên đổi
`web_secret` rồi `mdview restart`.

**Kiểm `hostname` trước khi viết hướng dẫn SSH tunnel.** Trên HOME sạch, đặt `hostname = "localhost"`
rồi chạy `restart` và `open`: link in ra là `http://localhost:<port>/s/…`. Hướng dẫn này đúng cho tunnel
`-L 7700:localhost:7700`. Tunnel thật thì chưa chạy thử.

**Installer PowerShell khác bản shell.** Bản PowerShell cài vào `%USERPROFILE%\.mdview\bin`, không theo
chuỗi `/usr/local/bin`. Phát hiện ra khi đọc `install.ps1` trước khi viết câu về PATH. Bài học: câu nào
nói "the installer", phải đọc cả hai bản installer.

## 2026-10-27, vòng 3: chụp lại "See it"

**Bố cục mới.** Ảnh Reading đặt full width. Hai hàng 2 cột ghép theo tỉ lệ khung gần nhau: Mermaid với
Search (khoảng 1,5 và 1,8), Bảng với Edit (khoảng 1,2 và 1,35). Hàng cuối là hai ảnh điện thoại, mỗi ảnh
rộng 260px. Ghép sai cặp thì hàng trong bảng bị lệch chiều cao.

**Icon thư mục hiện thành ô vuông rỗng.** CSS dùng emoji `\1F4C1`, còn WSL không có font emoji. Cách giải:
tạo một `fonts.conf` tạm có thêm thư mục chứa `seguiemj.ttf` (copy từ `/mnt/c/Windows/Fonts`), rồi chạy
với `FONTCONFIG_FILE=…`. Không cài gì vào hệ thống. Trên điện thoại thật icon vẫn hiện bình thường.

**Click vào dòng bị xuống hàng trong CodeMirror.** `End` chỉ nhảy tới cuối dòng hiển thị, nên chữ bị chèn
vào giữa link. Cách giải: chọn một dòng ngắn không bị xuống hàng, rồi `End` và `Enter`.

**Lệnh `rm` với biến bị chặn, và cả lệnh không chạy.** Một bước hỏng kéo theo cả chuỗi lệnh ghép bằng
`&&`, kể cả các bước trước nó, nên ảnh xem trước không được cập nhật. Bài học: đọc lỗi trước khi xem
kết quả, và dùng đường dẫn cụ thể với `rm`.

**Lỗi UI mới phát hiện:** ở bề rộng 1000px, trang search mất lề trái (ô search và các thẻ kết quả dính
sát mép). Trên điện thoại, thanh zoom Mermaid đè lên các khung diagram.

## 2026-10-27, vòng 4: commit và push

**Push bị từ chối vì origin/main có thêm 2 commit.** Trong lúc em làm, anh đã thêm link video hero qua trình
sửa trên web (`79515ee`, `9836b23`), đặt ở đầu mục "See it". Cách giải: `git fetch`, xem diff của hai commit
đó, rồi rebase. Khi gỡ xung đột, em giữ bản README mới, đặt link video đúng chỗ anh chọn, và bỏ comment
`HERO DEMO`. Đã push lên main: `552fb44`. Bài học: chạy `git fetch` ngay trước khi commit chứ không chỉ lúc
bắt đầu làm, vì người khác (và chính anh, trên web) vẫn đang sửa song song.
