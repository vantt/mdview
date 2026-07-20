---
area: daemon
updated: 2026-07-20
sources: [daemon-auto-spawn-detach, hostname-port-truth, windows-daemon-fixes]
decisions: [625c69fa, 1c8473f4, 08b4c8c3, d1429530]
coverage: partial
---

# Spec: Daemon lifecycle

The single background viewer process that owns the project registry and serves
every browser tab, CLI command, and agent call. This area is about how that one
process is started, kept unique, kept alive, and stopped — not about what it
serves (see the web-interface and agent-integration areas for that).

## Entry Points & Triggers

- `mdview open <file>` (CLI) → if no daemon is running, one is started
  automatically before the URL is returned.
- An agent's `view_file` call (MCP) → same automatic start if none is running.
- `mdview serve [--host <h>] [--port <p>]` → starts the daemon explicitly. This
  is optional; its only reasons to exist are pre-starting the daemon or binding
  a non-default host/port.
- `mdview status` → reports whether a daemon is currently running and where.
- `mdview stop` → stops the running daemon.
- `mdview restart` → stops the running daemon (if any) and starts a fresh
  detached one; used to apply config changes to the live server.

## Data Dictionary

| # | Element | Meaning | Values | Default |
|---|---|---|---|---|
| 1 | Running state | Whether a daemon is currently up and answering | running / not running | — |
| 2 | Bind host | Address the daemon actually listens on | any bindable host/IP | `0.0.0.0` (all interfaces — LAN-reachable; the no-auth server prints an exposure warning at startup) |
| 3 | Port | Port the daemon listens on (auto-increments if taken) | 1–65535 | 7700 |
| — | Daemon record (not shown) | The on-disk marker identifying the one live daemon: its process id, host, port, start time | — | — |
| — | Readiness wait (not shown) | How long an auto-start waits for the new daemon to answer before giving up and returning a best-effort URL | ~2 seconds | — |
| — | Best-effort URL (not shown) | The URL returned when the readiness wait expires without the daemon confirming it answers. Per D 1c8473f4: if the daemon's on-disk record already exists at that point (it was written the moment the daemon bound its port, before the daemon starts answering health checks), the returned port is that **real bound port**, even though liveness isn't yet confirmed. Only when no record exists at all does it fall back to the configured port. | real bound port (record present) or configured port (no record) | — |

## Behaviors & Operations

### Automatic start (on first use)

- **Triggers:** a `mdview open` or an agent `view_file` when no daemon is
  running.
- **What changes:** a daemon is launched in the background and, once it answers,
  becomes the single live daemon.
- **Side effects:** the new daemon is fully detached into its **own session** —
  it does not share the session or process group of the command/agent that
  launched it.
- **Afterwards:** the caller gets a viewable URL. The daemon keeps running
  after the launching command exits, after the launching agent/session ends,
  and after the launching terminal closes — until it is explicitly stopped or
  the machine restarts (per R1). The operator never has to run `mdview serve`
  first.

### Explicit start (`mdview serve`)

- **Blocked when:** a daemon is already running — the command reports that and
  does nothing rather than starting a second one (per R2).
- **What changes:** a daemon is started in the foreground of that command,
  binding the configured (or overridden) host/port.
- **On startup it lists every reachable address:** when bound to a wildcard
  host (`0.0.0.0`/`::`) it prints one `http://<ip>:<port>` line per non-loopback
  machine IP (the literal `http://0.0.0.0:PORT` is a dead link); a `hostname`
  override collapses this to that single URL; a concrete bind host prints just
  that one. When the bind is non-loopback it also prints the no-auth exposure
  warning. This is the same display-URL enumeration `open`/`restart` use — a
  display concern only, never the connectivity host.
- **Afterwards:** it serves until interrupted or `mdview stop`.

### Single-daemon coordination

- **What it guarantees:** at most one daemon owns the registry at a time; every
  launcher (CLI, agent integration, desktop shell) checks the daemon record and
  reuses a live daemon instead of starting another (per R2).
- **On a stale record:** if the record names a process that is no longer
  answering, it is treated as "not running" and a fresh daemon may be started;
  the stale record does not block startup.
- **Liveness check on an all-interfaces bind (per D 08b4c8c3):** when the
  daemon is bound to listen on all interfaces (no single specific host), the
  liveness check dials the local loopback address rather than the wildcard
  address itself — dialing the wildcard address directly is unreliable across
  operating systems. This keeps R2's single-daemon guarantee intact regardless
  of which host the daemon is bound to; before this fix, a launcher on some
  operating systems could wrongly conclude an all-interfaces-bound daemon was
  not running and start a duplicate.

