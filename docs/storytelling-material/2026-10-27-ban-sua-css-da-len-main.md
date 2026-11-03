---
id: 2026-10-27-ban-sua-css-da-len-main
date: 2026-10-27
repo: mdview
status: raw
signals: [khác]
source: "chat Claude, claude:f2200b35"
sessions: ["claude:f2200b35"]
captured_by: harvest
hit_by: [harvest]
lines: ["claude:f2200b35#L38", "claude:f2200b35#L53", "claude:f2200b35#L64", "claude:f2200b35#L306", "claude:f2200b35#L309", "claude:f2200b35#L1265", "claude:f2200b35#L1270"]
hook: "Bản sửa CSS đã lên main nhưng người cài vẫn nhận 0.8.9"
tension: tuong_ma_hoa_ra
sides: ["Tưởng sửa lỗi và merge lên main là xong", "Chưa có bản phát hành thì installer vẫn cho bản cũ"]
strength: 2
stakes: 1
hits: 1
runs: 1
links: []
threads: []
privacy: checked
---

# Bản sửa CSS đã lên main nhưng chưa phát hành, binary build từ main vẫn báo 0.8.9

**Bối cảnh:** Build từ main rồi chạy --version thì ra 0.8.9, trong khi main đã có commit sửa lề sau tag v0.8.9.
**Chuyện gì xảy ra:** Tag mới nhất là v0.8.9. Main đã có commit sửa CSS nhưng chưa có bản phát hành, nên người cài qua installer sẽ không nhận được bản sửa. Agent đưa việc ra bản 0.8.10 vào danh sách việc còn lại.
**Bài học:** Code trên main và bản người dùng nhận được là hai thứ khác nhau. Khi viết về trạng thái sản phẩm, ghi rõ tag và commit đang nói tới.
**Artefact:** Bảng tag và commit, hoặc output của --version.
