# windows-installer-ps1-1 and windows-installer-ps1-2 — worker report (Tim)

## windows-installer-ps1-1 — Create install.ps1

- Status: [DONE]
- Outcome: Created `install.ps1` at the repo root mirroring `install.sh` 1:1 for Windows per D1 (same `MDVIEW_INSTALL_DIR`/`MDVIEW_VERSION` env vars, same download-else-cargo-fallback logic, PATH instruction only, never silently modified).
- Deviation (auto-fixed bug, rule 1): the cell's verbatim script text used backslash-escaped quotes (`\"`) inside a PowerShell double-quoted string on the `SetEnvironmentVariable` hint line. Backslash is not an escape character in PowerShell string literals (PowerShell uses backtick `` ` ``), so this failed to parse. Fixed by switching to backtick-escaped quotes (`` `" ``), preserving the exact same rendered output.
- Files touched: `install.ps1`
- Commit: `0ca966c` — feat(windows-installer-ps1-1): add install.ps1 Windows installer
- Full trace/evidence: `.bee/cells/windows-installer-ps1-1.json`

## windows-installer-ps1-2 — Add Windows install instructions to README

- Status: [DONE]
- Outcome: Added a "Windows (PowerShell)" subsection to `README.md` immediately after the existing `curl | sh` block and before "Or from source (needs Rust):", containing the `irm ... install.ps1 | iex` one-liner and `mdview doctor --fix`. No other README content changed.
- Deviations: none.
- Files touched: `README.md`
- Commit: `10524cd` — docs(windows-installer-ps1-2): add Windows install instructions to README
- Full trace/evidence: `.bee/cells/windows-installer-ps1-2.json`
