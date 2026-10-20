---
title: mdview update refreshes the skill after swapping the binary
date: 2026-10-20
summary: update now runs a narrow post-update doctor step through the new binary instead of full doctor --fix
---

# mdview update refreshes the skill after swapping the binary

## What happened
Question: should `mdview update` also restart the daemon and run `doctor --fix`? Restart was already done. Full `doctor --fix` was rejected after reading doctor.rs: its agent-instruction check upserts the MDView block into AGENTS.md/CLAUDE.md in the current directory (creating them), so running it from `update` anywhere would litter files; permission and MCP registration do not depend on the version.

## Decision
- New hidden `mdview doctor --after-update`, run by `update` through the new binary (the templates are compiled in, so the old process would write old content).
- It refreshes ~/.claude/skills/mdview/SKILL.md only if it already exists, reports an outdated block in the current directory's AGENTS.md/CLAUDE.md without writing, and reports running `mdview mcp` processes.
- `mdview mcp` is not a proxy: it opens the SQLite index and runs the engine in its own process, so a daemon restart does not upgrade it; the agent must reconnect MCP.
- Under sudo the step is skipped (HOME is root's) and the user is told to run restart and doctor --fix themselves.
- Kept the instruction block duplicated in AGENTS.md and CLAUDE.md: each agent loads only one, both come from one template, and `@AGENTS.md` would import the user's whole file.

## Verification
Unit tests for the ps parser and stale-block detection; e2e with a fake HOME: an outdated skill was refreshed, an outdated AGENTS.md block was reported and left untouched, a running `mdview mcp` was counted, and a missing skill was not created.

## Next steps
MCP detection is skipped on Windows (no args in the process list without extra deps).

> Historical work record — not durable authority. Prefer docs/specs/ADRs for current decisions.
