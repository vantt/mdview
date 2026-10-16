---
title: mdview update self-upgrade command
date: 2026-10-16
summary: "Added mdview update: verified GitHub-release download, in-place binary swap, daemon restart"
---

# mdview update self-upgrade command

## What happened
Added `mdview update [--check] [--version <tag>] [--force]` (crates/mdview/src/update.rs). It fetches the GitHub release, picks the `mdview-<target>` asset (same naming as release.yml/install.sh), verifies byte size and the API's `sha256:` digest, smoke-tests `--version` with a timeout, then renames the file over the canonicalized current exe. A running daemon is restarted through the new binary's `restart`.

First e2e run exposed a bug: after the swap, Linux `current_exe()` resolves to `<path> (deleted)`, so spawning the daemon from the updating process failed with ENOENT. Fix: restart via the new file's path; `spawn_daemon_detached` also strips the ` (deleted)` suffix for long-lived processes.

## Decision
Review (plans/reports/code-reviewer-261016-1720-mdview-self-update.md) led to: swap before touching the daemon (a failed swap never leaves it stopped); never restart the daemon as root under sudo; Windows parks the old exe as `.old-<pid>` with checked rollback; HTTP connect/overall timeouts; only `--version` may downgrade.

## Next steps
Windows swap path is untested (no target toolchain locally) — verify on a Windows machine after the next release. Version must be bumped and released before users get the command.

> Historical work record — not durable authority. Prefer docs/specs/ADRs for current decisions.
