---
id: 2026-10-27-nguoi-doc-hoi-agent-tra-link
date: 2026-10-27
repo: mdview
status: raw
signals: [người duyệt thứ máy soạn]
source: "chat Claude, claude:f2200b35"
sessions: ["claude:f2200b35"]
captured_by: harvest
hit_by: [harvest]
lines: ["claude:f2200b35#L1279", "claude:f2200b35#L1285", "claude:f2200b35#L1290", "claude:f2200b35#L1307", "claude:f2200b35#L1334"]
hook: "README chưa nói khi nào agent trả link, nên tôi thêm hội thoại mẫu"
tension: none
sides: []
strength: 0
stakes: 2
hits: 1
runs: 1
links: []
threads: []
privacy: checked
---

# Người đọc hỏi: agent trả link khi nào? README thiếu ví dụ hội thoại

**Bối cảnh:** Người duyệt đọc README và hỏi: chưa có phần nào nói rõ khi nào agent sẽ trả ra link.
**Chuyện gì xảy ra:** Agent đọc instruction block và skill template để xác định đúng hành vi, rồi thêm một đoạn hội thoại mẫu kết thúc bằng link markdown. Link ví dụ trỏ tới một ảnh trong repo vì không có daemon thật để bấm, và có ghi chú kèm theo.
**Bài học:** Một câu hỏi của người đọc mới chỉ ra phần thiếu lớn hơn lỗi kỹ thuật: tài liệu cần cho thấy một cuộc trò chuyện thật.
**Artefact:** Đoạn hội thoại mẫu trong README, đã trích.
