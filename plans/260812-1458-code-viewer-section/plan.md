# Code viewer section

**Status:** planned · **Branch target:** feature branch off `main` (v0.7.0)

Second UI section next to the markdown viewer: browse and read source files in a
registered project. No index, no search, no live-reload — a convenience reader.

## Decisions (locked by user, 2026-08-12)

| # | Decision | Rationale |
|---|---|---|
| D1 | **No index.** Files resolved on-demand from disk. | Keeps `MARKDOWN_EXTS` (`indexer.rs:13`) and FTS5 md-only; avoids index bloat on big repos. |
| D2 | **Separate section** with its own sidebar, under `/p/:id/_code/*path`. | Follows the existing reserved `_`-prefix convention (`_search`, `_jump`). Keeps project registry, theme, topbar. |
| D3 | **gitignore + denylist**, not an extension allowlist. | Daemon is unauthenticated and may bind wildcard on LAN; see [phase 1](phase-01-safe-source-access.md) threat model. |
| D4 | **Lazy per-directory** listing, server-rendered. | Bounded cost on any repo size; no client state, no JS, free no-JS fallback. |
| D5 | **No Cmd+K for code in v1.** | `_jump` is index-backed; a code equivalent needs a cached walk. Deferred. |

## Phases

| Phase | Scope | Depends on |
|---|---|---|
| [01 — Safe source access](phase-01-safe-source-access.md) | `mdview-core::code_source`: path resolution guard, denylist, gitignore-aware dir listing, binary/size policy | — |
| [02 — Per-line highlight](phase-02-per-line-highlight.md) | `RenderService::highlight_source` — line-anchored syntect output reusing the existing `SyntaxSet` and `/highlight.css` | — |
| [03 — Section UI](phase-03-section-ui.md) | Routes, `code_page` / `code_dir_page` views, Docs\|Code topbar switch, sidebar + CSS | 01, 02 |

Phases 01 and 02 touch disjoint files and share no API, so they run in
parallel; only 03 needs both.

## Acceptance criteria

1. `/p/:id/_code/` lists the project root: directories first, then files, alphabetical.
2. Clicking a directory navigates one level; clicking a file opens it highlighted with a line-number gutter and `#L<n>` anchors.
3. Sidebar shows the containing directory of the open file, with the file marked active.
4. Topbar switches between **Docs** and **Code** without losing the project.
5. A gitignored file, a denylisted file, and anything under `.git/` are **not listed and not servable** — the deny path returns 404 even when the URL is typed directly and even if the user removed `.git` from `exclude_patterns`.
6. Symlink pointing outside the project root → 404 (post-canonicalisation check).
7. Binary file → "not displayable" notice, never a garbled render. File over the size cap → truncated with a banner.
8. Markdown files keep rendering through the existing Docs path; nothing about the md pipeline changes except the topbar gaining the section switch.
9. `cargo fmt` + `cargo clippy` clean, full workspace tests green.

## Deferred (not v1)

- Fuzzy jump over code files (needs a cached `ignore` walk + `nucleo`).
- Live reload for code files — watcher filters md (`watch.rs:84`).
- `_raw` endpoint / binary download.
- Diff view — separate feature, builds on phase 02's per-line renderer.

## Docs to sync on completion

`docs/specs/system-overview.md` (new section + routes), `docs/usage.md` (how to
reach the code view), `docs/backlog.md` (new PBI row, mark done).
