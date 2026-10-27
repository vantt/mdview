#!/usr/bin/env bash
# Record the hero video's scenes, from the repository root: 1+2 and 3 with VHS, 4-6 with Playwright.
#
#   video/hero-demo/tapes/render.sh [warmup|scene12|scene3|browser|scene1 ...]
#   (default: warmup scene12 scene3 browser: the whole video, scenes 1-6, in order)
#
# The agent really runs (claude, logged in as you), in the demo project made by sandbox.sh.
# `warmup` starts claude once off camera in the demo project and trusts the folder, so the
# first-run safety check never shows in a take.
# scene3 leaves the agent's plan in place and `browser` records scenes 4-6 on it, so cuts match.
# `scene1` (scene 1 alone) is kept for reference. Videos go to video/hero-demo/out/.
set -euo pipefail
# Started from inside another Claude Code session, these would show a warning in the take.
unset CLAUDECODE CLAUDE_CODE_CHILD_SESSION CLAUDE_CODE_ENTRYPOINT 2>/dev/null || true
# No greyed prompt suggestion under the reply ("open it in mdview" would give scene 3 away).
export CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=0

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
cd "$repo"
sandbox=video/hero-demo/sandbox.sh
export DEMO_DIR="${DEMO_DIR:-${TMPDIR:-/tmp}/mdview-hero-demo}"
export DEMO_PROJECT="$DEMO_DIR/acme-payments"
export DEMO_KIT="$repo/video/hero-demo"
mkdir -p video/hero-demo/out

for tool in vhs ttyd ffmpeg claude node fresh; do
  command -v "$tool" >/dev/null 2>&1 || { echo "render.sh: $tool not found on PATH" >&2; exit 1; }
done

for scene in "${@:-warmup scene12 scene3 browser}"; do
  for s in $scene; do
    case "$s" in
      warmup)
        "$sandbox" up >/dev/null
        vhs video/hero-demo/tapes/warmup.tape >/dev/null
        rm -f video/hero-demo/out/warmup.gif ;;
      scene1)
        "$sandbox" up >/dev/null
        "$sandbox" plain >/dev/null
        rm -f "$DEMO_PROJECT/docs/plans/payment-retry.md"
        vhs video/hero-demo/tapes/scene1-path.tape
        "$sandbox" up >/dev/null ;;
      scene12)
        "$sandbox" up >/dev/null
        "$sandbox" plain >/dev/null
        vhs video/hero-demo/tapes/scene12-path-hunt.tape
        "$sandbox" up >/dev/null ;;
      scene3)
        # Leaves the agent's plan in place: scenes 4-6 (browser) record that same file, so the
        # cut from the link to the page matches.
        "$sandbox" up >/dev/null
        rm -f "$DEMO_PROJECT/docs/plans/payment-retry.md"
        vhs video/hero-demo/tapes/scene3-link.tape ;;
      browser)
        node video/hero-demo/record.mjs ;;
      *) echo "render.sh: unknown scene $s" >&2; exit 2 ;;
    esac
  done
done
