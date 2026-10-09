---
area: doctor
updated: 2026-07-17
sources: [mdview-hostname-doctor-fix, doctor-multi-agent-mcp]
decisions: [864f6f00]
coverage: partial
---

# Spec: Doctor

A self-check the local operator runs to confirm mdview is set up correctly on
this machine, and to have the safe, mechanical parts of setup fixed
automatically instead of by hand.

## Entry Points & Triggers

- CLI: `mdview doctor [--json] [--dry-run] [--fix] [--mcp]`, run from any directory.
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
| 4 | `--mcp` | Also check (and, with `--fix`, register) the MCP server in each detected agent tool. Without it the three MCP checks report SKIP and existing registrations are left alone — the CLI (`mdview open --json`) is the primary agent path, MCP is for agents without a shell | on / off | no | off |

Checks (each produces one result row: OK / FIXED / MANUAL / WARN / SKIP, plus a
one-line detail). SKIP means the target agent tool is not installed on this
machine, so nothing was written for it — mdview never registers blindly.

| # | Check | Confirms | Auto-fixable |
|---|---|---|---|
| 1 | Binary in PATH | The `mdview` executable can be found on the operator's PATH | No — reported WARN with the current executable's actual location; the operator edits PATH by hand |
| 2 | Config | The configuration file exists and loads | Yes — see Rule R2 below (not gated by `--fix`) |
| 3 | Daemon | A viewer server is currently running and answers its health check, and was started by this binary's version | No — WARN when not running (the operator starts one with `mdview serve`); MANUAL when it is a different version (`mdview restart`) |
| 4 | Index schema | The registry database (`registry-v4.db`) matches the current schema version, read from the file header without opening it — a diagnostic never creates or resets the index. A mismatch only warns: the database is rebuilt on next start. Also reports legacy `registry.db*` files | Only the legacy-file removal, with `--fix` (never under `--dry-run`); the index itself is a disposable cache and is never modified |
| 5 | Claude permission | If Claude Code is present, `Bash(mdview open:*)` is in `permissions.allow` of `~/.claude/settings.json`, so agents can run `mdview open` without a prompt | Yes, with `--fix`; SKIP when Claude Code isn't detected — see Rule R3 |
| 6 | MCP · Claude Code | With `--mcp`: if Claude Code is present, mdview is registered as an MCP server in `~/.claude.json` (`mcpServers`, JSON) | Yes, with `--fix --mcp`; SKIP without `--mcp` or when Claude Code isn't detected |
| 7 | MCP · Codex | With `--mcp`: if Codex is present, mdview is registered in `~/.codex/config.toml` (`[mcp_servers.mdview]`, TOML) | Yes, with `--fix --mcp`; SKIP without `--mcp` or when Codex isn't detected |
| 8 | MCP · Antigravity | With `--mcp`: if Antigravity is present, mdview is registered in `~/.gemini/config/mcp_config.json` (`mcpServers`, JSON — shared by the IDE/CLI/2.0) | Yes, with `--fix --mcp`; SKIP without `--mcp` or when Antigravity isn't detected |
| 9 | Agent instruction | AGENTS.md and CLAUDE.md, in the current directory, carry mdview's current instruction block (marker-delimited). AGENTS.md is the shared instruction file every agent tool (Claude Code, Codex, Antigravity CLI) reads | Yes, with `--fix` |
| 10 | Skill | The global Claude Code skill `~/.claude/skills/mdview/SKILL.md` (the `/mdview <path>` command) is installed and matches the shipped template | Yes, with `--fix`; SKIP when Claude Code isn't detected |

**Detection** (a tool counts as installed when either signal is present):
Claude Code — `~/.claude.json`, a `~/.claude/` directory, or `claude` on PATH.
Codex — a `~/.codex/` directory or `codex` on PATH. Antigravity — a
`~/.gemini/config/` directory or `antigravity` on PATH.

## Behaviors & Operations

### Run diagnostics (`mdview doctor`)

- **Triggers:** the CLI command, with or without `--json`.
- **What happens:** the checks run in order (PATH, Config, Daemon, Index
  schema, Claude permission, MCP · Claude Code, MCP · Codex, MCP · Antigravity,
  Agent instruction, Skill) and each
  reports OK / FIXED / MANUAL / WARN / SKIP with a one-line detail.
- **Side effects:** the Config check writes a default configuration file the
  moment one is missing, **whenever `--dry-run` is not given** — see Rule R2;
  this is the one check whose write is not conditional on `--fix`.
- **Afterwards:** a summary line counts MANUAL items and, if any exist,
  suggests re-running with `--fix`; zero MANUAL items prints "All good."

### Apply safe fixes (`mdview doctor --fix`)

