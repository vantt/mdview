---
id: 2026-10-27-nguoi-la-cai-mdview-tren-may
date: 2026-10-27
repo: mdview
status: raw
signals: [nguyên nhân bất ngờ, artefact nhìn được]
source: "chat Claude, claude:f2200b35"
sessions: ["claude:f2200b35"]
captured_by: harvest
hit_by: [harvest, outsider]
lines: ["claude:f2200b35#L122", "claude:f2200b35#L164", "claude:f2200b35#L184", "claude:f2200b35#L186", "claude:f2200b35#L632"]
hook: "Người lạ cài xong mở link, gặp trang login trống và token chỉ ở log"
tension: tuong_ma_hoa_ra
sides: ["README bảo token tự sinh, 0.0.0.0 là tuỳ chọn", "Token thật là 'demo' chỉ in trong log daemon, và cả hai câu đều sai"]
strength: 3
stakes: 3
hits: 1
runs: 1
links: []
threads: []
privacy: checked
---

# Người lạ cài mdview trên máy sạch: login không có gợi ý, README nói sai

**Bối cảnh:** Chạy README trên HOME trống (env -i) để đi như người chưa từng dùng: curl | sh, rồi mdview doctor --fix, rồi bấm link mở trang.
**Chuyện gì xảy ra:** Ngay sau khi cài, doctor --fix báo 'mdview: not found' vì thư mục cài chưa có trong PATH. Link mở ra trang login trống, không có gợi ý nào về token. Token mặc định 'demo' chỉ in ra log của daemon nên người dùng không thấy. README còn viết token là 'auto-generated' và 0.0.0.0 là tuỳ chọn, cả hai đều sai.
**Bài học:** Thử README trên một HOME sạch trước khi launch. Chỗ người lạ vấp đầu tiên thường không nằm trong đầu tác giả.
**Artefact:** Bảng từng bước thử trên HOME sạch (lệnh và output rút gọn). Che đường dẫn home và địa chỉ IP.
**Góc người ngoài:** cách làm phổ biến là Tác giả đọc lại README, chạy lệnh trên máy đang dùng (đã có sẵn PATH, config, token), hoặc nhờ bạn bè thử. Ít ai dựng môi trường trống để mô phỏng người cài lần đầu; ở đây khác ở Môi trường sạch lộ những lỗi mà máy dev che mất: `~/.local/bin` chưa có trong PATH, token mặc định không hiển thị ở đâu, lệnh tạo file ở thư mục đang đứng. Người đọc muốn có cách tái hiện 'ngày đầu của người dùng' chỉ với một dòng `env -i HOME=...`, không cần VM hay Docker.
