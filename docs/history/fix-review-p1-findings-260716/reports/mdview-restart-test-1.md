# mdview-restart-test-1

Status: [DONE]

Outcome: Extracted `cmd_stop`'s 3-way and `cmd_restart`'s 2-way `stop_daemon()` outcome→message mappings into named pure functions (`stop_outcome_message`, `restart_stop_message`) in `crates/mdview/src/cli.rs`, both called from production code; added 5 unit tests covering every branch. No printed text or control flow changed (D3).

Files touched: `crates/mdview/src/cli.rs`

Full trace/evidence: `.bee/cells/mdview-restart-test-1.json`
