---
date: 2026-07-16
feature: fix-review-p1-findings-260716
categories: [security, testing, process]
severity: high
tags: [verify-command, symlink, path-traversal, evidence-gate, cell-capping]
---

# Fixing the 4 P1s from the full-app review

## What Happened

Review session `review-2026-07-16-full-app` (a comprehensive retrospective review across all 17 shipped features) found 4 P1 findings: (1) `asset_path`'s HTTP file-serving fallback exposed any file under a project root — including `.env`, `.git/config`, private keys — with no extension check and no exclude-directory check; (2-4) three already-shipped behavior-change cells (`config-edit-cli-1`, `copy-as-markdown-2`, `mdview-restart-1`) were capped with `verify_command` set to prose describing a manual live run, not a runnable command.

Fixing all 4 required a full exploring → planning → validating → swarming pass (high-risk lane, triggered by the security fix's hard-gate flag). During validating, a 4-persona panel (coherence, feasibility, security, cold-pickup) caught a genuine BLOCKER before any code was touched: the security cell's own `verify` field, as originally planned, was *also* unrunnable prose describing a manual live-curl check — the exact same failure mode that produced the original 3 P1s was about to recur inside the very fix meant to close them. The security persona separately caught a load-bearing implementation gap: if the extension allowlist checked the raw URL path instead of the canonicalized (symlink-resolved) path, a symlink like `pretty.png -> .env` would bypass the fix entirely and reopen the original hole.

Both were fixed pre-execution. All 4 cells then capped clean: `cargo test --workspace` went from 45 passing (baseline) to 60 passing, 0 failed, with real named tests replacing every manual-only proof.

## Root Cause

The entire `mdview` binary crate layer (`cli.rs`, `mcp.rs`, `server.rs`, `views.rs`, `watch.rs`) had zero `#[cfg(test)]` modules at the time the 3 evidence-gap cells were capped, while `mdview-core` was well covered. Manual/live-E2E verification had become the *default habit* for that layer, not an exception — so writing a prose `verify` field for a `behavior_change:true` cell didn't feel like cutting a corner, it felt normal. Nothing mechanically rejected a non-runnable `verify` field at cap time for those 3 cells, even though AGENTS.md's critical rule 2 already states the requirement in prose.

## Recommendation

1. **When capping any `behavior_change: true` cell, treat "verify is a runnable command" as load-bearing, not decorative** — a `verify` field that reads like a procedure description ("register a project, curl the endpoint, confirm...") is prose wearing a command's clothes. Reject it at cap time; convert to a real `#[cfg(test)]` unit test wherever the logic is reachable without a live server (most of it is — `Engine`/`asset_path`-style functions take a store/config directly and don't need HTTP).
2. **Any check that inspects a filesystem path for a security decision (extension, exclusion, permission) must run on the canonicalized path, never on the pre-resolution request/URL segment.** A symlink can make the two disagree, and the attacker controls which one you see if you pick the wrong one. Write the regression test as a `#[cfg(unix)]` symlink case specifically — it is the one input class that proves canonicalization order was respected, not just claimed.
3. When a project's binary/adapter layer has systematically thinner test coverage than its core library, expect exactly this failure mode to recur there specifically — a review or validating pass touching that layer should scrutinize `verify` fields harder than in a well-tested area.
