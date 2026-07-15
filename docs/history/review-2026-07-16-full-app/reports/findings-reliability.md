# Reliability Review — mdview (daemon lifecycle, process detach, external CDN, watcher)

**Scope reviewed:** the cumulative diff (focus: `runtime.rs`, `cli.rs` restart/stop/spawn, `server.rs`, `watch.rs`, `app.js`, `views.rs` Mermaid loader, `daemon.rs`, `engine.rs`, `indexer.rs`), plus `docs/specs/daemon.md`, the three `plan.md` files, and cell traces (`mdview-restart-1`, `daemon-auto-spawn-detach-1`, etc.).

Single-user local dev tool; severities weighted to real blast radius. **No P1** — no data-loss or silent-corruption path in the reliability surface. `watch.rs`/`indexer.rs` sit in the **un-celled pre-bee scaffold** (no on-disk verification evidence); F2 is drawn from the diff alone and flagged as such.

---

### [P2] `mdview restart` readiness wait is a no-op — port drift and a transient double daemon   (autofix_class: manual)

**What it does:** `cmd_restart` calls `stop_daemon()` (which sends the kill signal **and immediately deletes the lock**), then runs a wait loop whose predicate is `runtime::running_daemon().is_none()`. `running_daemon()` = `read_lock()? && health_check(...)`; since the lock was just deleted, `read_lock()` returns `None`, so the loop breaks on the **first** iteration — no actual wait. The old process may still hold port 7700; the new `serve` runs `bind_with_retry` starting at the configured port and **auto-increments** to 7701 if 7700 is still held.

**Why it's a problem:** the comment "Wait for the old process to actually exit so the port and lock are free" cannot be delivered — it polls the lock `stop_daemon` just erased, not process liveness. `mdview config edit` tells users "Restart the daemon to apply: mdview restart", so a config change meant to stay on 7700 can silently move the server to 7701, breaking existing tabs/URLs, plus a brief two-daemon window.

**Failure scenario:** daemon on 7700 (pid A) → `restart` → SIGTERM to A + lock deleted → wait loop breaks in <1ms while A still holds 7700 → new `serve` (B) binds 7701 and writes a lock for 7701 → server is now on 7701 with no message; both alive transiently.

**Evidence:**
- `crates/mdview/src/cli.rs` ~347-357 (`stop_daemon` ends with `runtime::remove_lock();`) and ~374-385 (`cmd_restart` wait loop tests `running_daemon().is_none()`). The predicate depends on the lock deleted one call earlier.
- `crates/mdview/src/server.rs` ~361-369 (`bind_with_retry`: `for p in port..port.saturating_add(10)`), which silently increments the port.

**Fix:** poll the **old pid's** liveness (Unix `kill(pid,0)` / Windows `OpenProcess`) or the old port until it refuses, before spawning; optionally warn if the new daemon's port ≠ configured. `stop_daemon` already returns the pid.

**Acceptance:** after `stop_daemon`, restart blocks until the old pid is gone (bounded timeout) before spawning; restarting when the configured port is free re-binds the same port every time; a port change prints a visible notice.

---

### [P2] Filesystem watcher covers only projects present at daemon start; later-registered projects never live-reload, and watch errors are swallowed   (autofix_class: manual)

**What it does:** `serve()` calls `spawn_watchers()` exactly once at boot, iterating `engine.list_projects()` at that instant, each root via `.watch(...).ok()`. Nothing re-invokes it. The MCP server (`mdview mcp`) and CLI `register`/`open` run in **separate processes** and write new projects into the shared `registry.db`; the daemon serves them (reads the DB per request) but its watcher never learns of them.

**Why it's a problem:** live-reload + incremental reindex (README: "Edits on disk live-reload the page"; FR-08/09) silently do not happen for any project registered after the daemon is up — the common agent flow, since the daemon auto-starts on the *first* `view_file` and later calls register *new* projects. The user edits a doc and the page never refreshes, with no hint why. Separately, `.watch(...).ok()` discards errors like inotify exhaustion (`ENOSPC`), so even an in-scope project can silently fail to watch.