- **Triggers:** the CLI command with `--fix` (and without `--dry-run`).
- **What changes:**
  - Claude Code permission: `Bash(mdview open:*)` is added to
    `permissions.allow` in `~/.claude/settings.json` (the file is created if
    missing). An existing file is backed up first as
    `settings.json.mdview-<unix-time>.bak`, and the write is aborted if the
    backup fails. Every other key, including `permissions.deny`, is preserved;
    a file that is not a JSON object (or has a wrongly typed `permissions`) is
    never written and is reported MANUAL.
  - Legacy index files: `registry.db`, `registry.db-wal` and `registry.db-shm`
    left by an older build are removed. Without `--fix` they are reported (WARN)
    only.
  - MCP registration (only with `--mcp`), per detected tool, if not already registered: mdview is
    added to that tool's MCP server list (Claude Code, Codex, Antigravity),
    leaving every other registered server untouched. A tool that isn't installed
    is skipped entirely — no config file is created for it. The JSON targets
    (Claude, Antigravity) merge into the `mcpServers` object; the Codex TOML
    target is edited format-preserving (`toml_edit`) so existing settings and
    comments survive, and a malformed `config.toml` is reported WARN and left
    untouched rather than clobbered.
  - Agent instruction, for each of AGENTS.md and CLAUDE.md whose managed block
    is missing or out of date: mdview's instruction snippet is written as a
    marker-delimited block (`<!-- mdview:START -->` … `<!-- mdview:END -->`).
    If the markers already exist, only the text between them is replaced in
    place; otherwise the block is appended, creating the file if it does not
    exist yet. Content outside the markers is never touched. The two files are
    handled independently.
  - Skill, if the global `~/.claude/skills/mdview/SKILL.md` is missing or does
    not match the shipped template: the file (and its parent directories) is
    created/overwritten with the current template. Unlike the Agent-instruction
    block, mdview owns this file entirely, so the check is a whole-file content
    match and the fix is a full rewrite — it is global (per-user), not tied to
    the current directory.
- **Side effects:** the MCP-registration fix saves an untouched copy of
  `.claude.json` before changing it (see Rule R1). The Agent-instruction fix
  writes no `.bak`: the marker block bounds exactly what it edits, so
  everything the operator wrote outside the markers is preserved directly.
- **Afterwards:** re-running `--fix` immediately reports OK for everything
  just fixed — running it twice in a row never duplicates content or fixes
  the same thing again.

## Actors & Access

Not applicable — one local operator runs the command directly; there is no
remote caller and no distinct roles.

## Business Rules

- **R1.** Every MCP-registration fix preserves the original config as a `.bak`
  sibling before changing it (`~/.claude.json.bak`, `config.toml.bak`,
  `mcp_config.json.bak`), so nothing an operator configured is lost; a tool that
  isn't installed is never written to at all. The Agent-instruction fix does not
  need this: it edits only the text between its
  `<!-- mdview:START -->` / `<!-- mdview:END -->` markers and leaves all other
  content in place, so there is nothing to preserve separately. (Supersedes the
  `.bak`-for-agent-instruction clause of D 864f6f00, which predated the marker
  block.)
- **R2.** The Config check is the one check `--fix` does not gate: whenever
  the command is run without `--dry-run`, a missing configuration file is
  always replaced with a fresh default one, whether or not `--fix` was passed.
  `--dry-run` is what prevents this write, not the absence of `--fix`.
- **R3.** The Claude permission fix is additive and conservative: it never
  edits or removes `permissions.deny`, never rewrites a settings file it cannot
  parse as a JSON object, and always keeps a timestamped backup of an existing
  file, so it cannot loosen anything the operator restricted.

## Edge Cases Settled

- Running plain `mdview doctor` (no flags at all) in a directory with no
  configuration file yet **will** create a default one, because Rule R2 is
  not gated by `--fix` — only `--dry-run` prevents it.
- Running `mdview doctor --fix` twice in a row is a no-op the second time for
  every check that passed the first time: no duplicated content, no repeated
  registration entry.
- An AGENTS.md/CLAUDE.md whose managed block is already current is left
  untouched; one whose block is present but out of date is rewritten in place
  (only the marker region changes, never a duplicate). Each file of the pair
  is decided independently.

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

- `crates/mdview/src/doctor.rs` — all checks + `run()` (including
  `check_index_schema` and `ensure_claude_permission`); detection helpers
  (`claude_present`/`codex_present`/`antigravity_present`, `bin_on_path`) and the
  two registrars (`register_json_mcp`, `register_toml_mcp`).
- `crates/mdview/src/cli.rs` — `Command::Doctor { json, dry_run, fix }` flag
  definitions and dispatch.
- `docs/mdview-agents-template.md` — source text for the Agent-instruction
  fix's snippet (the file's content after its `---` separator is what gets
  copied; the preamble above it is not).
- MCP targets: `~/.claude.json` (Claude Code), `~/.codex/config.toml` (Codex),
  `~/.gemini/config/mcp_config.json` (Antigravity).
- `./AGENTS.md`, `./CLAUDE.md` (relative to the command's working directory)
  — the Agent-instruction fix's target files.
