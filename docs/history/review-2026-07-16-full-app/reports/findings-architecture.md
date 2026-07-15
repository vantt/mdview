# Architecture Review — mdview full-app retrospective

Focus: boundaries, coupling, API design, maintainability, drift from plan.md. Read-only. Inputs read in full: the 7975-line cumulative diff, the frozen review-scope record, the three feature plan.md files, and all six area specs. Provenance is stated per finding; the core scaffold (`config/domain/engine/error/indexer/link_resolver/render/repository.rs`) has no plan.md or cell evidence, so those findings are drawn from the diff alone and labelled as such.

**Summary: 3 × P2, 5 × P3. No P1.** Layering is fundamentally sound — a dependency-inverted `mdview-core` (no Axum/Tauri) with thin HTTP/MCP/CLI adapters is the right shape and largely holds. Findings are boundary leaks and duplication that compound as the surface grows, plus one setting plumbed end-to-end but consumed nowhere.

---

### [P2] The `Engine` facade is bypassed — adapters reach through `engine.store` into SQLite, and one page render issues ~6 redundant lookups (autofix_class: manual)
Engine is documented as "the facade the HTTP/MCP/CLI adapters call," but `store` is a public field and adapters call SQLite through it directly. Rendering one file page does: `get_project` ×2 (handler + inside `render_file`), `get_file` ×3, plus `file_abs_paths`, `list_files`, `backlinks` — ~6 project/file round-trips where 2 would do.
- Evidence: `crates/mdview-core/src/engine.rs` ~L1264 `pub struct Engine { pub store: SqliteStore, pub config: Config, render: RenderService }` — persistence adapter is a public field of the facade, so nothing enforces the boundary.
- Evidence: `crates/mdview/src/server.rs` ~L5707-5716 `st.engine.store.get_file(&id,&path)...is_some()` then `render_file(...)` then `st.engine.store.get_file(&id,&path).unwrap().unwrap()` — web layer reads SQLite directly and repeats the lookup the facade just did. Also `server.rs` ~L5564 and `cli.rs` ~L4493 use `engine.store.total_file_count()`.
- Provenance: `Engine`/`store` pre-bee scaffold; `server.rs` call sites accreted across later cells.
- Failure scenario: a future core-level cache or permission check on the render path is silently bypassed because the web layer reads `engine.store` directly; divergence is invisible until a stale/unauthorized page is served.
- Fix: make `store` non-public; add `Engine::total_file_count()` and a single `render_page(id, path) -> Result<Option<PageBundle>>` returning page + file + file list + backlinks, fetching each row once. Adapters call only that.
- Acceptance: `SqliteStore` not reachable as a public field of `Engine`; `crates/mdview` compiles with no `engine.store.` access; a file page render performs `get_project`/`get_file` at most once each.

---

### [P2] Daemon spawn/detach lives in the binary crate, so the desktop shell reimplements it — as a weaker, non-detached copy (autofix_class: manual)
`spawn_daemon_detached` (Unix `setsid` / Windows `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`), `ensure_bind`, and the readiness poll live in `crates/mdview/src/runtime.rs` (binary crate). The desktop crate depends only on `mdview-core`, cannot reach them, and reimplements spawn in `main.rs` (`ensure_daemon` + `spawn_mdview_serve`) with a plain `Command::new().spawn()` — **no detach flags**. The reusable capability is in the wrong layer; lock/health already live one layer down in `mdview-core::daemon`, where spawn/detach belong.
- Evidence: `crates/mdview/src/runtime.rs` ~L5356-5393 `pub fn spawn_daemon_detached()` with `libc::setsid()` / `creation_flags(...)` — detach defined in the binary crate, invisible to core dependents.
- Evidence: `crates/mdview-desktop/src/main.rs` ~L3454-3462 `Command::new(find_mdview()).arg("serve")...spawn()` — second, weaker spawn with no detach.
- Provenance: documented (daemon-auto-spawn-detach plan.md, D 625c69fa), which explicitly names the desktop duplicate as out-of-scope follow-up — known, acknowledged drift.
- Failure scenario: desktop launched over SSH+X spawns `mdview serve` without detaching; when the session/desktop ends the daemon gets SIGHUP and dies — the exact failure the CLI's `setsid` prevents — and the web UI the desktop pointed the user to 404s.
- Fix: move `spawn_daemon_detached`/`ensure_bind`/readiness poll into `mdview-core::daemon`; CLI and desktop both call it; delete the desktop copy.
- Acceptance: single detach-capable spawn in `mdview-core::daemon`; desktop uses it; `spawn_mdview_serve` removed; desktop-spawned daemon survives launcher exit (`sid == pid` on Unix).

---

