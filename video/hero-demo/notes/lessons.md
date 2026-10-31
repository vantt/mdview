# A1. Nhật ký khó khăn và bài học

Ngày bắt đầu: 2026-10-26. Ghi lại trong lúc làm video [A1 mdview hero demo](script.md): vấn đề gặp phải, nguyên nhân, cách giải quyết, bài học rút ra. Mỗi mục ghi ngay khi xảy ra, không viết lại sau.

Bộ công cụ ở `~/projects/mdview/video/hero-demo/`.

## 1. Định hướng và nội dung

| # | Vấn đề | Nguyên nhân | Cách giải quyết | Bài học |
|---|---|---|---|---|
| 1 | Em chẩn đoán sai vì sao anh chưa ưng video skillstation ("mascot", "không hợp người xem dev") | Suy từ README và bảng góp ý, không hỏi người làm | Anh nói rõ: graphic, màu sắc, phong cách không đẹp. Sửa đề xuất A2 | **Hỏi người làm vì sao họ chưa ưng, trước khi đoán.** Góp ý về thẩm mỹ khác góp ý về nội dung |
| 2 | Kịch bản bản 1 dẫn bằng "link 404" | Lấy pain point từ PRD, đó là triệu chứng kỹ thuật | Anh nêu pain thật: agent trên remote server trả đường dẫn không click được; mdview cho link, đọc đẹp, sửa tại chỗ. Viết lại kịch bản | **Pain point lấy từ người dùng thật, không lấy từ tài liệu thiết kế.** Hook phải là nỗi đau người xem nhận ra ngay |
| 3 | Em đưa câu sai "There is no login" vào README mdview rồi push | Đọc README trong repo local đã cũ 145 commit, không kiểm bản đang chạy | Phát hiện khi chụp giao diện thì gặp trang đăng nhập; sửa và push thêm commit | **Kiểm sự thật về sản phẩm trên bản đang chạy hoặc bản mới nhất,** nhất là trước khi push. `git fetch` trước khi đọc |
| 4 | Chọn phần nhìn không có căn cứ | Chưa có mẫu tham chiếu | Gom 5 phong cách từ video dev tool thật, anh chọn P1 + nhấn P2 (mdview), P3 (skillstation) | **Cho người duyệt chọn từ mẫu có thật**, không mô tả bằng lời |
| 5 | Có nguy cơ làm hỏng cả video vì phần nhìn | Lần trước (skillstation) dựng xong mới thấy xấu | Làm style frame tĩnh, duyệt trước khi quay | **Duyệt khung tĩnh trước khi animate.** Sửa khung tĩnh rẻ hơn sửa video đã render |

## 2. Sản phẩm phát hiện lỗi nhờ làm video

| # | Vấn đề | Nguyên nhân | Cách giải quyết | Bài học |
|---|---|---|---|---|
| 6 | Cảnh 3 style frame: nội dung dính sát mép sidebar | Lỗi CSS thật của mdview: `.fg-reading` căn giữa bằng hai cột `minmax(0,1fr)`, co về 0 ở màn hình 700–1300px | Thêm `padding-inline` khi >700px, đo trước/sau ở 6 khổ màn hình, push commit `5f2c3a2` | **Làm video sản phẩm là một vòng QA.** Chụp ở nhiều khổ màn hình, đo bằng số, không chỉ nhìn |

## 3. Quay tự động (Playwright, VHS)

