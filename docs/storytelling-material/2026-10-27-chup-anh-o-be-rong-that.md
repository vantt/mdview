---
id: 2026-10-27-chup-anh-o-be-rong-that
date: 2026-10-27
repo: mdview
status: raw
signals: [artefact nhìn được]
source: "chat Claude, claude:f2200b35"
sessions: ["claude:f2200b35"]
captured_by: harvest
hit_by: [harvest, outsider]
lines: ["claude:f2200b35#L1107", "claude:f2200b35#L1270"]
hook: "Chụp ảnh ở bề rộng thật, lòi ra 4 lỗi UI chưa ai thấy"
tension: tuong_ma_hoa_ra
sides: ["Tưởng giao diện đã ổn", "Chụp ở bề rộng thật mới thấy 4 lỗi: mất lề, đè diagram, gãy chữ, markdown thô"]
strength: 2
stakes: 1
hits: 1
runs: 1
links: []
threads: []
privacy: checked
---

# Chụp ảnh ở bề rộng thật lộ ba lỗi UI chưa ai biết

**Bối cảnh:** Chụp lại bảy ảnh ở nhiều bề rộng để làm README thì thấy các lỗi mà trước đó không ai để ý.
**Chuyện gì xảy ra:** Ở khoảng 1000px, trang search mất lề trái. Thanh zoom Mermaid đè lên diagram, trên điện thoại còn rõ hơn. Trên điện thoại, inline code trong ô bảng bị gãy chữ. Kết quả search còn hiện markdown thô. Các lỗi này được ghi lại để sửa sau, không sửa trong phiên.
**Bài học:** Chụp ảnh ở đúng viewport của người dùng là cách kiểm UI rẻ. Lỗi thường lộ ra ở chỗ không ai định chụp.
**Artefact:** Ảnh trang search ở 1000px cạnh ảnh ở bề rộng bình thường.
**Góc người ngoài:** cách làm phổ biến là Kiểm thử giao diện bằng mắt ở một bề rộng desktop; lỗi responsive thường chỉ lộ khi người dùng báo; ở đây khác ở Chụp ảnh marketing vô tình thành một vòng kiểm thử trực quan nhiều viewport. Người đọc muốn biết rằng quy trình tạo ảnh cũng là cơ hội bắt bug, và cách tách bug ra khỏi phạm vi công việc.
