# bound-port-truth-1 — report

**Status:** [DONE]

**Outcome:** `ensure_bind()`'s timeout fallback now prefers the daemon
lock's real bound port over the configured one, via a new pure
`bind_fallback(lock, cfg)` helper in `crates/mdview/src/runtime.rs`,
unit-tested with in-memory `DaemonInfo`/`Config` values.

**Files touched:** `crates/mdview/src/runtime.rs`

**Commits:**
- `7e697fe` — fix(bound-port-truth-1): ensure_bind() timeout fallback prefers the real bound port
- `80979c0` — style(bound-port-truth-1): rustfmt the bind_fallback tests and import list (goal-check caught an unformatted import list + assert_eq! call; rescued)

**Verify:** `cargo fmt --all --check && cargo test --workspace` — fmt
clean, 74 passed, 0 failed (see `.bee/cells/bound-port-truth-1.json` for
full trace, verify output, and verification evidence).

Full trace and evidence: `.bee/cells/bound-port-truth-1.json`
