# Why the content_hash backfill hung on large indexes

mdview `v0.7.0` shipped a database migration that could hang indefinitely on any
index with a few thousand files. This page explains what went wrong, how it was
measured, why the fix looks the way it does, and what now stops it from coming
back.

## The symptom

Version `v0.7.0` introduced a `content_hash` column and a migration to backfill
it for files already indexed. On a real 228 MB index holding 15,480 files,
running `mdview doctor` — which triggers the migration — sat for over three
minutes with no progress at all.

"No progress" is a stronger claim than "slow", and it was verified rather than
assumed: the `registry.db-wal` file's size did not change across the entire
observation window. Nothing was being written, which means the run had not even
reached the `UPDATE` loop yet. It was still stuck inside the `SELECT`.

This is the worst shape a bug can take for a user, because there is nothing to
see. The daemon or `mdview doctor` simply appears to freeze on first start after
upgrading. That tag was already pushed to GitHub before the problem was found,
so anyone who upgraded to `v0.7.0` with more than a few thousand files hit it.

## The cause: a join with nothing to join on

The backfill needed each file's content, which lives in the `files_fts`
full-text table, in order to hash it. It got there with a `LEFT JOIN` from
`files` to `files_fts` on `project_id` and `rel_path`.

Both of those columns are declared `UNINDEXED` in the FTS5 table:

```sql
CREATE VIRTUAL TABLE files_fts USING fts5(
    project_id UNINDEXED, rel_path UNINDEXED, title, content);
```

`UNINDEXED` means exactly what it says — FTS5 stores those columns but builds no
index over them. SQLite therefore had no index to drive the join through and fell
back to a nested-loop scan: for every row on the left, a full scan on the right.
That is O(n×m), and at 15,480 files on each side it stops being a performance
problem and becomes a hang.

This is the same family of mistake the short-link work hit earlier, where a
`GLOB` pattern concatenated in SQL disabled SQLite's index optimisation, and the
same one the live-reload work cited when it chose a dedicated `content_hash`
column over comparing against `files_fts` on those same `UNINDEXED` columns. The
common shape is what makes it dangerous: **the query returns exactly the right
rows.** Correctness tests pass. Only the cost is wrong, and cost is the one thing
a functional test does not look at.

## The fix: do the join in Rust

The repair replaces the SQL join with two independent single-pass scans and a
`HashMap` lookup, turning O(n×m) into O(n+m):

```rust
let pending: Vec<(String, String)> = {
    let mut stmt =
        conn.prepare("SELECT project_id, rel_path FROM files WHERE content_hash = ''")?;
    ...
};

let content_by_key: std::collections::HashMap<(String, String), String> = {
    let mut stmt = conn.prepare("SELECT project_id, rel_path, content FROM files_fts")?;
    ...
};
```

One scan collects the rows still needing a backfill, one scan builds the content
map, and the `UPDATE` loop resolves each key in constant time. Nothing needs an
index because nothing is asking the database to match rows against rows.

The scope was kept to exactly this one function. The earlier `path_hash` backfill
from the short-link work reads only from `files` and joins nothing, so it never
had this defect and was left untouched.

## How the fix was verified

The measurements are worth recording, because the gap between them is the whole
story:

| What was measured | Result |
|---|---|
| Old JOIN, 3,000 synthetic rows | 2.58 s |
| Old JOIN, real 15,480-row index | > 3 min, no progress (WAL unchanged) |
| HashMap, 16,000 synthetic rows | 0.027 s (select + scan + lookups) |
| HashMap, real production index copy | **0.988 s**, migration `v1 → v2` complete |

The final row is the one that matters most, because it is not synthetic. A real
228 MB `~/.mdview/registry.db` that had not yet been through migration v2 was
copied into an isolated `HOME`, and the fixed binary was run against it. It
finished in under a second and reported `index schema: v2, every file has a
short-link code`, with zero rows still missing `content_hash` and zero missing
`path_hash`.

Correctness was checked separately from speed: five real rows had their
`content_hash` recomputed by hand from `files_fts.content` and compared against
what the migration had written. All five matched exactly. A fast migration that
writes wrong hashes would be a worse bug than a slow one.

## What prevents a regression

A functional test cannot catch this. Both the JOIN version and the HashMap
version produce identical output, so any test that compares results passes on
both. Only elapsed time distinguishes them.

So the guard is a timing test, `backfilling_thousands_of_rows_stays_fast`, which
builds 4,000 rows against the real schema — including the `UNINDEXED` FTS
declaration that caused the problem — and fails outright if the backfill takes
two seconds or more. If someone later rewrites this as a SQL join because it
reads more naturally, the test fails on duration.

The broader lesson this incident pins down: when a query's correctness and its
cost can diverge, test the cost explicitly. The short-link work reached the same
conclusion from a different direction and asserts a SQLite *query plan* rather
than a duration. Either way, something other than the returned rows has to be
under test.

## Sources

Synthesised from the record of `tsk-155`:
`docs/history/fix-content-hash-backfill-quadratic-join/CONTEXT.md` (decision D1
and its evidence list), the fix in `crates/mdview-core/src/repository.rs`
(`backfill_content_hash`, `backfilling_thousands_of_rows_stays_fast`), and commit
`dfab829`. Related:
[why mdview file links are short](./why-mdview-file-links-are-short.md) and
[why live reload only refreshes the file you're viewing](./why-live-reload-only-refreshes-the-file-youre-viewing.md).
