---
id: 2026-10-27-doc-code-de-xac-minh-tung
date: 2026-10-27
repo: mdview
status: raw
signals: []
source: "chat Claude, claude:f2200b35"
sessions: ["claude:f2200b35"]
captured_by: harvest
hit_by: [outsider]
lines: ["claude:f2200b35#L164", "claude:f2200b35#L165", "claude:f2200b35#L176", "claude:f2200b35#L184", "claude:f2200b35#L691", "claude:f2200b35#L694"]
hook: "README viết từ trí nhớ: 3 câu tôi đọc code kiểm lại, 2 câu sai"
tension: tuong_ma_hoa_ra
sides: ["Tài liệu tưởng là đúng vì tác giả tự viết", "Đối chiếu với code thì lệch khỏi hành vi thật"]
strength: 2
stakes: 2
hits: 0
runs: 1
links: []
threads: []
privacy: checked
---

# Đọc code để xác minh từng câu trong README trước khi giữ hay sửa (token 'auto-generated', `0.0.0.0` là tuỳ chọn, hostname trong link) rồi thử lại bằng cách đặt `hostname = "localhost"` trên HOME sạch xem link in ra đổi thế nào.

**Chuyện gì xảy ra:** Đọc code để xác minh từng câu trong README trước khi giữ hay sửa (token 'auto-generated', `0.0.0.0` là tuỳ chọn, hostname trong link) rồi thử lại bằng cách đặt `hostname = "localhost"` trên HOME sạch xem link in ra đổi thế nào.
**Góc người ngoài:** cách làm phổ biến là README thường được viết từ trí nhớ của tác giả và để nguyên; tài liệu lệch khỏi hành vi thật mà không ai phát hiện cho đến khi có issue; ở đây khác ở Mỗi khẳng định trong README được coi như một giả thuyết và kiểm bằng grep code cộng chạy thật. Kết quả là tìm ra hai câu sai sự thật. Người đọc muốn áp dụng cho tài liệu của chính mình.
