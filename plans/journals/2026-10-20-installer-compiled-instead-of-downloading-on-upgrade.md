---
title: Installer compiled instead of downloading on upgrade
date: 2026-10-20
summary: install.sh fell back to cargo because curl could not overwrite the running mdview daemon binary
---

# Installer compiled instead of downloading on upgrade

## What happened
Re-running `curl -fsSL .../install.sh | sh` tended to build from source with cargo instead of downloading the release binary, even though release assets existed (HTTP 200).

Root cause: `curl -o "$DIR/mdview"` wrote straight into the binary. When `mdview serve` runs from `~/.local/bin/mdview` (auto-started by `mdview open`), Linux refuses writes to a running executable (ETXTBSY). curl exited 23, `2>/dev/null` hid the error, and the cargo branch ran with the misleading message "no prebuilt release found". Any network or proxy failure took the same silent path.

## Decision
- install.sh: download to `$DIR/.mdview.download.$$`, `chmod +x`, then `mv -f` over the binary (a rename replaces a running binary; the daemon keeps the old inode). An EXIT trap removes the temp file.
- Fall back to cargo only on HTTP 404 (no asset for this platform or version). Other failures exit with the curl error and HTTP code.
- install.ps1: same pattern; a running mdview.exe is renamed to `mdview.exe.old` before the new one is moved in.

## Verification
Reproduced with a running executable at the install location: old script fell back to cargo, new script downloaded mdview 0.8.7. Also checked a nonexistent version (404 path) and a dead proxy (clear error, no cargo, no leftover temp file).

## Next steps
- Intel macOS still always compiles (no x86_64-apple-darwin asset by design).
- action-gh-release publishes the release when the first matrix job uploads, so `latest` can briefly lack some assets; consider draft-then-publish.
- A running daemon keeps serving the old version until restarted.

> Historical work record — not durable authority. Prefer docs/specs/ADRs for current decisions.
