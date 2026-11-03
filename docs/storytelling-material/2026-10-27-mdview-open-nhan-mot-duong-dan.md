---
id: 2026-10-27-mdview-open-nhan-mot-duong-dan
date: 2026-10-27
repo: mdview
status: raw
signals: [khác]
source: "chat Claude, claude:f2200b35"
sessions: ["claude:f2200b35"]
captured_by: harvest
hit_by: [hotspot, harvest]
lines: ["claude:f2200b35#L598", "claude:f2200b35#L604", "claude:f2200b35#L611", "claude:f2200b35#L616", "claude:f2200b35#L618"]
hook: "mdview open chỉ nhận một đường dẫn, nên phải mở từng file một"
tension: none
sides: []
strength: 0
stakes: 0
hits: 1
runs: 1
links: []
threads: []
privacy: checked
---

# mdview open nhận một đường dẫn, không nhận nhiều

**Bối cảnh:** Agent muốn mở hai file cùng lúc bằng một lệnh mdview open --json.
**Chuyện gì xảy ra:** Lệnh báo 'unexpected argument' vì open chỉ nhận một đường dẫn. Agent đổi sang vòng lặp, mở từng file và nhận JSON riêng cho mỗi file.
**Bài học:** Đọc usage trước khi đoán cú pháp. Với CLI dành cho agent, lỗi cú pháp xảy ra thường xuyên.
**Artefact:** Hai dòng JSON trả về cho mỗi file.
