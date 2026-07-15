# config-edit-cli-test-1

Status: DONE
Outcome: Extracted `resolve_editor()` and `classify_config_edit_outcome()` as pure functions from `cmd_config_edit` in `crates/mdview/src/cli.rs`; added 8 unit tests covering `$VISUAL`/`$EDITOR` precedence, platform fallback, arg-splitting (`"code --wait"`), and valid/invalid/unreadable-file TOML outcomes. `cmd_config_edit`'s printed messages and exit behavior are unchanged.

Files touched: `crates/mdview/src/cli.rs`

Commit: `195f648` — test(cli): unit-test config-edit editor resolution and TOML outcome (config-edit-cli-test-1)

Full trace/evidence: `.bee/cells/config-edit-cli-test-1.json`
