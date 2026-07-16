---
artifact_contract: bee-plan/v1
artifact_readiness: implementation-ready
mode: small
---

# Surface the app version (PBI-16)

Single source of truth = `env!("CARGO_PKG_VERSION")` (the workspace Cargo
version, already used by `/health`, `/api/status`, MCP `serverInfo`, and clap's
`--version`). This wires the same value into the three places PBI-16 asked for.
3 files, 1 flag → small. No overlap with the concurrent host_name/port session.

## Cell

`version-surface-1`:
- **CLI** — add a `version` subcommand printing `mdview <version>` (clap already
  provides `--version`; the subcommand is a convenience).
- **Settings** — a `mdview v<version>` footer on the settings page.
- **install.sh** — echo the installed version (`"$DIR/$BIN" --version`) after install.

## Verification

`mdview version` and `mdview --version` both print the Cargo version; settings
footer shows it (live-confirmed); `bash -n install.sh` valid; `cargo test
--workspace` green.

## Release note

The version is bumped in one place — the workspace `Cargo.toml` `version` — and a
release tag `vX.Y.Z` should match it. No code carries a hard-coded version.
