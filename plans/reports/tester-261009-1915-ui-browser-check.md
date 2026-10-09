# UI browser check: viewed-scope index (daemon http://127.0.0.1:7799, project uiproj)

Date: 2026-10-09. Tool: agent-browser 0.38.2. Screenshots: /tmp/claude-1000/-home-vantt-projects-mdview/5b62e0d2-df2d-4394-bcce-773b6c21271e/scratchpad/ui-shots/

## Results
| # | Item | Result | Evidence |
|---|------|--------|----------|
| 1 | README renders; guide/plan live; .env not a page | PASS (see note) | links /docs/guide.md, /plans/alpha/plan.md; /p/uiproj/.env -> "404 file not found", no SECRET_KEY in body. Note: README still renders an `env` anchor pointing at /.env (dead link, 404). 01-readme.png |
| 2 | Sidebar lists filesystem tree incl. unviewed | PASS | Subfolders .grok, docs, plans; docs -> guide ("Hướng dẫn sử dụng"), other.md, sub -> deep.md. 02-sidebar.png, 02b-docs-folder.png |
| 3 | README -> guide, mermaid | PASS | guide h1 "Hướng dẫn sử dụng", 1 mermaid svg (A->B). 03-guide.png |
| 4 | Ctrl+K palette | PASS (note) | Empty: lists 7 files. "plan" -> Alpha plan plans/alpha/plan.md; "Hướng" -> guide by title; ArrowDown+Enter opened /p/uiproj/plans/alpha/plan.md. Note: "Huong" (no diacritics) does NOT match the guide title (only the content-search row shows); the task allowed either spelling. 04a/04b |
| 5 | Search row default + URL encoding | PASS | Row 1 has class active. From docs/guide.md, 'a&b c#d' -> /_search?q=a%26b%20c%23d&dir=docs. 05-search.png |
| 6 | Search results, excerpts, status, no-diacritic | PASS | "sqlite": phase-01, hidden .grok skill, deep.md, other.md, guide with <mark> highlights; status "Index reused (synced moments ago)". "duoc danh": only docs/guide.md, marks "được","đánh", status "Synced 7 files (0 read) in 0.00 s". 06-diacritics.png |
| 7 | Toggles | PASS | This folder -> scope=dir&dir=docs, 3 docs/ results; Newest keeps q and scope; Whole project -> 5 results; aria-current marks active toggle; order differs Relevance vs Newest. Default scope is Whole project even when dir=docs is present. 07a, 07b |
| 8 | Escaping | PASS | document.title = "search: </title><script>alert(1)</script> · mdview" (literal text), no injected inline script, no dialog; palette also escapes `<img onerror>` (&lt;). 08-escape.png |
| 9 | Live reload | PASS | Appended LIVE-RELOAD-CHECK to docs/guide.md; text present within 1 s (page reloaded itself) |
| 10 | Mobile 390x844 | PASS (cosmetic note) | scrollWidth==clientWidth==390 on README, search, guide; no overflow. Cosmetic: content, search box and toggles sit flush against the left edge (0 padding). 10a/10b/10c |
| 11 | Console errors | PASS | `console` and `errors` empty |

## Bugs
1. FAIL-class (visual, pre-existing CSS): the markdown editor is visible on every doc page without clicking Edit. Repro: open /p/uiproj/README.md; the Cancel/Save buttons and an empty textarea render under the article (visible in 01-readme.png, 03-guide.png, 10a). Cause: `<div id="md-editor" hidden>` is computed `display:flex` because `.md-editor { display: flex }` at crates/mdview/assets/app.css:595 overrides the `hidden` attribute and there is no `.md-editor[hidden]{display:none}` rule (the `.jump-overlay[hidden]` rule at line 746 shows the pattern). Not in the checklist, but breaks every doc page.
2. Minor: palette title matching is diacritic-sensitive ("Huong" does not find "Hướng dẫn sử dụng") while content search is not.
3. Minor: README `.env` link remains a clickable anchor to a 404.
4. Cosmetic: no horizontal padding on mobile main column and search page.

Counts: 11 PASS, 0 checklist FAIL; 1 real defect found (editor visibility) plus 3 minor notes.
