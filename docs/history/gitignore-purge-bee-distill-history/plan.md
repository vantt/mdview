---
artifact_contract: bee-plan/v1
artifact_readiness: implementation-ready
mode: small
---

# Purge bee/distill skill code from git tracking + full history

## Scope (locked, D-ID 508d9502)

1. Add `.gitignore` entries so `.bee/` (minus what's already selectively ignored),
   `.claude/skills/bee-*`, `.agents/skills/bee-*`, `.claude/skills/distill`,
   `.agents/skills/distill` are never tracked again.
2. Rewrite the entire git history (`git filter-repo --invert-paths`) to remove
   those same paths from every commit, past and present, on every branch/tag.
3. Keep `docs/distillery/*` and `plans/reports/distill-*.md` untouched — output,
   not skill code.
4. Force-push the rewritten `main` and `feat/mvp-implementation` to `origin`,
   plus rewritten `v0.1.0`/`v0.1.1` tags.

## Mode gate

Flags: **external systems** (GitHub remote force-push) + **existing covered
behavior** (rewrites history everyone with a clone already depends on) = 2 →
would mechanically land on `standard`, but the actual protection standard mode
buys — informed human sign-off before an irreversible action — was already
obtained directly: the user was shown the full survey (28 commits, 14 touching
these paths, 2 tags, 2 remote branches) and explicitly chose "xoá thật khỏi
lịch sử (filter-repo)" + "rewrite cả tag" via AskUserQuestion. Running this as
`small` with one more concrete go/no-go right before the force-push (the actual
point of no return) honors the same intent without redundant ceremony.

## Approach

1. Mirror-clone `origin` to a scratch dir as an untouched backup (kept until
   the user confirms the result is good).
2. Second, separate full clone (all branches/tags) in scratch — filter-repo
   runs there, never on the actual working directory.
3. `git filter-repo --invert-paths --path .bee/ --path-glob '.claude/skills/bee-*/' --path-glob '.agents/skills/bee-*/' --path .claude/skills/distill/ --path .agents/skills/distill/`
   on that clone. This rewrites every ref (both branches, both tags) in one pass.
4. Verify in the rewritten clone: `docs/distillery/*` and `plans/reports/distill-*.md`
   still present; none of the purged paths appear in any commit
   (`git log --all -- .bee` empty); both tags still resolve; both branches
   still resolve.
5. Add the `.gitignore` entries as one commit on top of the rewritten `main`.
6. Re-add the `origin` remote (filter-repo drops it) and force-push
   `main`, `feat/mvp-implementation`, and both tags.
7. Back in the actual working directory: `git stash` the 8 currently-modified
   non-bee files (the just-finished host_name/doctor feature, still
   uncommitted), `git fetch origin`, `git reset --hard origin/main`, `git stash pop`.
   Untracked `.bee/*` runtime files on disk are unaffected either way (bee
   keeps working locally — only git tracking changes).

Risk: LOW for steps 1-5 (nothing pushed, fully reversible — the scratch clone
can be deleted and nothing on `origin` has changed yet). risk concentrates
entirely in step 6 (the force-push itself is the irreversible action) — a
second explicit go/no-go is asked immediately before it, separate from step 7's
local sync which only touches already-stashed/safe local state.

## Verification

- `git log --all --oneline -- .bee .claude/skills/bee-* .agents/skills/bee-* .claude/skills/distill .agents/skills/distill` on the rewritten clone → empty.
- `git log --all --oneline -- docs/distillery plans/reports` on the rewritten clone → non-empty, same content as before.
- `git tag` and `git branch -a` on the rewritten clone → same names, valid.
- After push + sync: `git status` in the working dir shows the 8 non-bee files
  still modified exactly as before (diff-identical), nothing lost.
- `node .bee/bin/bee.mjs status --json` still runs fine afterward (bee's on-disk
  files are untouched, only untracked-by-git now).
