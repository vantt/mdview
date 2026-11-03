---
id: 2026-10-27-script-chup-anh-hong-vi-selector
date: 2026-10-27
repo: mdview
status: raw
signals: [nguyên nhân bất ngờ]
source: "chat Claude, claude:f2200b35"
sessions: ["claude:f2200b35"]
captured_by: harvest
hit_by: [harvest, outsider]
lines: ["claude:f2200b35#L832", "claude:f2200b35#L835", "claude:f2200b35#L848", "claude:f2200b35#L850", "claude:f2200b35#L853", "claude:f2200b35#L855", "claude:f2200b35#L858", "claude:f2200b35#L860", "claude:f2200b35#L870", "claude:f2200b35#L872"]
hook: "Script chụp ảnh trả về [] vì selector tôi đoán theo tên class"
tension: tuong_ma_hoa_ra
sides: ["Đoán class là .search-result", "DOM thật là a.fg-card nên không khớp phần tử nào"]
strength: 1
stakes: 1
hits: 1
runs: 1
links: []
threads: []
privacy: checked
---

# Script chụp ảnh hỏng vì selector đoán theo tên class

**Bối cảnh:** Script Playwright chụp trang search và bảng bị TimeoutError. Selector viết theo tên class đoán không khớp với DOM thật.
**Chuyện gì xảy ra:** Chạy lại để đọc lỗi thì thấy selector không khớp phần tử nào, kết quả là []. Đọc DOM thật thì thấy thẻ kết quả là a.fg-card, không phải .search-result như giả định. Sau khi sửa selector, ảnh search chụp được ở kích thước 1228x593.
**Bài học:** Selector đoán từ tên class dễ sai. Đọc DOM thật, hoặc in số phần tử khớp, trước khi viết logic chụp.
**Artefact:** Đoạn terminal: số phần tử khớp trước và sau khi sửa selector.
**Góc người ngoài:** cách làm phổ biến là Chụp màn hình thủ công bằng công cụ của OS hoặc trình duyệt, lưu thẳng vào repo. Ảnh nhanh cũ, lệch với bản hiện tại, kích thước và nền không đồng nhất; ở đây khác ở Ảnh sinh lại được bằng một lệnh mỗi khi UI đổi, nội dung là dữ liệu mẫu sạch (không lộ path hay project thật), độ phân giải gấp đôi. Người đọc muốn có quy trình để ảnh README không bị lỗi thời.
