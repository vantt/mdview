---
date: 2026-07-16
feature: fix-atelier-regressions
categories: [frontend, regression]
severity: medium
tags: [semantic-hook, css-rewrite, mermaid, copy-as-markdown]
---

# Fixing two Atelier regressions — the promoted pattern recurred

## What Happened

The Atelier adoption (PBI-08) renamed the markdown article's class from
`.markdown-body` to `.fg-prose`. That single rename broke two shipped features:
- **copy-as-markdown** — `app.js` finds the article via
  `document.querySelector(".markdown-body")`; the guarded IIFE early-returned on
  the now-null result, silently disabling the feature.
- **mermaid diagrams** — `<pre class="mermaid">` sits inside `.fg-prose`, so it
  inherited editorial's `.fg-prose pre` fixed-dark "code panel" background and
  rendered diagrams on a dark cocoa surface.

Fix: restore `.markdown-body` as a second class on the article (keep the semantic
hook), and add a glue rule so `pre.mermaid` renders on `var(--color-surface)`
instead of the code panel.

## Root Cause

This is exactly the pattern promoted to `critical-patterns.md` in the PBI-08
compounding pass — "the rendered DOM comes from two sources (`views.rs` +
`app.js`); a CSS/markup change scoped from server markup alone misses what the
class rename breaks downstream." The prediction landed one feature later: the
rename's blast radius reached both an `app.js` selector and an inherited editorial
CSS rule, neither visible from `views.rs`.

## Recommendation

- The existing critical pattern stands and is now **confirmed** — no new
  promotion needed. When renaming a class that server markup emits, grep BOTH
  `app.js` (selectors) AND the design-system CSS (inherited element rules like
  `.fg-prose pre`) for everything that keyed off the old class.
- Treat a semantic DOM hook (`.markdown-body`, `data-*` anchors) as a contract:
  when a design system wants its own class, **add** it, do not **replace** the
  hook (the Atelier AGENTS.md "keep semantic hooks intact" rule).
- A rendered diagram is not code: keep `pre.mermaid` off any code-block styling.