**Failure scenario:** agent `view_file` for X → daemon auto-starts, watches {X}. Later `view_file` for Y (new) → served fine. Editing `Y/docs/foo.md` → no reindex, no reload signal until `mdview restart`.

**Evidence:**
- `crates/mdview/src/server.rs` ~45: `let _watch = crate::watch::spawn_watchers(...)?;` — constructed once, no re-scan hook.
- `crates/mdview/src/watch.rs` ~3 (comment "known at daemon start"), ~19-30 (one-time `for project in engine.list_projects()...` loop), ~38-42 (`.watch(&root, Recursive).ok()`).

**Fix:** diff `list_projects()` against watched roots and `watcher().watch(new_root, Recursive)` the additions (lightweight interval or on-request); at minimum replace `.ok()` with `tracing::warn!`. Needs a shared mutable watched-roots set in `AppState`.

**Acceptance:** a project registered after the daemon is running gets edits reindexed and live-reloaded without restart; a failed `.watch()` warns instead of being dropped.

---

### [P2] Concurrent auto-spawn race leaks an orphaned daemon holding a port   (autofix_class: manual)

**What it does:** `ensure_bind()` checks `running_daemon()`, and if none, spawns — with **no cross-process mutual exclusion**. Each spawned `serve` runs `bind_with_retry` (auto-increment) then `write_lock()` (atomic temp+rename, last-writer-wins, no existing-daemon check).

**Why it's a problem:** two racing callers (`open` + agent `view_file`) each see "no daemon" and spawn. First binds 7700, second binds 7701; both write the lock, last wins. The daemon on the losing port is invisible to `status`/`stop` (they read the lock) → leaked process holding 7701 until reboot. Violates R2. `docs/specs/daemon.md` → "Open Gaps" explicitly acknowledges this race was never exercised.

**Evidence:**
- `crates/mdview/src/runtime.rs` ~24-34 (`ensure_bind`: `let _ = spawn_daemon_detached(); for _ in 0..20 { sleep; if running_daemon()... }`) — no lock/lease before spawn.
- `crates/mdview-core/src/daemon.rs` ~31-35 (`write_lock` → `write_atomic`, atomic replace, no exclusive-create / health check).

**Fix:** serialize spawns with an exclusive filesystem lock (`create_new(true)` or `flock`), re-check `running_daemon()` under it, only the holder spawns; on the daemon side, `serve` exits if a healthy daemon already answers the port. Add a staleness timeout so a crashed spawn-lock doesn't wedge future starts.

**Acceptance:** two near-simultaneous auto-spawns yield exactly one live daemon and one lock; a second `serve` while a healthy daemon answers the port exits without binding a second port.

---

### [P2] Mermaid renderer loads from a CDN with no fallback, retry, or failure signal; the observer waits forever   (autofix_class: advisory)

**What it does:** when a page has Mermaid blocks, `views.rs` injects an unguarded remote ESM import (`https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs`) — no `onerror`, no timeout, no local fallback. In `app.js`, each `pre.mermaid` without an SVG gets a `MutationObserver` that disconnects only on SVG success.

**Why it's a problem:** this is the product's one external network dependency, with a "hope it loads" posture. In an air-gapped or CDN-blocked env (offline laptop, corporate proxy) — adjacent to the README's documented remote/SSH workflows — every Mermaid page silently degrades to raw text with zero feedback, and one MutationObserver per diagram leaks forever (never disconnected). No retry/backoff, no "diagram failed" affordance. (Only saving grace: the theme toggle's re-render is guarded by `if (window.__mermaid)`, so it no-ops.)

**Evidence:**
- `crates/mdview/src/views.rs` ~83-94 (`head_extra` Mermaid `<script type="module">` import, unguarded).
- `crates/mdview/assets/app.js` ~369-373 (`new MutationObserver(... if svg { disconnect })` — disconnects only on success).

**Fix:** wrap the import in `try/catch` (or `.catch` on dynamic `import()`) and mark failed `pre.mermaid` blocks with a visible "diagram could not be rendered (offline?)" note; give the app.js observer a bounded timeout. For true offline support, vendor mermaid to `/static`. (SRI/supply-chain framing → security reviewer.)

**Acceptance:** with the CDN unreachable, the page visibly indicates a diagram failed; a never-rendering `pre.mermaid` has its observer disconnected within a bounded time.

