# cli-mcp-url-parity-1

**Status:** [DONE]

**Outcome:** `cmd_open`/`cmd_restart` now call `ensure_daemon_bases()` (CLI/MCP
parity, D3). `cmd_open --json` keeps `url` as the primary URL and adds a
`urls` array (`url == urls[0]`); text mode adopts MCP's "pick a reachable IP"
framing for >1 URL. Added a real cargo integration test
(`crates/mdview/tests/e2e_open.rs`) that spawns the compiled binary, runs a
live daemon on `127.0.0.1` with an OS-assigned port, and asserts `mdview open
--json`'s returned URL port matches the daemon's actual bound port.

**Files touched:** `crates/mdview/src/cli.rs`, `crates/mdview/src/runtime.rs`
(deviation: removed now-dead `ensure_daemon_base()`/`display_base_url()`),
`crates/mdview/tests/e2e_open.rs` (new).

**Commit:** `9084a7d`

Full trace, verify output, and verification evidence:
`.bee/cells/cli-mcp-url-parity-1.json`.
