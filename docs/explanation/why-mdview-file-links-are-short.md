# Why mdview file links are short

When mdview hands you a link to a file, it gives you something like
`http://design-lap:7700/s/ea3387e1bec9` rather than the full
`http://design-lap:7700/p/mdview/docs/history/short-link-for-file-urls/DISCUSSION.md`.
This page explains why that form exists, what the code in it actually is, and
what you can and cannot rely on.

## The problem was the terminal, not the browser

The full URL is `/p/<project-id>/<rel-path>`, so its length grows with how deep
the file sits in the tree. Once a path is nested a few levels, the URL runs past
80 characters. A terminal wraps it at the column edge, and a wrapped URL usually
stops being one clickable thing — you get two halves, neither of which opens.

The short form is a fixed length no matter how deep the file is. Measured on
mdview's own docs, the change turns a 90-character URL into 36. The design note
recorded the same comparison against `design-lap:7700`: 37 characters for the
short link versus 81 for the long one.

Nothing about the browser was broken, which is why the fix is confined to how
the link is *emitted* and to one new route. The long `/p/<id>/<rel-path>` URL
still exists and still works — the short link is an extra entrance, not a
replacement.

## The code is derived, not allocated — so there is nothing to clean up

The obvious way to build a link shortener is a table: allocate a code, store the
row, and then, eventually, decide when to delete it. That decision is the
expensive part. It means a TTL, a cleanup job, and a class of bug where a code
outlives what it pointed at.

mdview does not do that. The code is a hash of what the file already is:

```
path_hash = FNV-1a-64( project_id + "\0" + rel_path )   →  16 hex characters
code      = the leading 12 characters of that hash
```

The hash is stored in a `path_hash` column on the `files` table — the same row
the indexer already maintains for every file. As the module puts it:

> The code is *derived* from `(project_id, rel_path)` rather than allocated, so
> it has no lifetime of its own: it exists exactly as long as the file's index
> row does, and the indexer already deletes that row when a file disappears.
> That is why there is no shortlink table, no TTL, and nothing to garbage
> collect.

The design note reached the same conclusion from the other direction: a
cleanup/TTL mechanism is only needed when a shortlink is an entity with its own
lifecycle. A derived code lives and dies with the file's row, so the indexer is
already the cleanup job.

The `\0` between the two strings is not decoration. Without it, concatenating
`("ab", "c")` and `("a", "bc")` would produce identical input and therefore
identical codes.

## Why the hash function is hand-written

FNV-1a 64-bit is implemented directly in `mdview-core` rather than pulled in as
a dependency, and it is deliberately not Rust's `std` `DefaultHasher`.

`DefaultHasher` makes no promise of stability across Rust releases. A code that
depends on it would keep working right up until a toolchain upgrade, at which
point every link ever handed out would silently resolve to nothing — or worse,
to a different file. The rationale recorded against the decision says exactly
that: using `DefaultHasher` means "every old link dies silently after one
`cargo update` / toolchain bump."

There is no adversary to defend against here, so a cryptographic hash would only
add a dependency without adding value: the server does not authenticate, and the
short code is an addressing convenience, not a capability or a secret. Treat a
short link exactly as you would treat the long URL it redirects to.

The stability requirement is pinned by test vectors that are documented as
never allowed to change:

```rust
assert_eq!(path_hash("mdview", "docs/a.md"), "ea3387e1bec96ab7");
assert_eq!(path_hash("mdview", "README.md"), "0eb211fd487e7bb0");
```

## Why 12 characters, and why the column stores 16

An earlier version of the design used git-style elastic prefixes: emit the
shortest prefix that is still unique at the moment the link is generated. That
was replaced with a fixed 12 characters, which deleted the shortest-unique-prefix
function, the ambiguity branch, and the entire question of what to do when a
prefix stops being unique later.

Twelve hex characters is 2.8×10¹⁴ values. At 100,000 indexed files the expected
number of colliding pairs is about 1.8×10⁻⁵. For scale, the measurement taken
while designing this was 15,480 files across 12 projects. Eight characters would
collide near-certainly at 100k files; longer than 12 buys nothing.

