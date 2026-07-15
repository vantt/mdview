# Validation Report — fix-review-p1-findings-260716, current slice (4 cells)

## Reality Gate Report

```
REALITY GATE REPORT
Mode: high-risk
Current work: fix the 4 P1 findings from review-2026-07-16-full-app (1 security fix + 3 test-coverage fixes)
MODE FIT: PASS — 1 hard-gate flag (audit/security) mechanically forces high-risk; the 3 test-only cells stay tagged small inside the same slice, least ceremony that still honestly protects the security cell
REPO FIT: PASS — all named files/functions/line-ranges verified directly against current source: engine.rs::asset_path (L242-253), server.rs::content_type (L422-438) and project_path (L260-282), config.rs::IndexingConfig::default (L77-91), indexer.rs::scan_markdown_files (L88-108), cli.rs::cmd_config_edit (L124-167) and stop_daemon/cmd_stop/cmd_restart (L346-400), views.rs::file_page source_json escape (L79-81)
ASSUMPTIONS: PASS — every blocking assumption listed in the feasibility matrix below
SMALLER PATH: PASS — mode gate is mechanical; a hard-gate flag cannot be routed below high-risk regardless of the fix's small size
PROOF SURFACE: PASS — `cargo test --workspace` is a real, currently-passing command in this workspace (confirmed: 45/45 green pre-fix); each cell's verify is this same command after the BLOCKER fix below
Decision: proceed
Evidence: direct file reads during exploring/planning/validating; `cargo test --workspace` run this session (45 passed, 0 failed)
```

## Feasibility Matrix

| Assumption | Risk | Proof Required | Evidence | Result |
|---|---|---|---|---|
| `asset_path` is the sole choke point for arbitrary-file exposure (no second hole via the render path) | Medium | Trace `project_path`'s two branches | Security persona traced `render_file`'s call to `get_file`, which only returns rows the indexer put there, and the indexer already filters to markdown + honors `exclude_patterns`/`.gitignore` (indexer.rs L103, L110-114) — non-markdown files never reach the index, so they always fall through to `asset_path`. Confirmed sole choke point. | READY |
| Extension allowlist duplicated across the `mdview-core`/`mdview` crate boundary won't silently drift uncaught | Low-Medium | Code comment + CONTEXT.md D1 note | Accepted as a documentation risk, not proof-gated this slice (approach.md Risk Map) | READY WITH CONSTRAINT (documented, not proof-gated) |
| `exclude_patterns` component-matching is exact-equality, not glob, and is mirrored correctly | Low | Read `scan_markdown_files`'s actual match logic | Confirmed: `WalkBuilder::filter_entry` does `name.as_ref() == ex.as_str()` per walked entry (indexer.rs L96-98) — exact equality, not glob/substring. Cell action pinned to mirror this exactly. | READY |
| `exclude_patterns` check must run against relative-path components, not absolute canonical components | Medium | Trace `scan_markdown_files`'s walk root | Confirmed: the walker only ever walks/matches within `root`, never above it. Feasibility persona flagged the original cell text left this ambiguous; cell patched to require relative-to-root components explicitly, avoiding false-positive exclusion of a project whose root sits under a pattern-named ancestor dir. | READY (post-patch) |
| Extension check must run on `canonical` (post-symlink-resolution), never on `rel_path`/the URL segment | **High** | Trace the symlink-bypass scenario | Security persona identified: a symlink `pretty.png -> .env` inside root canonicalizes to the real `.env` target and passes the traversal check; if the extension were read from the URL path ("png") instead of `canonical`, the fix is silently bypassed and the original hole reopens. Cell patched to pin the check to `canonical` explicitly, mirroring how `content_type(&canonical)` already reads canonical's extension (server.rs L277), plus a mandatory unix symlink regression test added to must_haves. | READY (post-patch, load-bearing fix) |
| The 3 test-coverage cells' pure-function extractions compile cleanly with no new dependencies or naming collisions | Low | Read target files for existing test infra / imports | Feasibility persona confirmed: neither `cli.rs` nor `views.rs` has an existing `#[cfg(test)]` module (clean insert); `toml`/`serde_json`/`Config` already imported and used in the target functions | READY |
| `cargo test --workspace` actually exercises the new unit tests added by all 4 cells | Low | Confirm workspace member wiring | Cold-pickup reviewer confirmed the workspace root `Cargo.toml` includes both `mdview-core` and `mdview` as members; `cargo test --workspace` runs both crates' test suites | READY |
| Two independent cells (`config-edit-cli-test-1`, `mdview-restart-test-1`) touching the same file (`cli.rs`) is safe | Low | Cell scheduling check | `node .bee/bin/bee.mjs cells schedule` reports zero cycles, 2 waves, with the two `cli.rs` cells auto-serialized into separate waves — legal overlap per planning rules, not a scoping error | READY |

