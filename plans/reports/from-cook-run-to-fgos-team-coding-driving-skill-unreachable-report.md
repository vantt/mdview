# fgos-coding-driving unreachable from a session using fgOS as an external plugin

## Symptom

Running `/fgOS:cook <task>` from a Claude Code session working in a repo
that only has the `fgOS` plugin installed globally (not a forgentX
checkout itself — e.g. `mdview`) fails at step 2 with:

```
Skill(fgos-coding-driving)
Error: Unknown skill: fgos-coding-driving
```

`submit` and `pick`/claim succeed first; the failure is specifically the
skill hand-off `cook`'s own SKILL.md instructs (`plugins/fgOS/skills/cook/
SKILL.md`, step 2: "invoke the `fgos-coding-driving` skill for it").

## Root cause (verified by reading files directly, not guessed)

1. `fgos-coding-driving` and its sibling dev-skills (`fgos-coding-
   exploring`, `fgos-coding-planning`, `fgos-coding-validating`,
   `fgos-coding-implement`, `fgos-coding-discovering`, `fgos-clarifying`,
   `fgos-researching`) only exist as **project-local** skills at
   `forgentX/.claude/skills/<name>/SKILL.md` (confirmed: `find
   forgentX/.claude/skills -iname 'fgos-coding-driving'` finds it there,
   and in every `.claude/worktrees/*/.claude/skills/` copy — but NOT
   under `forgentX/plugins/fgOS/skills/`, which only contains the
   launcher/command skills — `submit`, `discover`, `plan`, `pick`, `cook`,
   `move`, `return`, etc. — plus `coding-shape`/`coding-shape-distill`).
2. Claude Code's `Skill` tool only resolves names from the currently
   loaded project's own skills plus installed plugins. A session whose
   project root is `mdview` (fgOS installed there only as the `fgOS`
   plugin) never sees forgentX's own project-local `.claude/skills/`
   directory — so any launcher skill's instruction to invoke a
   `fgos-coding-*` dev-skill fails there, even though the exact same
   command works fine from inside forgentX itself.
3. Separately, and compounding the confusion: the `fgos` CLI itself, run
   from a repo with no local `bin/fgos.mjs` (any non-forgentX checkout),
   falls back to a globally npm/pnpm-installed `forgent` package
   (resolved here to `~/.local/share/pnpm/global/.../node_modules/
   forgent/bin/fgos.mjs`). That installed snapshot is evidently older
   than forgentX's current HEAD: it still runs the **pre-retrofit** stage
   set (`clarify -> discover -> decompose -> executing`, `schema_version
   2.0`), not HEAD's current one (`discovery -> exploring -> planning ->
   executing`, with `clarify` fully retired per decision `tsk-qod`
   D1/D2 — confirmed by reading `forgentX/src/state/
   workflow-stage-graphs.mjs`, which documents the retirement in detail).
   Verified directly against the installed binary:
   - `fgos discover --verdict` only accepts `"clear"`/`"unclear"` (the
     OLD `clarify`-stage judgment), not a `discovery`-stage verdict.
   - `fgos decompose --verdict` only accepts `"pass-through"`/
     `"need-human"`/`"decompose"` — the pre-rename verb (`decompose`,
     not `plan`).
   So even a session that COULD reach the dev-skills would find their
   HEAD-written prose (which talks about `discovery`/`exploring`/
   `planning`) doesn't match the actually-installed engine's real stage
   names/verdicts.

## Repro

From any repo that is not a forgentX checkout, with the `fgOS` plugin
installed:

```
/fgOS:cook <any free-text task>
```

Submit + claim succeed; the `fgos-coding-driving` hand-off in `cook`'s
step 2 (or `discover`'s step 3, or `pick`'s equivalent step) throws
`Unknown skill: fgos-coding-driving`.

## Questions for the fgOS team (not prescribing the fix)

- Should `fgos-coding-driving` and its sibling dev-skills be packaged
  INTO the `fgOS` plugin (`plugins/fgOS/skills/`) so they travel with it
  to every install, the way `coding-shape`/`coding-shape-distill`
  already are? That would make `cook`/`discover`/`plan`/`pick` actually
  work as documented outside forgentX.
- If dev-skills are meant to stay forgentX-internal on purpose (e.g. they
  assume a full monorepo checkout for `fgos-researching`/heavy tooling),
  should the launcher skills detect "not inside a forgentX checkout" and
  fail with a clear, actionable error instead of a bare `Unknown skill`
  — or fall back to a verb-only flow, since the CLI verbs themselves ARE
  portable (published in the `forgent` npm package)?
- Should `fgos doctor` (or an equivalent check) detect and warn when the
  resolved `fgos` binary is an older/stale snapshot relative to what the
  installed plugin's skill prose assumes — e.g. comparing `schema_version`
  or a stage-name it expects to exist? Right now the mismatch is silent:
  the CLI just quietly runs the old FSM (`clarify`/`discover`/`decompose`)
  under skills written for the new one (`discovery`/`exploring`/
  `planning`).

## Workaround used this session

Read the dev-skill file directly via the `Read` tool
(`forgentX/.claude/skills/fgos-coding-driving/SKILL.md`, bypassing the
`Skill` tool entirely since it isn't resolvable), then drove the item's
stages by hand through the raw `fgos` verbs the skill would have called
(`fgos discover --verdict clear --verify "<cmd>"`, `fgos decompose
--verdict pass-through`, `fgos pick`, `fgos return`), matching the
OLD/installed engine's real stage names and verdict enums (probed via
each verb's own `--help` output and one deliberately-invalid `--verdict`
call, which returns the accepted enum in its error message without
mutating state). This reached `awaiting-approval` correctly, but it
required manually re-deriving what `fgos-coding-driving`'s loop would
have done — not a fix, just a way to still finish one item.
