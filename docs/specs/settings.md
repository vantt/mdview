---
area: settings
updated: 2026-07-16
sources: [mdview-hostname-doctor-fix, hostname-port-truth]
decisions: [07c1ac9f, bcfcf737]
coverage: partial
---

# Spec: Settings

The single place to view and change mdview's local configuration: server
binding, the renderer theme, indexing behavior, and the MCP integration. There
is one operator (whoever runs mdview on their own machine) and no
authentication — anyone who can reach the settings page can change it.

## Entry Points & Triggers

- `/settings` (browser) → the settings page, always read fresh from the
  configuration file on disk (so it reflects the last save even before the
  running server restarts).
- `/settings?saved=1` → the same page with a "Saved" confirmation banner,
  reached automatically right after a successful save.
- The settings page shows the running application version (`mdview v<version>`)
  in a footer — the same single-source version reported by the CLI and `/health`.
- Save button on the settings page → posts the form, then redirects back to
  `/settings?saved=1`.
- Start-up CLI overrides (`serve --host <host> --port <port>`) → persist the
  given value(s) into configuration before the server starts, without visiting
  the settings page.
- `mdview config edit` (CLI) → opens the configuration file in the operator's
  `$EDITOR` (a full-file text edit, all fields at once — the terminal counterpart
  to the `/settings` form). The file is created with current/default values first
  so nothing is blank; after saving, an invalid edit is warned about (the server
  would otherwise ignore it and fall back to defaults) and the operator is told to
  restart the daemon to apply. Same "changes apply on restart" rule as R2.

## Data Dictionary

| # | Element | Meaning | Values | Required | Default |
|---|---|---|---|---|---|
| 1 | Port | TCP port the viewer server listens on | 1–65535 | yes (a value below 1 is silently ignored, keeping the previous port) | 7700 |
| 2 | Host | The address the viewer server actually binds and is health-checked on | any bindable host/IP, e.g. `127.0.0.1` (local only) or `0.0.0.0` (reachable from the LAN) | no (blank/whitespace-only is ignored, keeping the previous host) | `127.0.0.1` |
| 3 | Display hostname | Optional stand-in for Host used only in links handed to a person or an agent (see R1) | any hostname string, or left blank | no | blank (unset) |
| 4 | Open browser on start | Whether a browser tab opens automatically when the server starts | on / off | no | off |
| 5 | Theme | Overall light/dark appearance of rendered pages | `system` — follows the OS/browser preference · `light` · `dark` | no (an unrecognized value is ignored, keeping the previous theme) | `system` |
| 6 | Syntax highlight theme | Color theme used for fenced code blocks | any theme name known to the renderer | no (blank is ignored, keeping the previous value) | `github-dark` |
| 7 | Debounce (ms) | How long the indexer waits after a file change before re-indexing it | milliseconds, ≥0 | no | 200 |
| 8 | Max file size (MB) | Files larger than this are skipped by the indexer | megabytes, ≥1 | no (a value below 1 is ignored, keeping the previous size) | 10 |
| 9 | Exclude patterns | Folder/file name patterns the indexer never scans | one pattern per line; blank lines and surrounding whitespace are dropped | no | `.git`, `node_modules`, `.venv`, `target`, `dist` |
| 10 | MCP enabled | Whether the agent-integration tool is available | on / off | no | on |
| 11 | MCP transport | How an agent's MCP client talks to mdview | `stdio` · `http` | no (an unrecognized value is ignored, keeping the previous value) | `stdio` |

## Behaviors & Operations

### View settings

- **Triggers:** opening `/settings`.
- **What happens:** the page is built from whatever is on disk right now, not
  from the currently-running server's in-memory copy.
- **Side effects:** none.
- **Afterwards:** the operator sees every current value pre-filled, plus a
  note that Server/Indexing/MCP changes need a restart (`mdview stop && mdview
  serve`) to take effect.

### Save settings

- **Blocked when:** nothing blocks the save outright; individual fields that
  fail their own validation (row 1–2, 5–8, 11 above) are silently dropped from
  the update and the previous value is kept for that field only — the rest of
  the form still saves.
- **What changes:** every accepted field is written to the configuration file
  immediately.
- **Side effects:** none sent to any other actor; the already-running server
  process (if any) keeps using the configuration it started with until it is
  restarted.
