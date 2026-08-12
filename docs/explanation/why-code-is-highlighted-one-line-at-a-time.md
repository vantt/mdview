# Why code is highlighted one line at a time

The Code viewer renders each source line as its own self-contained piece of HTML,
rather than highlighting the whole file into one block and splitting it up
afterwards. That sounds like a detail of no consequence. It is actually the
entire reason this part of the work existed.

## The trap: syntax spans legitimately cross line boundaries

mdview already had a `highlight` function for fenced code blocks in markdown. It
uses syntect's `ClassedHTMLGenerator` and emits **one block of HTML** for the
whole snippet. Reusing it for the Code viewer looks trivial: highlight the file,
split the result on newlines, and pair each piece with a line number for the
gutter.

That produces broken HTML. A syntax highlighter's spans do not respect line
boundaries, and they are not wrong to cross them — a block comment, a raw string,
or a heredoc is genuinely one token spanning several lines. Splitting the
rendered blob on `\n` therefore leaves an unclosed `<span>` at the end of one
line and an orphaned `</span>` at the start of the next.

The failure is nasty because it is content-dependent. Highlight a file with no
multi-line constructs and everything looks perfect. Add a three-line block
comment and the page's markup silently degrades, in a way that depends on how
forgiving the browser happens to be.

## The fix: a lower-level API that reopens spans per line

syntect has an API designed for exactly this, which tracks the scope stack across
lines and closes and reopens spans at each boundary:

```rust
let mut parse_state = ParseState::new(syntax);
let mut stack = ScopeStack::new();

for line in LinesWithEndings::from(text) {
    let ops = parse_state.parse_line(line, &self.syntaxes).unwrap_or_default();
    ...
}
```

Each emitted line is balanced on its own. Line numbers, per-line anchors, and
per-line CSS become straightforward, because a line is a real unit of HTML rather
than a slice of one.

The regression test that guards this is stated in terms of the bug rather than
the implementation: take a Rust file with a block comment spanning three lines,
and assert that **every** emitted line contains as many `<span` as `</span>`.

## Why it lives on the existing render service

`highlight_source` was added to the existing `RenderService` instead of getting a
new type of its own, for a specific reason: that service already owns a
`SyntaxSet` built with `SyntaxSet::load_defaults_newlines()`. Loading it is
expensive, and the process already pays that cost exactly once. A second owner
would double it and buy nothing.

## Why the CSS did not have to change

The emitted classes use `ClassStyle::Spaced`, which is what the existing
`theme_css` generator targets. Matching it means the already-served
`/highlight.css` colours source files with no second palette, no additional theme
CSS, and no re-render when the theme changes.

Because this coupling is invisible — nothing breaks loudly if it drifts — there
is a test asserting a known token class is emitted, so that a future change to
`ClassStyle` fails noisily instead of quietly producing uncoloured code.

## Highlighting never fails

Syntax selection tries three things in order: the file extension, then the first
line (which catches shebangs like `#!/bin/bash` on a file with no extension), and
finally plain text.

The last step is the point. An unknown file type is not an error — it renders as
plain text with line numbers and stays perfectly usable. The same applies at the
line level: `parse_line` returns a `Result`, and a failure falls back to escaped
plain text *for that line* rather than breaking the page. This mirrors what the
existing markdown highlighter already did.

Hostile content is escaped, so a source file containing `<script>` or `&` renders
as text rather than as markup.

## What was deliberately left to the layout

A minified single-line file produces one enormous line. That is real, and it is
deliberately not the renderer's problem to solve — it is handled by the line
cell's `overflow-x: auto` in the section's CSS. The renderer's job ends at
producing balanced, escaped, per-line HTML.

## Sources

Synthesised from the record of `tsk-1hb-2`: its task specification (the
cross-line span trap, the required tests, and the named risks) and the shipped
`highlight_source` in `crates/mdview-core/src/render.rs`. Commit `eef419e`.
Related: [why the Code viewer refuses some
files](./why-the-code-viewer-refuses-some-files.md).
