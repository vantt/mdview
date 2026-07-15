# Distill session — mdserve (jfernandez) learning scan

**Date:** 2026-07-15 · **Source:** https://github.com/jfernandez/mdserve @ `f84ae3e`
**Type:** git-repo · **Cursor:** sealed at `f84ae3e` / 2026-07-15
**Inventory reports:** `distill-mdserve-rust-core-260715.md`,
`distill-mdserve-frontend-packaging-260715.md`

## Bottom line

mdserve is the closest prior art to mdview (PRD names it directly). It nails the
*single-project preview* experience — live reload, GFM+Mermaid rendering,
theming, a Claude Code plugin/skill — and deliberately stops exactly where
mdview begins: **non-recursive, flat, no cross-folder link resolution, no
multi-project registry, no MCP.** Its confirmed gaps are mdview's reason to
exist; its polished mechanics are mdview's shortcut.

## What was learned (14 features indexed)

Fitting existing taxonomy domains:
- **skills** — `claude-skill-render-heuristics`: a shipped SKILL.md encoding the
  *when-to-render* decision boundary (long/table/diagram → serve; trivial → skip).
- **config-packaging** — `claude-plugin-manifest`, `template-embed-at-build`
  (minijinja-embed → single binary), `multi-channel-install` (curl/brew/cargo/
  pacman/nix, layered install-dir fallback).
- **tooling** — `cli-surface` (clap, zero-config, loopback default),
  `git-cliff-changelog` (conventional-commits → auto CHANGELOG, CI-enforced).
- **safety** — `path-traversal-guard` (canonicalize + prefix check).
- **ux** — `theme-system-no-flash` (5 themes, blocking head script vs FOUC),
  `sidebar-file-nav` (unified single/dir template), `mermaid-client-render`
  (bundled, ETag-cached, theme-aware).

Core server mechanics with NO fitting domain (held under `## unclassified`):
- `markdown-render-pipeline` — markdown-rs GFM, pre-render-to-memory cache,
  frontmatter strip, server-side highlight.
- `websocket-live-reload` — `/ws` broadcast → `ServerMessage::Reload` → client
  reload; auto-reconnect 3s; reload-signal not content-diff.
- `file-watcher-notify` — notify, non-recursive, **ignores deletes** to survive
  editor rename-save (the non-obvious robustness lesson).
- `unified-http-router` — one Axum router both modes; **port auto-increment**.
- `static-asset-and-etag-cache` — images + embedded mermaid, 304 revalidation.
- `flat-filename-url-routing` — literal filenames, **no link rewriting** (the gap).

## Confirmed absences (mdview differentiators, recorded in matrix)

Recursive scan ✗ · cross-folder link resolution ✗ · multi-project registry ✗ ·
persistent registry ✗ · MCP ✗ · search ✗. These are *deliberate* in mdserve —
worth keeping as the baseline mdview must exceed, not treated as oversights.

## Porting candidates (proposed — adoption is your call; `distill rank`)

| Score | Candidate | Why |
|---|---|---|
| 4.0 | file-watcher atomic-save handling (ignore delete) | Cheap, high-leverage robustness; real bug (v0.5.1) |
| 2.0 | when-to-render agent skill heuristic | Reusable for mdview's MCP/skill (G3) |
| 2.0 | canonicalize+prefix path-traversal guard | mdview's cross-folder serving widens the traversal surface |
| 2.0 | WebSocket reload-signal live reload | Covers G5; simpler/robuster than DOM push |
| 2.0 | pre-render-to-memory markdown pipeline | Central perf pattern; adapt to mdview's stack |
| 1.0 | no-flash theme, port auto-increment, sidebar nav | UX/quality polish |

## Taxonomy — realigned (human-approved)

The old taxonomy was agent-tooling oriented; mdview is a markdown viewer/server.
Applied decision "add 5, retire misfits":
- **Added:** `rendering`, `live-reload`, `http-serving`, `link-resolution`,
  `file-indexing`.
- **Retired (8 empty agent-workflow domains):** harness, hooks, workflow,
  orchestration, context-memory, planning, quality-gates, self-improvement.
- **Kept `repo-layout` + `testing-evals`** — the preview would have dropped them,
  but marky's scan (which landed after the question) populates both with real
  content (module boundaries, Vitest/Rust test strategy). Flagged to the human;
  they can still be retired if preferred.

Both source indexes re-slotted out of `## unclassified` into the new domains.

## Reconciliation note (concurrent session)

`marky` was scanned in parallel by another session (17 features, cursor
`5d02237`) — matrix and 9 marky porting candidates were filled there. This
session reconciled it: fixed frontmatter, moved its `unclassified` entries into
`rendering`/`live-reload`/`file-indexing`. `check` now clean of hard errors.

## State after session

- 2 sources sealed: mdserve (`f84ae3e`), marky (`5d02237`).
- 16 porting candidates ranked; top: sanitize-before-serve (6.0), file-watcher
  ignore-delete (4.0), atomic settings persistence (4.0), recursive folder tree
  (3.0).
- Remaining `check` "backfill needed" is informational: marky ⌿ http-serving/
  link-resolution/skills (N/A); mdserve ⌿ docs-style/repo-layout/testing-evals
  (real, optional backfill).

## Outstanding questions

1. Retire `repo-layout` + `testing-evals` after all, or keep (current)?
2. Backfill mdserve for docs-style/repo-layout/testing-evals, or leave?
3. Ready to hand top candidates to a porting job, or scan more sources first?
</content>
