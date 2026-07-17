# mdview — Hướng dẫn quay video giới thiệu

Mục tiêu: một clip **15–25 giây**, loop được, cho người xem **cảm nhận nỗi đau
link gãy** rồi **thấy mdview gỡ nó** — kèm live-reload / search / mermaid.

---

## 1. Góc kể chuyện (cái làm người ta quan tâm)

**Nỗi đau:** AI agent sinh ra docs nhiều tầng thư mục, chằng chịt link `../`.
Mở một file bằng previewer **1-file** (ví dụ `/ck:preview <file>`) — đọc *file đó*
thì được, nhưng **bấm link sang folder khác là 404**, không đi xuyên cả bộ docs.

**Lời giải:** mdview index **toàn bộ project** và viết lại mọi internal link vào
namespace URL của nó → click xuyên thư mục **chạy mượt**, cộng live-reload,
full-text search, sơ đồ Mermaid zoom được.

> Cả video chỉ có **một việc**: cho thấy link gãy → rồi cảm giác nhẹ nhõm khi nó chạy.

---

## 2. Tool nhanh nhất (Linux)

### Quay màn hình
| Tool | Khi nào dùng | Ghi chú |
|---|---|---|
| **GNOME built-in** (`Ctrl+Alt+Shift+R`) | Nhanh nhất, có sẵn, chạy cả Wayland | Ra `.webm`, không cần cài gì |
| **Peek** | Quay vùng nhỏ → xuất GIF/MP4 thẳng | `apt install peek` (X11 tốt nhất) |
| **OBS Studio** | Cần chất lượng/điều khiển cao, clip dài | `apt install obs-studio`, xuất MP4/WebM |
| **wf-recorder** (Wayland) / **SimpleScreenRecorder** (X11) | CLI/nhẹ | tuỳ session |

### Cắt + đổi định dạng
- **ffmpeg** — cắt, crop, scale, và xuất GIF (có sẵn hầu hết máy).
- **gifski** — GIF chất lượng cao nhất, nhẹ (`cargo install gifski` hoặc `apt`).

### Chọn định dạng (quan trọng cho README)
- **README hero PHẢI là GIF** (`docs/assets/hero-demo.gif`): GitHub render GIF
  từ path repo là hiện + auto-loop ngay. **MP4 đặt theo path repo KHÔNG tự phát**
  trong README.
- Cách làm tối ưu: **quay ra MP4/WebM nét** → giữ MP4 cho social (X/Twitter,
  Product Hunt, LinkedIn — nơi MP4 phát tốt & nhẹ) → **convert MP4 → GIF** cho README.

---

## 3. Chuẩn bị (2 phút)

- **Repo demo sạch, nhiều tầng + có cross-link + 1 file Mermaid.** Có thể dùng
  luôn repo `mdview` (thư mục `docs/` có link chéo, specs, `docs/mermaid-demo.md`),
  hoặc một sample nhỏ. Đừng để lộ path cá nhân trong breadcrumb/sidebar.
- **Cửa sổ rộng ~1280px**, font terminal to (14–16pt), zoom trình duyệt ~110–125%.
- **Theme nhất quán** (light hoặc dark, đừng đổi giữa chừng).
- Mở sẵn: 1 terminal + 1 tab trình duyệt trống. Gõ trước lệnh cho khỏi lỗi.
- Con trỏ chuột: di chậm, dứt khoát; mỗi hành động dừng ~0.5s cho người xem kịp thấy.

---

## 4. Kịch bản shot-by-shot

### Phương án A — "Đau trước, nhẹ sau" (ĐỀ XUẤT, đúng thông điệp của bạn)

