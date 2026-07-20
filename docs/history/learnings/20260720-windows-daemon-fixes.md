---
date: 2026-07-20
feature: windows-daemon-fixes
categories: [pattern, decision, failure]
severity: critical
tags: [cross-platform, daemon, cell-authoring, decision-provenance, environment-setup]
---

# Learning: windows-daemon-fixes — extraction pattern, cell-authoring gap, decision provenance

**Category:** pattern, decision, failure
**Severity:** critical (3 findings below cleared promotion; see critical-patterns.md)
**Tags:** [cross-platform, daemon, cell-authoring, decision-provenance, environment-setup]
**Applicable-when:** any future feature splitting platform-conditional logic across crates, writing a cell with a test recipe, or superseding an old locked decision.

## What Happened

`windows-daemon-fixes` fixed two known Windows P2 bugs (health_check misdetecting a wildcard-bound daemon as dead; `mdview-desktop`'s spawned daemon not detaching) and added a `windows-latest` CI job. All 4 cells capped clean with independently re-run verify evidence. The interesting events all happened before execution, not during it.

## Root Cause / Findings

**1. Extraction vs. duplication is decided by target-crate CI coverage, not by "is this shared logic."** D1 (`is_wildcard`, a 2-line predicate) was correctly *duplicated* into `mdview-core/src/daemon.rs` rather than shared with `runtime.rs`'s existing copy, because `runtime.rs` already has test coverage and mdview-core cannot depend on the binary crate anyway. D3 (`apply_detach`) was correctly *extracted* into `mdview-core/src/process.rs` (re-exported from `runtime.rs` via `pub(crate) use`, so the existing `apply_detach_puts_child_in_its_own_session` test needed zero changes) specifically because `mdview-desktop` has zero compile coverage anywhere in this repo's CI — writing the detach logic directly in `main.rs` would have made it permanently unverifiable. Same shape of decision, opposite correct answers; the discriminator is coverage of the *target* crate, not "does this look like duplicate code."

**2. A cell's test recipe must be checked against the actual function body, not just a decision-level description.** `windows-daemon-fixes-1`'s first draft (lifted from CONTEXT.md's Outstanding-Question phrasing, "bind a real `0.0.0.0` listener and assert `health_check` finds it") instructed binding a bare `TcpListener` and asserting `health_check` returns true. `health_check`'s real success predicate (`daemon.rs:74`) requires an actual HTTP response body (`buf.contains("\"mdview\"") || buf.contains("200 OK")`) — a listener that never `accept()`s/responds leaves the buffer empty and the function returns `false`, so the recipe as drafted could never pass. Two independent validating-phase subagents (plan-checker, cell-reviewer) converged on the identical root cause from different angles in the same pass — a strong seen-twice signal.

**3. Superseding an old locked decision whose original record has been pruned leaves a lossy citation chain.** D1 explicitly supersedes `multi-ip-urls-1` (PBI-04)'s prior `must_haves` prohibition on touching `health_check`. `docs/history/multi-ip-urls-1/` no longer exists, and no `.bee` decision-log entry records the *original* lock — the only surviving anchor is prose in another feature's review findings report (`findings-code-quality.md`) plus a backlog row with no `[history](...)` link. The supersession itself is well-reasoned and correctly scoped (verified against the PBI-04 backlog row's own text), but a future agent re-deriving "what exactly was locked and why" has only a paraphrase to work from, not the original constraint.

**4. Environment assumptions (Rust toolchain, `mdview-desktop`'s GTK/webkit2gtk system libs) were both missing at session start and discovered late, during planning's discovery pass rather than at session/feature start.** The recorded baseline gate (`commands.verify` in `.bee/config.json`) exists precisely to catch this, but AGENTS.md's trigger for it is "before claiming any cell," not "before Gate 1" — so CONTEXT.md got locked and planning got underway before the environment was actually known to be capable of running the work. Also: `mdview-desktop` is excluded from the root Cargo workspace by design, so even a perfectly-timed `cargo test --workspace` baseline run would never have caught its separate system-lib gap — that needed its own `cargo check --manifest-path crates/mdview-desktop/Cargo.toml` probe.

**5. A background subagent's result was flagged by the harness as containing an "instruction-shaped pattern (settings-json)" and neutralized, with no trace surviving in any `.bee/` artifact.** Investigation found no evidence of anything actually wrong with the flagged cell's work (a clean D3 extraction, no JSON touched). The most plausible explanation: the cell's verify command (`cargo test --workspace`) sweeps in an unrelated pre-existing test suite (`crates/mdview/src/doctor.rs::mcp_register_tests`) whose fixtures contain literal JSON strings shaped like agent-settings/MCP-registration files (`{"mcpServers": {...}}`, `{"command": ..., "args": [...]}`) — plausibly enough to trip a pattern scanner watching worker output, despite having zero connection to the actual change. Recorded here as a single occurrence, not promoted — worth a second look if it recurs.

## Recommendation

- When platform-conditional (or otherwise environment-sensitive) logic needs to exist in two crates, check whether the *target* crate has CI/compile coverage before choosing extract-vs-duplicate. No coverage → extract to the nearest covered crate, re-exporting at the original call site so existing tests need no changes (mirror `runtime.rs`'s `pub(crate) use mdview_core::process::apply_detach;` seam). Has coverage already → duplicating a small, pure predicate is fine and lower-risk than adding a cross-crate dependency.
- When drafting a cell whose action includes a test recipe, read the actual body of the function under test and state its real success/failure predicate in the action text — not just the test's setup shape. Do this at authoring time, not only as something validating's reviewers are expected to catch.
- When a decision supersedes an older lock whose original CONTEXT.md/plan.md/decision-log entry no longer exists, say so explicitly in the new decision's rationale (cite whatever secondary source establishes the old lock existed) rather than presenting the supersession as though the original were still directly checkable.
- Run the configured baseline verify (`commands.verify`) before Gate 1, not only before claiming a cell — and when a feature's CONTEXT.md names a workspace-excluded crate as an integration point, add that crate's own `cargo check`/`cargo build` as a supplementary baseline probe in the same pass, since `cargo test --workspace` never reaches it.
