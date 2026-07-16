---
artifact_contract: bee-plan/v1
artifact_readiness: implementation-ready
mode: small
---

# Fix Atelier regressions (PBI-11, PBI-12)

Two regressions introduced by the Atelier adoption (PBI-08). Both are in the
feature's own files; no overlap with the concurrent host_name/port session.

## Mode Gate

1 flag (existing-covered-behavior). 2 files, no gray areas → **small**.

## Inline Reality Check

- **MODE FIT** — PASS: 2 files, behavior restoration, no API/data change.
- **REPO FIT** — PASS: root cause confirmed by code read —
  - PBI-11: `app.js:233` `querySelector(".markdown-body")`; the article is now
    `<article class="fg-prose">` (`views.rs:102`), so `.markdown-body` is gone →
    the copy IIFE's guard `if (!article) return` (`app.js:235`) silently disables
    copy-as-markdown. `#mdsource` + `data-sourcepos` are intact.
  - PBI-12: the mermaid `<pre class="mermaid">` sits inside `.fg-prose`, so
    editorial's `.fg-prose pre { background: var(--signature-dark-bg) }` paints a
    dark cocoa panel behind diagrams. Zoom JS (`app.js:281-373`) and controls CSS
    (`app.css:342-392`, `pre.mermaid.zoomable`) are intact and correct.
- **ASSUMPTIONS** — the copy handler keys off the article element found by
  `.markdown-body`; restoring that class re-enables it without touching app.js.
- **SMALLER PATH** — PBI-11 is a one-class restore (the "keep semantic hooks
  intact" rule); PBI-12 is a few glue lines. No smaller honest path.
- **PROOF SURFACE** — `cargo test --workspace` (build + existing tests) + grep
  asserting the fixes; visual confirm by serving and copying / viewing a mermaid
  diagram.

## Cells (current slice)

| Cell | Owns | Fix |
|---|---|---|
| `fix-atelier-regressions-1` | `crates/mdview/src/views.rs` | restore `.markdown-body` on the article: `<article class="fg-prose markdown-body">` (PBI-11) |
| `fix-atelier-regressions-2` | `crates/mdview/assets/app.css` | add glue so `pre.mermaid` (and `.zoomable`) render on a neutral surface, not the code signature-dark panel (PBI-12) |

Disjoint files. `data-scheme`/mermaid theme + zoom JS unchanged.

## Out of Scope (noted)

- Mermaid loading from CDN (offline → no render) is pre-existing and not caused by
  PBI-08; not in this fix. Bundling mermaid offline can be a separate backlog item.

## Verification

- `cargo test --workspace` green.
- `grep -q 'markdown-body' views.rs` (hook restored); `grep -q 'pre.mermaid' app.css` for the neutral-surface rule.
- Serve + confirm: copy inside a rendered file yields raw markdown; a mermaid
  diagram renders on a normal surface with working zoom controls.