---

### [P3] `stop`/`restart` kill the daemon without graceful shutdown; the daemon's own lock cleanup never runs   (autofix_class: advisory)

**What it does:** `shutdown_signal()` awaits only `tokio::signal::ctrl_c()` (SIGINT). `stop_daemon` sends `kill <pid>` (default SIGTERM) / `taskkill /F`. SIGTERM is unhandled → hard termination; the `runtime::remove_lock()` after `axum::serve(...)` never runs on this path.

**Why it's a problem:** the graceful-shutdown machinery is effectively dead code for the normal stop route (only interactive Ctrl+C on a foreground `serve` exercises it). Low impact — `cmd_stop` removes the lock itself, stale locks self-heal via `health_check`, SQLite writes are atomic — but an externally delivered SIGTERM can interrupt an in-flight reindex batch and leaves a stale lock relying entirely on the health check.

**Evidence:** `crates/mdview/src/server.rs` ~69: `async fn shutdown_signal() { let _ = tokio::signal::ctrl_c().await; }`.

**Fix:** also await SIGTERM (Unix `SignalKind::terminate()`) selected alongside `ctrl_c()`.

**Acceptance:** `mdview stop` triggers the daemon's graceful-shutdown path (daemon removes its own lock / logs shutdown) rather than a hard kill.

---

### [P3] Auto-spawn errors and kill-by-PID are silent about real failure modes   (autofix_class: advisory)

**What it does:** `ensure_bind()` uses `let _ = spawn_daemon_detached();` — a failed launch (e.g. `current_exe()` error, fork failure) falls through to returning `(cfg.host, cfg.port)` as a best-effort URL with no log. `stop_daemon()` sends `kill info.pid` with no identity check, despite `DaemonInfo.started_at` being available.

**Why it's a problem:** silent spawn failure produces a URL that refuses to connect with no diagnostic (user can't tell mdview failed to start vs. a browser/network issue). Kill-by-raw-PID exposes a PID-reuse mis-kill of an unrelated process — low probability locally but a genuine correctness hazard in the process-management focus area.

**Evidence:** `crates/mdview/src/runtime.rs` ~27 (`let _ = spawn_daemon_detached();`); `crates/mdview/src/cli.rs` ~347-357 (`Command::new("kill").arg(info.pid...)` with no verification).

**Fix:** `tracing::warn!`/stderr on spawn `Err` (surface to `cmd_open`); best-effort verify process identity before signaling, or explicitly document the PID-reuse limitation in the daemon spec.

**Acceptance:** a failed spawn produces a visible warning; the PID-reuse limitation is mitigated or documented.

---

### [P3] `bind_with_retry` failure message can overflow `u16` at high ports   (autofix_class: gated_auto)

The loop bounds use `port.saturating_add(10)` but the `bail!` message uses `port + 10` directly — at `port >= 65526` this overflows `u16` (debug panic / release wrap), garbling the error text exactly when it fires. Blast radius: a bad log line.
**Evidence:** `crates/mdview/src/server.rs` ~361-369: `anyhow::bail!("no free port in {port}..{}", port + 10);`.
**Fix:** use `port.saturating_add(10)` in the message too.

---

## Checked and judged NOT a defect
- **WebSocket reconnect** (`app.js` `setTimeout(connect, 3000)` on close): unbounded fixed-interval reconnect is *correct* here — it's what lets a tab recover after `mdview restart`. No backoff cap needed.
- **Config load resilience** (`config.rs` `load_from`): corrupt → defaults with a warning, atomic temp+rename save — solid.
- **Indexer per-file resilience** (`indexer.rs` `index_file`): unreadable/oversize files skipped (`Ok(None)`) rather than aborting the scan; `compute_file_links`/`reindex_links` read with `unwrap_or_default()` (empty links until next edit, self-correcting).

Status: DONE
Summary: 4 P2 (restart no-op wait/port drift, watcher misses later-registered projects, concurrent-spawn orphan-daemon race, Mermaid CDN no-fallback) and 3 P3 (SIGTERM not handled, silent spawn error + PID-reuse kill, port-message overflow); no P1.
