# Why live reload only refreshes the file you're viewing

mdview watches your files and refreshes the browser when something changes.
That refresh is deliberately narrow: a tab reloads only when the exact file it
is displaying actually changed. This page explains why the narrow rule exists,
what counts as "changed", and which pages never reload at all.

## What the old behaviour was, and why it was annoying

The watcher used to broadcast the same bare `"reload"` string to every connected
browser, and it sent that signal on any filesystem event for a `.md` file — with
no comparison of the file's old and new content.

Two separate consequences fell out of that:

- **Unrelated tabs flickered.** Editing one file reloaded every open mdview tab,
  including tabs showing completely different projects.
- **Non-edits triggered reloads.** A `git checkout`, or an editor auto-save that
  wrote identical bytes, was indistinguishable from a real edit. The file was
  "touched", so everyone reloaded.

The `/settings` page made this most visible: the form would reload out from
under you mid-typing because some unrelated file had been touched.

Worth naming what was *not* the problem: the filesystem scan frequency. The
"constant rescanning" in the original complaint is normal watcher behaviour, so
`indexing.debounce_ms` and the debouncer were left alone. The bug was never how
often mdview looked — it was who got told, and whether anything had really
changed.

## "Related" means one exact file, not the project

The rule is narrow on purpose: a tab reloads only when both the `project_id`
*and* the `rel_path` of the changed file match the file that tab is displaying.
Not "same project", and not "links to that file".

The tradeoff is explicit and accepted. If a *different* file in the same project
is added or deleted, an open tab's sidebar may be slightly stale until you
navigate. That is the cost of never flickering again, and it was judged the
better deal.

## The server holds no per-connection state

The obvious implementation is for the server to remember which socket is viewing
which file, then send each reload only to matching sockets. mdview does not do
that, and the reason is that it doesn't have to.

A file page's URL *is* its identity: the route is `/p/:id/*path`, so
`location.pathname` on any file page already reads `/p/<project_id>/<rel_path>`.
The browser therefore knows what it is showing without the server telling it
anything. So the broadcast carries the `(project_id, rel_path)` it is about, and
each client decides for itself:

```js
function matchesCurrentFile(ev, id) {
  return !!id && ev && ev.project_id === id.projectId && ev.rel_path === id.relPath;
}
```

The server broadcasts to everyone exactly as before; the filtering moved to where
the identity already lived. No socket registry, no per-connection bookkeeping, no
new state to keep in sync.

## Pages with no file identity never reload

Every mdview page shares one `layout()`, so the project list, `/settings`, and
the search page all open a `/ws` connection too. Under the old global signal,
they all reloaded.

Under the new rule they are excluded automatically rather than by a special
case — they simply have no `(project_id, rel_path)` to match:

```js
function currentFileIdentity() {
  var m = location.pathname.match(/^\/p\/([^/]+)\/(.+)$/);
  if (!m) return null;
  ...
}
```

There is no `else` branch to write. The rule that fixed the flickering also
fixed the `/settings` form, as a direct consequence rather than as a second
mechanism.

One exception is handled explicitly. `/p/:id/_search` *does* match the URL shape
of a file page, even though `_search` is a reserved static route and never a real
indexed file. It is excluded by name:

```js
// "_search" is a reserved static route (server.rs), never a real indexed
// file (the indexer only ever indexes .md/.markdown). Excluded explicitly
// rather than left to that invariant alone: this is the client's own
// boundary to hold, not something to inherit implicitly from the server.
if (relPath === "_search") return null;
```

This gap was found during implementation by a live proof run against the real
committed `app.js` (executed in a Node `vm`, not a reimplementation of it), which
showed a same-project search tab could have reloaded incorrectly. The fix names
the reserved route directly rather than leaning on the indexer's file-extension
rule — the client owns this boundary, so the client states it.

## "Changed" means the bytes changed

Telling a real edit from a no-op touch requires knowing what the file used to
contain. Every indexed file now carries a `content_hash` column, added by a
second migration through the same append-only `MIGRATIONS` / `PRAGMA
user_version` mechanism the short-link work introduced. `upsert_file` compares
the stored hash against the newly read content, and the watcher emits an event
only when they differ.

The alternative — reusing the existing `files_fts.content` instead of adding a
column — would have avoided a migration but cost a full table scan on every
watcher pass. In `files_fts`, `project_id` and `rel_path` are `UNINDEXED`
columns, so an `=` comparison against them scans linearly, whereas `content_hash`
is looked up by primary key in O(1). This is the same class of trap the
short-link work hit with a concatenated `GLOB` pattern: a query that returns
correct rows while quietly degrading to a scan. The watcher is a hot path that
runs continuously, so the migration was the cheaper choice.

**Deletion is the deliberate exception.** A removed file always signals its
viewer, skipping the content comparison entirely — there is no new content to
hash, and a tab showing a file that no longer exists needs to find that out.
Filtering deletions too would leave that tab sitting on ghost content forever.
Both event kinds reload when they match:

```js
// Both "changed" and "removed" reload when they match: a removed file's
// own viewer needs to see it go away too (D4), regardless of kind.
```

## What is still a full-page reload

Narrowing *who* receives the signal did not change *what* happens when it
arrives. A matching tab still does a full `location.reload()`. No partial DOM
patching was added; that was explicitly out of scope.

## A note on how this was verified

The reindex and event-decision logic is fully covered by tests. One gap is worth
recording honestly: a full OS-level end-to-end run was blocked because the
sandboxed test environment had exhausted its inotify watch limit — confirmed by a
raw `inotify_add_watch` syscall that bypassed mdview entirely and returned
`ENOSPC`. That limit is environmental and has nothing to do with mdview's own
code, which never touches inotify in the reindex path. The uncovered piece was
narrowed to the small debouncer-to-broadcast glue and tested directly as a pure
function instead.

## Sources

Synthesised from the record of `tsk-2io`:
`docs/history/scoped-live-reload/CONTEXT.md` (decisions D1–D4), the shipped
implementation in `crates/mdview/src/watch.rs`, `crates/mdview/src/server.rs`,
`crates/mdview-core/src/repository.rs`, and `crates/mdview/assets/app.js`, and
commit `68c9ebe`.
