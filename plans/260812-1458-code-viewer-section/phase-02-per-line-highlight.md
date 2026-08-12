# Phase 02 — Per-line highlight

Extend `crates/mdview-core/src/render.rs` so a whole source file can be rendered
as line-addressable HTML, reusing the `SyntaxSet` the service already owns.

## Why it goes in `RenderService`, not a new type

`RenderService` holds `syntaxes: SyntaxSet` built with
`SyntaxSet::load_defaults_newlines()` (`render.rs:20,26`). That load is
expensive and already paid for once per process. A second owner would double it
for no benefit.

## The one non-obvious problem

The existing `highlight` (`render.rs:215`) uses `ClassedHTMLGenerator` and emits
**one blob**. Splitting that blob on `\n` to build a line gutter produces broken
HTML: syntect spans legitimately cross line boundaries (block comments, raw
strings, heredocs), so a naive split leaves unclosed `<span>` on one line and
orphan `</span>` on the next.

Use the lower-level path instead, which is designed for exactly this and closes
/ reopens spans at each line boundary:

```rust
use syntect::parsing::{ParseState, ScopeStack};
use syntect::html::{line_tokens_to_classed_spans, ClassStyle};

let mut parse = ParseState::new(syntax);
let mut stack = ScopeStack::new();
for line in LinesWithEndings::from(text) {
    let ops = parse.parse_line(line, &self.syntaxes)?;
    let (html, _delta) = line_tokens_to_classed_spans(line, &ops, ClassStyle::Spaced, &mut stack)?;
    lines.push(html);
}
```

`ClassStyle::Spaced` must match what `theme_css` (`render.rs:263`) generates, so
`/highlight.css` (`server.rs:105`) keeps styling code with **zero new CSS
themes** and theme switching keeps working without re-render.

## Public surface

```rust
pub struct HighlightedSource {
    pub lines: Vec<String>,   // one balanced HTML fragment per line
    pub syntax_name: String,  // e.g. "Rust" — shown in the header
}

impl RenderService {
    /// `path` is used only to pick the syntax (by extension, then by first
    /// line, then plain text). Never touched on disk here.
    pub fn highlight_source(&self, path: &Path, text: &str) -> HighlightedSource;
}
```

Syntax selection order: `find_syntax_by_extension` → `find_syntax_by_first_line`
(shebangs) → `find_syntax_plain_text`. Never fail: an unknown file type renders
as plain text with line numbers, which is still useful.

## Files

- modify `crates/mdview-core/src/render.rs` (add `highlight_source` + struct)
- modify `crates/mdview-core/src/lib.rs` if the struct needs re-export

## Tests

1. **Span balance across lines** — a Rust source with a `/* … */` spanning three
   lines: assert every emitted line has equal `<span` and `</span>` counts. This
   is the regression this phase exists to prevent.
2. Line count of output == line count of input, including a file with no
   trailing newline.
3. Unknown extension (`.wat`, `.xyz`) → plain text, `lines.len()` correct, no panic.
4. Shebang-only file (`#!/bin/bash`, no `.sh`) → picks Bash via first-line match.
5. Emitted classes are the `ClassStyle::Spaced` shape that `theme_css` targets —
   assert a known class (e.g. `class="source rust"` wrapper is *not* added here;
   check a token class) so a future `ClassStyle` change breaks loudly.
6. HTML-hostile content (`<script>`, `&`) in the source comes out escaped.

## Risks

- `parse_line` returns `Result` in syntect 5 — the existing `highlight` swallows
  errors; do the same (fall back to escaped plain text for that line) rather
  than failing the page.
- Minified single-line files produce one enormous line; that is a CSS problem
  (phase 03 gives the line cell `overflow-x: auto`), not a renderer problem.
