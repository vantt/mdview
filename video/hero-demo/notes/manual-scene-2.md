# A1. Hướng dẫn quay cảnh 2: đi tìm file

Ngày: 2026-10-27. Thuộc [kịch bản A1](script.md). Bộ công cụ ở `~/projects/mdview/video/hero-demo/`.

> **Cập nhật 2026-10-27:** cảnh 2 giờ quay tự động cùng cảnh 1 trong một take VHS (`render.sh`, bước `scene12`), để chỗ nối không lộ. Hướng dẫn dưới đây chỉ còn là cách dự phòng, nếu anh muốn một take có chuột thật.

Cảnh 1, 3, 4, 5, 6 đã quay tự động. Cảnh 2 dùng editor terminal **Fresh** để tìm và mở file: vẫn phải tự đi tìm, nhưng là cách người dùng thật hay làm. Cảnh 2 là cảnh duy nhất anh quay tay, vì nó là thao tác thật của người dùng.

## 1. Cảnh 2 cần gì

| Mục | Nội dung |
|---|---|
| Vị trí | 0:05–0:13 trong bản 45 giây, ngay sau cảnh 1 |
| Ý | Cùng câu trả lời của agent ở cảnh 1. Ctrl+click vào đường dẫn: không có gì xảy ra. Phải tự đi tìm file và đọc markdown thô. |
| Quay thô | 25–40 giây, một take liền |
| Khi dựng | Tua nhanh 3×, còn khoảng 8–12 giây. Thêm đồng hồ đếm giây ở góc, chữ **Not clickable.** rồi **Connect. Dig. Squint.** |
| Phong cách | Phải giống hệt cảnh 1 (quay bằng VHS) để chỗ nối không lộ: nền cocoa `#140e0b`, chữ `#f4ede7`, con trỏ ember, Geist Mono 26 px, padding 64 px, khung 1920×1080 |

## 2. Terminal: WezTerm, cấu hình sẵn

Dùng **WezTerm trên Windows** (máy anh đã có, bản 20240203), mở bằng script trong bộ công cụ. Lý do:

- Đặt được **kích thước cửa sổ theo pixel**: cửa sổ mở ra đúng 1920×1080 dù màn hình là 3440×1440.
- Ép `dpi = 96`, nên chữ đúng 26 px như VHS, không bị thang chia của Windows làm lệch.
- Đúng bảng màu, font, padding và độ cao dòng của `tapes/settings.tape`. Font nạp từ thư mục bên cạnh config, không cần cài gì lên Windows.
- Không có tab, thanh tiêu đề hay thanh cuộn. OBS bắt cửa sổ ra hình sạch.

Không dùng Windows Terminal, vì nó không đặt được kích thước theo pixel và khó khớp màu, font chính xác.

File cấu hình: `video/hero-demo/wezterm.lua`. Script mở: `video/hero-demo/wezterm.sh`.

## 3. Chuẩn bị (làm một lần)

Chạy trong WSL, từ repo mdview:

1. Bật máy demo, rồi giấu mdview khỏi agent và **xóa file plan có sẵn** (giống cảnh 1; nếu còn file, agent sẽ hỏi có ghi đè không):

   ```bash
   video/hero-demo/sandbox.sh up
   ```

   ```bash
   video/hero-demo/sandbox.sh plain
   ```

2. Mở terminal quay hình:

   ```bash
   video/hero-demo/wezterm.sh
   ```

   Một cửa sổ WezTerm 1920×1080 hiện ra, prompt `dev@build-01:/tmp/mdview-hero-demo/acme-payments$`.

   Mỗi lần bộ công cụ thay đổi, **đóng cửa sổ cũ và mở lại** bằng lệnh này. Kiểm nhanh: gõ `alias fresh`, phải thấy `fresh --config …/fresh.json`. Nếu không, Fresh sẽ dùng theme riêng của anh và lệch màu.

3. **OBS** (đã cài):
   - *Settings → Video:* Base (Canvas) **1920×1080**, Output (Scaled) **1920×1080**, FPS **30**.
   - *Settings → Output → Recording:* định dạng **mkv** (an toàn khi OBS treo; sau đó *File → Remux Recordings* ra mp4). Encoder phần cứng (NVENC, QuickSync hoặc AMF) nếu có, không thì x264. Chất lượng: CQP/CRF **18**. Không cần âm thanh: tắt Desktop Audio và Mic trong *Audio Mixer*.
   - *Settings → Hotkeys:* đặt phím **Start Recording** và **Stop Recording** (ví dụ Ctrl+Shift+F9 / F10), để không phải bấm vào cửa sổ OBS khi đang quay.
   - Thêm source **Window Capture** → chọn `[wezterm-gui.exe]`. *Capture Method:* Windows 10 (1903 and up). Bật **Capture Cursor** (cần thấy chuột Ctrl+click).
   - Trong *Properties* của source, tích **Client Area** (chỉ lấy phần trong cửa sổ, bỏ viền và thanh tiêu đề).
   - Chuột phải source → *Transform → Edit Transform*: kiểm Size đúng **1920×1080**, Position 0,0. Nếu thấy viền đen hai bên, nghĩa là cửa sổ cao hơn 1080: trong *Edit Transform* đặt **Crop Top** bằng phần thừa (take đầu thừa 44px), hoặc báo em chỉnh WezTerm.

## 4. Quay

