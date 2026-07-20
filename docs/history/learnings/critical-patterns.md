# Critical Patterns

Mandatory pre-planning / pre-execution context for this repository.
bee-compounding appends hard-won patterns here; keep it short and current.

- **Never conflate a display value with a functional value on the same
  field.** `DaemonInfo.host` is read both for real TCP connectivity
  (`health_check`/`running_daemon`) and for building the URL shown to a
  user/agent (`base_url()`). Any future "show X differently than the real
  value" feature must substitute at the specific read site that builds the
  *displayed* string, never at the shared underlying field — otherwise the
  connectivity path silently breaks. (2026-07-15,
  `docs/history/learnings/20260715-mdview-hostname-doctor-fix.md`)
- **`crates/mdview-desktop/src/main.rs` duplicates the daemon-spawn / daemon-URL
  logic of `crates/mdview/src/runtime.rs` — it is NOT shared code.** Two known
  drifts already: `ensure_daemon()` (URL building) and the non-detached
  serve-spawn (missing the setsid detach that `runtime.rs::spawn_daemon_detached`
  now has). Before changing either in `runtime.rs`, grep the desktop crate for
  the same shape and apply the fix there too. (2026-07-15,
  `20260715-mdview-hostname-doctor-fix.md`, `20260715-daemon-auto-spawn-detach.md`)
- **Detachment must be proven, not assumed from a function name.** For any
  backgrounded child in this repo, the daemon must survive its spawner's
  session/process-group teardown — verify with
  `ps -o pid,ppid,sid,pgid -p <pid>`: `sid == pid` (own session leader) and
  `ppid == 1` (reparented to init). If `sid` equals the spawner's session, the
  detach did not happen. Use the setsid(unix)/creation-flags(windows) form from
  `runtime.rs::spawn_daemon_detached`, never stdio-null-only `.spawn()`.
  (2026-07-15, `20260715-daemon-auto-spawn-detach.md`)
- **Rust CLI E2E testing in this repo:** never invoke `./target/...` directly
  (blocked by the scout hook) and never let `HOME` overrides break rustup.
  Use: `cd <scratch-dir> && HOME=<fake> RUSTUP_HOME=/home/vantt/.rustup
  CARGO_HOME=/home/vantt/.cargo cargo run -q --manifest-path
  <repo>/Cargo.toml --bin mdview -- <args>` — cwd of the child process is the
  scratch dir, so cwd-relative behavior (e.g. `doctor`'s AGENTS.md/CLAUDE.md
  handling) is exercised correctly. (2026-07-15, same learnings file)
  **This binary has no dedicated config-path override — `HOME` (read via
  `dirs::home_dir()`) is the only isolation lever.** Guessing a plausible but
  wrong env var name (or forgetting to set `HOME` at all) produces no error:
  the child process silently resolves the REAL `~/.mdview`, so a "scratch"
  test can mutate the live daemon's config/registry for real. After any
  manual run meant to be isolated, spot-check `mdview status` / `mdview list`
  against the real `~/.mdview` before trusting nothing leaked. (2026-07-16,
  `20260716-ui-polish-settings-sidebar.md` — a real incident: a bad first
  attempt overwrote the live config's port and registered a scratch project
  into the live registry; caught and reverted before capping.)
  **This manual recipe is for an agent interactively probing behavior**
  (exploring, a validating-phase spike) — it is not something `cargo test
  --workspace` can run or fail on. When a cell needs *automated, CI-safe* e2e
  coverage of a binary in this repo, use `env!("CARGO_BIN_EXE_<bin-name>")`
  inside a real `#[test]` under `crates/<crate>/tests/` instead — it spawns
  the actual compiled binary as part of the normal test run, satisfying
  "verify must be a runnable command" (AGENTS.md critical rule 2) for genuine
  e2e behavior, not just unit-level. (2026-07-16,
  `20260716-hostname-port-truth.md`)
- **A cell that forbids a live-timing test on a polling/timeout code path
  must explicitly authorize extracting the fallback decision into a pure,
  parameter-injected helper function** — a cold worker cannot infer that a
  refactor is the only way to satisfy "prove this without a live sleep."
  Two independent review-tier subagents (plan-checker, cell-reviewer) caught
  this gap from different angles on the same cell during validating: the fix
  was `fn bind_fallback(lock: Option<DaemonInfo>, cfg: &Config) -> (String,
  u16)` in `runtime.rs`, unit-tested with in-memory values only — never
  writing the real global lock path (`~/.mdview/daemon.lock`) from a test,
  and never sleeping the real poll window. Any future cell testing a
  timing/polling branch needs the same explicit extraction step written into
  its `action`, not just implied by its `plan.md`. (2026-07-16,
  `20260716-hostname-port-truth.md`)
