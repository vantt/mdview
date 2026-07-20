# Windows daemon fixes — Context

**Feature slug:** windows-daemon-fixes
**Date:** 2026-07-19
**Exploring session:** complete
**Scope:** Standard
**Domain types:** RUN

## Feature Boundary

Fix two known P2 bugs (from `docs/history/review-2026-07-16-full-app/reports/`) that
break daemon liveness detection and process detachment on Windows/macOS, and add a
`windows-latest` CI test job so the affected `#[cfg(windows)]` code paths get real
automated coverage going forward. Ends at: `health_check` correctly detects a
wildcard-bound daemon, `mdview-desktop`'s spawned daemon is properly detached on every
platform, and `cargo test --workspace` runs on Windows in CI. Does not include the
Windows installer gap (no native `.ps1`/`.bat`, `install.sh` rejects non-Linux/Darwin)
or the separate `restart`-reliability finding — both deferred (see below).

## Locked Decisions

| ID | Decision | Rationale (only if it changes implementation) |
|----|----------|-----------------------------------------------|
| D1 | `health_check`/`running_daemon` (`crates/mdview-core/src/daemon.rs`) substitute a wildcard bind host (`0.0.0.0` / `::`) with loopback (`127.0.0.1` / `::1`) at the `TcpStream::connect` call site only, on every platform — no `cfg(platform)` branching. | Wildcard-bound connect is rejected on macOS/Windows (`WSAEADDRNOTAVAIL`), so a live daemon reads as dead and a duplicate gets spawned. Fix targets only the connect call, never `DaemonInfo.host` itself, per the existing display-vs-functional critical pattern. This explicitly **supersedes** `multi-ip-urls-1`'s (PBI-04) prior `must_haves` constraint against touching `health_check` — that lock protected an unrelated display-URL feature, not this connectivity fix. No standalone cell/decision record for the original lock survives in `.bee/`; it is anchored here by the two independent review citations that confirm it existed and why: `findings-code-quality.md:5` ("locked in unaddressed") and `:57` ("`multi-ip-urls-1`'s `must_haves` prohibit touching `health_check`"). |
| D2 | Add a `windows-latest` job to `.github/workflows/ci.yml` running `cargo test --workspace`, in scope for this feature. | Both bugs live inside `#[cfg(windows)]`/cross-platform paths `ci.yml` has never tested (`release.yml` only `cargo build`'s the Windows target); shipping the fix without CI coverage leaves it as unverified as the bug it replaces. |
| D3 | `mdview-desktop`'s `spawn_mdview_serve()` (`crates/mdview-desktop/src/main.rs`) gains the same cross-platform detach guarantee as `runtime.rs::spawn_daemon_detached` (Unix `setsid` + Windows `DETACHED_PROCESS \| CREATE_NEW_PROCESS_GROUP`) — parity on both platforms, not a Windows-only patch. | `findings-architecture.md`'s P2 says the desktop spawn has no detach flags on any platform today. |

### Agent's Discretion

- D3's *how* — duplicate the `cfg` blocks directly in `mdview-desktop/src/main.rs`, or
  extract a shared helper into `mdview-core` so both crates call the same function — is
  left to planning. The architecture finding suggests extraction (spawn/detach belongs
  one layer down, next to `daemon.rs`'s lock/health), but this is an implementation
  choice, not a locked product decision.
- Exact CI job shape for D2 (matrix entry vs standalone job, whether to also add
  `cargo clippy` on Windows) is left to planning.

## Existing Code Context

### Reusable Assets

- `crates/mdview/src/runtime.rs::apply_detach` (L227-255) — the working cross-platform
  detach implementation (`libc::setsid()` on Unix, `creation_flags` on Windows). D3's
  fix should reuse or mirror this exactly, not reinvent it.
- `crates/mdview/src/cli.rs::stop_daemon` (L404-428) — already has a working
  `#[cfg(unix)]`/`#[cfg(not(unix))]` split (`kill` vs `taskkill /PID … /F`) as a
  reference pattern for platform-conditional process control in this repo.
- `crates/mdview/src/runtime.rs::is_wildcard` (L174) — existing wildcard-bind-host
  detection D1 needs, but it is **private in the binary crate**; `mdview-core` cannot
  depend on `mdview` (dependency runs the other way). D1's fix must lift or reimplement
  this check inside `mdview-core` — `findings-code-quality.md:63` names this exact move
  ("is_wildcard already exists in runtime.rs; lift to core").

### Established Patterns

- Display-value vs functional-value separation (`docs/history/learnings/critical-patterns.md`
  entry 1) — the exact pattern D1's fix must follow: substitute only at the connectivity
  read site, never touch the shared `DaemonInfo.host` field.
- `crates/mdview-desktop/src/main.rs` duplicates daemon-spawn logic from `runtime.rs`
  and is NOT shared code (critical-patterns.md entry 2, already flags this exact gap
  as a known drift) — any fix to one must be checked against the other.

### Integration Points

- `crates/mdview-core/src/daemon.rs` — `health_check(host, port)` (L58), called by
  `running_daemon()` (L48). D1's change point.
- `crates/mdview-desktop/src/main.rs` — `ensure_daemon()` (L104), `spawn_mdview_serve()`
  (L119-127). D3's change point.
- `.github/workflows/ci.yml` — current single `test` job on `ubuntu-latest`. D2's change
  point.

## Canonical References

- `docs/history/review-2026-07-16-full-app/reports/findings-code-quality.md` — P2,
  the `health_check`/wildcard-bind finding (D1's source).
- `docs/history/review-2026-07-16-full-app/reports/findings-architecture.md` — P2,
  the desktop spawn/detach-duplication finding (D3's source).
- `docs/history/review-2026-07-16-full-app/reports/findings-test-coverage.md` — P3,
  notes the Windows detach branch is "compile-guarded, never run in CI" (D2's source).
- `docs/history/learnings/critical-patterns.md` — entries 1 and 2, both directly
  govern this feature's fix shape.

## Outstanding Questions

### Deferred To Planning

- [ ] Extract-vs-duplicate for D3 (see Agent's Discretion).
- [ ] Exact `ci.yml` matrix shape for D2.
- [ ] How to prove D1/D3 in an automated test (e.g. `#[test]` binding a real
      `0.0.0.0` listener and asserting `health_check` finds it, following the existing
      `apply_detach_puts_child_in_its_own_session` test style for D3) vs. what stays a
      manual/documented characterization, per the "detachment must be proven, not
      assumed" critical pattern.

## Deferred Ideas

- Native Windows installer (`install.ps1`/`.bat`) — `install.sh` currently rejects any
  OS other than Linux/Darwin outright. Real gap, but a distinct feature from these 2
  runtime bugs. → added to `docs/backlog.md` as a `proposed` row.
- `findings-reliability.md`'s `stop_daemon`/`restart` pid-reuse race (kill-by-pid
  without a liveness recheck before respawn) — cross-platform reliability issue
  mentioned in the same review pass, but the user's "known issues" ask was scoped to
  the 2 P2 findings above; not pulled in here.
- Desktop crate (Tauri) has no release/packaging pipeline at all today (`release.yml`
  only builds the CLI); PRD's NFR-04 Windows `.exe`/MSI desktop bundle is aspirational,
  not implemented. Out of scope for a bug-fix feature.

## Handoff Note

CONTEXT.md is the source of truth. Decision IDs (D1-D3) are stable. Planning reads
locked decisions, code context, canonical references, and deferred-to-planning
questions. Validating and reviewing use locked decisions for coverage and UAT.
