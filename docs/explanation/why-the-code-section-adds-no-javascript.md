# Why the Code section adds no JavaScript

mdview's Code section — directory browsing, syntax-highlighted source, line
anchors, a working sidebar, and a Docs/Code switch — was built without adding a
single line of JavaScript. Everything is rendered on the server. This page
explains why that was the target rather than an accident, and what it cost.

## Bounded cost is the reason, not minimalism

The listing is lazy: one directory at a time, rendered per request. That decision
is about cost having a ceiling. A tree view that loads a whole repository has a
cost proportional to the repository, which for a large checkout is unbounded from
mdview's point of view. One directory per request is bounded no matter what you
point it at.

Once listings are per-directory and server-rendered, the client has nothing left
to remember. There is no tree state to keep in sync, no JSON endpoint to version,
no loading states to design, and the whole thing keeps working with JavaScript
disabled. Every entry is an ordinary link:

```html
<a href="/p/:id/_code/…">
```

The absence of JavaScript is a consequence of that choice, not a goal pursued for
its own sake.

## The sidebar works on mobile because it reuses the existing classes

mdview already has JavaScript that opens the sidebar as a drawer on mobile. It
binds to `.layout`, `.sidebar`, and `#sidebar-toggle`.

The Code sidebar is server-rendered from a directory listing rather than built by
that script — but it deliberately emits the *same* class names, `.sidebar`,
`.chapter`, and `.chap-file`, and renders the same toggle. The result is that the
mobile drawer works on Code pages without a single line of that script being
modified.

This is the payoff of matching an existing contract instead of inventing a
parallel one. A new set of class names would have meant either duplicating the
drawer logic or generalising it, both of which are work with a chance of
regressions attached, in exchange for nothing.

The sidebar also mirrors the markdown sidebar's existing "always show exactly one
directory" model: a `..` entry when not at the root, directories first with a
trailing `/`, then files, with the open file marked active.

## Why the code page does not reuse the file page

`code_page` is a separate view rather than a reuse of the existing `file_page`,
and the reason is that `file_page` is tied to things the Code section does not
have. It is bound to `IndexedFile` and `RenderedPage`, and it carries a table of
contents, backlinks, and a source blob for copy-as-markdown. Code files are not
indexed, so none of those exist here.

Reusing it would have meant threading emptiness through all of them. What *is*
reused is everything genuinely shared: `layout`, `topbar_full`, `breadcrumb`,
`theme_toggle`, and the sidebar toggle — all unchanged.

The one change to existing markdown pages is the Docs/Code switch, which slots
into a `center` position `topbar_full` already provided, with the active side
marked `aria-current="page"`. No other markdown behaviour moved.

## Why the source view is a table

The line gutter and the code are laid out as a table:

```html
<table class="codeview__table">
  <tr id="L1">
    <td class="codeview__num"><a href="#L1">1</a></td>
    <td class="codeview__line"><code>…</code></td>
  </tr>
</table>
```

A table keeps the number column and its line in exact alignment without anyone
having to pin a fixed `line-height` and hope every font and zoom level agrees.
Rows carry `id="L1"`-style anchors, so `#L42` is a real link into a file, and
`:target` highlights the row you arrived at.

Two details are deliberate. `.codeview__num` sets `user-select: none`, so
selecting code does not drag line numbers into your clipboard. And
`.codeview__line` sets `overflow-x: auto`, so a minified single-line file scrolls
inside its own cell rather than stretching the page — this is where the
per-line renderer explicitly left that problem to be solved.

Colours come from the already-served `/highlight.css` and the existing CSS
variables, so no new palette was introduced and both themes work with no extra
work.

## Where the routes sit, and what they refuse

The routes are registered before the catch-all `/p/:id/*path`, exactly as
`_search` and `_jump` already are:

```rust
.route("/p/:id/_code/",      get(code_root))
.route("/p/:id/_code/*path", get(code_path))
```

A directory renders a listing, a text file renders highlighted source, and a
binary renders a "binary, N bytes" notice in place of the source. A banner
appears above the table when a file was truncated at the size cap or detected as
binary.

Handlers do no path arithmetic of their own — every filesystem touch goes through
the `code_source` gate. A refused path and a path that does not exist return the
identical "file not found" response, so the response never reveals that a blocked
file exists. That property is [explained in full
here](./why-the-code-viewer-refuses-some-files.md), and the end-to-end tests
assert it against a real config file inside the git directory.

## What changed after this

This page describes the Code section as originally built, where the claim was
literal: no JavaScript at all. That later stopped being exactly true. When the
Code sidebar adopted the Docs sidebar's collapsible subfolder disclosure, a small
separate script was added to make that widget interactive and to remember its
state.

The reasoning above still holds — the markup is still server-rendered, listings
are still plain links, and the section still works without scripts — but the
"zero JavaScript" claim now has one deliberate exception. See [how the mdview
layout converged](./how-the-mdview-layout-converged.md), round 5.

## Sources

Synthesised from the record of `tsk-1hb-3`: its task specification (routes,
views, sidebar class reuse, markup and CSS requirements, and the required
end-to-end tests) and the shipped section in `crates/mdview/src/server.rs`,
`crates/mdview/src/views.rs`, and `crates/mdview/assets/app.css`. Commit
`057e138`. Related: [why the Code viewer refuses some
files](./why-the-code-viewer-refuses-some-files.md) and [why code is highlighted
one line at a time](./why-code-is-highlighted-one-line-at-a-time.md).
