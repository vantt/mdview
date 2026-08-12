# How the mdview layout converged

mdview's chrome — the topbar, the sidebar, the breadcrumb, the scroll panes —
reached its current shape through a series of small, successive rounds rather
than one design pass. This page records what each round changed and, more
usefully, *why* the obvious first attempt in each case was not enough.

The recurring theme across all of them: the Docs view was the reference, the Code
view had to converge onto it, and each round exposed one more place where the two
had drifted or where a plausible CSS assumption turned out to be wrong.

## Round 1: the topbar's three areas align with the columns below

The page body has three columns on desktop: a 280px sidebar, a flexible content
column, and a 240px rightbar. The topbar above them was an ordinary flex row —
brand, a breadcrumb taking the free space, then actions — with no relationship to
those columns at all.

The result was that nothing in the bar lined up with anything beneath it. The
breadcrumb did not start where the content started; the actions did not sit over
the rightbar.

The fix has two halves. First, the bar's contents were grouped into three
explicit areas: brand and hamburger on the left, the breadcrumb in the centre,
and actions, settings, and the theme toggle on the right. Second — and only at
desktop widths where the rightbar can actually be visible — the bar switches from
flex to a grid whose tracks are `280px / 1fr / 240px`, exactly matching the
columns below.

The width-conditional part is the substance. The three-column grid is only
correct while three columns exist; applying it at narrower widths would align the
bar to a layout that is not on screen. So the grid is scoped to the same
condition that governs the columns themselves, and the bar falls back to grouped
flex elsewhere.

## Round 2: the Code sidebar gets the Docs sidebar's structure

The Docs sidebar has a search form at the top and a section label above its list
of chapters. The Code sidebar had neither — it opened straight into a bare list
of directories and files.

The difference was not cosmetic in the way it sounds. Two sidebars that occupy
the same position with different vertical structure make the list start at a
different height depending on which section you are in, so switching between Docs
and Code visibly jolts the content.

So the Code sidebar gained a search box and a `Files` section label above its
listing, matching the Docs structure.

The search box is deliberately **inert**. Wiring real search for code was out of
scope — code files are not indexed at all — but leaving the box out would have
left the structural mismatch in place. An inert control that reserves the correct
space is the honest way to match a layout you are not yet ready to match in
function, and it is preferable to shifting every element up by the height of a
control that will later come back.

One process note from this round, recorded because it cost a cycle: the work
item's verify command still held the placeholder text that submission fills in
mechanically, so the first attempt to return the item tried to execute the
placeholder and failed. The fix was to the item's own metadata; no code changed.

## Round 3: the breadcrumb becomes a sticky bar spanning the content column

The breadcrumb was a centred block constrained to the reading-column width. Two
things followed from that, both wrong for what a breadcrumb is:

It scrolled away. A breadcrumb answers "where am I", which is a question you ask
part-way down a long file, exactly when it had already disappeared off the top.

And it was narrower than the column it belonged to. Constraining it to the
reading measure is right for prose — long lines of body text are hard to read —
but a breadcrumb is navigation chrome, not prose. Centring it inside the content
column left it visually detached from the column's own edges.

So it became sticky, spanning the full width of the `.content` column, and split
into two equal halves. The path sits on the left; the right half is deliberately
reserved and currently empty.

An empty reserved half is worth defending, because it looks like an oversight.
The alternative is letting the left half expand to the full width now and
shrinking it later when something lands on the right — which moves the
breadcrumb, the one element whose job is to stay put. Reserving the space fixes
the geometry once.

The same `breadcrumb()` output is shared by the Docs file page and both Code
pages, so all three converged in a single change. The topbar was explicitly left
alone here; it was being reworked in its own round at the same time, and touching
both at once would have made either result impossible to attribute.

## Round 4: code stops being laid out like prose

This round fixed three things that shared one root cause — Code pages were
inheriting decisions made for reading prose.

**The main pane was capped at the reading column width.** The code table, its
header, its banner, and the directory listing all carried the `--reading-col`
maximum. That limit exists because long lines of prose are genuinely hard to
read, and it is right for a markdown article. It is wrong for source code, which
has its own line lengths and benefits from horizontal room. Dropping the cap lets
the Code section use the full content column and makes it read as a source
browser rather than a text page.

**The sidebar was one flat list.** Directories and files were rendered into a
single list, while the Docs sidebar separates its subfolders into their own
labelled block. The Code sidebar was split the same way, into `Folders` and then
`Files`.

**The right half of the breadcrumb finally got its content.** The previous round
reserved that space; here the file's language and size moved into it. That
information already existed — it was displayed inside the code view's own header
— so this moved it rather than adding it, removing the duplication in the
process.

That sequencing is the point worth keeping: reserving the slot and filling it
were separate rounds, which is why filling it moved nothing else on the page.

## Sources

Synthesised from the retrospective records of the layout rounds. Round 1:
`tsk-2k4`, commit `27b24a4` (`crates/mdview/assets/app.css`,
`crates/mdview/src/views.rs`). Round 2: `tsk-5eq`, commits `1f97d0e` and
`cb7d6c6` (`code_tree()` in `crates/mdview/src/views.rs`). Round 3: `tsk-612`
(`breadcrumb()` in `crates/mdview/src/views.rs`, `.breadcrumb` rules in
`crates/mdview/assets/app.css`).
 Round 4: `tsk-5yf`, commit `760857a`
(`code_tree()`/`breadcrumb()`/`code_page` in `crates/mdview/src/views.rs`,
`.codeview__*` and `.codelist` in `crates/mdview/assets/app.css`).
