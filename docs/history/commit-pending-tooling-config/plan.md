---
artifact_contract: bee-plan/v1
artifact_readiness: implementation-ready
mode: tiny
---

# Plan: commit pending tooling/config changes

## Mode gate

0 risk flags: no auth, no data model, no external system, no public contract,
no existing covered behavior touched. Files are local dev tooling config
(Claude Code statusline, Codex hooks, .gitignore), already written and
manually verified working during the session. 3 files → `tiny`.

## Reality check

- MODE FIT: tiny — pure config/tooling, no app logic. PASS
- REPO FIT: matches existing `.claude/`, `.codex/` config conventions already
  in the repo. PASS
- ASSUMPTIONS: local branch is 18 commits behind `origin/main` with 0 ahead
  (verified via `git rev-list --left-right --count main...origin/main`) — a
  clean fast-forward-style rebase is expected, no conflicts anticipated since
  changed files (`.claude/settings.local.json`, `.codex/hooks.json`,
  `.gitignore`) are not touched upstream in the intervening commits (spot
  checked via `git log --oneline` on origin/main). PASS
- SMALLER PATH: none smaller — already the minimal unit (stage, commit,
  rebase, push).
- PROOF SURFACE: `git status` clean after commit, `git push` succeeds.

## Current slice

1. Commit `.claude/settings.local.json` + `.claude/statusline-command.sh` +
   `.claude/statusline-usage.mjs` (statusline feature) — already done this
   session.
2. Commit `.codex/hooks.json` (SubagentStart audit hook) — already done this
   session.
3. Commit `.gitignore` (bee runtime dirs) — already done this session.
4. `git pull --rebase origin main` to replay local commits onto upstream.
5. `git push`.

Note: `AGENTS.md` critical-rule-15 commit was already made directly (docs
lane, no gate needed).

## Verify

`git status` shows clean tree and `git log` shows local commits on top of
latest `origin/main`; `git push` exits 0.
