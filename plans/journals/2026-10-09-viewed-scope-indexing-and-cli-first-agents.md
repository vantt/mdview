---
title: Viewed-scope indexing and CLI-first agents
date: 2026-10-09
summary: Index what is viewed plus lazy project sync on search; contentless rowid-linked FTS; CLI-first agent wiring
---

# Viewed-scope indexing and CLI-first agents

## What happened
mdview copied every markdown file of a project into FTS on the first `view_file`
(one project: 2,372 rows / 24 MB, 90% agent skill packs in hidden dirs), and FTS
deletes filtered on UNINDEXED columns, making refresh and cleanup O(n²).

## Changes (branch feat/viewed-scope-index)
- Viewed-scope indexing: a page view indexes the file (one read), then its markdown
  link targets and siblings off the request path; content search lazily syncs the
  whole project (single-flight, 10 s reuse window).
- Contentless FTS5 linked by `files.fts_rowid`; mdview folds text itself (`đ`→`d`)
  because `remove_diacritics 2` does not.
- `registry-v4.db`, drop-and-rebuild instead of migrations; 14-day project TTL only.
- Markdown-only, canonical-root confinement for anything indexed, linked or rendered.
- CLI-first agents: `mdview open --json`, `doctor --fix` adds `Bash(mdview open:*)`, MCP opt-in.

## Lessons
- The Agent tool's worktree isolation branches from `main`, not HEAD: create worktrees
  manually from the base commit for stacked parallel work.
- A repo hook blocks shell commands containing `target` / `node_modules`.
- Red-team before coding caught non-markdown link targets entering the index, deferred
  `BEGIN` busy failures and swallowed live reloads; post-merge review caught a daemon
  restart keyed on an unchanged version and a pre-existing `<title>` XSS.

## Results
286 tests green; isolated smoke 18/18; full sync of 2,359 files in 0.78 s, DB 7.4 MB.

## Next steps
Merge and bump the version; known limits: VACUUM holds the store mutex briefly,
recreated directories are re-watched only after a view or search.

> Historical work record — not durable authority. Prefer docs/specs/ADRs for current decisions.