### [P2] `renderer.syntax_highlight_theme` is plumbed end-to-end but consumed nowhere — a dead setting (autofix_class: manual)
The setting is defined in `RendererConfig` (default `github-dark`), rendered in the settings form, validated/saved in `update_config`, and echoed in `settings_page` — but `build_highlight_css` hardcodes `"InspiredGitHub"` / `"base16-ocean.dark"` and explicitly discards the config value. Changing the setting does nothing.
- Evidence: `crates/mdview/src/server.rs` ~L5822-5826 `let light = theme_css("InspiredGitHub"...); let dark = theme_css("base16-ocean.dark"...); let _ = &engine.config.renderer.syntax_highlight_theme; // reserved` — highlight CSS built from constants, configured theme dropped.
- Evidence: `docs/specs/settings.md` L44 documents row 6 as a functional field with default `github-dark` — spec promises behavior the code doesn't deliver.
- Provenance: `build_highlight_css` pre-bee scaffold; field/form plumbing extended in mdview-hostname-doctor-fix. No cell wires it to output.
- Failure scenario: user changes the code theme in Settings, saves, restarts as instructed, sees no change; concludes settings/restart is broken.
- Fix: resolve the (dark, and optionally light) theme name from config in `build_highlight_css`, falling back to constants for unknown names; OR remove the field from config/form/spec so the surface matches reality.
- Acceptance: setting a valid syntect theme changes `/highlight.css`, OR the field is removed everywhere; no dead `let _ = ...syntax_highlight_theme` read remains.

---

### [P3] Path/link resolution logic duplicated three ways (`resolve_link`, `resolve_to_rel`, `resolve_asset`) (autofix_class: manual)
The defining "href → project-relative target" algorithm (strip anchor/query, resolve against source dir or root, `normalize` clamp, try `+.md`/`README.md`/`index.md` candidates) is written three times: `resolve_link` and `resolve_to_rel` in `link_resolver.rs` share an identical candidate array; `resolve_asset` in `render.rs` repeats the same plumbing for images.
- Evidence: `crates/mdview-core/src/link_resolver.rs` ~L2071-2105 and ~L2113-2181 — `let candidates = [abs.clone(), with_md_extension(&abs), abs.join("README.md"), abs.join("index.md")];` in both functions.
- Evidence: `crates/mdview-core/src/render.rs` ~L2637-2664 `resolve_asset` — third copy of strip+normalize+strip_prefix+component-join.
- Provenance: pre-bee scaffold.
- Failure scenario: a candidate added to `resolve_link` but not `resolve_to_rel` makes rendered links resolve while the "Linked from" backlinks panel silently omits the edge.
- Fix: extract one private `resolve_to_abs(...) -> Option<PathBuf>`; have all three format from its result.
- Acceptance: candidate array + `normalize` exist in one place; existing tests pass unchanged.

---

### [P3] Dead workspace dependencies (`minijinja`, `tower-http`) — and the CORS/trace middleware they imply is never wired (autofix_class: gated_auto)
`Cargo.toml` `[workspace.dependencies]` declares `minijinja` (loader) and `tower-http` (cors, trace), but no member crate lists either, and `server.rs` builds its `Router` with no `.layer(...)`. Views are hand-built strings, not minijinja. So the server has neither CORS nor request tracing despite the manifest implying both.
- Evidence: `Cargo.toml` L686-688 — both declared.
- Evidence: `crates/mdview/src/server.rs` ~L5523-5540 `Router::new().route(...).with_state(state)` — no middleware attached.
- Provenance: pre-bee scaffold.
- Failure scenario: a contributor exposes the daemon on `0.0.0.0` assuming CORS is configured (the `cors` feature is in the manifest); no `CorsLayer` exists, so cross-origin behavior differs from expectation.
- Fix: remove both from the manifest; or actually wire `TraceLayer`/`CorsLayer` in the same change.
- Acceptance: both deps are consumed or removed; if retained, the layer/template use appears in code.

---

### [P3] Small helpers duplicated across crates: `is_markdown`, HTML-escape (`esc`/`html_escape`), rel-path join (autofix_class: manual)
Correctness-bearing utilities are copy-pasted. Two identical `is_markdown` (`indexer.rs` L1855, `watch.rs` L6389); two identical HTML-escapers (`render.rs::html_escape` L2666, `views.rs::esc` L6301); the URL-path join (`components().filter_map(Normal).join("/")`) in three places (`indexer::rel_path_str`, `link_resolver::to_url_path`, inline in `render::resolve_asset`).
- Provenance: pre-bee scaffold (watch.rs/views.rs are scaffold-era binary files).
- Failure scenario: `render.rs::html_escape` later hardened to escape `'` but `views.rs::esc` missed → attribute values built in views.rs remain injectable; or a markdown extension added to `indexer` but not `watch` makes the watcher ignore files the indexer accepts.
- Fix: expose one `is_markdown`, one `html_escape`, one `rel_to_url` from `mdview-core` and reference them everywhere. views.rs reusing the core escaper is the highest-value consolidation.
- Acceptance: one of each helper, referenced by all call sites.

