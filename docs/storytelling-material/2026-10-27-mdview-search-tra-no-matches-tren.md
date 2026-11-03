---
id: 2026-10-27-mdview-search-tra-no-matches-tren
date: 2026-10-27
repo: mdview
status: raw
signals: [nguyên nhân bất ngờ]
source: "chat Claude, claude:f2200b35"
sessions: ["claude:f2200b35"]
captured_by: harvest
hit_by: [harvest]
lines: ["claude:f2200b35#L203", "claude:f2200b35#L211", "claude:f2200b35#L213", "claude:f2200b35#L218", "claude:f2200b35#L219", "claude:f2200b35#L250", "claude:f2200b35#L252"]
hook: "Search báo 'No matches' dù daemon và index vẫn chạy bình thường"
tension: tuong_ma_hoa_ra
sides: ["Tưởng search hỏng vì không ra kết quả", "Chỉ thiếu cờ --project, và README đã ghi đúng từ đầu"]
strength: 2
stakes: 1
hits: 1
runs: 1
links: []
threads: []
privacy: checked
---

# mdview search trả 'No matches' trên project mới vì thiếu --project

**Bối cảnh:** Trên HOME sạch, chạy search với từ 'Arch' trên project mới thì ra 'No matches'. Nghi ngờ đầu tiên là hai daemon chạy song song.
**Chuyện gì xảy ra:** Chạy search từ thư mục khác lại ra kết quả của project khác, nên daemon và index vẫn hoạt động. Đọc help mới thấy phải truyền --project ID thì mới ra kết quả trên project mới. README đã ghi kèm --project, còn code chưa sửa.
**Bài học:** Khi kết quả rỗng, thử đúng lệnh có phạm vi trước khi nghi môi trường. Đoán nguyên nhân thường tốn thời gian hơn đọc help.
**Artefact:** Hai dòng terminal: lệnh không có --project và có --project, cho thấy kết quả khác nhau.