- **After a `git filter-repo` history rewrite + force-push, syncing the local
  working directory with `git reset --hard origin/<branch>` silently deletes
  any file that was tracked-and-clean (no uncommitted diff) at the old HEAD
  but is absent from the rewritten tree.** `git stash` only protects files
  with an uncommitted diff — it does nothing for unmodified tracked files, so
  reset --hard removes them from disk with zero warning. Before resetting a
  working dir onto a rewritten history, either (a) restore missing paths
  afterward from a pre-rewrite backup clone via `rsync -a --ignore-existing`
  (never overwrite anything already present — that could be today's newer,
  still-uncommitted content), or (b) do the whole rewrite on a fresh clone and
  never `reset --hard` the real working directory at all. (2026-07-15,
  `docs/history/gitignore-purge-bee-distill-history/plan.md`)
- **A path-based security check (extension allowlist, exclusion, permission)
  must run on the canonicalized path, never on the raw request/URL segment.**
  `asset_path` checks the file extension on `canonical` (post-symlink-
  resolution), not `rel_path` — a symlink named e.g. `pretty.png` pointing at
  `.env` would canonicalize to the real target and bypass a check done on the
  pre-resolution name, silently reopening whatever the check exists to close.
  Any future path-based guard in this repo needs the same ordering, proven by
  a `#[cfg(unix)]` symlink regression test, not just a traversal test.
  (2026-07-16, `20260716-fix-review-p1-findings.md`)
- **The `mdview` binary crate layer (`cli.rs`, `mcp.rs`, `server.rs`,
  `views.rs`, `watch.rs`) has a standing habit of manual/live-E2E-only
  verification — `mdview-core` does not.** Three behavior-change cells in
  this layer were capped with a prose `verify` field ("live E2E, fake
  EDITOR") instead of a runnable command, and the same failure mode almost
  recurred inside the very fix pass meant to close them. When capping a
  `behavior_change` cell touching this layer, treat "verify must be a
  runnable command" (AGENTS.md critical rule 2) as load-bearing and convert
  to a `#[cfg(test)]` unit test wherever the logic doesn't actually require a
  live server — most of it doesn't. (2026-07-16,
  `20260716-fix-review-p1-findings.md`)
- **The rendered DOM comes from TWO sources: `views.rs` (server markup) and
  `app.js` (client-injected elements).** Any UI change scoped by reading only
  the server markup is blind to widgets `app.js` builds at runtime — the
  fuzzy-jump palette (`.jump-*`) and mermaid controls (`.mermaid-controls`) are
  created via `el.className = …` in `app.js`, so an `app.css` rewrite mapped
  from `views.rs` alone silently dropped their styling. Before any CSS
  rewrite/restyle, grep `app.js` for `className`/`classList.add` and cover those
  classes too. Relatedly, when a change's observable effect is a string/attribute
  (a CSS class, an HTML attribute, a `concat!`), assert it with a grep-based
  verify (`grep -q 'data-scheme'`, `! grep -q "getAttribute('data-theme')"`) —
  `cargo test --workspace` alone proves nothing about a non-logic edit. (2026-07-16,
  `20260716-adopt-atelier-design-system.md`)
- **A backlog row's itemized examples ("host + port on one row", a named
  value, a specific field) are individually load-bearing acceptance
  criteria, not illustrative color.** PBI-15 ("Tối ưu layout form Settings")
  named "host + port chung một hàng" as an example and was marked `done`
  after `polish-settings-form-1` shipped fieldset/card/legend restyling —
  but never actually put Host+Port (or Debounce+Max-file-size) on one row.
  The gap sat unnoticed until the user re-reported the exact same request
  later. Before capping a cell against a backlog row that names concrete
  examples, check each one literally in the rendered output — a thematic
  match ("it's restyled") is not evidence a named example was delivered.
  (2026-07-16, `20260716-ui-polish-settings-sidebar.md`)
