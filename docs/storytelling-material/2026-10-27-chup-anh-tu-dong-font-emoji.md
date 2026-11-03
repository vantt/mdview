---
id: 2026-10-27-chup-anh-tu-dong-font-emoji
date: 2026-10-27
repo: mdview
status: raw
signals: [nguyên nhân bất ngờ, artefact nhìn được]
source: "chat Claude, claude:f2200b35"
sessions: ["claude:f2200b35"]
captured_by: harvest
hit_by: [hotspot, harvest, outsider]
lines: ["claude:f2200b35#L1001", "claude:f2200b35#L1005", "claude:f2200b35#L1011", "claude:f2200b35#L1016", "claude:f2200b35#L1023", "claude:f2200b35#L1026"]
hook: "Icon thư mục hiện thành ô vuông vì WSL không có font emoji"
tension: nho_ma_to
sides: ["Một ký tự emoji 📁 trong CSS", "Trình duyệt vẽ ra ô vuông, phải mượn font của Windows để chụp ảnh"]
strength: 1
stakes: 1
hits: 1
runs: 1
links: []
threads: []
privacy: checked
---

# Chụp ảnh tự động: font emoji của WSL làm icon thư mục thành ô vuông

**Bối cảnh:** Chụp drawer trên điện thoại bằng Playwright và Chromium headless trong WSL. Icon thư mục hiện thành ô vuông rỗng.
**Chuyện gì xảy ra:** Icon của mdview là ký tự emoji 📁 trong CSS. WSL không có font emoji nên trình duyệt không vẽ được. Agent kiểm tra và thấy không có font emoji nào trong hệ thống. Mượn font emoji của Windows qua fontconfig riêng cho lần chụp; mdview không đổi gì.
**Bài học:** Ảnh chụp trên máy dev có thể lệch với máy người dùng vì font. Ghi rõ môi trường chụp, và đừng sửa app để bù cho môi trường chụp.
**Artefact:** Hai ảnh drawer, trước và sau khi có font emoji. Che tên người và đường dẫn nếu có lộ trong ảnh.
**Góc người ngoài:** cách làm phổ biến là Thấy ô vuông thì hoặc ghi nhận là bug của ứng dụng, hoặc bỏ qua, hoặc cài cả gói font vào hệ thống; ở đây khác ở Phân biệt lỗi sản phẩm với lỗi môi trường chụp, rồi sửa bằng cách cô lập (fontconfig riêng chỉ cho tiến trình chụp). Người đọc dùng WSL/Docker headless chắc chắn sẽ gặp ô vuông tương tự.
