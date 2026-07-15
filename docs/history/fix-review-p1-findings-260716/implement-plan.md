---
artifact_contract: bee-implement-plan/v1
feature: fix-review-p1-findings-260716
lane: high-risk
status: Ready for Review
updated: 2026-07-16
sources: [CONTEXT.md, approach.md, plan.md, docs/history/review-2026-07-16-full-app/reports/findings-security.md, docs/history/review-2026-07-16-full-app/reports/findings-test-coverage.md]
decisions: [D1, D2, D3]
---

# Implementation Plan: Fix Review P1 Findings

> Human-layer projection of the truth artifacts. Truth lives in CONTEXT.md
> (decisions), plan.md + cells (work), and the validating report (evidence).
> Feedback on this document flows back to those artifacts, then this re-renders.

## 1. Goal

Close the 4 P1 findings that review session `review-2026-07-16-full-app` raised against the whole app, so that session's Gate 4 can move past its P1 block.

**Success looks like**
- The HTTP asset endpoint no longer hands out arbitrary files from a registered project root — only a known-safe media allowlist, and never anything inside an excluded directory (D1, D2).
- The 3 behavior-change cells that shipped with only manual/prose verification (`config-edit-cli-1`, `copy-as-markdown-2`, `mdview-restart-1`) each gain a real, named automated test proving the behavior the finding flagged (D3).

## 2. Current State

`crates/mdview-core/src/engine.rs::asset_path` (~L242-253) resolves any path under a project root and returns it as long as it stays inside the root (traversal-guarded) — no extension check, no `exclude_patterns` check. `crates/mdview/src/server.rs::project_path` (~L275-279) reads whatever bytes `asset_path` resolves and serves them with a best-effort content-type, `application/octet-stream` for anything unrecognized. Three already-shipped behavior changes have no automated test: `cmd_config_edit`'s editor resolution/TOML-validation (cli.rs ~L124-167), the `</script>`-breakout escape in `file_page`'s `source_json` build (views.rs ~L79-81), and `stop_daemon`'s outcome-to-message mapping used by `cmd_stop`/`cmd_restart` (cli.rs ~L346-400) — each was capped on a manual live run only.

## 3. Scope

**In scope**
- Extension allowlist + `exclude_patterns` enforcement inside `asset_path` (D1, D2).
- Unit tests for the three named P1 evidence gaps, via pure-function extraction with no behavior change (D3).

