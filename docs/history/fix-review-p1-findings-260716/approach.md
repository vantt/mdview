# Approach: Fix Review P1 Findings

## Recommended path

Enforce D1 (extension allowlist) and D2 (`exclude_patterns`) directly inside `crates/mdview-core/src/engine.rs::asset_path` — the single choke point every asset read already passes through (`crates/mdview/src/server.rs::project_path` calls only this function). No new abstraction, no new config surface: both checks reuse data that already exists (`content_type()`'s known extensions, duplicated as a small const list since `mdview-core` can't import the binary crate; `config.indexing.exclude_patterns`, already read elsewhere in `Engine`). For D3, extract the three already-identified pure operations (editor resolution/arg-split/TOML-validate; the JSON `<`-escape; the `stop_daemon` outcome→message mapping) into standalone functions in their existing files and add `#[cfg(test)]` modules — no new files, no behavior change.

## Rejected alternatives

- **Add auth/token gating to the whole daemon instead of narrowing `asset_path`.** Rejected: out of scope (D1/D2 only cover this specific over-broad-read P1; auth is a separate, already-filed P2 that changes product posture, not something to fold into a P1 fix silently).
- **Filter by MIME-sniffing file contents instead of extension.** Rejected: adds a new dependency and runtime cost for a local dev tool; extension allowlist matches what `content_type()` already does and is trivially auditable.
- **Route the 3 test-coverage fixes through separate small features/cells outside this slice.** Rejected: they're independent but tiny, share the same review-session closure requirement, and splitting them into separate exploring/planning passes would be ceremony with no benefit (CONTEXT.md D3 already locked they're implementer-only decisions).

## Risk map

| Component | Risk | Reason | Proof needed |
|---|---|---|---|
| `asset_path` allowlist (D1) | LOW | Additive restriction on an existing choke point; extensions match an already-audited list | `cargo test --workspace` green + a live serve check: allowed ext 200, disallowed 404 |
| `asset_path` exclude_patterns (D2) | LOW | Mirrors an existing, already-tested pattern (`scan_markdown_files`) applied to one more call site | Unit test: path under an excluded dir → `Err`/`None` regardless of extension |
| Duplicated extension list across crate boundary | LOW-MEDIUM | `content_type()` (binary crate) and the new core-side list can drift if one changes without the other | Note left in code comment + CONTEXT.md D1; not proof-gated (documentation risk, not correctness risk this slice) |
| S2-S4 extractions | LOW | Pure-function extraction of logic that already ships; tests characterize existing behavior, they don't change it | `cargo test --workspace` green; prohibitions in each cell forbid CLI/output-shape changes |

## Files and order

1. `crates/mdview-core/src/engine.rs` — `asset_path` (D1+D2), independent of the others.
2. `crates/mdview/src/cli.rs` — `cmd_config_edit` extraction + tests (S2), and `stop_daemon`/`cmd_restart` extraction + tests (S4). Same file, no ordering dependency between the two — noted as cross-cell file overlap (legal per planning rules, costs one serialized wave).
3. `crates/mdview/src/views.rs` — `file_page` escape extraction + test (S3).

No cross-file dependency; all four can execute in any order.

## Relevant learnings

- `docs/history/learnings/critical-patterns.md` — "Rust CLI E2E testing in this repo" pattern (never invoke `./target/...` directly; use `cargo run --manifest-path` with explicit `HOME`/`RUSTUP_HOME`/`CARGO_HOME`) applies to S1's live-serve verification step.
- No existing decision or learning touches `asset_path`/`exclude_patterns` directly — this is new ground within an established pattern (D1/D2 both cite the precedent function they mirror).

## Questions for validating

- Does restricting `asset_path` to the allowlist break any currently-served asset in the live app (e.g. a project that links to a `.woff`/`.mp4`/other type not in the 9-token list)? **Answered during planning:** `rg` across `crates/mdview-core/src/*.rs`, `crates/mdview/src/*.rs`, and the embedded assets found no reference to a non-allowlisted extension going through `asset_path`. The app's own static assets (`app.css`, `app.js`, `highlight.css`) are served by dedicated `include_str!`-backed routes (`css_asset`/`js_asset`/`highlight_asset`), never through `project_path`'s `asset_path` fallback — so D1 cannot break them. No open question remains for validating on this point.
