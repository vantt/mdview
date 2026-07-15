---
area: daemon
updated: 2026-07-15
sources: [daemon-auto-spawn-detach]
decisions: [625c69fa]
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

## Data Dictionary

| # | Element | Meaning | Values | Default |
|---|---|---|---|---|
| 1 | Running state | Whether a daemon is currently up and answering | running / not running | — |
| 2 | Bind host | Address the daemon actually listens on | any bindable host/IP | `127.0.0.1` |
| 3 | Port | Port the daemon listens on (auto-increments if taken) | 1–65535 | 7700 |
| — | Daemon record (not shown) | The on-disk marker identifying the one live daemon: its process id, host, port, start time | — | — |
| — | Readiness wait (not shown) | How long an auto-start waits for the new daemon to answer before giving up and returning a best-effort URL | ~2 seconds | — |

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
- **Afterwards:** it serves until interrupted or `mdview stop`.

### Single-daemon coordination

- **What it guarantees:** at most one daemon owns the registry at a time; every
  launcher (CLI, agent integration, desktop shell) checks the daemon record and
  reuses a live daemon instead of starting another (per R2).
- **On a stale record:** if the record names a process that is no longer
  answering, it is treated as "not running" and a fresh daemon may be started;
  the stale record does not block startup.

### Stop (`mdview stop`)

- **What changes:** the running daemon is terminated and its record removed.
- **Afterwards:** `mdview status` reports "not running"; the next `open`/agent
  call auto-starts a new one.

## Actors & Access

| Capability | Local operator (CLI) | Agent (MCP) | Desktop shell | Browser tab |
|---|---|---|---|---|
| Auto-start a daemon | ✓ (via `open`) | ✓ (via `view_file`) | ✓ | — |
| Explicitly start/stop | ✓ | — | — | — |
| Be served by the daemon | ✓ | ✓ | ✓ | ✓ |

## Business Rules

- **R1 (per D 625c69fa).** An auto-started daemon runs in its own session
  (detached from the launching process's session and process group), so it
  survives the exit of the command or agent that started it, and a session
  close (terminal hang-up) or a process-group signal aimed at the launcher does
  not stop it. Only an explicit stop or a machine restart ends it.
- **R2.** At most one daemon owns the registry at a time; launchers reuse a live
  daemon and never start a second concurrent one.

## Edge Cases Settled

- Configured port already in use → the daemon tries the next ports in sequence
  rather than failing outright.
- Daemon record present but the process is dead → treated as not running; a new
  daemon can start.
- `mdview serve` while one is already running → no-op with a message, never a
  duplicate daemon.

## Open Gaps

- The exact number of fallback ports tried, and what the operator sees if all
  are taken, is not restated here (implementation detail, not yet spec-settled).
- Behavior when two auto-starts race within the readiness window (two callers,
  no daemon yet) is not characterized — the single-daemon guarantee is asserted
  by R2 but the race timing was not exercised this session.

## Visuals

Not applicable — background process, no screen.

## Pointers (implementation)

- `crates/mdview/src/runtime.rs` — `ensure_daemon_base` (auto-start + readiness
  wait), `spawn_daemon_detached` (session detach: Unix `setsid`, Windows
  `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`).
- `crates/mdview-core/src/daemon.rs` — the daemon record (`~/.mdview/daemon.lock`),
  `running_daemon`, `health_check`.
- `crates/mdview/src/server.rs` — `serve` (bind with port auto-increment, write
  the record).
- `crates/mdview/src/cli.rs` — `serve` / `status` / `stop` commands.
