# hostname-port-truth — validation report (current slice: all 3 cells)

## Reality gate

| Check | Result | Evidence |
|---|---|---|
| MODE FIT | PASS | plan.md counts 3 risk flags (data model, existing covered behavior, weak proof around area), no hard-gate flag → standard. Matches mode-gate table. |
| REPO FIT | PASS | Every cited function/line verified by direct read this session: `ServerConfig.host_name` config.rs:26; `serve()` writes `addr.port()` to lock server.rs:48-55; `ensure_bind()` timeout fallback runtime.rs:129-130; `mcp.rs::handle_tool_call` already calls `ensure_daemon_bases()` mcp.rs:106; `cli.rs::cmd_open`/`cmd_restart` call single-URL `ensure_daemon_base()` cli.rs:255,452; `daemon.rs::read_lock()` is a pure file read fully decoupled from `health_check()`/`running_daemon()` (daemon.rs:37-55). |
| ASSUMPTIONS | PASS | D2's core assumption ("read_lock() works without a live health check") verified directly against daemon.rs source — `read_lock()` never calls `health_check`. |
| SMALLER PATH | PASS | Considered splitting into 3 separate features (one per PBI) — rejected per CONTEXT.md: they share files (config.rs, runtime.rs, server.rs, cli.rs), and prior decisions already anticipated one combined session. One feature, 3 sequential cells, is the smaller-friction path, not a bigger one. |
| PROOF SURFACE | PASS | Spiked the exact e2e invocation cli-mcp-url-parity-1 needs (see below) — real binary, real daemon, real JSON output, confirmed working end-to-end and cleaned up. |

## Spike: e2e CLI invocation proof surface

Question: does the documented "Rust CLI E2E testing in this repo" recipe (critical-patterns.md) still work for `mdview open --json` against a real daemon?

Ran (scratch dir, fake HOME, real RUSTUP_HOME/CARGO_HOME):
```
mdview open project/test.md --json
{"project_id":"project","url":"http://127.0.0.1:7700/p/project/test.md"}
```
Confirmed current (pre-change) JSON shape is `{project_id, url}` only — baseline for the "add urls array, keep url" back-compat requirement in D3. Daemon stopped and scratch cleaned after the spike (`mdview stop`, `rm -rf`).

**Result: YES** — recipe works, e2e coverage for cli-mcp-url-parity-1 is achievable exactly as planned.

## Feasibility matrix

| Assumption | Risk | Proof required | Evidence | Result |
|---|---|---|---|---|
| serde alias lets old `host_name` config.toml load into new `hostname` field | LOW | serde docs / existing pattern in repo | `#[serde(alias = "...")]` is a standard serde attribute; `Config`/`ServerConfig` already use `#[serde(default)]` throughout (config.rs:9,19) so the struct is already alias-friendly (no `deny_unknown_fields`) | PASS |
| `ensure_bind()`'s timeout fallback can be fixed to prefer `read_lock()` without needing a live 2s-sleep test | MEDIUM | source inspection of `daemon.rs` | `read_lock()` (daemon.rs:37-40) is a 4-line pure file read + JSON parse, zero coupling to `health_check`/`running_daemon` | PASS |
| MCP path already implements PBI-14's multi-IP requirement (no MCP-side change needed) | LOW | source inspection | `mcp.rs:106` already calls `ensure_daemon_bases()`; `build_display_urls` already unit-tested (runtime.rs tests) | PASS |
| CLI `open`/`restart` do NOT yet have multi-IP parity (real gap, not just a missing test) | LOW | source inspection | `cli.rs:255,452` call the single-URL `ensure_daemon_base()`, confirmed by grep + read | PASS |
| e2e CLI test is executable in this repo without hitting the scout hook | LOW | live command | Spiked above — works | PASS |
| 3-cell dependency chain has no cycles, matches real code dependency (rename must land before code that reads the renamed field compiles) | LOW | `bee cells schedule` | Wave 1: hostname-rename-1, Wave 2: bound-port-truth-1, Wave 3: cli-mcp-url-parity-1 — 3 waves, 0 cycles | PASS |

## Plan-checker & cell-reviewer