---

### [P3] `mdview-desktop` version pinned at 0.1.0 while the workspace is 0.4.0 (autofix_class: gated_auto)
All workspace crates use `version.workspace = true` (0.4.0), but the deliberately-out-of-workspace desktop crate is hardcoded `version = "0.1.0"` and was skipped by the `version-bump-0-4-0` cell.
- Evidence: `crates/mdview-desktop/Cargo.toml` L3243 `version = "0.1.0"`.
- Provenance: version-bump-0-4-0 cell bumped the workspace only.
- Failure scenario: a 0.4.x bug report cites "desktop 0.1.0" (the only version the shell reports), costing triage time.
- Fix: bump desktop to match; add a one-line note to the release procedure that the out-of-workspace crate is bumped alongside the workspace.
- Acceptance: desktop version matches the workspace, or a documented policy states why it lags.

---

### [P3] Returned-URL contract (`url`/`urls`/`path`/`project_id`) rebuilt inline per adapter with no shared type (autofix_class: manual)
`mcp.rs::handle_tool_call` builds `structuredContent: { url, urls, path, project_id }` by hand; `cli.rs::cmd_open` builds `{ url, project_id }` by hand. The two public surfaces already disagree (CLI omits `urls`/`path`). No shared struct defines the contract.
- Evidence: `crates/mdview/src/mcp.rs` ~L5198-5208 inline `structuredContent`.
- Evidence: `crates/mdview/src/cli.rs` ~L4417-4421 inline `json!({ "url": full, "project_id": ... })`.
- Provenance: mcp.rs scaffold-era; `urls` added by multi-ip-urls cell, still inline.
- Failure scenario: a field added to the MCP result never reaches CLI `--json` consumers because the CLI builds its own object.
- Fix: define a serde `ViewFileResult { url, urls, path, project_id }`; both adapters serialize it; decide deliberately whether CLI adopts `urls`/`path` for parity.
- Acceptance: one typed struct defines the contract; MCP and CLI both serialize it.

---

## Advisories (not filed as findings)
- **Mermaid from external CDN** (`views.rs::file_page` imports `mermaid` from `cdn.jsdelivr.net`). Architecture angle: couples a *local* viewer to network reachability — diagrams fail offline, contradicting the "local background server" boundary in system-overview.md. The reliability reviewer owns the external-CDN failure mode per the manifest; flagging only the boundary contradiction.
- **`_`-prefixed route namespace fragility**: `/p/:id/_search` and `/p/:id/_jump` differ from `/p/:id/*path` only by the underscore; a file whose rel-path is exactly `_search`/`_jump` (no extension) would be shadowed. Very unlikely; noted only.
- **`find_project_root` markers broad**: `cli.rs` uses `[".mdview.json",".git","CLAUDE.md","README.md"]`; because README.md is ubiquitous, `mdview open` on a nested file may pick a surprisingly high ancestor root. Product/UX judgement, not an architecture defect.
- **Positive drift (non-issue, stated per instructions)**: mdview-hostname-doctor-fix plan.md proposed making `daemon.rs::base_url` prefer `host_name`. The implementation deliberately did NOT — `base_url` stays pure connectivity and the display-only substitution lives in `runtime.rs::display_base_url`/`build_display_urls` (pure, unit-tested). This is a *better* boundary than the plan sketched and matches settings.md R1. Recorded so a later audit does not "correct" it back toward the plan text.

## Checked and clean
- Core dependency rule holds: `mdview-core` free of Axum/Tauri (verified crate manifests + imports); adapters only in the `mdview` binary and desktop crate.
- `link_resolver::normalize` clamps `..`; `asset_path` re-checks `starts_with(root)` — traversal boundary structurally present (exhaustive judgement left to security reviewer).
- The two-phase `Action` AST walk in `render.rs` is deliberate (avoids holding the AST borrow across `collect_text`), not accidental complexity.
- Config load/save is atomic (temp+rename) and corrupt-resilient — a sound persistence boundary.

Status: DONE
Summary: 8 architecture findings — 3 × P2 (facade bypass + redundant queries; daemon spawn/detach in wrong layer causing a non-detached desktop duplicate; dead `syntax_highlight_theme` setting), 5 × P3 (triplicated link resolution, dead workspace deps, duplicated helpers, desktop version skew, un-typed returned-URL contract), plus 4 advisories; core layering otherwise sound.
