# Validation — adopt-atelier-design-system (design-swap slice)

Mode: standard · Cells: atelier-css-1..6 · Baseline: `cargo test --workspace` green this session.

## Reality Gate

| Check | Verdict | Evidence |
|---|---|---|
| MODE FIT | PASS | 3 flags (cross-platform, existing-covered-behavior, weak-proof), no hard-gate → standard. `plan.md` §Mode Gate. |
| REPO FIT | PASS | Every target file exists: `views.rs`, `server.rs:232/380`, `assets/app.{css,js}`, `mdview-desktop/ui/index.html`. Atelier bundle exists under `docs/design/`. Serving path is `include_str!` const `APP_CSS` → concatenation is a real seam. |
| ASSUMPTIONS | PASS | Two blocking assumptions proven below (concat compile, roles exist). |
| SMALLER PATH | PASS | Partial states render broken (new markup on old CSS); atomic slice is the honest minimum. Only editorial pattern vendored (YAGNI on task/crm/financial/media). |
| PROOF SURFACE | PASS (constrained) | Unit tests prove compile + escaping; the MEDIUM visual risks are proven by manual light/dark E2E in the verify step, not unit tests — recorded as a known constraint, not hidden. |

## Feasibility Matrix

| Assumption | Risk | Proof required | Evidence | Result |
|---|---|---|---|---|
| `concat!(include_str!(a), "\n", include_str!(b))` compiles to a `&'static str` const | HIGH (blocks D1 serving) | rustc compile + run | Spike `.bee/spikes/adopt-atelier-design-system/concat_probe.rs` → `rustc` OK, prints `TOKENS\n\nROLES`, `SPIKE_RESULT=YES` | PROVEN |
| The `.fg-*` roles the cells map to actually exist | MEDIUM (blocks D2) | grep vendored sources | `.fg-card/.fg-btn/.fg-field/.fg-input/.fg-select/.fg-banner/.fg-nav/.fg-chip` present in `components.css` (118 roles total); editorial ships `.fg-prose/.fg-article-title/.fg-reading/.fg-toc/.fg-chapters/.fg-codeblock` | PROVEN — cells 2+3 corrected to exact names (`.fg-btn` not `.fg-button`, `.fg-chip` not `.fg-tag`) |
| `build_highlight_css` scheme swap is a safe local edit | LOW | source inspection | `server.rs:380-390` already scopes light/dark by `:root[data-theme=…]`; D5 = 2-token change to `data-scheme`, no logic change | PROVEN |
| No external runtime fetch after adoption | MEDIUM (offline constraint) | grep design CSS | Only external ref is `atelier.css:17-18` Google Fonts `@import`; cell-1 strips both, verify greps `! https?://` | PROVEN (mitigated in cell-1) |
| Schedule has no cycles | — | `bee cells schedule` | Wave 1: css-1,2,4,5,6 · Wave 2: css-3. No cycles. | PROVEN |

## Spikes

- `concat_probe.rs` — YES. `concat!` accepts `include_str!` operands and yields a compile-time `&str`. Disposable; lives under `.bee/spikes/`.

## Plan-Checker + Cell Review (adversarial, review slot / sonnet)

Verdict: DONE_WITH_CONCERNS — 3 blockers + weak-verify finding, all confirmed and repaired.

| # | Finding | Confirmed? | Resolution |
|---|---|---|---|
| BLOCKER 1 | `file_page` mermaid init script reads `data-theme` (`views.rs:86`), which becomes the fixed literal `"atelier"` → first-load dark renders mermaid light. css-4 only fixed the toggle path in app.js, not the initial-render script. | YES (contradicted CONTEXT discretion line) | css-3 action now REQUIRES changing the mermaid init to read `data-scheme`; verify greps `! getAttribute('data-theme')` in views.rs. |
| BLOCKER 2 | `.fg-prose pre` uses `--signature-dark-bg` (fixed dark cocoa, not per-scheme — verified `atelier.css:151-152`), but css-5 scoped a light palette (InspiredGitHub) to light scheme → dark tokens on a dark panel = unreadable in Light. | YES | D5 faithful implementation: code panel is always dark (Atelier signature), so css-5 now emits a DARK syntect palette (base16-ocean.dark) in both schemes, dropping the light branch. Logged as a decision. |
| BLOCKER 3 | css-2 (glue) assumed markup carries `.fg-toc`/`.fg-chapters`; css-3 (markup) said "keep app-glue names" — contradictory, and editorial `.fg-chapters` is an in-document concept, not mdview's cross-file tree. | YES | Dropped the reuse suggestion from both cells: file tree + right panel are pure app-glue (own `.chapter`/`.toc` classes) styled from tokens. |
| WEAK VERIFY | css-2/3/4/5 verified only via `cargo test --workspace`, which asserts none of the actual change (data-scheme, concat, roles) — "an assertion is not evidence". | YES | Added grep-based verify to css-2 (no raw hex + uses tokens), css-3 (data-scheme + fg-prose + concat! + no data-theme read), css-4 (data-scheme, no data-theme read/write), css-5 (base16-ocean.dark, no InspiredGitHub), css-6 (also forbids `e6edf3`/`8b949e`). |
| MINOR | settings checkboxes lack `.fg-check` mapping | YES | Added `.fg-check` to css-3 action. |
| NOTE | D1 vendors `editorial.css` not the `patterns.css` barrel | Justified (YAGNI, documented) | No change. |

Cold-pickup: all CRITICAL flags fixed. Schedule re-checked after css-3 dep add: Wave 1 (css-1,2,4,5,6) · Wave 2 (css-3), no cycles.

## Decision

**READY.** Both blocking assumptions proven (concat spike, roles exist); all 3 plan-checker blockers + weak-verify repaired; no HIGH unknown remains. Visual correctness is proven by manual light/dark E2E after the code lands (recorded constraint). Gate 3 handled per bypass level.
