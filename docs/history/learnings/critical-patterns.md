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
- **`crates/mdview-desktop/src/main.rs::ensure_daemon()` duplicates
  `crates/mdview/src/runtime.rs::ensure_daemon_base()` — it is NOT shared
  code.** Before changing daemon-URL-building logic in `runtime.rs`, grep the
  desktop crate for the same shape; it will not pick up the change
  automatically and can silently drift. (2026-07-15, same learnings file)
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
