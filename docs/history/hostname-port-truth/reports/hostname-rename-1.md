# hostname-rename-1 — Execution Report

**Status:** [DONE]

**Outcome:** Renamed `ServerConfig.host_name` field to `hostname` with `#[serde(alias = "host_name")]` for config.toml back-compat. Propagated rename through `SettingsForm`, `normalize_hostname` function, and runtime URL builders. All 72 tests pass.

**Files Touched:**
- `crates/mdview-core/src/config.rs` — field rename + serde alias + Default impl + test rename
- `crates/mdview/src/server.rs` — SettingsForm field, function rename, test rename
- `crates/mdview/src/runtime.rs` — config reads updated
- `crates/mdview/src/views.rs` — form input name and template variable updated
- `docs/mdview-skill-template.md` — prose reference updated

**Verification:** `cargo test --workspace` — 72 tests passed, 0 failed. Includes:
- `config::tests::hostname_defaults_to_none_and_roundtrips_when_set` — verifies serde alias works
- `server::asset_response_tests::hostname_form_value_normalizes_blank_to_none` — verifies form normalization

**Commit:** `ede0c57` — `feat(hostname-rename-1): rename ServerConfig.host_name to hostname with serde back-compat alias`

**Cell Trace:** [.bee/cells/hostname-rename-1.json](.bee/cells/hostname-rename-1.json)

**Next:** Ready to execute D2 (bound-port-truth-1) — no dependencies on this cell's output.
