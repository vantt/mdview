# MCP proxy, Claude Desktop registration, slimmed agent text

Status: DONE_WITH_CONCERNS

- `mdview mcp` is now a thin stdio proxy: exe resolved once at startup, each tool call runs `<exe> view-file --json -- <root> <rel>` (CREATE_NO_WINDOW on Windows); no Engine/SQLite in mcp.rs.
- Hidden `view-file` subcommand shares `print_view` with `open` (no duplicated URL building); adds `rel_path`.
- doctor: removed MCP-reconnect hint and dead process counting; added `MCP · Claude Desktop` check (macOS, Windows, WSL via wsl.exe); `register_json_mcp` takes the entry.
- uninstall: also removes the entry from Claude Desktop configs.
- Agent block and skill template are CLI-only; CLAUDE.md/AGENTS.md blocks match the template.
- install.sh/install.ps1 hints, README.md, docs/usage.md updated.

Verification: fmt, clippy -D warnings, cargo test --workspace (165 + 6 + 163 passed) all clean. Smoke test with temp HOME and port 17731: initialize, tools/list, ping, tools/call all correct; temp daemon stopped; user daemon 2145667 untouched.
Not verified: Windows target (`cargo check --target x86_64-pc-windows-msvc` fails on bundled SQLite: lib.exe missing).
