---
artifact_contract: bee-plan/v1
mode: standard
approved_gate2: 2026-07-20
---

# Plan: windows-daemon-fixes

Mode: `standard` — 2 risk flags: cross-platform, multi-domain (mdview-core + mdview binary + mdview-desktop + CI infra).
Why this is the least workflow that protects the work: two concrete, already-diagnosed bugs plus a CI gap — no gray areas left after exploring, but the fix spans 3 crates and touches process-spawn/network code with real platform-specific failure modes, which is more than a `small` fix should carry without a plan-checker/cell-reviewer pass.

## Requirements (from CONTEXT.md)

- D1: `health_check`/`running_daemon` substitute a wildcard bind host (`0.0.0.0`/`::`) with loopback (`127.0.0.1`/`::1`) at the `TcpStream::connect` call site only, on every platform.
- D2: add a `windows-latest` job to `.github/workflows/ci.yml` running `cargo test --workspace`.
- D3: `mdview-desktop`'s daemon spawn gains the same cross-platform detach guarantee as `runtime.rs::spawn_daemon_detached` (Unix `setsid` + Windows `DETACHED_PROCESS|CREATE_NEW_PROCESS_GROUP`).

## Discovery

L1 (quick verify), 3 findings that changed the shape of D3 and the verify strategy:

1. `is_wildcard` already exists at `crates/mdview/src/runtime.rs:174-176` — a trivial 2-line predicate (`matches!(host, "0.0.0.0" | "::" | "[::]")`). Confirmed via `grep`.
2. ~~This session's sandbox has no Rust toolchain~~ — **resolved.** The user installed `rustup`/`cargo` plus `build-essential` (the sandbox was missing a C linker) mid-session. Re-verified live: `cargo test --workspace` passes 36/36 (`mdview-core`; `mdview` binary crate has no unit tests today) in this exact checkout.
3. ~~`mdview-desktop` has zero compile coverage and unknown system-lib availability~~ — **resolved.** After the user also installed `pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`, `cargo check --manifest-path crates/mdview-desktop/Cargo.toml` compiles cleanly (verified live, ~1m36s). It remains true that `crates/mdview-desktop` is excluded from the root Cargo workspace and from every CI/release job (`ci.yml` never touches it, `release.yml` only builds `-p mdview`) — so this proof is only good for *this machine*, not CI — but the D3 approach below (minimize what has to compile inside the desktop crate) still stands on its own merits (mirrors `findings-architecture.md`'s recommendation) even though the original blocking motivation is gone.

## Approach

**D1 — duplicate the tiny predicate, touch nothing else.** Add a private `is_wildcard` copy directly in `crates/mdview-core/src/daemon.rs` (2 lines, same `matches!` pattern as `runtime.rs`) and use it inside `health_check` to pick the dial target. Rejected: importing/sharing with `runtime.rs`'s copy — `mdview-core` cannot depend on the binary crate (wrong dependency direction), and reversing it (making `runtime.rs` call into core) would touch a second, currently-untouched, already-tested file for a 2-line pure predicate with no meaningful drift risk. Duplication here is bounded and cheap; the earlier concern about not-conflating display-vs-functional values (critical-pattern #1) is about `DaemonInfo.host` itself, not this helper.

**D3 — extract `apply_detach` into `mdview-core`, do not duplicate it.** This reverses the "duplicate it in place" instinct that would otherwise apply (mirroring D1): Discovery finding #3 makes duplication the wrong call here specifically, because `mdview-desktop` has no compile coverage anywhere in this repo's automation. If the platform-cfg detach logic is written *inside* `main.rs`, there is no way to unit-test it without compiling the whole Tauri-dependent binary — something neither this sandbox nor CI can currently do. Moving `apply_detach(cmd: &mut Command)` verbatim into a new `crates/mdview-core/src/process.rs` (workspace-covered, no GUI deps) makes it directly unit-testable by `cargo test --workspace` (mirroring the existing `apply_detach_puts_child_in_its_own_session` proof style — "detachment must be proven, not assumed," critical-pattern #3), and reduces `mdview-desktop`'s own change to a single call-before-spawn line — the smallest possible edit to the one crate nothing can currently verify by compiling. `runtime.rs` keeps `apply_detach` working via a `pub(crate) use mdview_core::process::apply_detach;` re-export, so its existing test needs zero changes. This is also exactly what `findings-architecture.md`'s P2 recommended (spawn/detach belongs one layer down, next to `daemon.rs`'s lock/health) — the environment constraint and the architecture finding point the same direction.

**D2 — mirror `release.yml`'s existing matrix entry.** `release.yml` already has a working `windows-latest` / `x86_64-pc-windows-msvc` matrix row; `ci.yml`'s new job copies that OS/target pairing for `cargo test --workspace` (no cross/`cross` tool needed — native runner).

### Risk map

| Component | Risk | Reason | Proof needed |
|---|---|---|---|
| Rust toolchain in this sandbox | ~~HIGH~~ **RESOLVED** | Installed mid-session (rustup + build-essential) | `cargo test --workspace`: 36/36 pass, verified live |
| `mdview-desktop` compiles at all | ~~MEDIUM~~ **RESOLVED** (this machine) | GTK/webkit2gtk installed mid-session | `cargo check --manifest-path crates/mdview-desktop/Cargo.toml`: clean, verified live |
| D1 health_check fix | LOW | Pure logic change, one file, existing test pattern to extend | `cargo test -p mdview-core` |
| D3 extraction (`process.rs` + `runtime.rs` re-export) | LOW | Mechanical move, existing test keeps passing via re-export | `cargo test --workspace` |
| D2 CI YAML | LOW | Copies a pattern that already works in `release.yml` | YAML parses; real confirmation is the next CI run |

## Shape

Single current slice — all three decisions are small enough that no future-slice deferral is warranted. Ordered by dependency, not calendar phases:

| Phase | What changes | Why now | Demo | Unlocks |
|---|---|---|---|---|
| 1 | D1: `health_check` wildcard→loopback fix in `crates/mdview-core/src/daemon.rs` | No dependency on anything else; smallest, lowest-risk fix first | `cargo test -p mdview-core` green, new test proves a `0.0.0.0`-passed host reaches a real `127.0.0.1` listener | Independently shippable |
| 2 | D3a: extract `apply_detach` into `crates/mdview-core/src/process.rs`, re-export from `runtime.rs` | Must exist before the desktop crate can call it | `cargo test --workspace` green, existing `apply_detach_puts_child_in_its_own_session` test unchanged and passing | Phase 3 |
| 3 | D3b: `crates/mdview-desktop/src/main.rs::spawn_mdview_serve` calls the shared `apply_detach` | Depends on phase 2 existing | `cargo check --manifest-path crates/mdview-desktop/Cargo.toml` — confirmed runnable in this environment | Independently shippable |
| 4 | D2: `windows-latest` job in `.github/workflows/ci.yml` | Independent of 1-3; can land any time, but placed last since it verifies the others going forward | Next push runs `cargo test --workspace` on Windows | Ongoing regression coverage for this whole feature |

## Test matrix

One pass over all 12 dimensions, most N/A for an infra/reliability fix:

1. **User types** — N/A (no user-facing surface; daemon lifecycle only).
2. **Input extremes** — `health_check` input space is just `host: &str`; new test covers `"0.0.0.0"`, `"::"`, and a normal literal host, plus the existing dead-port case.
3. **Timing** — not touched (no new polling/timeout logic).
4. **Scale** — N/A.
5. **State transitions** — daemon alive→dead→respawned is exactly what D1 fixes; must_haves assert the duplicate-spawn symptom is gone.
6. **Environment** — the whole point of this feature: Linux/macOS/Windows dial behavior, GTK/webkit2gtk availability for the desktop crate, CI runner OS. Covered by the risk map above.
7. **Error cascades** — `health_check` already fails closed (`false`) on any connect error; unaffected by this change.
8. **Authorization** — N/A.
9. **Data integrity** — N/A (no persisted data touched).
10. **Integration** — `mdview-desktop` ← `mdview-core::process::apply_detach` is the one new integration point; covered by phase 3's must_haves (call site present and ordered before `.spawn()`).
11. **Compliance** — N/A.
12. **Business logic** — N/A.

## Out of scope

- Native Windows installer (`install.ps1`) — deferred to `docs/backlog.md` PBI-18 (proposed).
- `findings-reliability.md`'s `stop_daemon`/`restart` pid-reuse race — separate issue, not part of the user's "known issues" ask (see CONTEXT.md Deferred Ideas).
- Desktop crate packaging/release pipeline (PRD NFR-04's `.exe`/MSI) — does not exist today in any form; out of scope for a bug-fix feature.
- Adding `mdview-desktop` compilation to CI as a standing job — D2 only locked `cargo test --workspace` (which excludes the desktop crate by design); whether to also compile the desktop crate in CI is a separate, un-locked decision, not silently added here.
