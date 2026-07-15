# API-Contract Review — mdview full-app audit

Focus: client-visible contract stability across the CLI (`cli.rs`), HTTP routes (`server.rs`, `views.rs`), the MCP server (`mcp.rs`), and the doctor CLI (`doctor.rs`).

## Framing (read before the severities)

The entire reviewed surface is **net-new in this diff range**. Baseline `d52b4969` is the first pre-bee scaffold commit, and `cli.rs`, `mcp.rs`, `server.rs`, `views.rs`, `doctor.rs`, `runtime.rs` all appear as `new file mode 100644` (e.g. diff line 4192, `@@ -0,0 +1,400 @@`). There is **no prior published contract to break** — nothing is a client-visible breaking change against an earlier version, and mdview is a single-user local tool with no external API consumers yet. Weighed against that blast radius, **all findings are P3**: cross-surface inconsistencies and spec/behavior drift that will bite an agent scripting the JSON/MCP surfaces, not production blockers.

**Pre-bee scaffold range (`d52b4969..5c8d5e6`, no cell, no verification preflight):** the CLI/HTTP scaffolding originates here and was reviewed from the diff alone. It is internally coherent; the naming/shape inconsistencies below were seeded by the scaffold and then propagated (not corrected) by later feature cells — marked explicitly. The two documented intentional contracts (MCP multi-IP `urls` = settings spec R3; `host_name` substitution = R1) are implemented as specified — not findings.

---

### [P3] `mdview open` emits an unusable `http://0.0.0.0:PORT` URL under a wildcard bind (advisory)
`cmd_open` builds its URL from `runtime::ensure_daemon_base()` (singular), which passes the raw bind host through verbatim (cli.rs line 4415-4416; runtime.rs `display_base_url` lines 5293-5303). The MCP path deliberately uses the plural `ensure_daemon_bases()` that expands a wildcard bind into per-IP URLs (mcp.rs line 5181; runtime.rs 5286-5350). The multi-IP fix was scoped to MCP only (cell `multi-ip-urls-2`), and settings spec **R3 names only the MCP tool** — but **R1 explicitly lists "the browser URL from the CLI `open` command" alongside MCP** as consumed URLs. So an operator who follows the settings UI hint to bind `0.0.0.0` (blank Display hostname) and runs `mdview open README.md` gets a dead `http://0.0.0.0:7700/...` link, while the same file over MCP returns real IP links. Default host is `127.0.0.1`, so this only bites LAN binds — hence P3.
**Fix:** route `cmd_open` through `ensure_daemon_bases()` (print first URL in plain mode, add `urls` array in `--json`), OR document in the settings spec that `open` intentionally does not expand wildcards. Either resolves the R1-vs-R3 asymmetry.

### [P3] JSON project-identifier field name is inconsistent (`project_id` vs `id`) (manual)
Same concept, two names: `register --json` → `"project_id"` (cli.rs 4393), `open --json` → `"project_id"` (4420), MCP `structuredContent` → `"project_id"` (mcp.rs 5206); but `list --json` → `"id"` (cli.rs 4457) and `GET /api/projects` → `"id"` (server.rs 5581). An agent that stores `.project_id` from `register` then matches `.projects[].project_id` from `list` always gets null. Seeded by the scaffold, never reconciled. For a tool billed "for AI agent workflows," this is exactly the drift that silently breaks glue code.
**Fix:** pick one name (`project_id` reads clearest and matches MCP/`open`) everywhere; touches `list` and `/api/projects`. Free now (no external consumers).

### [P3] Two divergent "status" JSON shapes (advisory)
CLI `status --json` → `{running, server_url, version, project_count, indexed_file_count}` (cli.rs 4496-4504); HTTP `GET /api/status` → `{running, app, version, project_count, indexed_file_count}` (server.rs 5565-5571). CLI carries `server_url` and no `app`; HTTP carries `app` and no `server_url`, with `running` hard-coded `true`. `/health` adds a third near-duplicate `{status, app, version}` (5559). A client can't treat them interchangeably.
**Fix:** converge on one field set (add `app` to CLI JSON and/or `server_url` to `/api/status`), or document why they differ.

