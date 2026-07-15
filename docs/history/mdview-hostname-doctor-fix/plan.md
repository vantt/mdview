---
artifact_contract: bee-plan/v1
artifact_readiness: requirements-only
mode: small
---

# mdview: settings host_name + doctor --fix agent-instruction sync

## Scope (locked, D-IDs 07c1ac9f / 864f6f00)

1. **host_name setting** — add optional `server.host_name` config field. When set (non-empty), rendered view URLs use it instead of the current host/IP value.
2. **doctor --fix** — `check_agent_instruction` currently only warns; make `--fix` write the agent-instruction snippet into both `AGENTS.md` and `CLAUDE.md` when the marker is missing, mirroring the existing `check_mcp_registration` backup-then-write pattern.

## Mode gate

Flags counted: **multi-domain** (config/core + web UI + CLI doctor) = 1 flag. No auth, no data-loss, no external provider, no migration, no cross-platform, no public-contract break (new optional field, additive). 0-1 flags, ~5-6 files total but two independent, well-understood slices → **small**.

## Discovery: L0

Both patterns already exist in-repo — no external research needed:
- `host_name` follows the exact shape of the existing `host`/`port` fields in `ServerConfig` (crates/mdview-core/src/config.rs:8-24).
- `doctor --fix` writing a file already has a working precedent: `check_mcp_registration` (crates/mdview/src/doctor.rs:158-224) does backup-then-write via `write_atomic`.

## Approach

### Feature A: host_name

- `crates/mdview-core/src/config.rs`: add `pub host_name: Option<String>` to `ServerConfig` (after `host`, line ~10); default `None` in the Default impl (~line 55-63).
- `crates/mdview/src/server.rs`: add `host_name: Option<String>` to `SettingsForm` (154-165); in `update_config` (168-215) set `cfg.server.host_name = form.host_name.filter(|s| !s.trim().is_empty())`.
- `crates/mdview/src/views.rs` (270-306): render an additional input field for host_name in the settings form.
- Host resolution: wherever `DaemonInfo.host` / `ensure_daemon_base()` currently reads `cfg.server.host` for building the URL (`crates/mdview-core/src/daemon.rs` base_url, `crates/mdview/src/runtime.rs:20-33` ensure_daemon_base, and the daemon-bind-host population in `server.rs` ~46-53) — prefer `cfg.server.host_name` when `Some` and non-empty for the **URL-building** path only; the actual TCP bind must keep using `cfg.server.host`/IP (host_name is a display/link substitution, not a bind address).

Risk: LOW. Rejected alternative: environment-variable override — rejected, config.toml is already the single settings source of truth in this repo (no env-var precedent found).

### Feature B: doctor --fix

- `crates/mdview/src/doctor.rs`: change `check_agent_instruction()` signature to accept `(dry_run: bool, fix: bool)` (mirrors `check_mcp_registration`). When a file lacks the marker and `fix` is true: back up existing file (if present) then write/append the agent-instruction snippet to that file. Apply to **both** `AGENTS.md` and `CLAUDE.md` independently (a file already containing the marker is left untouched).
- Snippet source: reuse content pattern from `docs/mdview-agents-template.md` (currently unused by code) — embed via `include_str!` or an equivalent const so doctor doesn't depend on a doc file's runtime path.
- Call site: `run()` (doctor.rs:42-50) passes `dry_run, fix` into `check_agent_instruction`.

Risk: LOW. Existing files are appended-to via the established backup-then-write helper, not overwritten wholesale — avoids clobbering user content.

## Files touched

`crates/mdview-core/src/config.rs`, `crates/mdview/src/server.rs`, `crates/mdview/src/views.rs`, `crates/mdview/src/runtime.rs`, `crates/mdview-core/src/daemon.rs`, `crates/mdview/src/doctor.rs`.

## Verification

- `cargo build --workspace` compiles clean.
- `cargo test --workspace` passes (add/extend a doctor.rs unit test for the fix-writes-both-files behavior, and a config.rs round-trip test for host_name serialize/deserialize).
- Manual: run `mdview doctor --fix` in a scratch dir with neither AGENTS.md/CLAUDE.md present → both created with marker; re-run → no duplicate writes (idempotent).
- Manual: set host_name in settings, view a file, confirm returned URL uses the hostname not the IP; confirm server still binds to the configured host/IP (host_name doesn't change bind behavior).

## Test matrix (12 edge dimensions, small-lane depth)

- Empty/whitespace-only host_name → treated as unset (falls back to existing host).
- host_name set but daemon not yet running (first ensure_daemon_base call) → still substitutes correctly.
- doctor --fix when AGENTS.md/CLAUDE.md already contain marker → no rewrite, no duplicate content.
- doctor --fix when one of the two files exists and the other doesn't → both end up correct independently.
- doctor --dry-run (no --fix) → unchanged, still just reports Warn, no writes (existing behavior preserved).

## Open questions for validating

None — both slices are additive, mirror existing in-repo patterns, no gray areas.
