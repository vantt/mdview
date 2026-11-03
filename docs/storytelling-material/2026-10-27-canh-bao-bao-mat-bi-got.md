---
id: 2026-10-27-canh-bao-bao-mat-bi-got
date: 2026-10-27
repo: mdview
status: raw
signals: [đảo quyết định có lý do, người duyệt thứ máy soạn, ràng buộc định hình thiết kế]
source: "chat Claude, claude:f2200b35"
sessions: ["claude:f2200b35"]
captured_by: harvest
hit_by: [harvest]
lines: ["claude:f2200b35#L164", "claude:f2200b35#L176", "claude:f2200b35#L637", "claude:f2200b35#L669", "claude:f2200b35#L768", "claude:f2200b35#L773"]
hook: "Khung cảnh báo bảo mật bị gọt thành một câu nhắc nhẹ"
tension: khac
sides: ["Rủi ro bảo mật mặc định được báo rõ, có khung [!WARNING]", "Người duyệt gỡ khung, chỉ còn câu nhắc nhẹ để tiện dùng ngay"]
strength: 2
stakes: 2
hits: 1
runs: 1
links: []
threads: []
privacy: checked
---

# Cảnh báo bảo mật bị gọt thành câu nhắc nhẹ

**Bối cảnh:** Mặc định mdview nghe trên 0.0.0.0 với token 'demo' công khai, và có nút Edit ghi thẳng xuống đĩa. Trong mạng LAN, ai biết token là đọc và sửa được docs.
**Chuyện gì xảy ra:** Agent phát hiện khi đọc config mặc định, báo rủi ro và đề xuất khung cảnh báo. Người duyệt quyết giữ 'demo' để tiện dùng ngay, không đổi code. Agent viết khung [!WARNING] vào README. Sau đó người duyệt đề nghị bỏ khung, chỉ nhắc nhẹ rằng mặc định chạy được ngay và ai muốn an toàn thì tự đặt web_secret.
**Bài học:** Mặc định tiện cho người dùng có thể đồng thời là mặc định mở cả LAN. Ghi rủi ro vào tài liệu không thay được việc cân nhắc mặc định an toàn hơn. Đó là quyết định sản phẩm, máy không nên chốt.
**Artefact:** Hai đoạn README trước và sau khi đổi (từ khung cảnh báo sang câu nhắc nhẹ). Không đưa token hay IP vào ảnh.