### [P3] MCP advertises protocolVersion `2024-11-05` but returns `structuredContent` with no `outputSchema` (advisory)
`PROTOCOL_VERSION = "2024-11-05"` is negotiated in `initialize` (mcp.rs 5088, 5112), the tool schema declares only `inputSchema` (5132-5147), yet the tool result returns a `structuredContent` object (5198-5208). `structuredContent`/`outputSchema` belong to the newer 2025-06-18 MCP revision. A client strictly honoring the negotiated version may drop `structuredContent`. Mitigated today because the URLs are also in the human-readable `content` text block (the contractual/back-compat path), so nothing breaks — but consumers parsing `structuredContent.urls` rely on out-of-version behavior.
**Fix:** either bump the version to `2025-06-18` and add a matching `outputSchema`, or keep 2024-11-05 and document that the text `content` block is the guaranteed output. Claude Code is the only consumer today, so documenting it is the smaller move.

### [P3] Settings UI and spec still say `mdview stop && mdview serve` after `mdview restart` shipped (advisory)
The settings save banner hard-codes `mdview stop && mdview serve` (views.rs 6221) and the settings spec repeats it (`docs/specs/settings.md` ~7356), while `Command::Restart`/`cmd_restart` now exist and `cmd_config_edit` already tells the operator the correct single command "Restart the daemon to apply: mdview restart" (cli.rs 4268-4270, 4352, 4569). Two parts of the same product give different restart guidance for the same purpose.
**Fix:** change the banner and the spec line to `mdview restart`. One-line HTML + one spec line.

### [P3] Mutating CLI commands (`refresh`, `unregister`, `stop`, `restart`) offer no `--json` (advisory)
`register/open/list/search/status/doctor` all carry a `json` flag; `Refresh/Unregister/Stop/Restart` do not (cli.rs 4262-4287) and print free-form prose (e.g. `cmd_restart` → `"Started daemon (pid {}) at {}"`, with a "not yet confirmed up" fallback line at 4594). An agent that registers/opens via JSON must switch to brittle string-scraping to restart and read the new port. Consistency gap, not a break.
**Fix:** add `--json` to these four emitting small stable objects, or state in the spec which commands are intentionally human-only.

---

## Checked and clean (not findings)
- **MCP multi-IP `urls`** — matches settings spec R3 and cell `multi-ip-urls-2`: `url` = first (back-compat), `urls` = all IPs, `path`/`project_id` retained; no dropped fields (mcp.rs 5198-5208).
- **`host_name` substitution** — display-only per R1; `DaemonInfo.host`/health untouched, unit-tested (runtime.rs 5252-5350; cell `multi-ip-urls-1`).
- **Doctor `--json`** — stable `{check, status, detail}` array, uppercase `OK/FIXED/MANUAL/WARN` enum; `--dry-run`/`--fix` semantics match `docs/specs/doctor.md` R1/R2 (doctor.rs 4655-4662).
- **`doctor --fix` marker block** — idempotent `<!-- mdview:START/END -->` upsert preserving surrounding content (cell `agent-instruction-markers-1`).
- **Config schema** — `#[serde(default)]` on every struct + corrupt→default load makes field additions (`host_name`) forward/backward compatible for `config.toml` and `/api/config` (config.rs 857-990).
- **URL relative-vs-absolute split** — listing surfaces return relative `/p/...`; the two "viewable link" outputs (`open`, MCP `url`) return absolute. Coherent split, not drift.

## Security-adjacent note (out of scope for this reviewer, forwarded to security/reliability)
`GET /api/config` returns the entire serialized `Config` and `POST /api/config` mutates it, both unauthenticated (server.rs 5589-5591, 5620-5671). On a `0.0.0.0` bind anyone on the LAN can read/rewrite settings. The settings spec explicitly accepts this, so it is a documented product decision, not an API-contract defect.

Status: DONE
Summary: 6 findings, all P3 (0 P1, 0 P2) — cross-surface JSON/MCP naming and shape inconsistencies plus spec/behavior drift on an all-greenfield, single-user, no-external-consumer surface; no client-visible breaking changes.
