# Plan: mdview MVP implementation

**Status:** MVP DONE + verified end-to-end (autonomous run, gate-bypass full) · **Branch:** feat/mvp-implementation
**Result:** S0-S11 complete. 21 core tests green; HTTP/MCP/CLI/doctor verified by driving the binary. Remaining: desktop shell (Phase 4), scoped live-reload, UX polish.
**Source of truth:** PRD.md · **Porting picks:** docs/distillery/porting-log.md (12 planned)

## Kiến trúc (PRD §7.4 — ports & adapters)

```
Cargo.toml (workspace)
crates/
  mdview-core/   lib: domain + application + adapters
    domain/      types: Project, IndexedFile, Link, Heading, Config
    ports        traits: ProjectRepository, Clock (Watcher là adapter-side)
    app/         services: LinkResolver, Indexer, RenderService, SearchService, ViewFile
    adapters/    sqlite (ProjectRepository+index+FTS5), scan (WalkBuilder), watch (notify)
    render/      comrak → syntect (class-based) → ammonia
  mdview/        bin: clap CLI + Axum daemon + MCP + doctor
```

## Build order (mỗi bước compile + test + commit)

- [ ] **S0** Workspace scaffold, `cargo build` xanh
- [ ] **S1** Core domain types + Config (toml, atomic load/save)
- [ ] **S2** LinkResolver (thuật toán §7.3) **+ unit tests** — core differentiator G2
- [ ] **S3** Renderer: comrak + syntect class-based + ammonia + frontmatter + mermaid mark
- [ ] **S4** SQLite adapter: registry + file index + FTS5; atomic; scan (WalkBuilder recursive)
- [ ] **S5** Axum server: `/`, `/p/{id}/*path`, images, `/health`, `/api/*`, `/ws` live reload
- [ ] **S6** Watcher: notify incremental + debounce 200ms + re-scan trigger
- [ ] **S7** MCP server: `mdview_view_file` (stdio) + auto-create project
- [ ] **S8** CLI: serve/register/open/list/search/status/refresh/stop + daemon.lock
- [ ] **S9** Web UI: embedded templates + theme(no-flash) + file tree + TOC + mermaid + reload JS
- [ ] **S10** `mdview doctor` + `install.sh`
- [ ] **S11** AGENTS.md integration template + README

## Acceptance (MVP)

- `cargo build --release` xanh, `cargo test` xanh.
- `mdview register <dir>` → scan; `mdview serve` → xem ở browser; click cross-folder link không 404 (G2).
- `mdview_view_file(root, rel)` trả url, auto-create project.
- Sửa file trên disk → browser live reload.
- `mdview doctor` báo trạng thái + fix MCP registration.

## Scope MVP (theo open-questions đã chốt)

IN: Phase 1 + MCP 1-tool + CLI + doctor/install + web UI cơ bản.
OUT: desktop shell (Phase 4), cross-project resolution (non-goal), semantic search (defer),
scoped live-reload (Phase 3, dùng full-page), command-palette/copy-as-markdown (defer).

## Risks

- Syntect class-based CSS theme: cần sinh CSS per theme.
- notify cross-platform incremental: bắt đầu Linux (inotify), giữ port dễ.
- Context budget: nếu ~65% → HANDOFF sạch, ghi bước đang dở.
