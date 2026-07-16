# mdview — Usage Guide

Practical guide to installing and running mdview on Linux/macOS, including the
common **SSH-into-a-server** workflow.

- Web mode is the primary way to use mdview (works great over SSH).
- Desktop mode is a native window and needs a graphical display.

---

## 1. Install

### From a release (recommended, once published)
```bash
curl -fsSL https://raw.githubusercontent.com/vantt/mdview/main/install.sh | sh
```
The script detects your OS/arch, downloads the `mdview` binary, and puts it on
your PATH. Check the latest release at
<https://github.com/vantt/mdview/releases>.

### From source (works anytime; needs Rust)
```bash
cargo install --git https://github.com/vantt/mdview mdview
# or, inside a clone:
cargo build --release -p mdview     # binary at target/release/mdview
```
The CLI/daemon does **not** need any GUI system libraries.

---

## 2. Web mode (recommended)

```bash
mdview register /path/to/project     # recursive scan + index
mdview open README.md                # print a URL — auto-starts the daemon if needed
mdview status                        # is it running?
```

The daemon **auto-starts** on the first `open` or MCP call and keeps running in
its own session (it survives the terminal that launched it). You only need
`mdview serve` to pre-start it or to bind a custom host/port — see §4.

Open <http://localhost:7700> in a browser. You get: project list, per-file
rendering with **cross-folder links that don't 404**, live reload on file
change, full-text search, backlinks, table of contents, and breadcrumbs.

### Running on a remote Ubuntu server over SSH
The server binds to `127.0.0.1` by default. To view it from your laptop,
forward the port:

```bash
# on your laptop:
ssh -L 7700:localhost:7700 user@ubuntu-host
# then open http://localhost:7700 in your laptop's browser
```

Alternatively, expose it on the LAN (less secure — no auth):
```bash
mdview serve --host 0.0.0.0
# then browse http://<server-ip>:7700 from another machine on the network
```

---

## 3. Agent integration (Claude Code / MCP)

```bash
mdview doctor --fix     # register the MCP server in ~/.claude.json (backs up first)
mdview doctor           # re-check: PATH, config, daemon, MCP registration
```

After that, an agent calls the single tool
**`mdview_view_file(project_root, relative_path)`** and gets a browser URL back.
The project is auto-registered on first use — no separate registration step.

Drop the snippet from [`mdview-agents-template.md`](mdview-agents-template.md)
into your project's `AGENTS.md` / `CLAUDE.md` so agents surface a viewable URL
after writing docs.

---

## 4. CLI reference

```bash
mdview serve [--port 7700] [--host 0.0.0.0]     # optional: pre-start the daemon (auto-starts otherwise)
mdview register <dir> [--name "My App"]         # index a project
mdview open <file.md>                           # print the browser URL for a file
mdview list                                     # list projects
mdview search "query" [--project <id>]          # full-text search
mdview refresh [<project-id>]                   # re-scan to reconcile the index
mdview status                                   # daemon status
mdview config edit                              # edit ~/.mdview/config.toml in $EDITOR
mdview unregister <project-id>                  # remove a project (files kept)
mdview stop                                     # stop the daemon
mdview restart                                  # restart the daemon (apply config changes)
mdview doctor [--fix] [--json] [--dry-run]      # diagnose & repair integration
```
Most commands accept `--json` for scripting.

---

## 5. Settings

Two ways to change settings — server host/port, theme, indexing (debounce, max
file size, exclude patterns), and MCP options — both writing the same
`~/.mdview/config.toml`:

- **Web UI:** open <http://localhost:7700/settings> and edit the form.
- **CLI / editor:** run `mdview config edit`. It opens `~/.mdview/config.toml`
  in your `$VISUAL`/`$EDITOR` (falling back to `vi`, or `notepad` on Windows),
  pre-filled with the current values. On save it validates the TOML and warns if
  it's broken (an invalid file is ignored — mdview falls back to defaults until
  you fix it).

Server/indexing changes apply after a daemon restart — `mdview restart` (or
`mdview stop && mdview serve`).

---

## 6. Desktop app (needs a display)

The desktop shell is a native window onto the same daemon. It requires a
graphical session — it will **not** run over a bare SSH connection.

```bash
cd crates/mdview-desktop
cargo run                 # on a desktop session
```

Prerequisites (Linux): `libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev
libjavascriptcoregtk-4.1-dev libdbus-1-dev librsvg2-dev build-essential`.

Over SSH you have two options:
- Use **web mode** instead (recommended), or
- Enable X forwarding: `ssh -X user@ubuntu-host` then `cargo run`.

Run over a bare SSH session and mdview prints a helpful message with the web URL
instead of crashing.

Closing the window hides it to the system tray (the daemon keeps serving); quit
from the tray to stop it.

---

## 7. Where things live

- `~/.mdview/registry.db` — project registry + file index (SQLite)
- `~/.mdview/config.toml` — configuration
- `~/.mdview/daemon.lock` — the single-daemon coordination file

Only registered project roots are served; requests are path-traversal guarded.
mdview never writes to your files — it is a read-only viewer.
