# Test-Coverage Review — mdview full-app retrospective

**Reviewer focus:** missing edge cases, regression paths, weak/tautological assertions, untested behavior changes.

## What was checked
- The full cumulative diff (7975 lines), reading every `#[cfg(test)]` module and mapping each `diff --git` file range to whether it carries tests.
- All 21 cell JSON traces (`must_haves`, `behavior_change`, `trace.verify_command`, `trace.verification_evidence`, `trace.red_failure_evidence`), cross-referencing each behavior-change cell against whether a *named automated test* actually landed in the diff.
- The three on-disk plans and the frozen review scope record.

## Test-module map (from the diff)
Files **with** tests: `config.rs` (3), `daemon.rs` (2), `engine.rs` (1), `fuzzy.rs` (6), `indexer.rs` (2), `link_resolver.rs` (8), `render.rs` (9), `repository.rs` (3), `doctor.rs` (5), `runtime.rs` (6).
Files with **zero** tests despite real behavior: `crates/mdview/src/cli.rs`, `mcp.rs`, `server.rs`, `views.rs`, `watch.rs`, `mdview-desktop/src/main.rs`, plus `domain.rs`/`error.rs` (struct/enum only).

**Unifying failure mode:** every test lives in `mdview-core` plus two `mdview` modules (`doctor.rs`, `runtime.rs`). The entire `mdview` HTTP/daemon/CLI/MCP/view layer — where 7 of 14 behavior-change cells actually changed behavior — has **no automated test**, and those cells were capped on manual `curl`/live-E2E evidence. No integration test ever boots the axum app.

---

### [P1] `config-edit-cli-1` shipped a behavior change with no automated test and no characterization evidence  (autofix_class: manual)
**Plain-language:** `mdview config edit` (editor resolution, materialize-before-edit, post-edit TOML validation, 3 exit branches) is proven only by a manual live run with a fake `$EDITOR`.
**Today:** `cmd_config_edit()` resolves `$VISUAL`→`$EDITOR`→`vi/notepad`, whitespace-splits the editor string, materializes config, spawns editor, re-reads + `toml::from_str::<Config>` to warn on invalid TOML. `cli.rs` has no test module.
**Problem:** `behavior_change:true`, `verify_command:"mdview config edit (live E2E, fake EDITOR)"`, no `red_failure_evidence`. Per the evidence gate (no named automated test AND no red/characterization evidence) → P1. The resolution/split/validation logic is pure and trivially unit-testable.
**Failure scenario:** a refactor of `$VISUAL`/`$EDITOR` precedence or `split_whitespace()` handling breaks `code --wait` or drops the path arg; CI stays green.
**Evidence:** `crates/mdview/src/cli.rs` L4320-4345 (diff) — `let editor = std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR"))…`, no `#[test]` in file.
**Fix:** extract `resolve_editor()` + arg-splitting as pure fns; unit-test them and the invalid-vs-valid TOML branch.
**Acceptance:** [ ] `$VISUAL` beats `$EDITOR`, fallback `vi`/`notepad`; [ ] `"code --wait"` splits to program+args, path last; [ ] invalid TOML → warning not silent accept.

### [P1] `copy-as-markdown-2` `<`-escape and copy handler have no automated test  (autofix_class: manual)
**Plain-language:** the security-relevant `#mdsource` blob `<`→`<` escape (prevents `</script>` breakout) and the whole `app.js` copy handler are verified only by a one-off live `curl` byte count.
**Today:** `file_page()` builds `source_json = serde_json::to_string(&page.source)…replace('<', "\\u003c")` in `<script id="mdsource">`. `views.rs` has no test module; the *core* cell (copy-as-markdown-1) is tested, this cell is not.
**Problem:** `behavior_change:true`, `verify_command:"curl … (live, 7798)"`, no characterization → P1. This is a pure, breakout-preventing transform with zero coverage.
**Failure scenario:** someone gates the escape on `contains("</script>")` or reorders it before `to_string`, reintroducing a breakout in a doc whose markdown literally contains `</script>`.
**Evidence:** `crates/mdview/src/views.rs` L5986-5988 — `.replace('<', "\\u003c")`.
**Fix:** unit-test an extracted `escape_json_for_script(&str)` asserting inputs with `</script>`/`<!--` yield zero raw `<` and round-trip-parse; note the JS handler still needs a DOM harness (known gap).
**Acceptance:** [ ] source with `</script>` → no unescaped `<`; [ ] blob parses back to exact source.

