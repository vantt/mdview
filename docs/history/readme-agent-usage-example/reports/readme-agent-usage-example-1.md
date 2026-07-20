[DONE]

**Outcome:** Added agent-usage example paragraph to README's Use in 30 seconds section, documenting how agents (Claude Code, Codex) can call `mdview_view_file` directly or use the `/mdview` skill command.

**Files touched:**
- `README.md` — added ~8 lines to "Use in 30 seconds" section between auto-start paragraph and SSH paragraph

**Verification:** `grep -q '/mdview docs/spec/prd.md' README.md && grep -q 'mdview_view_file. itself' README.md` — passed ✓

**Trace:** [`.bee/cells/readme-agent-usage-example-1.json`](../../../.bee/cells/readme-agent-usage-example-1.json)