### Stop (`mdview stop`)

- **What changes:** the running daemon is terminated and its record removed.
- **Afterwards:** `mdview status` reports "not running"; the next `open`/agent
  call auto-starts a new one.

### Restart (`mdview restart`)

- **What it does:** stops the running daemon (if any), waits for it to exit, then
  starts a fresh **detached** daemon that outlives the command — the way to apply
  a config change (host, port, theme) to the live server in one step.
- **No daemon running:** it simply starts one (nothing to stop).
- **Afterwards:** a new daemon (a different process id) is serving on the
  configured host/port; unlike `serve`, the command returns instead of staying in
  the foreground.

## Actors & Access

| Capability | Local operator (CLI) | Agent (MCP) | Desktop shell | Browser tab |
|---|---|---|---|---|
| Auto-start a daemon | ✓ (via `open`) | ✓ (via `view_file`) | ✓ | — |
| Explicitly start/stop | ✓ | — | — | — |
| Be served by the daemon | ✓ | ✓ | ✓ | ✓ |

## Business Rules

- **R1 (per D 625c69fa; the guarantee extends to every launcher per D d1429530).**
  An auto-started daemon runs in its own session (detached from the launching
  process's session and process group), so it survives the exit of the
  command, agent, or desktop shell that started it, and a session close
  (terminal hang-up) or a process-group signal aimed at the launcher does not
  stop it. Only an explicit stop or a machine restart ends it. This holds
  identically no matter which launcher started the daemon — CLI, agent/MCP, or
  the desktop shell all detach it the same way.
- **R2.** At most one daemon owns the registry at a time; launchers reuse a live
  daemon and never start a second concurrent one.

## Edge Cases Settled

- Configured port already in use → the daemon tries the next ports in sequence
  rather than failing outright.
- Daemon record present but the process is dead → treated as not running; a new
  daemon can start.
- `mdview serve` while one is already running → no-op with a message, never a
  duplicate daemon.
- Configured port already in use (auto-increment landed on a different port)
  **and** the new daemon is still within its readiness wait when a caller asks
  for a URL → the caller still gets the real auto-incremented port (via the
  daemon record, per D 1c8473f4), not the originally-configured one. Before
  this fix, a caller unlucky enough to ask during that window could receive a
  URL pointing at the wrong port.
- Daemon bound to listen on all interfaces (no single specific host) → the
  liveness check still correctly finds it as running (per D 08b4c8c3), so
  `open`/`restart`/agent calls reuse it instead of spawning a duplicate.
- Daemon started by the desktop shell → detaches from its launching session
  exactly like a CLI/agent-started daemon (per D d1429530); it is not tied to
  the desktop app's own process lifetime.

## Open Gaps

- The exact number of fallback ports tried, and what the operator sees if all
  are taken, is not restated here (implementation detail, not yet spec-settled).
- Behavior when two auto-starts race within the readiness window (two callers,
  no daemon yet) is not characterized — the single-daemon guarantee is asserted
  by R2 but the race timing was not exercised this session.

## Visuals

Not applicable — background process, no screen.

## Pointers (implementation)

- `crates/mdview/src/runtime.rs` — `ensure_bind`/`ensure_daemon_bases`
  (auto-start + readiness wait; `bind_fallback` is the pure function deciding
  real-record-port vs. configured-port on a readiness timeout, per D 1c8473f4);
  re-exports the shared `apply_detach` (below) as its own `spawn_daemon_detached`
  detach step.
- `crates/mdview-core/src/process.rs` — `apply_detach` (the session-detach
  guarantee behind R1, per D d1429530: Unix `setsid`, Windows
  `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`), shared by every launcher.
- `crates/mdview-core/src/daemon.rs` — the daemon record (`~/.mdview/daemon.lock`),
  `running_daemon`, `health_check` (dials loopback instead of an all-interfaces
  bind address when checking liveness, per D 08b4c8c3).
- `crates/mdview/src/server.rs` — `serve` (bind with port auto-increment, write
  the record, print every reachable URL via `runtime::display_urls_for`).
- `crates/mdview/src/cli.rs` — `serve` / `status` / `stop` commands.
- `crates/mdview-desktop/src/main.rs` — `spawn_mdview_serve` (the desktop
  shell's launcher; calls the same shared `apply_detach`, per D d1429530).