### [P1] `mdview-restart-1` stop-then-respawn logic has no automated test  (autofix_class: manual)
**Plain-language:** `mdview restart` and the shared `stop_daemon()` helper (extracted from the pre-existing `cmd_stop`) are proven only by a live isolated-HOME run; the extraction is an unguarded regression risk to `stop`.
**Today:** `cmd_restart()` → `stop_daemon()`, wait none, `spawn_daemon_detached()`, wait up. `cmd_stop()` now delegates to the same helper "preserving its three messages". No test module in `cli.rs`.
**Problem:** `behavior_change:true`, `verify_command:"mdview restart (live E2E, isolated HOME)"`, no characterization → P1.
**Failure scenario:** the `stop_daemon() -> Option<(u32,bool)>` return shape changes and `cmd_stop`'s "Could not stop"/"No daemon running" branch is mis-wired; wrong message or false-success stop.
**Evidence:** `crates/mdview/src/cli.rs` L4310-4311 — `Command::Stop => cmd_stop(), Command::Restart => cmd_restart(),`, helper bodies untested.
**Fix:** make `stop_daemon()` pure over an injected lookup; assert each outcome→message mapping and the no-daemon restart branch.
**Acceptance:** [ ] each `stop_daemon()` outcome → correct message; [ ] restart with no daemon takes "just start".

---

### [P2] `multi-ip-urls-2` MCP response assembly is manual-E2E only  (autofix_class: manual)
The pure URL builder (cell-1) is well tested, but the `mcp.rs` assembly of `structuredContent.url`/`urls`/`path`/`project_id` + text block is proven only by a live `0.0.0.0` run; no characterization. Back-compat invariant "`url == urls[0]`" lives only in untested JSON-assembly code. **Evidence:** `crates/mdview/src/mcp.rs` L5181-5208 — `let primary = urls.first()… "url": primary, "urls": urls`. **Fix:** extract `build_view_result(vf,bases)->Value`; test `url==urls[0]`, single-vs-multi text, path/project_id survival. Risk P2 because the builder is covered.

### [P2] `nucleo-fuzzy-search-2` `_jump` route + 404/empty-q paths untested  (autofix_class: manual)
`GET /p/:id/_jump` (unknown-project 404, empty-`q`→`[]`) and the client palette are proven only by live curl; `server.rs` has no tests. **Evidence:** `crates/mdview/src/server.rs` L5774-5787 — `if matches!(st.engine.get_project(&id), Ok(None) | Err(_)) { return not_found(...) }`. **Failure:** narrowing that `matches!` to only `Ok(None)` turns a DB error into a 500 with `[]`. **Fix:** an axum `oneshot` test: unknown→404, empty-q→`[]`/200, match→non-empty JSON.

### [P2] `mermaid-zoom-1` is client-only behavior with no automated coverage  (autofix_class: advisory)
Entire pan/zoom/fullscreen feature is JS, verified by `node --check` (syntax) + curl (asset served). No behavior exercised. Technically P1 by the gate, rated P2: no Rust surface to test, no data/contract risk. **Evidence:** `crates/mdview/assets/app.js` L3799+ (MutationObserver/requestFullscreen), no harness. **Failure:** wrong `transform-origin` math ships; syntax check passes. **Fix:** record a decision on whether client JS gets a jsdom/Playwright smoke harness (note the JS surface now spans mermaid + copy + chapter-zoom + jump palette, so the harness would amortize).

