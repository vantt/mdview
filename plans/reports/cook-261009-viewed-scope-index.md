# Implementation report — viewed-scope index, contentless FTS search, CLI-first agents

Branch `feat/viewed-scope-index` (not merged, not pushed). Plan:
`plans/261009-1116-viewed-scope-index-and-cli-first/`.

## Outcome

All seven phases are implemented. Phase 1 ran first; phases 2–6 ran in
parallel worktrees (Sonnet agents) and merged without conflicts; phase 7
integrated, updated specs, reviewed and fixed.

## Verification

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` (286 tests) pass.
- Isolated end-to-end smoke test (scratch `HOME`, port 7799, release build):
  18/18 checks pass — `open --json` has `path` and creates one stub row; a page
  view indexes the file, its markdown link targets and siblings only; `.env` /
  `Cargo.toml` are never indexed or served; content search syncs hidden and
  nested dirs but never excluded dirs; `<mark>` excerpts; "duoc danh" finds
  "được đánh"; second search within 10 s reuses the index; folder scope works.
  The user's own daemon (port 7700, `~/.mdview`) was never touched.
- Measured on `workshop-ecosystem` (2,359 markdown files, hidden skill packs
  included): a full sync takes 0.78 s; DB 7.4 MB + 4.9 MB uncheckpointed WAL,
  vs. 24 MB + 4.4 MB WAL before. A project that is only viewed stays a few rows.

## Review

Plan: 1 validation pass + 4 red-team reviewers (18 findings accepted, applied
before coding). Code: one post-merge review; 2 High (daemon restart logic,
title XSS that predates this work) and 6 Medium findings; all but one fixed.

## Known limitations

- VACUUM after a large cleanup holds the store mutex (~1 s on a 24 MB DB) while
  some handlers still take that mutex on async workers; rare (hourly sweep,
  only after big deletes).
- A directory deleted and recreated is not re-watched until a file in it is
  viewed or searched again.
- Workspace version is still 0.7.8; the daemon restart now keys on the schema
  generation, so this upgrade restarts an old daemon without a version bump.
- Old `mdview mcp` processes keep the previous binary until their agent
  session restarts (documented in README/usage).

## Next steps for the user

1. Review and merge `feat/viewed-scope-index`; bump the version when releasing.
2. After installing: `mdview doctor --fix` (adds `Bash(mdview open:*)`, removes
   the legacy `registry.db*`), then restart agent sessions.
