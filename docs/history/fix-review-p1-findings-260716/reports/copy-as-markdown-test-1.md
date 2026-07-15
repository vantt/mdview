# copy-as-markdown-test-1

Status: [DONE]

Outcome: Extracted `file_page`'s inline `</script>`-breakout escape (`crates/mdview/src/views.rs`) into a named pure function `escape_json_for_script(source: &str) -> String`, called unchanged from `file_page`. Added a `#[cfg(test)] mod tests` with two tests: script-breakout neutralization (no raw `<` in output) and JSON round-trip back to the original source. `file_tree`'s separate escape (~L211) was left untouched, per scope.

Files touched: `crates/mdview/src/views.rs`

Full trace/evidence: `.bee/cells/copy-as-markdown-test-1.json`
