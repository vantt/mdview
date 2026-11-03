---
id: 2026-10-27-ghep-luoi-anh-theo-ti-le
date: 2026-10-27
repo: mdview
status: raw
signals: [số trước số sau, artefact nhìn được]
source: "chat Claude, claude:f2200b35"
sessions: ["claude:f2200b35"]
captured_by: harvest
hit_by: [harvest, outsider]
lines: ["claude:f2200b35#L1048", "claude:f2200b35#L1057", "claude:f2200b35#L1065", "claude:f2200b35#L1075", "claude:f2200b35#L1081", "claude:f2200b35#L1089"]
hook: "Ghép lưới ảnh theo tỉ lệ khung để hai hàng cao bằng nhau"
tension: none
sides: []
strength: 0
stakes: 1
hits: 1
runs: 1
links: []
threads: []
privacy: checked
---

# Ghép lưới ảnh theo tỉ lệ khung để hai hàng cao bằng nhau

**Bối cảnh:** Bảy ảnh chụp có nhiều tỉ lệ khác nhau. Bố cục 2 cột lệch chiều cao, nhìn không đều.
**Chuyện gì xảy ra:** Preview cho thấy hai hàng giữa lệch chiều cao. Agent đổi cặp: Mermaid đi với Search (tỉ lệ khoảng 1,5 và 1,8), Bảng đi với Edit (khoảng 1,2 và 1,35). Hai ảnh điện thoại xếp ở giữa, mỗi ảnh rộng 260px.
**Bài học:** Ghép ảnh theo tỉ lệ khung gần nhau, không theo thứ tự chức năng. Kiểm bằng preview trên nền GitHub sáng và tối.
**Artefact:** Ảnh preview lưới trên nền sáng và tối, rộng 880px như trên GitHub.
**Góc người ngoài:** cách làm phổ biến là Commit README rồi mở trên GitHub mới biết ảnh nền tối trông ra sao ở theme sáng, bố cục có lệch không; sửa bằng nhiều commit nhỏ; ở đây khác ở Chỉ với một file HTML nhúng ảnh base64 và một ảnh chụp là thấy cả hai theme cạnh nhau trước khi đẩy lên. Người đọc muốn có bước xem trước này cho README của mình.
