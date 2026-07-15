---
artifact_contract: bee-plan/v1
artifact_readiness: implementation-ready
mode: high-risk
---

# Plan: Fix Review P1 Findings

Mode: `high-risk` — 1 hard-gate risk flag: **audit/security** (the asset-fallback file-exposure fix touches what the HTTP server will and won't hand out from disk). The other 3 items in this slice (test-coverage-only fixes) are individually `tiny`/`small` (0 flags, pure extraction + unit tests, no behavior change) but travel inside the same high-risk plan because they're the same slice closing the same review session.
Why this is the least workflow that protects the work: one cell touches an actual security boundary (what bytes the daemon serves over HTTP) — that earns the full validating pass even though the fix itself is small; the other three are mechanical enough that the extra ceremony would be pure overhead, so they stay in the same slice but are marked `tiny`/`small` at the cell level.

## Requirements (from CONTEXT.md)

- D1: asset-fallback extension allowlist = the 9 tokens (8 content-types) `content_type()` already recognizes (png/jpg/jpeg/gif/svg/webp/ico/bmp/pdf); duplicated into `mdview-core` since `asset_path` can't import the binary crate's `content_type()`.
- D2: `asset_path` also rejects any path whose components match `config.indexing.exclude_patterns` (default `.git`, `node_modules`, `.venv`, `target`, `dist`).
- D3: extract the already-shipped pure logic behind config-edit-cli-1, copy-as-markdown-2, mdview-restart-1 into unit-testable functions and add tests — no behavior change.

## Discovery

L0 — no external unknowns. `content_type()` (server.rs ~L422-438) and `IndexingConfig::default()` (config.rs ~L77-91) were read directly during exploring's fresh-eyes check; `asset_path` (engine.rs ~L242-253) currently enforces only path containment. `scan_markdown_files` (indexer.rs) is the existing precedent for consulting `exclude_patterns` against a filesystem path — `asset_path` mirrors that check, not a new pattern.

## Shape — epic map

Feature outcome: the 4 P1 findings from `review-2026-07-16-full-app` are fixed with real evidence (a real allowlist enforced + real tests, not more prose), so the review session's delta re-review can pass and Gate 4 can close.
Repo-reality basis: all 4 target functions exist today, unchanged since the review; no scaffolding needed.

| Epic | Capability/Risk Area | Why It Exists | Slices | Proof Needed |
|---|---|---|---|---|
| E1 | Asset-serving boundary (security) | `asset_path` currently serves any readable file under a project root; D1/D2 shrink that to a safe, explicit surface | S1 | `.env`/`.git/config` 404, `logo.png` still serves, `node_modules/x/y.png` 404, traversal guard unchanged |
| E2 | Evidence-gate closure (test coverage) | 3 already-shipped behavior changes were capped on manual proof only; D3 replaces that with real unit tests | S2, S3, S4 | each named test exists and passes; no behavior change per prohibitions |

Slice queue (all four are independent — no cross-slice deps):

- **S1** `security-asset-allowlist-1` — high-risk, E1, per D1+D2. Feasible now (files/read paths confirmed above).
- **S2** `config-edit-cli-test-1` — small, E2, per D3. Feasible now.
- **S3** `copy-as-markdown-test-1` — small, E2, per D3. Feasible now.
- **S4** `mdview-restart-test-1` — small, E2, per D3. Feasible now.

Current slice to prepare: **all four** — independent, no ordering constraint, small enough to cap in one pass.

## Test matrix

- **Security boundary (S1):** allowed extension inside root (serves) / disallowed extension inside root (404) / allowed extension inside an excluded dir (404) / traversal (`../`) attempt (404, unchanged) / dotfile with no recognized extension (404, already covered by the allowlist alone per CONTEXT Agent's Discretion).
- **Evidence regression (S2-S4):** happy path (each function's documented normal input) / one edge each finding already named — `$VISUAL` beats `$EDITOR` fallback chain (S2), a source containing `</script>` (S3), each `stop_daemon` outcome → message mapping including "no daemon" (S4).
- **Prohibition check (all):** `cargo test --workspace` stays green; no route, CLI flag, or JSON field is added, renamed, or removed by any of the four cells (D3 explicitly forbids behavior change).

## Out of scope

- Auth/token gate on the daemon, CSRF protection on `POST /api/config`, Mermaid CDN pinning/vendoring — all review P2s, already filed to `.bee/backlog.jsonl`, not part of this P1-only slice.
- `.gitignore` consultation (beyond `exclude_patterns`) and unconditional dotfile denial on the asset path — explicitly deferred in CONTEXT.md Agent's Discretion, not silently added here.

<!-- implementation-ready additions (after Gate 2): -->
## Current slice

Slice: S1 + S2 + S3 + S4 (independent, same pass).
Entry state: 4 P1 findings open on review session `review-2026-07-16-full-app`; `cargo test --workspace` green (45/45).
Exit state: `asset_path` enforces D1+D2; three new/extended test modules exist and pass; `cargo test --workspace` still green; each cell capped with real verification evidence (not manual-only prose).
Files bounded: `crates/mdview-core/src/engine.rs`, `crates/mdview/src/cli.rs`, `crates/mdview/src/views.rs`.
Verify commands: `cargo test --workspace` (all four cells), plus S1's own live-serve check (see cell).

## Cells

- `security-asset-allowlist-1` (high-risk)
- `config-edit-cli-test-1` (small)
- `copy-as-markdown-test-1` (small)
- `mdview-restart-test-1` (small)
