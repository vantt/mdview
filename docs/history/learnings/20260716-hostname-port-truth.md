---
date: 2026-07-16
feature: hostname-port-truth
categories: [testing, validation-process, code-review]
severity: medium
tags: [bee-validating, bee-swarming, e2e-testing, pure-function-extraction, goal-check]
---

# Learnings: hostname-port-truth (PBI-10/13/14 combined)

## What Happened

Three related backlog items (rename a config field with back-compat, fix a
port-truth reliability gap, add CLI/MCP URL-building parity + e2e coverage)
were combined into one feature and shaped into 3 sequential cells. Two
independent review-tier subagents (plan-checker, cell-reviewer), dispatched
during validating with no cross-talk, both found the same real defect in the
`bound-port-truth-1` cell from different angles, plus the cell-reviewer found
a second defect in `cli-mcp-url-parity-1` that the plan-checker's dimensions
didn't cover. Both were fixed in the cell definitions before execution, and
execution then went cleanly except for one rustfmt drift caught only by the
orchestrator's own goal-check, not by the worker's own verify run.

## Root Cause

1. **Timing-dependent behavior described as testable without naming the
   testing mechanism.** The plan/cell said "prove this without a live 2s
   sleep, use a fixture" but never said the fallback logic first needed
   extracting into a pure function — the only shape a cold worker could
   actually satisfy that constraint with. A cell that assumes a refactor step
   without stating it is unexecutable as written, even when the *intent* is
   correct.
2. **A documented testing recipe was reused outside the context it was
   written for.** `critical-patterns.md`'s manual shell E2E recipe
   (scratch dir + `HOME`/`RUSTUP_HOME`/`CARGO_HOME`) is written for an agent
   driving a command by hand during exploration/validation — it was never
   meant to be `verify`'d by `cargo test --workspace`. A cell that needs
   automated, CI-safe e2e coverage of a binary in this repo has a different
   answer: `env!("CARGO_BIN_EXE_mdview")` inside a real `#[test]`, which
   `cargo test` runs and can fail on natively.
3. **`cargo test --workspace` (the recorded `verify` command) does not check
   formatting.** A worker whose own verify is green can still leave the repo
   rustfmt-dirty; only a dispatch prompt that explicitly told the worker to
   self-check `fmt`/`clippy` before reporting done caught it on the first
   pass (`cli-mcp-url-parity-1`) — the other two cells' dispatch prompts
   didn't say so, and one of them (`bound-port-truth-1`) needed a rescue
   round purely for formatting.

## Recommendation

- When a cell's `must_haves` forbids a live-timing test for a
  polling/timeout code path, the cell's `action` must explicitly authorize
  extracting the decision into a pure, parameter-injected helper function —
  don't leave a cold worker to infer that a refactor is the only way to
  satisfy the prohibition.
- When a cell needs genuine end-to-end coverage of a compiled binary in this
  repo and the coverage must be automated (not human-run), point it at
  `env!("CARGO_BIN_EXE_<bin-name>")` inside a `#[test]` in a `tests/`
  integration file, not at the manual shell E2E recipe in
  `critical-patterns.md` — reserve that recipe for an agent interactively
  probing behavior (e.g., a validating-phase spike).
- Every worker dispatch prompt for a Rust cell in this repo should state
  up front: "run `cargo fmt --all --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` yourself before
  reporting done" — `cargo test --workspace` passing is not sufficient
  evidence of a clean cap, and catching format drift only at the
  orchestrator's goal-check costs a full rescue round.
- Removing now-dead code outside a cell's declared `files` scope is
  acceptable during execution *only* when it is a direct, mechanical
  consequence of the in-scope change colliding with a hard lint gate
  (here: `-D warnings` dead_code) — not a redesign, not a "while I'm here"
  cleanup. The worker must disclose it as a deviation in the cap trace
  (as happened here) so the orchestrator can goal-check it explicitly.
