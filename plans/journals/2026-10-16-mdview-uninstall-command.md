---
title: mdview uninstall command
date: 2026-10-16
summary: "Added mdview uninstall/remove: plan-confirm removal of doctor integrations, binary, optional data purge"
---

# mdview uninstall command

## What happened
Added `mdview uninstall` (alias `remove`) with `--purge`, `--yes`, `--dry-run` (crates/mdview/src/uninstall.rs). It plans every change from current state, prints it, confirms (non-tty requires --yes), then stops the daemon, reverses `doctor --fix [--mcp]` (Claude permission, MCP entries in ~/.claude.json, ~/.codex/config.toml, ~/.gemini/config/mcp_config.json, /mdview skill, MDView block in the cwd's AGENTS.md/CLAUDE.md), and removes the binary plus PATH symlinks to it. Windows defers deleting the running exe to a detached cmd.

## Review fixes
Review (plans/reports/code-reviewer-261017-0016-mdview-uninstall.md) found real defects, all fixed:
- `config::write_atomic` replaced files with default permissions, so rewriting a 0600 ~/.claude.json made it 0664. It now carries the old permissions over (doctor benefits too).
- Edits were computed before the prompt and could clobber concurrent writes; each step now re-reads at apply time.
- Symlinked CLAUDE.md -> AGENTS.md became plain files, in both uninstall and doctor's write_agent_snippet; both now write through.
- Irreversible steps (purge, binary) are skipped once anything failed; a still-alive daemon counts as failure.
- Windows: canonical `\\?\` vs plain paths broke the "exe inside data dir" check; cmd paths are normalized, and paths with `%`/`"` are left for manual deletion.
- serde_json `preserve_order` enabled so JSON rewrites keep the user's key order.

## Decision
Uninstall only strips the instruction block in the current directory (mirrors doctor); registered projects still carrying it are listed, not edited, because those files are often git-tracked and shared.

## Next steps
Windows paths remain unexercised — verify update/uninstall on Windows after the next release.

> Historical work record — not durable authority. Prefer docs/specs/ADRs for current decisions.
