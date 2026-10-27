# Off camera: start claude once in the demo project and trust the folder, so takes never show
# the first-run safety check. Run through render.sh (output is thrown away).
Output video/hero-demo/out/warmup.gif
Source video/hero-demo/tapes/settings.tape
Type `cd "$DEMO_PROJECT" && clear`
Enter
Sleep 500ms
Type "claude"
Enter
Wait+Screen@60s /trust this folder|Claude Code v\d/
Sleep 800ms
Down
Sleep 300ms
Enter
Sleep 8s
Ctrl+C
Sleep 300ms
Ctrl+C
Sleep 800ms
