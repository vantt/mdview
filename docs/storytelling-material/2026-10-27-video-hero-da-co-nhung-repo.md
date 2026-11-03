---
id: 2026-10-27-video-hero-da-co-nhung-repo
date: 2026-10-27
repo: mdview
status: raw
signals: [nguyên nhân bất ngờ]
source: "chat Claude, claude:f2200b35"
sessions: ["claude:f2200b35"]
captured_by: harvest
hit_by: [harvest]
lines: ["claude:f2200b35#L736", "claude:f2200b35#L741", "claude:f2200b35#L748", "claude:f2200b35#L755", "claude:f2200b35#L759"]
hook: "Video hero quay xong rồi, nhưng README không phát được vì MP4 bị gitignore"
tension: cam_ma_van_lot
sides: ["Video 6,2 MB đã làm xong, bộ công cụ đã commit", "MP4 bị .gitignore loại, GitHub chỉ phát link user-attachments kéo thả trên web"]
strength: 2
stakes: 2
hits: 1
runs: 1
links: []
threads: []
privacy: checked
---

# Video hero đã có, nhưng repo chỉ có bộ công cụ, MP4 bị gitignore

**Bối cảnh:** Người duyệt nghĩ video đã commit vào repo. Khi agent tìm lại thì thấy commit chỉ có công cụ dựng video, không có file MP4.
**Chuyện gì xảy ra:** Commit 6c6b2a9 gồm sandbox, script ghi hình, mã Remotion và fixture. Thư mục out/ chứa các MP4 (bản 30 giây nặng 6,2 MB) bị .gitignore loại ra. Tìm trong cả hai repo không có link user-attachments nào. Theo ghi chú của người duyệt, GitHub chỉ phát video trong README khi dùng link user-attachments, và link này chỉ lấy được bằng cách kéo thả trên web.
**Bài học:** 'Đã làm' và 'đã commit' có thể là hai việc khác nhau. Trước khi báo cáo, kiểm tra cái thật sự nằm trong git.
**Artefact:** git show --stat của commit cạnh nội dung .gitignore, dạng hai khung chữ.