- **Afterwards:** the operator is redirected to `/settings?saved=1` and sees
  the confirmation banner.

### Start with CLI overrides

- **Triggers:** `mdview serve --host <h>` and/or `--port <p>`.
- **What changes:** the given value(s) are saved to configuration before the
  server binds, so they take effect on this very start (no separate save +
  restart needed).
- **Side effects:** none.
- **Afterwards:** the server binds on the (possibly just-overridden) host/port.

## Actors & Access

Not applicable in the role sense — there is exactly one actor (the local
operator running mdview) and no login; anything reachable at the bound
Host/Port can view and change every setting. The only other party is a
consuming system: an MCP client (e.g. Claude Code) that receives Display
hostname's effect indirectly, as part of the URL returned by the Agent
integration area.

## Business Rules

- **R1 (per D 07c1ac9f).** When Display hostname is set to a non-blank value,
  every link handed back to a person or an agent (the browser URL from the
  CLI `open` command, and the URL returned by the `mdview_view_file` MCP tool)
  uses Display hostname in place of Host. The server's actual bind address and
  its own health check always use Host — Display hostname is a display-only
  substitution and never changes what address the server listens on or is
  reachable at.
- **R2.** Server, Indexing, and MCP setting changes only take effect after the
  running server is stopped and restarted; the settings page states this
  explicitly at save time.
- **R3.** When Host is a wildcard bind (any-interface, e.g. `0.0.0.0`) **and**
  Display hostname is blank, every link-returning entry point — the
  `mdview_view_file` MCP tool, and the CLI `mdview open`/`mdview restart`
  commands (per D bcfcf737) — returns **one viewable link per reachable
  machine IP** (loopback and link-local excluded) instead of a single
  unusable wildcard link, so a caller on another host can pick an address
  that routes to it. A non-blank Display hostname (R1) or a specific
  (non-wildcard) Host still yields a single link everywhere. This is the same
  display-only substitution as R1: it never changes the real bind/connect
  address. Every one of these entry points keeps a single primary link (the
  first one) for compatibility with a caller expecting one link, and adds the
  full list alongside it: the MCP tool's structured result keeps `url` and
  adds `urls`; the CLI's `--json` output does the same.

## Edge Cases Settled

- Display hostname left blank (or made blank again after being set) falls
  back to Host for link-building — this is how the feature is turned back off.
- A settings form field with an invalid value (e.g., an out-of-range number,
  an unrecognized theme/transport) never fails the whole save — that one field
  is dropped and the rest of the form still saves.

## Open Gaps

- The exact page/response shown when the settings form itself cannot be
  parsed at all (e.g., a non-numeric value submitted for a numeric field) was
  not exercised this session — unverified.
- Whether/how a currently-running server picks up a changed Display hostname
  without a full restart was not tested (Business Rule R2 groups it with the
  other Server settings by inference, not by direct observation).

## Visuals

No settled screenshot captured for `/settings` yet — see Open Gaps.

## Pointers (implementation)

- `crates/mdview-core/src/config.rs` — `Config`/`ServerConfig`/etc. structs,
  defaults, load/save (`~/.mdview/config.toml`, TOML, atomic write).
- `crates/mdview/src/server.rs` — `settings_page_handler`, `SettingsForm`,
  `update_config` (routes `/settings`, `/api/config`).
- `crates/mdview/src/views.rs` — `settings_page` (form rendering).
- `crates/mdview/src/runtime.rs` — `ensure_daemon_bases`/`build_display_urls`/
  `machine_ipv4s` (single entry point for both Display hostname substitution
  and R3's multi-IP link list; used by every link-returning caller — the
  single-URL `ensure_daemon_base`/`display_base_url` helpers were retired
  once the last single-URL caller switched over).
- `crates/mdview-core/src/daemon.rs` — `DaemonInfo`/`base_url`/`health_check`
  (the real bind/connect host, unaffected by Display hostname).
- `crates/mdview/src/cli.rs` — `Command::Serve { port, host }`, `cmd_serve`
  (CLI override path); `cmd_open`/`cmd_restart` (R3 multi-IP `--json` output:
  `url` + `urls`).
- `crates/mdview/tests/e2e_open.rs` — end-to-end coverage: a real daemon is
  started and `mdview open --json` is asserted to return the daemon's actual
  bound port, not a stale configured one.
