# Scenes 1+2 in one take, so the cut between them can't jump: on the server the agent writes the
# plan and answers with a path you can't click (the click is added in the edit, during the hold
# on the reply); then the hunt: quit the agent, open the editor, find the file in the tree, read
# raw markdown. Run through render.sh (it runs `sandbox.sh plain` first: no mdview, no plan yet).
Output video/hero-demo/out/scene12-path-hunt.mp4
Source video/hero-demo/tapes/settings.tape

Hide
Sleep 1500ms
# Demo prompt and project; `claude` without MCP servers (so no mdview link); `fresh` with the
# demo look (terminal colours, no restored session).
Type `cd "$DEMO_PROJECT" && export PS1='dev@build-01:\w$ ' && alias claude='command claude --strict-mcp-config' && alias fresh="fresh --config $DEMO_KIT/fresh.json" && clear`
Enter
Wait+Line /\$\s*$/
Show

Sleep 800ms
Type "claude"
Sleep 400ms
Enter
Wait+Screen@60s /Claude Code v\d/
Sleep 1500ms
Type "Write an implementation plan for retrying failed payments to docs/plans/payment-retry.md. Reply in one line."
Sleep 600ms
Enter
# A few seconds of the agent starting, then hide the wait (see scene3-link.tape).
Sleep 4s
Hide
Sleep 120s
Show
# Hold on the reply: the edit puts the Ctrl+click (twice, nothing happens) here.
Sleep 4500ms

# The hunt.
Ctrl+C
Sleep 250ms
Ctrl+C
Sleep 900ms
Type "fresh"
Sleep 300ms
Enter
Sleep 1800ms
Ctrl+B
Sleep 900ms
Down
Sleep 400ms
Enter
Sleep 700ms
Down@350ms 3
Sleep 300ms
Enter
Sleep 700ms
Down
Sleep 400ms
Enter
Sleep 1200ms
# Squint: raw markdown, the table and the Mermaid block as text.
Down@180ms 22
Sleep 1500ms
Down@180ms 14
Sleep 2500ms

Hide
Escape
Ctrl+Q
Sleep 800ms
