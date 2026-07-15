# Fix Review P1 Findings — Context

**Feature slug:** fix-review-p1-findings-260716
**Date:** 2026-07-16
**Exploring session:** complete
**Scope:** Quick
**Domain types:** CALL, ORGANIZE

## Feature Boundary

Fix the 4 P1 findings from independent review session `review-2026-07-16-full-app` (security's asset-fallback file exposure, plus 3 behavior-change cells capped with no automated test) and close the loop back to that review session via a delta re-review; no other findings from that session (21 P2, 22 P3, already filed to backlog) are in scope here.

## Locked Decisions

| ID | Decision | Rationale (only if it changes implementation) |
|----|----------|-----------------------------------------------|
| D1 | The asset-fallback extension allowlist in `asset_path`/`project_path` is exactly the 9 extension tokens (8 content-types) `content_type()` (server.rs) already recognizes: png, jpg, jpeg, gif, svg, webp, ico, bmp, pdf. No new types added. `content_type()` lives in the `mdview` binary crate; `asset_path` lives in `mdview-core` — the list must be duplicated across the crate boundary (can't import across it), so keep both lists in sync if either changes. | Reuses an existing, already-audited list rather than inventing a new one — the code already treats these as "servable media" via `content_type()`. |
| D2 | `asset_path` also rejects any path whose components match `config.indexing.exclude_patterns` (default: `.git`, `node_modules`, `.venv`, `target`, `dist`), same as `scan_markdown_files` already does for indexing. | A file excluded from the index for being noise/vendor content must not be reachable via the raw asset URL either — closes the `node_modules/x/logo.png`-shaped gap. |
| D3 | Fixing config-edit-cli-1, copy-as-markdown-2, mdview-restart-1's missing-test P1s means extracting the already-identified pure logic (editor resolution/arg-split/TOML-validate; the `<`→`<` JSON escape; the `stop_daemon` outcome→message mapping) into unit-testable functions and adding tests — no behavior change, per the smallest-fix each finding already proposed. | These are evidence-gate P1s (missing proof), not functional bugs — the fix is real verification, not new logic. |

### Agent's Discretion

- Exact extracted-function names/signatures for D3's three test-coverage fixes are an implementation choice for planning/execution, not a locked product decision.
- Whether the security fix additionally denies dotfiles unconditionally (independent of extension) is left as an assumption below, not a separate locked decision — the extension allowlist alone already 404s any file without a recognized image/pdf extension (e.g. `.env` has no extension `content_type()` recognizes), so a dotfile would only pass if it also carried an allowed extension (e.g. `.env.png`), which is out of this fix's threat model.

## Existing Code Context

### Reusable Assets

- `crates/mdview/src/server.rs::content_type()` (~L422-438) — the exact allowlist source for D1.
- `crates/mdview-core/src/config.rs::IndexingConfig::default()` (~L77-91) — `exclude_patterns` default list, reused per D2.
- `crates/mdview-core/src/indexer.rs::scan_markdown_files` — existing precedent for consulting `exclude_patterns` on a filesystem path, pattern to mirror in `asset_path`.

### Integration Points

- `crates/mdview-core/src/engine.rs::asset_path()` (~L242) — where D1+D2 are enforced.
- `crates/mdview/src/server.rs::project_path()` (~L275-277) — caller of `asset_path`, no change expected beyond receiving the tightened `Result`.
- `crates/mdview/src/cli.rs::cmd_config_edit`, `stop_daemon`, `cmd_restart` — D3 extraction targets.
- `crates/mdview/src/views.rs::file_page` source_json escape — D3 extraction target.

## Canonical References

- `docs/history/review-2026-07-16-full-app/reports/findings-security.md` — full P1 writeup (asset fallback).
- `docs/history/review-2026-07-16-full-app/reports/findings-test-coverage.md` — full P1 writeups (3 evidence-gate findings).
- `.bee/reviews/review-2026-07-16-full-app.json` — the review session this fix closes the loop against (delta re-review required per bee-reviewing §6 before Gate 4).

## Outstanding Questions

### Deferred To Planning

- [ ] Whether the security fix and the 3 test-only fixes are one cell or split into a high-risk cell (security) + small/tiny cells (tests) — mode/lane shape is planning's call, not locked here.

## Deferred Ideas

- Full auth/token gate on the daemon, and CSRF protection on `POST /api/config` (review P2s, already in backlog) — out of scope for this P1-only fix pass.
- Mermaid CDN pinning/vendoring (review P2, already in backlog) — out of scope.

## Handoff Note

CONTEXT.md is the source of truth. Decision IDs D1-D3 are stable. Planning shapes cells from here; validating and reviewing use these locked decisions for coverage and UAT.
