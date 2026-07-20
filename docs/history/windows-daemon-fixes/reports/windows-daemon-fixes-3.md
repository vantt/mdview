# windows-daemon-fixes-3

[DONE] — Wired `mdview-desktop`'s `spawn_mdview_serve()` through the shared `mdview_core::process::apply_detach` helper: bound the `Command` to a variable, called `apply_detach(&mut cmd)` before `.spawn()`, matching `runtime.rs::spawn_daemon_detached`'s call shape. Desktop-spawned daemons now get the same cross-platform detach (Unix `setsid`, Windows `DETACHED_PROCESS|CREATE_NEW_PROCESS_GROUP`) as the CLI's daemon. `ensure_daemon`'s control flow was left unchanged.

Files touched:
- `crates/mdview-desktop/src/main.rs` (`spawn_mdview_serve`)

Verify: `cargo check --manifest-path crates/mdview-desktop/Cargo.toml` — clean compile, `Finished` in ~18s.

Commit: `83a17f1` (cell id in message).

Full trace/evidence: `.bee/cells/windows-daemon-fixes-3.json`.
