# atelier-css-6 Report

**Status:** [DONE]

**Outcome:** Restyle desktop splash to use Atelier colors with prefers-color-scheme (Light: #fdfbf7/#0f172a, Dark: #1a110c/#f5efe6); removed GitHub colors

**Files touched:**
- `crates/mdview-desktop/ui/index.html`

**Commit:** e3b02cd — style(splash): use Atelier colors with prefers-color-scheme

## Summary

Restyled the 20-line desktop pre-load splash to follow the OS color scheme preference using Atelier's literal hex values. Replaced static GitHub colors (#0d1117, #e6edf3, #8b949e) with two @media (prefers-color-scheme) blocks:

- **Light mode:** background #fdfbf7 (Atelier bone), text #0f172a (dark slate), muted #6c6f7c
- **Dark mode:** background #1a110c (Atelier deep cocoa), text #f5efe6 (cream), muted #a4988e

The splash now matches the OS theme preference, ensuring no visual jar before the web UI loads with Atelier styling (per D6).

## Verification

**Verify command:** `grep -q 'prefers-color-scheme' ... && grep -q '1a110c' ... && grep -q 'fdfbf7' ... && ! grep -qiE '0d1117|e6edf3|8b949e' ... && echo SPLASH_OK`

**Verify output:** `SPLASH_OK`

**Verify passed:** ✓

## Trace

Full trace (evidence, before/after) available in `.bee/cells/atelier-css-6.json`.