### [P2] `file-nav-ux-1` server-rendered chapter/topbar helpers are unit-testable but untested  (autofix_class: manual)
Has `red_failure_evidence` (so not P1), but the plan's own "Test matrix" (root file, deep nest, empty title, `</script>` in title, Settings on every page) is verified only by curl+`node --check`. `topbar`, `file_tree` fallback, `parent_dir`, `base_name`, and the `#filelist` `<`-escape are pure and untested. **Evidence:** `crates/mdview/src/views.rs` L6082-6150 — `fn parent_dir(rel){… None => ""}` and `serde_json::to_string(&payload)…replace('<',"\\u003c")`. **Failure:** a `parent_dir` refactor to `rsplit_once('/').unwrap()` panics on a root file; or dropping the `#filelist` escape lets a `</script>` title break the page. **Fix:** unit-test `parent_dir`/`base_name` (root/nested/no-sep), fallback for root file + empty-title basename fallback + `</script>` escaping, and `topbar()` always contains `href="/settings"`.

### [P2] Entire HTTP/daemon/MCP/watcher layer untested (systemic; scaffold range)  (autofix_class: manual)
`server.rs`, `mcp.rs`, `watch.rs` carry real logic with zero tests, and the scaffold commit range "carries no verification-evidence preflight guarantee." Untested: `update_config` validation (port≥1, host trim, theme allow-list, exclude parsing), `bind_with_retry` port auto-increment, `scope_css`/`strip_comments`/`content_type` (pure), `mcp::run` JSON-RPC dispatch + arg validation, and `watch::reindex_paths`. **Evidence:** `crates/mdview/src/watch.rs` L6371 — `let Some(project) = projects.iter().find(|p| path.starts_with(&p.root_path)) else {...}`. **Failure:** `starts_with` has no path-boundary check — projects `/a/proj` and `/a/proj-docs` can cross-match, reindexing under the wrong project; or a regression in the `exists()`-vs-remove branch silently stops live-reload on delete. **Fix:** (1) unit-test the pure helpers + an extracted `apply_form(cfg,form)`; (2) one `tower::ServiceExt::oneshot` test booting `router()` over an in-memory engine asserting `/health` 200, a rendered file page, a 404, and `_jump`; (3) a `reindex_paths` test incl. prefix-collision and delete.