| Time | Trên màn hình | Hành động | Caption (tuỳ chọn) |
|---|---|---|---|
| 0–4s | Previewer 1-file (vd `/ck:preview docs/architecture.md`) | Bấm một link `../api/README.md` → **404 / trang trắng** | "Mở 1 file thì được…" |
| 4–6s | Vẫn màn 404 | Zoom nhẹ vào chữ 404 | "…nhưng link chéo thì gãy." |
| 6–9s | Terminal | Gõ `mdview open docs/architecture.md` → Enter → in ra URL | "Một lệnh." |
| 9–13s | Trình duyệt mdview mở | Doc render, sidebar Chapters bên trái | "Cả project, có mục lục." |
| 13–17s | Cùng cái link `../api/README.md` | Bấm → **đi mượt, không 404** | "Link xuyên thư mục — không bao giờ 404." |
| 17–21s | Sửa file trên đĩa (nháy editor, `:w`) | Trang **tự reload** | "Sửa → tự cập nhật." |
| 21–24s | Lướt nhanh | Flash search + Mermaid zoom → dừng ở logo/tagline | "Search. Sơ đồ zoom được." |

### Phương án B — "Feature flow" (không cần cảnh 404)
Bỏ 2 beat đầu; bắt đầu từ `mdview open` → click xuyên link → live reload →
search → mermaid. Ngắn gọn ~15s, ít kịch tính hơn nhưng dễ quay.

### Cắt phụ (nice-to-have, quay riêng)
- **Agent/MCP:** trong Claude Code, agent viết doc → gọi `mdview_view_file` →
  trả URL → mở ra. Điểm khác biệt lớn nhất với dân dùng AI.
- **Mobile:** mở trên điện thoại → nút ☰ mở sidebar → pinch zoom Mermaid.

---

## 5. Xuất file

Giả sử clip quay ra là `raw.mp4` (hoặc `.webm`).

**Cắt gọn + scale (giữ MP4 cho social):**
```sh
# cắt từ giây 2 đến 24, rộng 1280
ffmpeg -i raw.mp4 -ss 2 -to 24 -vf "scale=1280:-2:flags=lanczos" -an hero-demo.mp4
```

**MP4 → GIF cho README (rộng 820, 15fps) — cách ffmpeg 2 bước (đáng tin):**
```sh
ffmpeg -i hero-demo.mp4 -vf "fps=15,scale=820:-1:flags=lanczos,palettegen" palette.png
ffmpeg -i hero-demo.mp4 -i palette.png \
  -vf "fps=15,scale=820:-1:flags=lanczos,paletteuse" docs/assets/hero-demo.gif
```

**Hoặc GIF đẹp/nhẹ hơn bằng gifski:**
```sh
ffmpeg -i hero-demo.mp4 -vf "fps=15,scale=820:-1:flags=lanczos" frame_%04d.png
gifski --fps 15 -o docs/assets/hero-demo.gif frame_*.png && rm frame_*.png
```

Mục tiêu: GIF **≤ 8 MB** (README load nhanh). Nếu to quá: giảm `fps=12`,
`scale=720`, hoặc rút ngắn clip.

---

## 6. Gắn vào README

1. Bỏ `docs/assets/hero-demo.gif` vào repo.
2. Bỏ comment khối `<!-- ▶ HERO DEMO ... -->` ở đầu `README.md` (mình đã comment
   sẵn để tránh ảnh vỡ) — hoặc ới mình bật lại hộ.
3. Commit + push (docs, không tag → không chạy release).

> Nếu muốn nhúng **MP4** thay vì GIF: kéo-thả file MP4 vào một GitHub issue/PR
> để lấy URL `user-attachments`, rồi dùng `<video src="...">` — nhưng cho README
> thì **GIF là chắc ăn nhất**.

---

## Checklist nhanh
- [ ] Repo demo sạch, có cross-link + Mermaid
- [ ] Cửa sổ 1280px, font to, theme nhất quán, ẩn path cá nhân
- [ ] Quay MP4/WebM (Phương án A)
- [ ] `hero-demo.mp4` (social) + `hero-demo.gif ≤8MB` (README)
- [ ] Bỏ vào `docs/assets/`, bật lại khối hero, commit/push
