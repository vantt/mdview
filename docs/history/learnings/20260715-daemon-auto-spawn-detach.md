---
date: 2026-07-15
feature: daemon-auto-spawn-detach
categories: [process-lifecycle, cross-platform]
severity: [medium]
tags: [daemon, setsid, spawn, detach]
---

# Learnings: reliable daemon auto-spawn (full setsid detach)

## What Happened

`spawn_daemon_detached()` was named "detached" but performed no session
detachment — it only redirected stdio to null and `.spawn()`ed. The auto-spawned
daemon inherited the launcher's session/process-group, so it could die on a
SIGHUP (terminal/session close) or a process-group-directed SIGTERM. This made
the already-existing auto-spawn feel unreliable, pushing developers to run
`mdview serve` manually. Fixed with Unix `setsid` (via `pre_exec`) and Windows
`DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`.

## Root Cause

A misleading function name masked a missing syscall. "Detached" in the name was
taken as done; nobody checked that setsid/new-session was actually happening.
The failure mode is invisible in the happy path (clean parent exit reparents the
child to init and it survives) and only appears under session/group teardown —
so it never showed up in casual testing.

## Recommendation

- **Prove detachment, don't assume it from a name.** The one-line proof for any
  backgrounded child in this repo: `ps -o pid,ppid,sid,pgid -p <pid>` — a truly
  detached daemon shows `sid == pid` (it is its own session leader) and
  `ppid == 1` (reparented to init). If `sid` equals the spawner's session, the
  detach did not happen regardless of what the function is called.
- When adding any new background spawn in this repo, use the same detach form as
  `runtime.rs::spawn_daemon_detached` (setsid on unix, creation_flags on
  windows) — do not copy the old stdio-null-only pattern.
- `crates/mdview-desktop/src/main.rs` still has the old non-detached
  serve-spawn; apply the same fix there when the desktop path is next touched.
