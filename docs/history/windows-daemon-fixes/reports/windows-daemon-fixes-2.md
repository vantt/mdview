# windows-daemon-fixes-2

[DONE] — Extracted `apply_detach` (Unix `setsid` / Windows `DETACHED_PROCESS|CREATE_NEW_PROCESS_GROUP`) out of `crates/mdview/src/runtime.rs` into a new `crates/mdview-core/src/process.rs`, verbatim body and SAFETY comment preserved. `runtime.rs` now re-exports it via `pub(crate) use mdview_core::process::apply_detach;`; the existing `apply_detach_puts_child_in_its_own_session` test was not touched and still passes.

Files touched:
- `crates/mdview-core/src/process.rs` (new)
- `crates/mdview-core/src/lib.rs` (`pub mod process;`)
- `crates/mdview-core/Cargo.toml` (`libc.workspace = true`)
- `crates/mdview/src/runtime.rs` (local fn replaced with re-export)
- `Cargo.lock` (libc now a dep edge of mdview-core)

Verify: `cargo test --workspace` — all passed (49 in `mdview` bin crate incl. `apply_detach_puts_child_in_its_own_session`, 37 in `mdview-core`, 1 e2e).

Commit: `dd4bc64` (cell id in message).

Full trace/evidence: `.bee/cells/windows-daemon-fixes-2.json`.
