---
id: 2026-10-27-kiem-tra-an-toan-chan-lenh
date: 2026-10-27
repo: mdview
status: raw
signals: [khác]
source: "chat Claude, claude:f2200b35"
sessions: ["claude:f2200b35"]
captured_by: harvest
hit_by: [hotspot, harvest]
lines: ["claude:f2200b35#L1060", "claude:f2200b35#L1075"]
hook: "Claude Code chặn lệnh xoá file tạm vì một biến shell có thể rỗng"
tension: cam_ma_van_lot
sides: ["Biến rỗng có thể xoá cả thư mục làm việc", "Lệnh bị chặn, bước sau chạy lại bằng script không dùng lệnh xoá"]
strength: 1
stakes: 0
hits: 1
runs: 1
links: []
threads: []
privacy: checked
---

# Kiểm tra an toàn chặn lệnh xoá khi dọn file tạm

**Bối cảnh:** Đang đổi thứ tự ảnh trong file preview tạm thì agent chạy một lệnh xoá trước khi ghi lại.
**Chuyện gì xảy ra:** Kiểm tra an toàn của Claude Code chặn lệnh vì biến shell có thể rỗng, dẫn tới xoá cả thư mục làm việc. Lệnh không chạy và không có người duyệt nào cho phép. Bước sau được chạy lại bằng một script khác, không dùng lệnh xoá.
**Bài học:** Với lệnh xoá dùng biến shell, kiểm tra biến có rỗng không trước khi chạy. Khi công cụ chặn, đọc lý do rồi đổi cách làm, đừng tìm cách vượt qua.
**Artefact:** Thông báo chặn của công cụ, đã che đường dẫn.