| # | Vấn đề | Nguyên nhân | Cách giải quyết | Bài học |
|---|---|---|---|---|
| 7 | Video Playwright có viền xám, nội dung chỉ chiếm góc | Playwright ghi theo kích thước CSS của viewport, `deviceScaleFactor` không áp vào video | Ghi đúng 1536×864, ffmpeg phóng lên 1920×1080 | **Đọc kỹ cách công cụ ghi hình tính kích thước**; xem khung hình thật ngay lần đầu |
| 8 | Chụp giao diện gặp trang đăng nhập | mdview bản mới bắt buộc token | Script tự đăng nhập bằng `MDVIEW_TOKEN` (mặc định `demo`) | Môi trường demo cần tự đăng nhập, không để màn hình login lọt vào video |
| 9 | Muốn link agent trả về hiện `build-01` mà không đụng mdview thật của anh | Config mdview theo `$HOME` | Daemon demo thứ hai với HOME riêng, cổng 7710, `hostname = build-01`; `.mcp.json` của project trỏ về đó | **Sandbox hóa môi trường demo**: HOME riêng, cổng riêng, không động vào máy thật |
| 10 | Lần đầu chạy `claude` trong thư mục demo bị hỏi trust, Enter chọn "No, exit" | Hộp thoại trust mặc định chọn thoát | Tape `warmup` ẩn, bấm ↓ rồi Enter | Mọi hộp thoại lần đầu phải xử lý ngoài máy quay |
| 11 | Mỗi take mất 5–7 phút, có lần treo tới timeout | Agent **đứng chờ duyệt lệnh Bash**; `acceptEdits` chỉ duyệt sửa file | Chặn Bash trong project demo (`deny: ["Bash"]`), agent đọc bằng tool file | **Agent thật trong video phải chạy không cần người duyệt.** Soi màn hình thật trước khi đoán nguyên nhân chậm |
| 12 | Render VHS bị kill, mất cả ảnh chụp | Ghi liên tục 7 phút (≈10.000 khung 1920px), ffmpeg hết bộ nhớ | `Hide` trong lúc agent chạy, chỉ ghi lúc gõ prompt và kết quả | **Đừng ghi đoạn chờ.** Đằng nào cũng cắt khi dựng |
| 13 | Ký tự `⏺` của Claude Code hiện ô vuông | Geist Mono không có glyph này | Cài thêm Noto Sans Symbols 2 làm font dự phòng | Kiểm glyph đặc biệt của TUI khi đổi font |
| 14 | Dòng vàng "Transcript saving is off" trong take | VHS thừa hưởng biến môi trường từ phiên Claude đang điều khiển | `render.sh` bỏ `CLAUDECODE`, `CLAUDE_CODE_CHILD_SESSION` | Môi trường quay phải sạch như máy người dùng |
| 15 | Tape chờ "Worked for" mãi không thấy | Claude Code đổi động từ ngẫu nhiên (Cooked, Baked, Cogitated…); lượt ngắn có khi không in dòng này | Đổi regex, rồi bỏ hẳn | **Đừng dựa vào chữ hiển thị không ổn định** |
| 16 | `Wait+Screen` của VHS không thấy câu trả lời dù agent đã xong | VHS đọc **phần đầu bộ đệm**, không đọc phần đang hiển thị khi output đã cuộn | Chờ cố định 120 giây trong lúc `Hide` | **Kiểm giả định về công cụ bằng thử nghiệm nhỏ** (chụp màn hình theo mốc) thay vì sửa regex mò |
| 17 | Gợi ý mờ "open it in mdview" trong ô nhập ở cảnh 1 và 2 | Tính năng gợi ý prompt của Claude Code, lộ ý cảnh 3 | Tắt bằng `CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=0` và `promptSuggestionEnabled: false` | Soi kỹ mọi chữ trên khung hình, kể cả chữ mờ |
| 18 | Tắt gợi ý xong thì tape không khởi động được | Tape chờ chữ `Try "…"`, chính là phần gợi ý vừa tắt | Chờ dòng banner `Claude Code v…` | **Sửa một chỗ có thể làm gãy chỗ khác dựa vào nó**; tìm điều kiện chờ không phụ thuộc tính năng tùy chọn |

## 4. Quay tay (WezTerm, OBS)

| # | Vấn đề | Nguyên nhân | Cách giải quyết | Bài học |
|---|---|---|---|---|
| 19 | Màn hình 3440×1440, toàn màn hình không ra 16:9 | Ultrawide 21:9 | WezTerm đặt cửa sổ theo pixel (`set_inner_size`), `dpi = 96` | Kiểm độ phân giải màn hình trước khi chọn công cụ |
| 20 | WezTerm mở xong là tắt | Bash không tương tác chưa đọc `.bashrc` nên không có PATH tới `claude`, `mdview`; `set -e` làm script thoát | Mở bằng `bash -ic`; `exit_behavior = 'Hold'` để giữ lỗi trên màn hình | **Cửa sổ đóng ngay = lỗi bị nuốt.** Giữ cửa sổ khi lỗi; thử đúng chuỗi lệnh bằng `wsl.exe -e` |
| 21 | Agent phàn nàn file plan đã có | `sandbox.sh up` chép sẵn file plan cho cảnh 4–6 | `plain` xóa luôn file plan | Hướng dẫn quay tay phải gồm cả bước dọn trạng thái giữa các take |
| 22 | `find` để tìm file trông "điên điên" | Không giống cách người dùng thật làm | Dùng editor terminal Fresh, theme `terminal` | **Thao tác "nỗi đau" phải giống thật**, không cường điệu |
| 23 | Take cảnh 2: viền đen hai bên, Fresh sai màu | Cửa sổ cao hơn 1080 (OBS chưa tích Client Area); WezTerm mở trước khi có alias `fresh` | Bản sửa hậu kỳ (crop + phóng 4%); hướng dẫn thêm Client Area và bước kiểm alias | Có bước **kiểm nhanh trước khi quay** (alias, biến môi trường, kích thước) |
| 24 | OBS không có hiệu ứng click | Window Capture chỉ ghi con trỏ | Thêm vòng click ember khi dựng, cho mọi cảnh | Hiệu ứng thống nhất làm ở hậu kỳ, không phụ thuộc công cụ quay |