Schedule evidence:
```
Wave 1: config-edit-cli-test-1, copy-as-markdown-test-1, security-asset-allowlist-1
Wave 2: mdview-restart-test-1
```

## Plan-Checker: High-Risk Persona Panel

Personas dispatched: **coherence** (always), **feasibility** (always), **security** (conditional — audit/security is the lane's own trigger flag). Product and scope-guardian lenses were not dispatched: no user-visible behavior addition and no growing API surface to check (all 4 cells are bugfix/test-only, confirmed by cell review).

**Coherence persona** — 1 BLOCKER, 3 WARNINGs, all resolved:
- BLOCKER: `security-asset-allowlist-1`'s `verify` field was prose describing a manual live-curl procedure, not a runnable command (violates AGENTS.md critical rule 2). **Fixed:** `verify` is now `cargo test --workspace`; the live-serve requirement was replaced with a `#[cfg(test)]` unit test directly against `asset_path` (better evidence than manual curl — this is exactly the kind of gap that caused the original P1s).
- WARNING: undeclared same-file overlap between `config-edit-cli-test-1`/`mdview-restart-test-1`. Confirmed legal (cross-cell file overlap, auto-serialized per planning rules); `cells schedule` proves it resolves to 2 clean waves. No cell change needed.
- WARNING: D1's case-insensitivity requirement had no test assertion. **Fixed:** added an uppercase-extension (`LOGO.PNG`) truth + test requirement.
- WARNING: `key_links` empty despite `project_path` being a named integration point. **Fixed:** added the `key_links` entry.

**Feasibility persona** — 0 BLOCKERs, 2 WARNINGs, both resolved:
- WARNING: exclude-pattern component-matching left ambiguous (absolute vs. relative path). **Fixed:** pinned to relative-to-root components, matching `scan_markdown_files`'s actual walk scope.
- WARNING: `mdview-restart-test-1`'s `cmd_restart` half only required "a named, tested assertion," not a full extraction symmetric with `cmd_stop`'s. **Fixed:** now requires an actual extracted function for both halves.

**Security persona** — verdict SECURITY FIX SOUND, 3 findings:
- Confirmed `asset_path` is the sole choke point (no second hole via the render/markdown path — non-markdown files are never indexed, so they always reach `asset_path`).
- Confirmed the fix's scope maps 1:1 to the original finding's 4 acceptance criteria.
- **WARNING (load-bearing, fixed):** extension check must run on `canonical` (symlink-resolved), never on `rel_path`/the URL segment — a symlink `pretty.png -> .env` would otherwise bypass the fix and reopen the original arbitrary-read hole. Cell patched to pin this explicitly, plus a mandatory unix symlink regression test added.
- WARNING (accepted, not fixed): double-extension files (e.g. `secret.key.png`) still pass — explicitly out of scope per CONTEXT.md D1 Agent's Discretion, documented in the cell's prohibitions.
- WARNING: exclude-pattern component scope — same finding as the feasibility persona's, already fixed.

## Cell Review (cold pickup)

```
CELL REVIEW REPORT
Work: fix-review-p1-findings-260716 current slice (4 cells)
Cells reviewed: 4
CRITICAL FLAGS: none
MINOR FLAGS: security-asset-allowlist-1 — live-serve verify mechanics underspecified — RESOLVED as a side effect of the coherence BLOCKER fix (live-serve requirement removed, replaced with a self-contained unit test)
CLEAN CELLS: config-edit-cli-test-1, copy-as-markdown-test-1, mdview-restart-test-1, security-asset-allowlist-1 (post-patch)
SUMMARY: All four cells are self-contained and executable by a worker with no session history. Every file/line reference was verified against current source and matches exactly.
```

## Approval Block

```
VALIDATION COMPLETE - APPROVAL REQUIRED BEFORE EXECUTION
Mode: high-risk
Work: fix-review-p1-findings-260716 current slice (4 cells)
Reality gate: PASS
Feasibility: READY WITH CONSTRAINTS (2 documented, accepted, non-blocking: crate-boundary allowlist duplication; double-extension files out of scope per D1)
Structure: PASS after 1 iteration (1 BLOCKER + 5 WARNINGs found and fixed in the same pass)
Spikes: none required — all assumptions resolved by direct code inspection
Cell review: PASS (4 cells, 0 CRITICAL open)
Unresolved concerns: none
```
