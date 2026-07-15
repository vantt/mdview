---
artifact_contract: bee-plan/v1
artifact_readiness: requirements-only
mode: small
---

# Reliable auto-spawn: fully detach the mdview daemon (no manual `serve`)

## Scope (locked, D-ID 625c69fa)

Make the already-existing daemon auto-spawn survive its spawner's death, so a
developer never needs to run `mdview serve` first.

1. `spawn_daemon_detached()` becomes truly detached: Unix `setsid` (new
   session), Windows `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`.
2. Add `libc` as a Unix-only direct dependency (already in Cargo.lock at
   0.2.186 transitively — no new fetch).
3. Docs: README + usage.md reframe `mdview serve` as optional.

Out of scope (noted, not done): `crates/mdview-desktop/src/main.rs` has its own
duplicate serve-spawn; the desktop process is long-lived so the detach gap is
low-risk there — separate follow-up.

## Mode gate

Flags: **cross-platform** (Unix setsid vs Windows creation flags, conditional
compilation) = 1 · **existing covered behavior** (process-lifecycle of an
existing spawn path) = 1. Two flags, one focused function + a dep + docs → still
within `small` (≤ a handful of files, no gray areas, one direct change). Not
`standard`: there is no multi-domain surface and the change is a single
well-understood function plus its docs.

## Discovery: L0/L1

- **L0** — the fix pattern is the standard Rust idiom for detaching a child:
  `std::os::unix::process::CommandExt::pre_exec` + `libc::setsid()` (Unix),
  `std::os::windows::process::CommandExt::creation_flags` (Windows). No research
  needed; this is how `nohup`/daemonizers work.
- **L1 verify** — `libc` version already resolved in `Cargo.lock` (0.2.186), so
  adding `libc = "0.2"` reuses it; confirmed the current spawn has no
  `setsid`/`process_group`/`pre_exec` anywhere (grep clean).

## Approach

Rewrite `spawn_daemon_detached()` in `crates/mdview/src/runtime.rs`:

```
let mut cmd = Command::new(exe);
cmd.arg("serve").stdin(null).stdout(null).stderr(null);

#[cfg(unix)] // SAFETY: setsid() is async-signal-safe; sole call between fork/exec.
unsafe { cmd.pre_exec(|| { if libc::setsid() == -1 { return Err(last_os_error()); } Ok(()) }); }

#[cfg(windows)]
cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS); // 0x200 | 0x08

cmd.spawn()?;
```

`Cargo.toml`: add libc to `[workspace.dependencies]` (`libc = "0.2"`) and to the
`mdview` crate as a Unix-target dep so Windows builds never pull it:

```
[target.'cfg(unix)'.dependencies]
libc = { workspace = true }
```

Docs: in README quickstart and `docs/usage.md`, note the daemon auto-starts on
the first `open`/MCP call; `mdview serve` is only for pre-starting or custom
host/port.

Risk map:
- `runtime.rs` spawn change — LOW-MEDIUM. Proof: it still auto-spawns and serves
  (functional), AND the spawned daemon is now its own session leader
  (`ps -o pid,sid` → sid == pid), which the old code could not achieve.
- `Cargo.toml` dep — LOW. Proof: `cargo build --workspace` clean, no new lockfile
  churn beyond the direct-dep line.
- docs — LOW.

## Verification

- `cargo build --workspace` — compiles (the `cfg(unix)` branch on Linux).
- `cargo test --workspace` — existing tests still pass (no test regression).
- Functional E2E: `mdview open <file>` in a scratch project auto-spawns and the
  returned URL answers `/health`; `mdview status` shows running.
- Detach proof: capture the auto-spawned daemon's pid, then
  `ps -o pid,ppid,sid,pgid -p <pid>` shows `sid == pid` (it is a session leader —
  the setsid took effect) and a session distinct from the spawning shell. Then
  `mdview stop` cleans it up.

## Test matrix (small-lane depth)

- Daemon already running → `ensure_daemon_base` returns existing, no second
  spawn (unchanged path).
- No daemon → spawn detaches into own session, becomes reachable within the 2s
  poll (unchanged timing, only detachment added).
- Build on non-Unix path not exercised here (no Windows CI), but the
  `cfg(windows)` branch uses only std APIs + integer flags — compile-guarded,
  reviewed by inspection.

## Open questions for validating

None — single-function change with a direct proof; no gray areas.