## 5. Nối cảnh

| # | Vấn đề | Nguyên nhân | Cách giải quyết | Bài học |
|---|---|---|---|---|
| 25 | Cảnh 1 (VHS) và cảnh 2 (quay tay) là hai lần agent chạy khác nhau, chữ "nhảy" khi cắt | Agent không tất định, mỗi lần viết khác | Gộp cảnh 1+2 thành một take VHS liền mạch; click thêm khi dựng | **Hai cảnh cần nối liền thì quay chung một take** |
| 26 | Cảnh 3 → 4 lệch: trang mở ra không phải plan agent vừa viết | Cảnh 4–6 dùng file mẫu có sẵn | Quay cảnh 4–6 trên chính file agent viết ở cảnh 3; `CLAUDE.md` có mẫu plan cố định (có mermaid, có `max_retries`) để cảnh 4–5 luôn có thứ để zoom và sửa | Khi dùng agent thật, **cố định cấu trúc đầu ra bằng quy ước của project**, không cố định nội dung |

## 6. Gộp cảnh 1+2 vào VHS

| # | Vấn đề | Nguyên nhân | Cách giải quyết | Bài học |
|---|---|---|---|---|
| 27 | Fresh vừa mở đã có sẵn file plan và cây thư mục bung sẵn, mất ý "đi tìm" | Fresh tự khôi phục phiên trước (lần anh quay tay đã mở file đó) | `fresh.json` của demo: `restore_previous_session: false`, `hot_exit: false` | **Công cụ có "trí nhớ" (phiên, lịch sử, gợi ý) phải được reset** cho mỗi take |
| 28 | Ảnh chụp cuối của tape thử bị mất | VHS bỏ khung `Screenshot` nếu ngay sau là `Hide` | Chèn `Sleep` sau `Screenshot` | Ghi lại tật của công cụ ngay khi gặp, tránh mất thời gian lần sau |
| 29 | Cảnh 4–6 cần sơ đồ để zoom, số để sửa, link để click; plan của agent không chắc có | Agent không tất định | `CLAUDE.md` của project demo có **mẫu plan** (Goals, bảng, Mermaid, YAML `max_retries`, Steps); cảnh 5 tìm và đổi bất kỳ giá trị `max_retries` nào, sao lưu rồi khôi phục file | Script quay phải **tìm theo cấu trúc** (regex `max_retries:\s*\d+`), không cứng một giá trị |
| 30 | `CLAUDE.md` vừa chứa mẫu plan vừa chứa hướng dẫn mdview; chế độ `plain` xóa cả hai | Hai mục đích trong một file | Tách thành `CLAUDE.md` (mẫu, luôn có) và `CLAUDE.mdview.md` (chỉ ghép vào khi bật mdview) | Tách cấu hình theo mục đích để bật/tắt từng phần |
| 31 | Chạy trọn bộ hỏng ngay ở `warmup` | Khi sửa mục 18, chỉ sửa tape cảnh 1 và 3, quên `warmup.tape` cũng chờ chữ gợi ý | Sửa `warmup.tape`, rồi `grep` mọi tape tìm điều kiện chờ cũ | **Sửa một mẫu lỗi thì `grep` mọi chỗ dùng cùng mẫu**, đừng sửa từng file theo trí nhớ |
| 32 | Kết quả sau khi gộp: một lệnh `render.sh` quay trọn cảnh 1–6 trong khoảng 6 phút, chỗ nối 1→2 và 3→4 khớp (cùng take; cùng file plan) | — | Kiểm bằng khung hình ở từng chỗ nối: dòng Goals cảnh 3 giống trang cảnh 4; `max_retries` agent chọn (4) được đổi thành 3 | **Kiểm chỗ nối bằng khung hình hai bên chỗ cắt**, không chỉ xem từng cảnh riêng |