### Phần chạy trước (không ghi)

Trong cửa sổ WezTerm:

1. Chạy agent không có MCP, như cảnh 1:

   ```bash
   claude --strict-mcp-config
   ```

2. Gõ đúng prompt của cảnh 1:

   ```text
   Write an implementation plan for retrying failed payments to docs/plans/payment-retry.md. Reply in one line.
   ```

   Nếu quay lại take khác, chạy lại `video/hero-demo/sandbox.sh plain` (ở terminal WSL khác) trước khi gõ prompt, để xóa file plan agent vừa viết.

3. Đợi agent trả lời xong (dòng `● Plan written to docs/plans/payment-retry.md…`). Màn hình lúc này giống khung cuối của cảnh 1.

### Take (ghi)

Thao tác chậm, đều tay. Trước khi bấm, đặt chuột ở giữa màn hình.

| # | Thao tác | Ghi chú |
|---|---|---|
| 1 | Bấm phím Start Recording. Đứng yên 1 giây. | Khung đầu khớp với khung cuối cảnh 1 |
| 2 | Di chuột chậm tới `docs/plans/payment-retry.md` trong câu trả lời. Giữ **Ctrl**, click. Đợi 1,5 giây. | Không có gì mở ra. Đây là ý chính của cảnh |
| 3 | Click thêm một lần nữa, hơi "sốt ruột". Đợi 1 giây. | Vẫn không có gì |
| 4 | Thoát agent: **Ctrl+C** hai lần. | Về prompt `dev@build-01` |
| 5 | Gõ `fresh`, Enter. | Mở editor Fresh trong terminal, theme `terminal` nên cùng màu cocoa/ember |
| 6 | **Ctrl+B** mở cây file. Dùng **↓** và **Enter** (hoặc chuột) mở `docs` → `plans` → `payment-retry.md`. Đi lạc vào `adr/` hay `api/` một nhịp rồi quay ra cũng được. | Đang đi tìm, chậm và mất công là đúng ý |
| 7 | Trong file, bấm **↓** từng nhịp (khoảng 2 dòng mỗi giây), qua bảng (dấu `\|` lệch nhau) và khối ` ```mermaid ` dạng code. Dừng 2 giây ở khối mermaid. | Phần "squint": markdown thô, không render |
| 8 | Đứng yên 1 giây. Bấm phím Stop Recording. Thoát Fresh sau khi đã dừng ghi. | |

Quay 3–4 take, giữ take mượt nhất. Không cần hoàn hảo: phần này bị tua nhanh 3×.

### Phần IDE (tùy chọn)

Nếu muốn có ý "Connect" (mở IDE remote) đúng như chữ trên màn hình:

1. Mở VS Code, *Remote Explorer → WSL: Ubuntu*, mở thư mục `/tmp/mdview-hero-demo/acme-payments`. Quay lúc thanh trạng thái hiện "Connecting…".
2. Trong cây file bấm `docs` → `plans` → `payment-retry.md`. Mở bằng editor thường, **không** bấm Preview.
3. Cuộn qua bảng và khối mermaid thô.

Để VS Code cùng tông màu, tạo một **Profile** riêng (*File → Preferences → Profiles → New Profile*, đặt tên `hero-demo`) rồi thêm vào settings của profile đó (không ảnh hưởng profile chính của anh):

```json
{
  "workbench.colorTheme": "Default Dark Modern",
  "workbench.colorCustomizations": {
    "editor.background": "#140e0b",
    "sideBar.background": "#1a120e",
    "activityBar.background": "#1a120e",
    "titleBar.activeBackground": "#1a120e",
    "statusBar.background": "#1a120e",
    "editor.foreground": "#f4ede7",
    "focusBorder": "#e9590c",
    "statusBarItem.remoteBackground": "#e9590c"
  },
  "editor.fontFamily": "Geist Mono, monospace",
  "editor.fontSize": 20,
  "editor.minimap.enabled": false,
  "workbench.startupEditor": "none"
}
```

Font Geist Mono phải cài trên Windows thì VS Code mới dùng được. Nếu chưa cài, VS Code sẽ dùng font mono mặc định, vẫn chấp nhận được cho cảnh tua nhanh. Cửa sổ VS Code không ép được đúng 1920×1080: trong OBS dùng *Fit to screen*, hoặc crop về 16:9.

## 5. Sau khi quay

1. *File → Remux Recordings* sang mp4. Đặt tên `scene2-hunt.mp4`, chép vào `~/projects/mdview/video/hero-demo/out/`.
2. Trả máy demo về trạng thái chuẩn cho các cảnh khác:

   ```bash
   video/hero-demo/sandbox.sh up
   ```

3. Khi dựng: tua 3×, cắt đoạn chết, thêm đồng hồ góc và hai dòng chữ của cảnh 2 theo [shot list](script.md).

## 6. Chưa kiểm

- Em mới kiểm config WezTerm bằng `wezterm ls-fonts` (đọc đúng font). Chưa mở cửa sổ thật trên màn hình anh để đo đúng 1920×1080; lần đầu chạy anh xem giúp, lệch thì báo em.
- Ctrl+click vào đường dẫn trong WezTerm: theo mặc định WezTerm chỉ biến URL thành link, đường dẫn tương đối không phải link, nên sẽ không có gì xảy ra. Đúng ý cảnh, nhưng chưa thử trên máy anh.
