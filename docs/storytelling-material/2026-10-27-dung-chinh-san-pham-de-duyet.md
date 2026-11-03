---
id: 2026-10-27-dung-chinh-san-pham-de-duyet
date: 2026-10-27
repo: mdview
status: raw
signals: []
source: "chat Claude, claude:f2200b35"
sessions: ["claude:f2200b35"]
captured_by: harvest
hit_by: [outsider]
lines: ["claude:f2200b35#L598", "claude:f2200b35#L616", "claude:f2200b35#L618", "claude:f2200b35#L699", "claude:f2200b35#L1138", "claude:f2200b35#L1156", "claude:f2200b35#L1158"]
hook: "Dùng chính sản phẩm để duyệt việc: gửi link thay vì dán diff"
tension: none
sides: []
strength: 0
stakes: 1
hits: 0
runs: 1
links: []
threads: []
privacy: checked
---

# Dùng chính sản phẩm để duyệt công việc: sau mỗi lần sửa, chạy `mdview open --json` rồi gửi link để người duyệt đọc báo cáo và README đã render trong trình duyệt, thay vì dán nội dung vào chat; còn dùng Playwright mở lại trang để xác nhận cả 7 ảnh tải đủ.

**Chuyện gì xảy ra:** Dùng chính sản phẩm để duyệt công việc: sau mỗi lần sửa, chạy `mdview open --json` rồi gửi link để người duyệt đọc báo cáo và README đã render trong trình duyệt, thay vì dán nội dung vào chat; còn dùng Playwright mở lại trang để xác nhận cả 7 ảnh tải đủ.
**Góc người ngoài:** cách làm phổ biến là Dán đoạn diff hoặc nội dung file vào khung chat, hoặc bảo người duyệt tự mở file. Ít ai kiểm tra ảnh trong README thật sự hiển thị; ở đây khác ở Người duyệt nhận link click được, đọc bảng và ảnh đã render, còn agent có bằng chứng ảnh tải đúng kích thước. Người đọc muốn có vòng 'agent trả link, người duyệt bấm xem' này cho tài liệu của mình.