Dispatched in background (review-tier subagents, `opus`, read-only):
- `plan-checker-hostname-port-truth` — 5-dimension structural check.
- `cell-reviewer-hostname-port-truth` — cold-pickup review of all 3 cells.

### Plan-checker findings (1 BLOCKER, 4 WARNING)

- **BLOCKER (cell completeness, `bound-port-truth-1`)** — the cell's
  no-2s-sleep test prohibition was unachievable as written; the action never
  authorized extracting a pure fallback helper, which is the only way to test
  without a real 2s poll or writing the real global lock file. **FIXED**:
  `bound-port-truth-1` action/must_haves now explicitly require extracting
  `bind_fallback(lock: Option<DaemonInfo>, cfg: &Config) -> (String, u16)`
  and unit-testing it with in-memory values; a prohibition now bars writing
  the real `~/.mdview/daemon.lock` from any test.
- **WARNING** — D4's stated rationale ("D2 reads the renamed field") was
  factually wrong: `ensure_bind()` reads `cfg.server.host`/`port`
  (connectivity), never the display `hostname` field. **FIXED**: CONTEXT.md
  D4 corrected — ordering is now justified by shared-file edit locality, not
  a semantic dependency.
- **WARNING** — CONTEXT claimed D3's e2e verifies D2's fix; it doesn't (the
  e2e happy-path never hits the timeout-fallback branch). **FIXED**: CONTEXT.md
  D4 now states this explicitly — D2 is proven by its own unit tests, not
  the e2e.
- **WARNING (stale-lock semantics)** — a lock found post-timeout may belong
  to a dead daemon, not a warming-up one; the port is a display heuristic,
  not a stronger liveness guarantee. **FIXED**: added as an explicit
  prohibition/note in `bound-port-truth-1`'s must_haves so the cap can't
  overclaim connectivity proof.
- No blockers on D1 rename coverage, key links, decision→cell mapping, or
  scope sanity — plan-checker confirmed these directly against source.

### Cell-reviewer findings (2 CRITICAL, 3 MINOR)

- **CRITICAL (`bound-port-truth-1`)** — same root cause as the plan-checker's
  BLOCKER, found independently (no testable seam without extraction).
  **FIXED** by the same patch above.
- **CRITICAL (`cli-mcp-url-parity-1`)** — the e2e truth (real daemon, real
  port assertion) was unverifiable via `verify: cargo test --workspace`, and
  neither `action` nor `read_first` routed a cold worker to an automatable
  path — they'd either try the raw compiled-binary path directly (blocked by
  the scout hook) or silently skip the e2e truth under a plain test run.
  **FIXED**: `cli-mcp-url-parity-1`'s action now specifies a real
  `crates/mdview/tests/e2e_open.rs` cargo integration test using
  `env!("CARGO_BIN_EXE_mdview")` (valid — `crates/mdview/Cargo.toml` defines
  `[[bin]] name = "mdview"` in the same package) — a genuine automated test
  that `cargo test --workspace` runs and can fail on, no manual recipe or
  human-driven shell step required.
- **MINOR** (`hostname-rename-1`) — the alias-load truth wasn't tied to a
  required artifact/test. Already covered: the cell's `must_haves.truths`
  already names the exact alias-load assertion as a truth; noted, no change
  needed.
- **MINOR** (`hostname-rename-1`) — runtime.rs doc-comments/tests also carry
  the old identifier; already covered by the existing "no remaining raw
  host_name identifier" prohibition. Noted, no change needed.
- **MINOR** (`cli-mcp-url-parity-1`) — parity claim (mcp.rs already correct,
  cli.rs needs the switch) independently confirmed correct by both reviewers.

## Verdict

**READY WITH CONSTRAINTS** — both BLOCKER/CRITICAL findings were fixed in the
cell definitions and CONTEXT.md before this verdict; the 4 WARNING/MINOR
items are either fixed or recorded as non-blocking notes. `bee cells
schedule` after the patch still reports 3 waves, 0 cycles (patches only
touched `action`/`must_haves`, not `deps`).

## Gate 3

`gate_bypass_level: normal` covers `standard` non-hard-gate work (no auth,
authorization, data-loss, audit/security, external-provider, or
validation-removal flag present). Per the routing contract, Gate 3 is
self-approved: no human question asked.
