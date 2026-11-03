---
id: 2026-10-27-doctor-fix-ghi-file-vao-thu
date: 2026-10-27
repo: mdview
status: raw
signals: [nguyên nhân bất ngờ]
source: "chat Claude, claude:f2200b35"
sessions: ["claude:f2200b35"]
captured_by: harvest
hit_by: [harvest]
lines: ["claude:f2200b35#L129", "claude:f2200b35#L137", "claude:f2200b35#L146"]
hook: "doctor --fix ghi AGENTS.md và CLAUDE.md vào bất cứ thư mục nào bạn đang đứng"
tension: giup_ma_hai
sides: ["Lệnh sinh ra để tự sửa cấu hình cho bạn", "Chạy sai thư mục thì nó rải file vào chỗ không thuộc project"]
strength: 1
stakes: 2
hits: 1
runs: 1
links: []
threads: []
privacy: checked
---

# doctor --fix ghi file vào thư mục đang đứng

**Bối cảnh:** Chạy doctor --fix từ HOME sạch, agent thấy AGENTS.md xuất hiện ở thư mục hiện tại chứ không ở gốc project.
**Chuyện gì xảy ra:** Lệnh tạo AGENTS.md và CLAUDE.md ở thư mục đang đứng. README không nói phải chạy ở gốc project. Đã ghi rõ điều đó trong README.
**Bài học:** Lệnh cài đặt cho agent nên nói rõ phạm vi ghi file, hoặc hỏi thư mục trước khi ghi.
**Artefact:** Cây thư mục trước và sau khi chạy lệnh.
