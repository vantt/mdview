---
area: doctor
updated: 2026-07-15
sources: [mdview-hostname-doctor-fix]
decisions: [864f6f00]
coverage: partial
---

# Spec: Doctor

A self-check the local operator runs to confirm mdview is set up correctly on
this machine, and to have the safe, mechanical parts of setup fixed
automatically instead of by hand.

## Entry Points & Triggers

- CLI: `mdview doctor [--json] [--dry-run] [--fix]`, run from any directory.
  File-based checks (Config, Agent instruction) act on the directory the
  command is run from and the operator's home directory — not on any
  registered project.

## Data Dictionary

Flags:

| # | Element | Meaning | Values | Required | Default |
|---|---|---|---|---|---|
| 1 | `--json` | Emit the check results as a JSON array instead of a human-readable list | on / off | no | off |
| 2 | `--dry-run` | Report only — no check performs any write, even if `--fix` is also given | on / off | no | off |
| 3 | `--fix` | Apply every safe, automatic repair for a check that is not already fine | on / off | no | off |

Checks (each produces one result row: OK / FIXED / MANUAL / WARN, plus a
one-line detail):

| # | Check | Confirms | Auto-fixable |
|---|---|---|---|
| 1 | Binary in PATH | The `mdview` executable can be found on the operator's PATH | No — reported WARN with the current executable's actual location; the operator edits PATH by hand |
| 2 | Config | The configuration file exists and loads | Yes — see Rule R2 below (not gated by `--fix`) |
| 3 | Daemon | A viewer server is currently running and answers its health check | No — reported WARN; the operator starts one with `mdview serve` |
| 4 | MCP registration | mdview is registered as an MCP server for Claude Code | Yes, with `--fix` |
| 5 | Agent instruction | AGENTS.md and/or CLAUDE.md, in the current directory, mention mdview's file-viewing tool | Yes, with `--fix` (per D 864f6f00) |

## Behaviors & Operations

### Run diagnostics (`mdview doctor`)

- **Triggers:** the CLI command, with or without `--json`.
- **What happens:** all 5 checks run in order (PATH, Config, Daemon, MCP
  registration, Agent instruction) and each reports OK / FIXED / MANUAL / WARN
  with a one-line detail.
- **Side effects:** the Config check writes a default configuration file the
  moment one is missing, **whenever `--dry-run` is not given** — see Rule R2;
  this is the one check whose write is not conditional on `--fix`.
- **Afterwards:** a summary line counts MANUAL items and, if any exist,
  suggests re-running with `--fix`; zero MANUAL items prints "All good."

### Apply safe fixes (`mdview doctor --fix`)

- **Triggers:** the CLI command with `--fix` (and without `--dry-run`).
- **What changes:**
  - MCP registration, if not already registered: mdview is added to the
    Claude Code MCP server list, leaving every other registered server
    untouched.
  - Agent instruction, for each of AGENTS.md and CLAUDE.md that is missing the
    mdview mention: the integration snippet is added to that file, creating
    it if it does not exist yet. The two files are handled independently — a
    file that already mentions mdview is left completely untouched.
- **Side effects:** before changing a file that already exists, both the MCP
  registration and the Agent instruction fixes save an untouched copy of it
  first (see Rule R1); a file that is newly created has no such copy, since
  there was nothing to preserve.
- **Afterwards:** re-running `--fix` immediately reports OK for everything
  just fixed — running it twice in a row never duplicates content or fixes
  the same thing again.

## Actors & Access

Not applicable — one local operator runs the command directly; there is no
remote caller and no distinct roles.

## Business Rules

- **R1 (per D 864f6f00).** Before the Agent-instruction fix writes to an
  AGENTS.md or CLAUDE.md that already has content, that original content is
  preserved first (as `<file>.bak`) so nothing an operator wrote is lost; a
  file that doesn't exist yet is simply created with no such copy. This
  mirrors how the MCP-registration fix already protected `.claude.json`.
- **R2.** The Config check is the one check `--fix` does not gate: whenever
  the command is run without `--dry-run`, a missing configuration file is
  always replaced with a fresh default one, whether or not `--fix` was passed.
  `--dry-run` is what prevents this write, not the absence of `--fix`.

## Edge Cases Settled

- Running plain `mdview doctor` (no flags at all) in a directory with no
  configuration file yet **will** create a default one, because Rule R2 is
  not gated by `--fix` — only `--dry-run` prevents it.
- Running `mdview doctor --fix` twice in a row is a no-op the second time for
  every check that passed the first time: no duplicated content, no repeated
  registration entry.
- An AGENTS.md/CLAUDE.md that already mentions mdview (in either file) is
  never rewritten by the Agent-instruction fix, even if the *other* file of
  the pair still needs it — each file's outcome is decided independently.

## Open Gaps

- The exact behavior when `~/.claude.json` exists but its content isn't a
  JSON object (recovery path) was not re-exercised this session; it predates
  this feature's changes.
- Whether the Binary-in-PATH check could itself become auto-fixable was not
  explored — it is currently report-only by design, not evaluated as a gap in
  the fix mechanism.

## Visuals

Not applicable — CLI output only, no screen.

## Pointers (implementation)

- `crates/mdview/src/doctor.rs` — all 5 checks, `run()` entry point.
- `crates/mdview/src/cli.rs` — `Command::Doctor { json, dry_run, fix }` flag
  definitions and dispatch.
- `docs/mdview-agents-template.md` — source text for the Agent-instruction
  fix's snippet (the file's content after its `---` separator is what gets
  copied; the preamble above it is not).
- `~/.claude.json` — the MCP-registration fix's target file.
- `./AGENTS.md`, `./CLAUDE.md` (relative to the command's working directory)
  — the Agent-instruction fix's target files.
