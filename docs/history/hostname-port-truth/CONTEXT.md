# hostname-port-truth — CONTEXT

Combines PBI-10, PBI-13, PBI-14 from `docs/backlog.md` — the user proposed
these together and noted (in prior session decisions on version-surface,
polish-settings-form, fix-atelier-regressions) that they share files and
should run as one "concurrent host_name/port session" rather than three
separate lanes.

Source: user request 2026-07-16 (backlog rows), routed via `kêu bee tuần tự
làm hết các task còn lại` (sequential clear-through of remaining proposed
work).

## Verified current state (read before deciding — L1 quick verify)

- `ServerConfig.host_name: Option<String>` in `crates/mdview-core/src/config.rs:26`.
- `serve()` (`server.rs:48-55`) already writes `DaemonInfo.port = addr.port()`
  — the REAL bound port, not `cfg.port` — into the daemon lock. Happy path is
  correct today.
- `runtime::ensure_bind()` (`runtime.rs:91-131`) polls `running_daemon()` for
  2s (20×100ms); on timeout it falls back to `(cfg.server.host, cfg.server.port)`
  — the CONFIGURED port, which can differ from the real bound port if
  `bind_with_retry` auto-incremented. This is the one real gap PBI-13 flags.
- `mcp.rs::handle_tool_call` already calls `runtime::ensure_daemon_bases()`
  (multi-URL, IP-aware) — PBI-14's requirement is already implemented on the
  MCP path and unit-tested (`build_display_urls` in `runtime.rs`).
- `cli.rs::cmd_open` (line 255) and `cmd_restart` (line 452) call
  `runtime::ensure_daemon_base()` — the SINGLE-URL variant — not
  `ensure_daemon_bases()`. This is a real behavioral gap versus MCP, not
  just a missing test: CLI `open`/`restart` do not list multiple IPs when
  bound to `0.0.0.0` with no `hostname` override.
- `docs/specs/settings.md` already documents the *business* concept as
  "Display hostname" (R1/R3) — no literal `host_name` identifier appears in
  `docs/specs/*.md`. Only `docs/mdview-skill-template.md:42` uses the raw
  identifier in prose.
- Critical pattern applies: `DaemonInfo.host` is dual-purpose (connectivity +
  display); the rename must not touch that field, only `cfg.server.hostname`
  reads at display-string build sites (`runtime.rs::display_base_url`,
  `build_display_urls`, `views.rs` form).

## Locked decisions

- **D1 (PBI-10 rename).** Rename `ServerConfig.host_name` → `ServerConfig.hostname`
  with `#[serde(alias = "host_name")]` so existing `config.toml` files with the
  old key still load. Rename follows through: `SettingsForm.host_name` →
  `hostname`, `normalize_host_name` → `normalize_hostname` (server.rs), form
  input `name="hostname"` (views.rs), all `cfg.server.host_name` reads
  (runtime.rs). `docs/mdview-skill-template.md` prose updated for consistency
  (non-blocking, doc-only). `docs/specs/settings.md` needs NO change — it
  already uses the tech-agnostic term "Display hostname".
- **D2 (PBI-13 port truth).** Fix `ensure_bind()`'s timeout fallback: instead
  of returning `(cfg.server.host, cfg.server.port)` unconditionally, first try
  `daemon::read_lock()` (the lock file, written by `serve()` right after bind,
  before the daemon is health-check-ready) — if a lock exists, its `port` is
  the real bound port even though `running_daemon()`'s health check hasn't
  confirmed liveness yet. Only fall through to `cfg.server.port` if no lock
  file exists at all (daemon truly never started). This closes the gap without
  touching `DaemonInfo.host`/connectivity semantics (critical pattern).
- **D3 (PBI-14 CLI parity + verify).** `cmd_open` and `cmd_restart` switch from
  `ensure_daemon_base()` to `ensure_daemon_bases()`, matching `mcp.rs`. Output
  shape: text mode prints the primary URL when there is one, or a
  "pick a reachable IP" list when there are several (mirrors MCP's text
  format); `--json` mode on `cmd_open` keeps `"url"` (primary, back-compat)
  and adds a `"urls"` array (mirrors the MCP `structuredContent` contract from
  decision `d88c028b`). Add integration/e2e coverage for both the MCP and CLI
  path using the real binary (per critical-pattern Rust E2E invocation) plus
  unit coverage for the port-truth fix (D2) using a fake stale-lock scenario.
- **D4 (sequencing).** Execute D1 → D2 → D3 in that order. D3 genuinely reads
  the renamed `hostname` field (via `ensure_daemon_bases()` → `display_base_url`).
  D2 does NOT — `ensure_bind()` reads `cfg.server.host`/`cfg.server.port`
  (connectivity), not the display `hostname` field — so D2 has no semantic
  dependency on D1; the ordering is kept anyway because D1 and D2 both edit
  `runtime.rs` and sequential single-file edits avoid merge friction. D3's
  e2e test (real daemon, loopback bind, happy path) does **not** exercise
  D2's timeout-fallback fix — that fix is proven separately by
  `bound-port-truth-1`'s own unit tests on the extracted pure helper, not by
  the e2e. (Corrected 2026-07-16 per plan-checker review — the original text
  overstated both the D1→D2 dependency and what the e2e covers.)

## Non-goals

- No change to `DaemonInfo.host``/connectivity, IPv6, or the wildcard-detection
  logic (`is_wildcard`) — all already correct and unit-tested (PBI-04).
- No change to `mdview-desktop`'s duplicate `ensure_daemon()` — out of scope
  per the existing critical pattern (desktop feeds an embedded webview, not an
  externally-shown link).

## Mode

3 flags: data model (field rename w/ back-compat alias), existing covered
behavior modification (URL builders, port-truth fallback), weak proof around
the area (known reliability gap, no prior automated coverage of the timeout
path). No hard-gate flag (no auth/security/external-provider/data-loss) →
**standard**. `gate_bypass_level: normal` covers standard non-hard-gate work,
so Gates 1-3 auto-approve.