- **Mermaid owns `pre.mermaid`'s innerHTML — never append overlays inside it.**
  `mermaid.run()` sets `element.innerHTML = <svg>` and may do so more than once
  (intermediate state, and again on theme re-render). Any control/toolbar
  appended as a *child of the `pre`* is silently wiped by a later innerHTML
  write, and if enhancement is guarded by a one-shot flag (a `.zoomable` class
  on the `pre`) it is never re-added — so you get `class present, controls
  gone` with no error. Fix: wrap the `pre` in a sibling container
  (`.mermaid-wrap`) and hang the toolbar on the *wrapper*; use the wrapper's
  presence as the idempotency guard; and in pan/zoom handlers query
  `pre.querySelector('svg')` fresh each call (mermaid may swap the svg). This
  cost many round-trips because the failure was client-only and silent
  (diagram renders fine, buttons just missing) — the thing that finally cracked
  it was a temporary on-page diagnostic overlay dumping DOM state
  (`hasSVG`/`zoomable`/`hasControls` + caught enhance error), since no server
  check and no headless tool available here could observe post-render client
  DOM. Reach for an on-page diagnostic early for "renders but a client-built
  widget is missing" bugs. (2026-07-17, mermaid-touch/vendor/zoom work)
- **Vendor client libs that must work on a LAN/offline daemon — and never
  inline a large JS bundle into `<script>…</script>`.** Mermaid was loaded from
  a CDN whose ESM build fetches more chunks at runtime; on a `0.0.0.0`/LAN or
  offline host those never arrive, so diagrams never render (→ no svg → no
  zoom). Fix: vendor the self-contained UMD build (`assets/mermaid.min.js`,
  served at `/static/mermaid.min.js`, loaded via `<script src>`), not the ESM.
  Separately: building a self-contained *preview* by inlining that 3.4 MB
  bundle into `<script>…</script>` silently breaks it — the bundle contains a
  literal `</script>`, which closes the tag early and leaves `window.mermaid`
  undefined. Inlined previews of pages that load big vendored scripts are
  therefore misleading; serve the real external files (e.g. a throwaway
  `python -m http.server`) instead. (2026-07-17, same work)
- **Cross-crate platform-conditional logic: extract vs. duplicate is decided by
  the *target* crate's CI coverage, not by "is this shared code."** D1
  (`is_wildcard`, a 2-line predicate) was correctly duplicated into
  `mdview-core/src/daemon.rs` rather than shared with `runtime.rs`'s existing,
  already-tested copy. D3 (`apply_detach`) was correctly extracted into
  `mdview-core/src/process.rs` — re-exported from `runtime.rs` via
  `pub(crate) use mdview_core::process::apply_detach;` so its existing test
  needed zero changes — specifically because `mdview-desktop` has zero
  compile coverage anywhere in this repo's CI; writing that logic directly in
  `main.rs` would have made it permanently unverifiable. Same shape of
  decision, opposite correct answers: check the target crate's coverage
  before choosing, not just whether the code "looks shared." A blanket
  "no coverage → always extract" rule is also wrong (would have wrongly
  recommended extracting D1's trivial predicate too). (2026-07-20,
  `20260720-windows-daemon-fixes.md`)
- **A cell's test-recipe action must cite the actual success/failure predicate
  of the function under test, not just the test's setup shape.** A cell
  drafted from CONTEXT.md's "bind a real `0.0.0.0` listener and assert
  `health_check` finds it" phrasing instructed a bare `TcpListener` with no
  responder — but `health_check`'s real success check
  (`buf.contains("\"mdview\"") || buf.contains("200 OK")`) needs an actual
  HTTP response body, so the recipe as drafted could never pass. Two
  independent validating-phase reviewers (plan-checker, cell-reviewer)
  converged on the identical root cause from different angles — read the
  target function's body and state its real predicate in the cell action at
  authoring time, not only as something validating is expected to catch.
  (2026-07-20, `20260720-windows-daemon-fixes.md`)
- **Superseding an old locked decision whose original CONTEXT.md/plan.md no
  longer exists must say so explicitly, citing whatever secondary source
  establishes the old lock existed.** D1 supersedes `multi-ip-urls-1`
  (PBI-04)'s prior prohibition on touching `health_check`, but
  `docs/history/multi-ip-urls-1/` no longer exists and no decision-log entry
  records the original lock — the only surviving anchor is prose in another
  feature's review-findings report plus a backlog row with no `[history](...)`
  link. The supersession here was well-reasoned (independently checked
  against the PBI-04 backlog row's own text), but a future agent re-deriving
  "what exactly was locked and why" from a pruned history dir has only a
  paraphrase to work from. Name the gap in the new decision's rationale rather
  than presenting the supersession as though the original were still directly
  checkable. (2026-07-20, `20260720-windows-daemon-fixes.md`)
