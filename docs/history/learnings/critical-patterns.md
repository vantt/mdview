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
