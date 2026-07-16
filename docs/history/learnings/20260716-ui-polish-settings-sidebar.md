---
date: 2026-07-16
feature: ui-polish-settings-sidebar
categories: [ui, process, tooling]
severity: [medium, low]
tags: [css, box-sizing, backlog-verification, manual-testing, home-override]
---

# UI polish batch: settings/sidebar/TOC — findings

## What Happened

A 10-item UI polish batch (settings layout, input overflow, sidebar/TOC
markers, breadcrumb spacing, project-root-path leak) was planned and executed
solo in one small-lane cell (`ui-polish-settings-sidebar-1`). Two findings
outlasted the feature itself:

1. **A previously-"done" backlog row's literal acceptance criteria were never
   delivered.** `docs/backlog.md` PBI-15 ("Tối ưu layout form Settings")
   explicitly named "host + port chung một hàng" (host+port same row) as an
   example acceptance criterion, and was marked `done` after
   `polish-settings-form-1` shipped fieldset/card styling. That cell delivered
   the card/legend/spacing restyle but never actually put Host and Port (or
   Debounce and Max file size) on one row — they stayed as separate stacked
   `.fg-field` blocks. The gap sat unnoticed until the user hit it again and
   re-reported it in this session's request.
2. **A real, if small, incident during manual verification.** The first
   attempt to spin up a scratch instance for a live check used a plain
   `HOME=<fake> nohup <path-under-target>/mdview serve ...` shell command. It
   was blocked by the scout hook (path contains `target/`), and — separately —
   an earlier variant of the same attempt used an invented env var name
   (`MDVIEW_CONFIG_HOME`) instead of actually overriding `HOME`, so the child
   process silently resolved the **real** `~/.mdview` (via `dirs::home_dir()`,
   which only reads `HOME`). That run detected the user's real running daemon,
   persisted a CLI `--port` override into the **live** `config.toml` (7700 →
   17172), and registered a throwaway `sample-proj` into the **live**
   registry. Caught immediately via `mdview status`/`mdview list` before
   capping the cell: config port restored to 7700, the scratch project
   unregistered, daemon confirmed unaffected.

## Root Cause

1. Capping a cell against a backlog row's *general theme* ("style the form")
   without checking every itemized example the row names literally. The
   styling looked done (cards, legends, spacing existed), so the "same row"
   detail was assumed covered by association.
2. This binary has **no dedicated config-path override** (env var or flag) —
   the only isolation lever is the real `HOME` env var itself, consumed via
   `dirs::home_dir()`. Guessing a plausibly-named override variable, or
   forgetting to set `HOME` at all, produces no error — it silently succeeds
   against production state instead of failing loud.

## Recommendation

- When capping a cell whose backlog row lists concrete examples ("X and Y on
  one row", specific values, specific fields), verify each named example
  literally in the rendered/tested output before marking `done` — a thematic
  match ("it's restyled") is not sufficient evidence that a named example was
  delivered.
- Before manually testing this binary against a scratch environment, confirm
  the actual isolation lever is `HOME` (not an invented app-specific env var)
  and use the known-good recipe (`HOME=<scratch> RUSTUP_HOME=... CARGO_HOME=...
  cargo run --bin mdview -- ...`, never a `target/...` path directly). After
  the run, spot-check the **real** `~/.mdview/config.toml` and `mdview
  list`/`mdview status` to confirm nothing leaked, before trusting the test
  was actually isolated.
