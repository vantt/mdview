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

## Sources

Synthesised from the retrospective records of the layout rounds. Round 1:
`tsk-2k4`, commit `27b24a4` (`crates/mdview/assets/app.css`,
`crates/mdview/src/views.rs`).