The column stores all 16 characters while links carry only 12. That asymmetry is
the point: changing the emitted length later is a one-line constant change, not a
database migration.

## Why the route redirects instead of serving the page

`/s/<code>` answers with a 302 to `/p/<project-id>/<rel-path>`. It does not
render the file itself:

```rust
match st.engine.store.find_by_hash_prefix(&code) {
    Ok(Some((project_id, rel_path))) => {
        Redirect::to(&format!("/p/{project_id}/{rel_path}")).into_response()
    }
    Ok(None) => not_found("no file for that short link"),
    Err(e) => internal_error(&e.to_string()),
}
```

Serving content directly from `/s/<code>` would mean every relative link inside
the rendered page needs rewriting, because the browser would resolve those links
against `/s/` instead of against the file's real directory. Redirecting reuses
the existing page handler untouched. The terminal was the thing with the
line-wrapping problem; the browser was not, and after the redirect the browser is
back on the URL it always used.

A code that resolves to nothing gets a 404 rather than a guess. Once a file has
left the index there is no correct file to show, and guessing would open the
wrong one.

## The index lookup is written to stay on the index

Resolving a code is a prefix match against `path_hash`, and the GLOB pattern is
built in Rust and bound as a single parameter:

```rust
pub fn hash_prefix_pattern(code: &str) -> String {
    format!("{code}*")
}
```

Writing it the other way — `path_hash GLOB ?1 || '*'` in SQL — returns exactly
the same rows, which is why no ordinary test would catch the difference. But it
makes the right-hand side an expression rather than a literal, and that disables
SQLite's LIKE/GLOB index optimisation, degrading the lookup to a full table
scan. Because the failure is invisible to a functional test, there is a test
asserting the *query plan*, not just the result.

The same reasoning ruled out the alternatives during design: an in-RAM cache can
drift from the database if any write path is missed (`remove_file`,
`delete_project`), whereas the column has exactly one write path — the
`upsert_file` statement — so no copy can disagree. A full table scan per request
was measured at roughly 10ms and rejected.

## What this means for an existing installation

Two consequences follow from adding a column to a database that predates it.

**Older databases are migrated in place.** mdview previously had no migration
mechanism at all — it only ran `CREATE TABLE IF NOT EXISTS` statements. Short
links introduced an append-only list of migration steps stamped into
`PRAGMA user_version`, each step additive and resumable.

**Upgrading the binary does not upgrade a running daemon.** A daemon started
before the upgrade keeps running the old code, and that process has no `/s/`
route — so short links handed out by the new CLI or MCP server would 404 against
it. `mdview doctor` therefore compares the live daemon's reported version against
the build you have, and reports the index schema state alongside it — for
example, `v<n>, every file has a short-link code`. A daemon old enough not to
report a version at all is reported as predating that field, and therefore older
than the current build.

If short links 404 for you, that mismatch is the first thing to check: restart
the daemon so it is running the binary you installed.

## Both entrances print the same thing

The MCP tool `mdview_view_file` and the CLI `mdview open` emit links in the same
format, because they share `Engine::view_file` and because the wrapping problem
is identical in both places. Two entrances with two formats would just mean
remembering two formats.

The emitted text replaces the long URL with the short link and carries the
file's relative path beside it as ordinary text. That keeps the line short
enough not to wrap while still letting you tell at a glance which file a link
points at — an opaque code alone would make earlier terminal output unreadable
after the fact.

For programmatic callers, the MCP response keeps both forms in its structured
content: `url` and `urls` are short, `long_url` and `long_urls` are the full
`/p/...` addresses, and `code` and `path` are exposed separately.

When mdview is bound to a wildcard host with no `hostname` override, it prints
one link per reachable machine IP rather than picking one, so you can choose an
address that is actually routable from where you are. With a `hostname` override
set, it prints exactly one.

## Sources

This page was synthesised from the record of `tsk-3sl`:
`docs/history/short-link-for-file-urls/CONTEXT.md` (decisions D1–D10) and its
`DISCUSSION.md`, together with the shipped implementation in
`crates/mdview-core/src/short_link.rs`, `crates/mdview-core/src/repository.rs`,
`crates/mdview/src/server.rs`, and `crates/mdview/src/mcp.rs`.
