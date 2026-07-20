# Validation report — windows-daemon-fixes, slice 1

## Reality Gate Report

```text
REALITY GATE REPORT
Mode: standard
Current work: fix health_check wildcard-bind detection (D1), extract shared cross-platform
  daemon detach into mdview-core and wire mdview-desktop through it (D3), add a
  windows-latest CI test job (D2)
MODE FIT: PASS       — 2 risk flags (cross-platform, multi-domain across 3 crates + CI);
  matches the standard threshold, not over-ceremony
REPO FIT: PASS        — every cited file:line re-verified live this session:
  crates/mdview-core/src/daemon.rs:58 (health_check), crates/mdview/src/runtime.rs:174
  (is_wildcard), :227 (apply_detach), :329 (its test), crates/mdview-desktop/src/main.rs:119
  (spawn_mdview_serve), .github/workflows/release.yml:36 (windows-latest matrix row),
  root Cargo.toml:48 (libc workspace dependency)
ASSUMPTIONS: PASS     — every blocking assumption listed in the matrix below, none implicit
SMALLER PATH: PASS    — small would have undercounted the cross-crate + platform risk; the
  standard-lane plan-checker + cell-reviewer pass caught 2 real structural gaps a small-lane
  inline check would likely have missed
PROOF SURFACE: PASS   — every cell's verify command confirmed runnable in this environment;
  cargo test --workspace and cargo check --manifest-path crates/mdview-desktop/Cargo.toml
  both re-run live and passed this session
Decision: proceed
Evidence: see Feasibility Matrix below
```

## Feasibility Matrix

| Assumption | Risk | Proof Required | Evidence | Result |
|---|---|---|---|---|
| Rust toolchain usable in this environment | was HIGH | live command output | `cargo test --workspace`: 36/36 pass (re-run live, this session) | RESOLVED |
| `mdview-desktop` compiles (GTK/webkit2gtk present) | was MEDIUM | live command output | `cargo check --manifest-path crates/mdview-desktop/Cargo.toml`: clean, ~1m36s (re-run live) | RESOLVED |
| `libc.workspace = true` resolves correctly for `mdview-core` despite `mdview-desktop` declaring its own nested `[workspace]` | MEDIUM | Cargo dependency-resolution semantics + file inspection | Plan-checker: root `Cargo.toml:48` declares `libc = "0.2"`; `mdview-core` is a root-workspace member; a package resolves `.workspace = true` against its own containing workspace regardless of how it's pulled in elsewhere — the nested desktop workspace never shadows this | READY |
| `apply_detach`'s body has no binary-crate-only dependency blocking the move to `mdview-core` | MEDIUM | full function-body inspection | Plan-checker read `runtime.rs:227-255` in full: only `std::process`, `libc`, `std::os::{unix,windows}::process::CommandExt` — all available in `mdview-core` | READY |
| `health_check`'s success predicate is satisfiable by a simple test listener | MEDIUM | function-body inspection | `daemon.rs:74`: `buf.contains("\"mdview\"") \|\| buf.contains("200 OK")` — a bare non-responding `TcpListener` does NOT satisfy this (found independently by both reviewers); cell 1 patched to require a responder thread writing `HTTP/1.1 200 OK\r\n\r\n` | RESOLVED (cell patched) |
| IPv6 loopback substitution (`"::"` → `"::1"`) produces a parseable `host:port` string | LOW | live `rustc`/`ToSocketAddrs` probe | Plan-checker verified `"::1:7700"` resolves via `ToSocketAddrs` to `[::1]:7700` | RESOLVED |
| Cell 3's `cmd` binding exists for `apply_detach(&mut cmd)` to attach to | was CRITICAL | cold-pickup review | Cell-reviewer: current `spawn_mdview_serve` is a fluent chain with no bound `cmd`; cell patched to specify `let mut cmd = ...; apply_detach(&mut cmd); cmd.spawn()` | RESOLVED (cell patched) |
| Cell dependency graph has no cycles / bad waves | LOW | `cells schedule` | 2 waves (`[1,2,4]`, `[3]`), 0 cycles, 0 unsatisfiable deps | RESOLVED |

## Spikes

None run — no unproven assumption remained blocking after the matrix above; every row resolved to accepted evidence (live command output, live function-body/file inspection, or a live `rustc` probe), never plausibility language.

## Plan-Checker (adversarial, 1 iteration)

PASS with 1 WARNING: cell `windows-daemon-fixes-1`'s test recipe was underspecified (see feasibility matrix row above) — same root cause independently found by the cell reviewer below, fixed once. Confirmed: full D1/D2/D3 → cell coverage, correct dependency DAG, all `must_haves.key_links` diff-verifiable, 4-cell split not arbitrary (D3's extract/wire split is forced by the cross-crate dependency).

## Cell Review (cold pickup)

```text
CELL REVIEW REPORT
Work: windows-daemon-fixes slice 1 (4 cells)
Cells reviewed: 4
CRITICAL FLAGS:
  windows-daemon-fixes-1 — action's test recipe omits the responder thread health_check's
    success predicate requires; literal implementation fails its own assertion.
    Fix applied: action + must_haves.truths now specify the responder thread and exact
    HTTP 200 OK payload.
  windows-daemon-fixes-3 — action assumed a `cmd` variable binding that doesn't exist in
    spawn_mdview_serve's current fluent-chain form.
    Fix applied: action now gives the exact bound-variable rewrite, mirroring
    runtime.rs::spawn_daemon_detached's existing shape.
MINOR FLAGS:
  windows-daemon-fixes-3 — verify (`cargo check --manifest-path crates/mdview-desktop/Cargo.toml`)
    needs Tauri system libs; flagged as an environment risk for whatever host executes it.
    Fix applied: cell now records that this exact environment was live-confirmed to have
    them (build-essential, pkg-config, libwebkit2gtk-4.1-dev, libgtk-3-dev,
    libayatana-appindicator3-dev, librsvg2-dev all installed and re-verified this session),
    and instructs the worker to report as friction (not silently downgrade) if its own
    execution environment differs.
  windows-daemon-fixes-4 — verify only checked for the literal string "windows-latest",
    not that the job actually runs cargo test --workspace.
    Fix applied: verify now also greps for "cargo test --workspace".
CLEAN CELLS: windows-daemon-fixes-2
REVISIONS MADE:
  windows-daemon-fixes-1 — action + must_haves.truths rewritten to require a responder thread
  windows-daemon-fixes-3 — action rewritten with the exact cmd-binding code shape; verify
    feasibility note added
  windows-daemon-fixes-4 — verify tightened with a second grep condition
SUMMARY: Two independent reviewers (cell-reviewer, plan-checker) converged on the same
  cell-1 root cause from different angles, which cross-validates the finding. Both CRITICALs
  and both MINORs are fixed. mdview-core::process::apply_detach extraction verified sound
  by direct function-body and dependency-resolution inspection. No open concerns remain.
```

## Approval Block

```text
VALIDATION COMPLETE - APPROVAL REQUIRED BEFORE EXECUTION
Mode: standard
Work: windows-daemon-fixes, current slice (4 cells: health_check fix, detach extraction,
  desktop wiring, Windows CI job)
Reality gate: PASS
Feasibility: READY
Structure: PASS after 1 iteration
Spikes: none needed
Cell review: PASS (4 cells, 0 CRITICAL open)
Unresolved concerns: none
```
