---
artifact_contract: bee-plan/v1
artifact_readiness: implementation-ready
mode: tiny
---

# Plan: remove bee from this project

## Mode gate

0 risk flags: no auth, no data model, no external system, no public
contract, no cross-platform concern, no existing covered runtime behavior
touched — pure local tooling/config removal. `tiny`.

## Reality check

- MODE FIT: tiny — deleting config/hook wiring and vendored skill copies,
  no app code. PASS
- REPO FIT: `.claude/settings.json` and `.codex/hooks.json` are 100% bee
  hook commands (verified via grep, every line references
  `.bee/bin/hooks/*.mjs`); `.claude/skills/bee-*` and `.agents/skills/bee-*`
  are gitignored/untracked (`git ls-files` returns 0 for both); `.bee/` is
  gitignored (`git check-ignore` confirms). PASS
- ASSUMPTIONS: user explicitly decided (AskUserQuestion, this session) to
  keep `docs/history/`, `docs/specs/`, `docs/backlog.md`, `docs/decisions/`
  as plain project docs, and to drop the "Local agent tooling" ignore block
  in `.gitignore` along with the BEE block. `AGENTS.md` has a separate
  `mdview:START/END` block outside `BEE:START/END` — verified via
  `sed`/`grep`, stays untouched. `CLAUDE.md`'s `## bee` section only imports
  `AGENTS.md` — no other content there. No CI workflow references bee
  (`.github/workflows/*.yml` checked, no hits).
- SMALLER PATH: none — this is the minimal file set for a complete removal.
- PROOF SURFACE: no `bee`/`.bee` hook wiring left in tracked files; `git
  status` clean after commit; app build/tests unaffected (no source files
  touched).

## Current slice

1. Delete `.claude/settings.json` (git rm).
2. Delete `.codex/hooks.json` (git rm) and the stray `.codex/hooks.json.bak`
   (plain rm, untracked).
3. Delete `.claude/skills/bee-*/` and `.agents/skills/bee-*/` (plain rm,
   untracked/gitignored).
4. Delete `.bee/` (plain rm, gitignored).
5. Strip the `<!-- BEE:START --> ... <!-- BEE:END -->` block from
   `AGENTS.md`, keep everything else (`# mdview`, `- README.md`, the
   `mdview:START/END` block).
6. Remove the `## bee` section from `CLAUDE.md` (heading + import line),
   keep the `mdview:START/END` documentation-viewing section.
7. Remove the `# BEE:START ... # BEE:END` block and the separate "Local
   agent tooling" ignore block from `.gitignore`, keep `# DISTILL:START ...
   # DISTILL:END` and the Rust section.
8. Commit the tracked-file changes (`.claude/settings.json`,
   `.codex/hooks.json`, `AGENTS.md`, `CLAUDE.md`, `.gitignore`) as one
   commit; push.

## Verify

`git status` clean after commit; `grep -rl "bee" .claude/settings.local.json
.codex/hooks.json AGENTS.md CLAUDE.md .gitignore` (post-change) finds no
bee-hook references; `.bee/`, `.claude/skills/bee-*`, `.agents/skills/bee-*`
absent from the filesystem; `git push` exits 0.
