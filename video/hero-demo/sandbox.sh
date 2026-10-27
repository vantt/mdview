#!/usr/bin/env bash
# Demo machine "build-01" for the hero video, without touching your own mdview.
#
#   video/hero-demo/sandbox.sh up      # copy the fixture, write config, start the demo daemon
#   video/hero-demo/sandbox.sh reset   # put the fixture docs back (after a take edits them)
#   video/hero-demo/sandbox.sh plain   # scenes 1-2: hide mdview from agents, remove the plan; `up` restores both
#   video/hero-demo/sandbox.sh down    # stop the demo daemon
#   video/hero-demo/sandbox.sh shell   # bash with the demo prompt, in the demo project
#
# It runs a second mdview daemon with its own HOME (config, registry, lock), on
# port 7710, that prints `build-01` in its links. The demo project gets a
# `.mcp.json` whose `mdview` server points at that daemon, so an agent started in
# the project hands out build-01 links while your own mdview keeps running as is.
#
# Settings (all optional):
#   DEMO_DIR    where the sandbox lives (default ${TMPDIR:-/tmp}/mdview-hero-demo)
#   DEMO_PORT   demo daemon port (default 7710)
#   DEMO_HOST   hostname shown in links (default build-01)
#   DEMO_TOKEN  login token of the demo daemon (default hero-demo)
#   MDVIEW_BIN  mdview binary (default: mdview on PATH)
#
# Browsers must resolve DEMO_HOST to this machine: add `127.0.0.1 build-01` to
# /etc/hosts (and to C:\Windows\System32\drivers\etc\hosts when the browser runs
# on Windows next to WSL). record.mjs maps it inside Chromium on its own.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEMO_DIR="${DEMO_DIR:-${TMPDIR:-/tmp}/mdview-hero-demo}"
DEMO_PORT="${DEMO_PORT:-7710}"
DEMO_HOST="${DEMO_HOST:-build-01}"
DEMO_TOKEN="${DEMO_TOKEN:-hero-demo}"
MDVIEW_BIN="${MDVIEW_BIN:-$(command -v mdview || echo mdview)}"
demo_home="$DEMO_DIR/home"
project="$DEMO_DIR/acme-payments"

demo_mdview() { HOME="$demo_home" "$MDVIEW_BIN" "$@"; }

copy_docs() {
  rm -rf "$project/docs"
  cp -r "$here/fixture/acme-payments/docs" "$project/docs"
}

up() {
  mkdir -p "$demo_home/.mdview" "$project"
  # House plan template, plus the mdview instructions (`plain` drops the latter).
  cat "$here/fixture/acme-payments/CLAUDE.md" "$here/fixture/acme-payments/CLAUDE.mdview.md" > "$project/CLAUDE.md"
  copy_docs
  cat > "$demo_home/.mdview/config.toml" <<EOF
[server]
port = $DEMO_PORT
host = "0.0.0.0"
hostname = "$DEMO_HOST"
open_browser_on_start = false
web_secret = "$DEMO_TOKEN"

[mcp]
enabled = true
transport = "stdio"

[renderer]
theme = "dark"
EOF
  # Project-scoped MCP server named like the user-level one, so it wins inside the project.
  cat > "$project/.mcp.json" <<EOF
{
  "mcpServers": {
    "mdview": {
      "command": "env",
      "args": ["HOME=$demo_home", "$MDVIEW_BIN", "mcp"]
    }
  }
}
EOF
  # Agent settings for the takes, scoped to the demo project: no permission prompts on camera.
  # Edits are accepted; shell commands are denied (the agent reads with its file tools), since a
  # shell approval prompt would otherwise stall a take until it times out.
  mkdir -p "$project/.claude"
  cat > "$project/.claude/settings.local.json" <<'EOF'
{
  "permissions": {
    "defaultMode": "acceptEdits",
    "allow": ["mcp__mdview__mdview_view_file"],
    "deny": ["Bash"]
  },
  "enableAllProjectMcpServers": true,
  "promptSuggestionEnabled": false,
  "env": { "CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION": "0" }
}
EOF
  demo_mdview restart >/dev/null
  demo_mdview open "$project/docs/plans/payment-retry.md"
  echo "demo daemon: http://$DEMO_HOST:$DEMO_PORT  (token: $DEMO_TOKEN)"
  echo "project:     $project"
}

down() { demo_mdview stop || true; }

shell() {
  cd "$project"
  # Demo prompt, and `fresh` with the demo config (theme "terminal": the terminal's own colours).
  # No prompt suggestions in claude: its greyed hint ("open it in mdview") would give scene 3 away.
  exec bash --rcfile <(printf 'PS1=%q\nalias fresh=%q\nexport CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=0\n' "dev@$DEMO_HOST:\\w\$ " "fresh --config $here/fresh.json") -i
}

case "${1:-}" in
  up) up ;;
  reset) copy_docs; echo "fixture docs restored in $project/docs" ;;
  plain)
    # Scenes 1-2: no mdview for the agent, and no plan yet (it writes one; the fixture's would
    # make it ask about overwriting).
    cp "$here/fixture/acme-payments/CLAUDE.md" "$project/CLAUDE.md"
    rm -f "$project/.mcp.json" "$project/docs/plans/payment-retry.md"
    echo "mdview hidden from agents and plan removed in $project (run 'up' to restore)" ;;
  down) down ;;
  shell) shell ;;
  *) sed -n '2,8p' "$0"; exit 2 ;;
esac
