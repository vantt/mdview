---
date: 2026-07-15
feature: mdview-hostname-doctor-fix
categories: [design-pattern, code-duplication, testing]
severity: [medium, medium, low]
tags: [config, daemon, url-building, doctor, e2e-testing]
---

# Learnings: mdview host_name setting + doctor --fix agent-instruction sync

## What Happened

Small-lane feature (solo execution, gate-bypass=normal): added an optional
`server.host_name` display override that substitutes into rendered view URLs
(CLI `open` + MCP `mdview_view_file`) without touching the daemon's actual
bind/health-check address, and changed `doctor --fix` to write the
agent-instruction snippet into both AGENTS.md and CLAUDE.md when missing
(previously it only warned, never wrote). Both cells capped clean; specs for
`settings` and `doctor` areas created from scratch (none existed before).

## Root Cause / Key Findings

1. **Display value vs. functional value sharing one field.**
   `DaemonInfo.host` (`crates/mdview-core/src/daemon.rs`) is read for two
   different purposes: `health_check`/`running_daemon` use it to actually
   open a TCP connection, while `base_url()` uses it to build the string
   shown to a user/agent. Substituting `host_name` directly into
   `DaemonInfo.host` would have silently broken daemon-liveness detection the
   moment `host_name` didn't resolve to the same machine. The fix intercepts
   only at the URL-building read sites (`runtime.rs::ensure_daemon_base` /
   `display_base_url`), leaving the connectivity-purpose field untouched.

2. **`mdview-desktop` duplicates `runtime.rs`'s daemon logic, not shares it.**
   `crates/mdview-desktop/src/main.rs::ensure_daemon()` is a separate,
   hand-copied version of `ensure_daemon_base()` — same shape, same
   `format!("http://{}:{}", cfg.server.host, cfg.server.port)` fallback, but a
   distinct function in a distinct crate. This feature deliberately left it
   unchanged (its URL feeds the desktop app's own embedded webview — a
   connectivity address, not a link shown externally) but the duplication
   itself is a landmine: any future change to daemon-URL-building logic in
   `runtime.rs` will NOT automatically apply to the desktop shell.

3. **`doctor.rs`'s `check_config` is the one check not gated by `--fix`.**
   Every other check (MCP registration, agent instruction) only writes when
   `fix && !dry_run`. `check_config(dry_run)` writes a default config file
   whenever `!dry_run`, regardless of `--fix`. Pre-existing asymmetry (not
   introduced by this feature), now documented in `docs/specs/doctor.md` R2 —
   easy to assume uniform gating and get it wrong in a future doctor change.

4. **The `cargo run --manifest-path <repo>/Cargo.toml --bin mdview -- <args>`
   E2E pattern, invoked while `cd`'d into a scratch test directory, correctly
   runs the binary with that scratch directory as cwd** — verified again
   here for `doctor`'s cwd-relative AGENTS.md/CLAUDE.md read/write. This
   avoids the `target/`-path scout-hook block and the `HOME`-override
   rustup breakage that a bare `./target/debug/mdview` invocation hits.

## Recommendation

- When adding a display-only override for a value that is also used for real
  connectivity (bind address, health check, auth), substitute at the specific
  read site that builds the *displayed* string — never at the shared
  underlying field. Verify by confirming the connectivity path (health check,
  bind, actual request) is unaffected, not just that the display path changed.
- Before changing daemon/URL-building logic in `crates/mdview/src/runtime.rs`,
  grep `crates/mdview-desktop/src/main.rs` for the same shape — it is
  duplicated, not shared, and will silently drift if only one side is updated.
- In `mdview doctor`, don't assume every check's write is gated by `--fix`;
  read each check's own signature (`check_config` differs from the other two)
  before adding a new fix or changing gating.
- For Rust CLI E2E tests in this repo, use
  `cd <scratch-dir> && HOME=<fake> RUSTUP_HOME=... CARGO_HOME=... cargo run -q --manifest-path <repo>/Cargo.toml --bin mdview -- <args>` —
  never touch `target/` paths directly.
