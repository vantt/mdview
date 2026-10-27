# Scene 3: same machine, same prompt, mdview wired in: the agent answers with a link.
# Run through render.sh (it runs `sandbox.sh up` first). The click and the browser are cut in
# during the edit (scene 4 starts on the opened page).
Output video/hero-demo/out/scene3-link.mp4
Source video/hero-demo/tapes/settings.tape

Hide
Sleep 1500ms
Type `cd "$DEMO_PROJECT" && export PS1='dev@build-01:\w$ ' && clear`
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
# A few seconds of the agent starting, then hide the wait (see scene1-path.tape).
Sleep 4s
Hide
# VHS's Wait+Screen reads the top of the buffer, not what is on screen once the agent's output
# scrolls, so it can't see the reply: wait a fixed time instead (the agent takes 15-60 s).
Sleep 120s
Show
Sleep 6s

Hide
Ctrl+C
Sleep 300ms
Ctrl+C
Sleep 500ms
