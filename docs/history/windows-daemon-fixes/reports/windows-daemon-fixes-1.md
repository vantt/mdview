# windows-daemon-fixes-1

[DONE] — `health_check` now dials loopback for a wildcard-bound host (D1), with a
new unit test proving it against a real `127.0.0.1` listener; existing dead-port
test unchanged and passing.

Files touched: `crates/mdview-core/src/daemon.rs`

Full trace/evidence: `.bee/cells/windows-daemon-fixes-1.json`
