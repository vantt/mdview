---
artifact_contract: bee-plan/v1
artifact_readiness: requirements-only
mode: standard
---

# hostname-port-truth — plan

Combines PBI-10 (rename `host_name`→`hostname`), PBI-13 (bound-port truth),
PBI-14 (CLI/MCP URL-building parity + e2e verify). See
`docs/history/hostname-port-truth/CONTEXT.md` for the locked decisions
(D1-D4) and verified current-state notes.

## Mode gate

Flags: data model (field rename + back-compat alias) · existing covered
behavior modification (URL builders, port fallback) · weak proof around the
area (no automated coverage of the timeout-fallback path today) = 3 →
**standard**. No hard-gate flag. `gate_bypass_level: normal` covers standard
non-hard-gate work.

## Discovery

L1 (quick verify only — pattern already exists in-repo, decision `d88c028b`
+ `faf29c1a` already built and unit-tested the multi-IP builder). Confirmed
by reading the current source (see CONTEXT "Verified current state"):
`serve()` already writes the real bound port to the lock; the only port-truth
gap is `ensure_bind()`'s 2s-timeout fallback; MCP already calls
`ensure_daemon_bases()` but CLI (`cmd_open`, `cmd_restart`) still calls the
single-URL `ensure_daemon_base()`.

## Approach

Three ordered slices (D4), each touching non-overlapping-enough surface to
verify independently, sharing one feature/session per the "concurrent
host_name/port" grouping already anticipated in prior decisions.

1. **Rename (D1).** `crates/mdview-core/src/config.rs`: field
   `host_name` → `hostname`, add `#[serde(alias = "host_name")]`. Propagate
   through `crates/mdview/src/server.rs` (`SettingsForm`, `update_config`,
   `normalize_host_name`→`normalize_hostname`), `views.rs` (form `name=`,
   template var), `runtime.rs` (`cfg.server.host_name` reads →
   `cfg.server.hostname`). Update the existing round-trip test name/body in
   `config.rs` and the `host_name_form_value_normalizes_blank_to_none` test
   in `server.rs`. Add one new test proving the `host_name` alias still
   loads an old config.toml. Touch `docs/mdview-skill-template.md:42` prose
   for consistency (doc-only, no gate).
   Risk: LOW — mechanical rename, serde alias is a well-known pattern,
   already covered by existing round-trip tests.

2. **Port truth (D2).** `runtime.rs::ensure_bind()`: on the 20×100ms
   readiness-poll timeout, call `daemon::read_lock()` before falling back to
   `Config::load()` — the lock file is written by `serve()` immediately after
   `bind_with_retry` succeeds (server.rs:50-55), so it holds the real bound
   port even if the daemon hasn't yet answered its own health check. Only
   fall through to `cfg.server.port` when `read_lock()` also returns `None`
   (nothing was ever spawned). `DaemonInfo.host`/connectivity untouched
   (critical pattern).
   Risk: MEDIUM — the exact race (lock written, health check not yet
   passing, and the poll window expires) is hard to hit in a real timed
   test; validate with a fixture that writes a fake lock file directly and
   asserts `ensure_bind()` returns its port without waiting the full 2s
   (inject health-check via existing `read_lock()`/`running_daemon()`
   separation rather than a live daemon).

3. **CLI parity + e2e verify (D3).** `cli.rs`: `cmd_open` and `cmd_restart`
   swap `ensure_daemon_base()` → `ensure_daemon_bases()`. `cmd_open --json`
   keeps `"url"` (`urls[0]`, back-compat) and adds `"urls"` (full list,
   mirrors MCP `structuredContent` per decision `d88c028b`). Text mode:
   single URL unchanged; multiple URLs get the same
   "pick a reachable IP" framing MCP already uses (extract a shared helper
   if the two call sites end up identical — YAGNI otherwise, KISS if a
   two-line duplication is cheaper than a premature helper). Add:
   - unit test: `cmd_open --json` shape includes both keys when
     `ensure_daemon_bases()` returns >1 URL (inject via the same pure
     `build_display_urls` already unit-tested — no live daemon needed for
     the shape assertion).
   - e2e test (real binary, per critical-pattern Rust E2E invocation): spin
     a real daemon bound to `127.0.0.1` with a known port, run `mdview open`
     against a real file, assert the returned URL's port matches the daemon
     actually listening (closes the request for "verify end-to-end", not
     just unit-level).
   Risk: LOW-MEDIUM — CLI JSON shape is an existing (if undocumented)
   contract; adding a field is additive/back-compat, matching the MCP
   precedent.

## Files (bounded)

- `crates/mdview-core/src/config.rs`
- `crates/mdview/src/server.rs`
- `crates/mdview/src/runtime.rs`
- `crates/mdview/src/views.rs`
- `crates/mdview/src/cli.rs`
- `crates/mdview/src/mcp.rs` (read-only reference point, no edit expected —
  confirm during D3 that its shape is already correct)
- `docs/mdview-skill-template.md` (doc-only prose consistency)

## Test matrix (edge dimensions, scaled to standard)

- Empty/whitespace `hostname` → `None` (existing coverage, keep green).
- Old config.toml with `host_name` key loads via serde alias (new).
- `ensure_bind()` timeout with a lock file present → real port, not config
  port (new).
- `ensure_bind()` timeout with NO lock file at all → config fallback,
  unchanged behavior (regression guard).
- `cmd_open --json` with a single reachable path → `url` only meaningfully
  set, `urls` has exactly one entry equal to `url`.
- `cmd_open --json` with wildcard bind + multiple machine IPs (mocked via
  the existing pure builder, not real interfaces) → `urls` has N entries,
  `url` == `urls[0]`.
- e2e: real `mdview open` against a real bound daemon returns a URL whose
  port equals the actual `TcpListener::local_addr()` port.

## Open questions for validating

- None blocking — D2's fixture approach (fake lock file) avoids flaky
  timing-based tests; validating should confirm this is achievable without
  sleeping the full 2s in CI.
