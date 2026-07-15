# security-asset-allowlist-1

Status: DONE
Outcome: `asset_path` (crates/mdview-core/src/engine.rs) now rejects any resolved path whose canonicalized extension is not one of the 9 tokens `content_type()` recognizes (checked case-insensitively on the canonicalized/symlink-resolved path, never the raw rel_path — closes a symlink bypass), and separately rejects any path whose relative-to-root components match `config.indexing.exclude_patterns` (exact component-name equality, mirroring `scan_markdown_files`). The existing traversal guard is unchanged. Added `#[cfg(test)] asset_path_enforces_allowlist_exclude_patterns_and_traversal_guard`, including a `#[cfg(unix)]` symlink-bypass regression case.

Files touched: `crates/mdview-core/src/engine.rs`

Commit: `ba7e12a` — fix(security-asset-allowlist-1): restrict asset_path to safe extensions + exclude_patterns

Full trace/evidence: `.bee/cells/security-asset-allowlist-1.json`
