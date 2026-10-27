# Scene 1: on the server, the agent writes the plan and answers with a path you can't click.
# Run through render.sh (it runs `sandbox.sh plain` first, so the agent has no mdview).
Output video/hero-demo/out/scene1-path.mp4
Source video/hero-demo/tapes/settings.tape

Hide
Sleep 1500ms
# Demo prompt and project; `claude` here runs without any MCP server (so no mdview link).
Type `cd "$DEMO_PROJECT" && export PS1='dev@build-01:\w$ ' && alias claude='command claude --strict-mcp-config' && clear`
Enter
Wait+Line /\$\s*$/
Show

Sleep 800ms
Type "claude"
Sleep 400ms
Enter
Wait+Screen@60s /Claude Code v\d/
Sleep 1500ms
Sleep 1200ms
Type "Write an implementation plan for retrying failed payments to docs/plans/payment-retry.md. Reply in one line."
Sleep 600ms
Enter
# A few seconds of the agent starting, then hide the wait: the edit cuts it anyway, and
# recording minutes of frames makes the render run out of memory.
Sleep 4s
Hide
# VHS's Wait+Screen reads the top of the buffer, not what is on screen once the agent's output
# scrolls, so it can't see the reply: wait a fixed time instead (the agent takes 15-60 s).
Sleep 120s
Show
Sleep 5s

Hide
Ctrl+C
Sleep 300ms
Ctrl+C
Sleep 500ms