## 7. Dựng (Remotion)

| # | Vấn đề | Nguyên nhân | Cách giải quyết | Bài học |
|---|---|---|---|---|
| 33 | Câu trả lời của agent nhắc "house template", lộ hậu trường | Agent kể lại cách nó làm theo `CLAUDE.md` | Thêm vào mẫu: trả lời chỉ gồm đường dẫn, link và một vế ngắn; không nhắc mẫu. Quay lại | **Đọc kỹ mọi câu agent nói trên hình** trước khi dựng |
| 34 | Video Playwright không có con trỏ chuột, người xem không thấy click ở đâu | Playwright không vẽ con trỏ khi ghi | `record.mjs` ghi vị trí và thời điểm mỗi click vào `<cảnh>.clicks.json`; Remotion vẽ con trỏ và vòng ember theo đó | **Công cụ quay xuất luôn dữ liệu cho bước dựng** (click, mốc thời gian), đừng đo tay |
| 35 | Cần mốc cắt trong video terminal | Không có log thời gian từ VHS | `ffmpeg select='gt(scene,0.02)'` liệt kê thời điểm màn hình đổi; vẽ lưới tọa độ lên khung hình để đo vị trí chữ | Đo bằng công cụ, đối chiếu bằng mắt |
| 36 | Mỗi lần quay lại, tọa độ và mốc thời gian trong terminal đổi theo câu trả lời của agent | Agent không tất định | Tọa độ để thành hằng số trong `data.ts`, đo lại sau mỗi lần quay | Việc đo lại nên tự động hóa (việc còn lại) |
| 37 | Khung giữ hình "Not clickable" đã có dòng "Press Ctrl-C again" | Điểm dừng đặt sau lúc bấm Ctrl+C lần đầu | Lùi điểm dừng về trước Ctrl+C | **Kiểm khung đứng yên ở độ phân giải đầy đủ**, vì nó ở trên hình lâu |
| 38 | npm chặn script postinstall của esbuild | npm bản mới yêu cầu duyệt script cài đặt | Không cần duyệt: binary esbuild đi theo gói riêng, Remotion vẫn render được | Đọc cảnh báo, thử chạy trước khi nới quyền |
| 39 | Bản dựng đầu không có âm thanh | Brief ghi "không voice, nhạc có thể tắt" vì README phát không tiếng; em hiểu thành không làm âm thanh | Thêm nhạc CC0 ("Mellow Cafe Vibe"), tiếng click và whoosh tự tạo bằng ffmpeg (không vướng bản quyền) | **"Có thể tắt" khác "không có".** Mỗi nơi đăng có yêu cầu riêng: README tắt tiếng, YouTube/X cần âm thanh. Kiểm license nhạc trước khi dùng |
| 40 | Anh thấy nhạc "Mellow Cafe Vibe" chán | Em chọn bài chỉ vì có sẵn và có license, không vì hợp video | Tìm 7 bài license rõ (CC0, CC BY) trên Wikimedia Commons; tự chọn đoạn to rõ bằng `ebur128`, cân âm lượng bằng `loudnorm`; ghép từng bài vào cùng bản 30s để nghe so sánh. Anh chọn Nova Serenade (CC0) | **Gu nghe để người làm chọn, bằng bản nghe thử cạnh nhau**; em không nghe được nên không tự quyết. Cân âm lượng trước khi so sánh |
| 41 | Lúc commit bộ công cụ, `.gitignore` bỏ qua cả `public/audio/`: clone mới không render được | Ignore cả thư mục cho tiện, không nghĩ tới người clone | Commit `click.wav`, `whoosh.wav` (tự tạo, nhỏ); chỉ ignore `*.ogg`, README ghi link tải nhạc CC0 và lệnh tạo bản 30s không tiếng | **Trước khi commit, hỏi: clone mới có chạy được không?** File nào bị ignore thì README phải nói lấy ở đâu |

## Bài học chung

Đã đúc kết thành nguyên tắc và danh sách kiểm ở [bai-hoc-lam-video.md](https://github.com/vantt/vlog/blob/main/docs/video-ideas/bai-hoc-lam-video.md) (repo vlog, private).