### [P2] `engine.rs` has one test; `asset_path` traversal guard + mutation paths untested  (autofix_class: manual)
`Engine` (view_file, refresh, remove_file, incremental index, backlinks, search, fuzzy_files, `asset_path`) has a single test covering only view_file auto-create. `asset_path` — the static-asset guard that `std::fs::read`s bytes off disk and returns them — has **no** test. The one traversal test that exists (`link_resolver::does_not_escape_project_root`) guards *link rewriting*, a different code path. **Evidence:** `crates/mdview-core/src/engine.rs` L1492 decl, L1506-1555 only test. **Failure:** an `asset_path` refactor that normalizes after joining lets `../../etc/passwd`-style paths escape root → direct file disclosure; the link_resolver test stays green. **Fix:** engine tests for `asset_path` rejecting `..` escapes and resolving in-root assets; add a `remove_file`/`backlinks` round-trip. (Cross-ref: security reviewer's P1 on this same fallback — over-broad, not an escape.)

### [P2] Security-critical HTML escapers in `views.rs` are untested  (autofix_class: manual)
`esc()` (used on every file-derived string; escapes `&<>"` but **not** `'`) and `highlight_excerpt()` (re-injects `<mark>` after escaping) are XSS-relevant pure fns with no tests. **Evidence:** `crates/mdview/src/views.rs` L6301-6306 (`esc`) and L6213-6217 (`highlight_excerpt`). **Failure:** a future single-quoted attribute using an `esc()` value lets an apostrophe title break out (esc doesn't escape `'`), an invariant nothing enforces; or `highlight_excerpt`'s restore step reopens injection on a crafted `&lt;mark&gt;`-shaped FTS snippet. **Fix:** unit-test `esc` per metacharacter and `highlight_excerpt` on a literal `<mark>`-shaped payload; encode the double-quote-only contract in a test name.

---

### [P3] `daemon-auto-spawn-detach-1` detach behavior has no regression guard (accepted characterization)  (autofix_class: advisory)
setsid detach proven by manual `ps -o sid,pid` + a solid `red_failure_evidence` (absent syscall pre-change), so **not** P1 by the gate. Residual risk: dropping `pre_exec(setsid)` in a refactor fails no test; the Windows branch is compile-guarded, never run in CI. **Evidence:** `crates/mdview/src/runtime.rs` L5356-5394. **Fix:** accept as documented, or add a Unix-only `getsid`/`/proc` assertion (flaky/harness-dependent — advisory).

### [P3] `render.rs` tests assert via loose `contains()` substrings  (autofix_class: advisory)
Several render tests use presence-only `page.html.contains("class=\"mermaid\"")` / `contains("<pre class=\"code\">")`, which pass on coincidental substrings and miss structural regressions (wrong element, duplication). Minor — the suite otherwise asserts exact values (vec equality, exact URLs, exact `5:1-5:11` sourcepos). **Evidence:** `crates/mdview-core/src/render.rs` L2777-2793. **Fix:** where cheap, assert counts/positions (e.g. exactly one `class="mermaid"`), or accept as adequate smoke coverage.

---

## Behavior-change cell vs. automated-test coverage
| Cell | behavior_change | named automated test? | red/characterization? | rating |
|---|---|---|---|---|
| agent-instruction-markers-1 | yes | YES (5 doctor) | yes | covered |
| config-edit-cli-1 | yes | **no** | no | **P1** |
| copy-as-markdown-1 | yes | YES (render) | — | covered |
| copy-as-markdown-2 | yes | **no** | no | **P1** |
| daemon-auto-spawn-detach-1 | yes | no | YES | P3 |
| file-nav-ux-1 | yes | no | YES | P2 |
| mdview-hostname-doctor-fix-1 | yes | partial (config roundtrip only) | YES | folded into HTTP-layer P2 |
| mdview-hostname-doctor-fix-2 | yes | YES (4 doctor) | yes | covered |
| mdview-restart-1 | yes | **no** | no | **P1** |
| mermaid-zoom-1 | yes | no (JS-only) | no | P2 |
| multi-ip-urls-1 | yes | YES (6 runtime) | — | covered |
| multi-ip-urls-2 | yes | no | no | P2 |
| nucleo-fuzzy-search-1 | yes | YES (6 fuzzy) | — | covered |
| nucleo-fuzzy-search-2 | yes | no | no | P2 |

Non-behavior-change cells (fix-ci-fmt, mdview-doc-block-migration, prd-desktop-mcp-revision, prd-highlight-porting-promote, prd-open-questions-resolve, usage-guide, version-bump-0-4-0) are docs/formatting/version work — no test expected, correctly none added.

**Note on assertion quality:** no egregiously tautological/self-referential assertions found. The `mdview-core` suite generally asserts exact values. The dominant problem is *absence* of tests in the `mdview` binary layer, not *weak* tests in the core.

Status: DONE
Summary: 3 P1 (behavior-change cells config-edit-cli-1, copy-as-markdown-2, mdview-restart-1 capped with no automated test and no characterization evidence), 7 P2 (untested mcp/server/watcher/engine layers, asset_path traversal guard, views escapers, JS-only features), 2 P3 (detach regression guard, loose render substring assertions); root cause is the entire mdview binary HTTP/CLI/MCP/view layer having zero automated tests.