**Out of scope**
- Daemon auth/token gating and CSRF protection on `POST /api/config` — review P2s, already filed to `.bee/backlog.jsonl`.
- Mermaid CDN pinning/vendoring — review P2, already filed.
- `.gitignore` consultation beyond `exclude_patterns`, and unconditional dotfile denial on the asset path — explicitly deferred (CONTEXT.md Agent's Discretion).
- All 21 P2 and 22 P3 findings from the review session other than the 4 P1s — non-blocking, already in the backlog.

## 4. Proposed Approach

Enforce D1 and D2 directly inside `asset_path` — the single choke point every asset read already passes through — rather than adding a new layer or config surface. For D3, extract the three already-identified pure operations into standalone functions in their existing files and add `#[cfg(test)]` coverage; no behavior change.

**Why this approach** — `asset_path` is already the one function every HTTP asset request funnels through (`project_path` calls nothing else), so tightening it there closes the hole with no new surface. The three test fixes reuse code that already ships correctly — the gap was proof, not correctness — so extraction + test is the smallest credible fix each finding itself proposed.
**Alternatives considered** — daemon-wide auth instead of narrowing `asset_path` (rejected: out of scope, a separate already-filed P2, not part of this fix); MIME-sniffing file contents instead of an extension allowlist (rejected: new dependency/runtime cost for a local dev tool, extension list already exists and is auditable); routing the 3 test fixes through separate small features (rejected: independent but tiny, same review-session closure, splitting adds ceremony with no benefit).

## 5. Technical Design

```text
GET /p/<id>/<path> -> project_path() -> asset_path() [D1 ext check, D2 exclude check] -> 200 + bytes, or 404
```

`asset_path` gains two checks after its existing traversal guard: (1) the resolved path's extension must be one of the 9 tokens `content_type()` already recognizes (png/jpg/jpeg/gif/svg/webp/ico/bmp/pdf) — duplicated as a small local list in `mdview-core` since it cannot import the `mdview` binary crate's `content_type()`; (2) none of the resolved path's components may match `self.config.indexing.exclude_patterns`, mirrored from the same style of check `scan_markdown_files` already applies. Either failure returns the existing error type; `project_path`'s caller-side handling is unchanged — a failed `asset_path` already falls through to `not_found` today, so the 404 behavior falls out with no change to `server.rs`.

For D3, no new components: each fix adds a pure function inside its existing file (`cli.rs` gets an editor-resolution/arg-split function and a TOML-outcome classifier; `cli.rs` also gets a stop-outcome-to-message function; `views.rs` gets a JSON-script-escape function), each called from the existing production code path unchanged, each covered by a new `#[cfg(test)]` module.

**Security / Permissions** *(mandatory, high-risk)* — this narrows what the daemon serves over HTTP from a registered project root: previously any readable file (including `.env`, `.git/config`, private keys); after this change, only files matching the 9-extension allowlist and not inside an excluded directory. No new attack surface is introduced; this is a pure restriction. Auth/CSRF (whether *any* client can reach even the allowed files) remains out of scope, per review P2s already in the backlog. **Validating's security persona identified one load-bearing implementation constraint:** the extension check must run on the canonicalized (symlink-resolved) path, never on the raw URL/`rel_path` segment — a symlink named e.g. `pretty.png` pointing at `.env` would otherwise pass an extension check done on the URL while the canonicalized target is the real secret, reopening the exact hole this fix closes. The cell now requires this explicitly, plus a unix-only regression test proving the bypass is closed. A known, accepted residual gap: double-extension files (e.g. `secret.key.png`) still pass the allowlist, since `Path::extension()` only sees the final token — out of scope per CONTEXT.md D1 Agent's Discretion.

## 6. Affected Files

| Action | File / Component | Purpose |
|--------|------------------|---------|
| Modify | `crates/mdview-core/src/engine.rs` | `asset_path` extension allowlist + `exclude_patterns` check (D1, D2) |
| Modify | `crates/mdview/src/cli.rs` | Extract + test `cmd_config_edit`'s editor resolution/TOML validation (D3); extract + test `stop_daemon`'s outcome-to-message mapping (D3) |
| Modify | `crates/mdview/src/views.rs` | Extract + test `file_page`'s `</script>`-breakout escape (D3) |

## 7. Implementation Steps

- [ ] Restrict `asset_path` to a safe extension allowlist + `exclude_patterns` (`security-asset-allowlist-1`)
- [ ] Unit-test `config-edit-cli-1`'s editor resolution and TOML validation (`config-edit-cli-test-1`)
- [ ] Unit-test `copy-as-markdown-2`'s `</script>`-breakout escape (`copy-as-markdown-test-1`)
- [ ] Unit-test `mdview-restart-1`'s stop-outcome message mapping (`mdview-restart-test-1`)

All four are independent (no deps between them); `security-asset-allowlist-1` and the two `cli.rs` test cells overlap on one file, which serializes their execution wave but does not block scope.

## 8. Validation Plan

**Automated** — `cargo test --workspace` (all four cells) → expected: still green (45/45 baseline confirmed this session), plus each cell's new named tests passing. `security-asset-allowlist-1`'s verify was revised during validating from a manual live-curl procedure to a self-contained `#[cfg(test)]` unit test against `asset_path` directly — a stronger, automated proof, and the manual step is no longer required to cap the cell.
**Evidence** — `docs/history/fix-review-p1-findings-260716/reports/validation-slice1.md`: reality gate PASS, feasibility matrix READY (2 items READY WITH CONSTRAINT, both documented/accepted, non-blocking), high-risk persona panel (coherence + feasibility + security) ran 1 iteration — 1 BLOCKER and 5 WARNINGs found and fixed in the same pass, including a load-bearing security fix (extension check must run on the canonicalized path, not the pre-resolution URL segment, or a symlink reopens the original arbitrary-read hole) — cell review PASS, 0 CRITICAL open.

## 9. Risks & Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| Allowlist too narrow, breaks a currently-served asset | Low | Checked during planning: `rg` found no in-app reference to a non-allowlisted extension going through `asset_path`; the app's own static assets (`app.css`/`app.js`/`highlight.css`) are served by separate `include_str!`-backed routes, not this path |
| `exclude_patterns` check applied incorrectly, over- or under-blocks | Low | Mirrors the existing, already-tested `scan_markdown_files` pattern rather than a new implementation |
| Duplicated extension list (`mdview-core` vs. `mdview`'s `content_type()`) drifts over time | Low-Medium | Documented in code comment + CONTEXT.md D1; accepted as a documentation risk, not gated by proof this slice |
| Pure-function extractions (D3) accidentally change printed messages or control flow | Low | Each cell's `prohibitions` explicitly forbid message/behavior changes; `cargo test --workspace` is the backstop |

## 10. Rollback Plan

Revert the commit(s) for the cell(s) in question. Each cell is independent and touches a bounded file set (`engine.rs` for the security fix; `cli.rs`/`views.rs` for the test fixes), so any one can be reverted alone without affecting the others — there is no shared migration, flag, or schema to unwind. Reverting `security-asset-allowlist-1` restores the pre-fix (over-broad) `asset_path` behavior; the three test cells are additive-only (new tests + extracted-but-behavior-identical functions), so reverting them removes coverage but changes no runtime behavior either way.

## 11. Open Questions

No blocking open questions. Ready for review.
