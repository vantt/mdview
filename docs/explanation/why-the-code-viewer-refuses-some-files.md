# Why the Code viewer refuses some files

mdview's Code viewer lets you browse a registered project's directory tree and
read its source files. Some files never appear in a listing, and typing their
URL directly gets you nothing. This page explains the reasoning behind that gate,
why it is a denylist rather than an allowlist, and why a refused file looks
exactly like a file that does not exist.

## The threat model is an unauthenticated daemon on a LAN

Everything about this design follows from one fact: the mdview daemon has no
authentication, and it can be bound to a wildcard address that makes it reachable
from the local network.

That means the Code viewer is not "a file browser for you". It is potentially a
file browser for anyone who can reach the port. A file reader that will serve
*any* path is, under those conditions, a credential-exfiltration tool pointed at
your home directory. The gate exists because of the missing auth, not in spite of
it.

## Why a denylist of names, not an allowlist of extensions

The instinct for a source-code reader is to allow known-good extensions — `.rs`,
`.js`, `.py`, and so on — and refuse the rest. That was considered and rejected.

An extension allowlist is unworkable for source code specifically: real projects
contain `Makefile`, `Dockerfile`, `.gitignore`, `justfile`, `go.mod`, shell
scripts with no extension at all, and a long tail of config files. An allowlist
either refuses most of a real repository or grows until it means nothing.

More importantly, the extension is not what makes a file dangerous. **Identity
is.** `id_rsa` has no extension. `.env` is nothing but an extension. What needs
blocking is a set of well-known names, so that is what the gate checks:

```rust
const EXACT: &[&str] = &[
    ".git", ".ssh", ".aws", ".gnupg", ".env",
    "id_rsa", "id_dsa", "id_ecdsa", "id_ed25519",
    ".netrc", ".npmrc", ".pypirc", ".git-credentials",
    "credentials", "secrets",
];
const PREFIXES: &[&str] = &[".env.", "credentials.", "secrets."];
const EXTENSIONS: &[&str] = &["pem", "key", "p12", "pfx", "keystore", "jks"];
```

The name check is case-insensitive and runs against **every component of the
path**, not just the final filename. That is what makes a directory match deny
everything beneath it: matching `.ssh` once refuses the whole subtree, without
needing to enumerate what is inside it.

Extensions do appear in the list, but only for formats whose entire purpose is to
hold a private key or a keystore. That is the inverse of an allowlist: a short
list of things that are always secret, rather than a long list of things that are
usually fine.

## Two independent gates: your .gitignore, and the denylist

A file is refused if *either* gate refuses it.

**The gitignore gate** honours the project's own `.gitignore` files, walking from
the project root down to the directory being listed, the same way git itself
does — so a subdirectory's own `.gitignore` counts. It reads literal `.gitignore`
files rather than git repository state, so it works on a project that is not a
git repository at all. This is what keeps your build output, your local config,
and anything else you already told git to ignore out of the viewer, without you
configuring anything twice.

**The denylist gate** is the one above, and it is deliberately independent of
configuration. The comment on it states the boundary directly:

> Independent of `exclude_patterns` and of whether the project has a
> `.gitignore` at all.

The git repository directory in particular is blocked unconditionally. It does
not matter if a user edits `exclude_patterns` and removes it — the tree holds
your entire history, including content you may have since deleted, so it is not
something a config edit is allowed to expose. There is a test named for exactly
this case (`git_directory_denied_even_with_empty_exclude_patterns`), alongside
ones proving a `.env` file is refused both when gitignored and when the project
has no `.gitignore` whatsoever.

## Listing and direct access use the same check

A subtle way to get this wrong is to filter the directory listing with one piece
of logic and guard direct URL access with another. The two then drift, and a file
that is hidden from the listing becomes readable by typing its path.

mdview avoids that structurally: `resolve_source_path` (single path) and
`list_dir` (per entry) both call the same `is_denied`. As its comment puts it,
the two "agree by construction rather than by parallel logic". Path traversal and
symlinks that escape the project root are refused by the same gate, with tests
named for both.

## A refused file and a missing file look identical

When the viewer refuses a path, the response body is byte-for-byte the same as
for a path that does not exist.

This is intentional, and the reasoning is short: a distinct "access denied"
message is itself a disclosure. If refusing tells you something different from
"not found", an attacker can enumerate which sensitive files exist on your
machine without ever reading one. Whether `~/project/.env` exists is information,
and the gate declines to give it up.

## Reading is capped, and binaries are not guessed at

Two more limits apply to files that *do* pass the gate. Reads are capped at 2 MiB
and cut at the last complete line when truncated, so a huge generated file cannot
be used to exhaust memory. Binary content is detected by sniffing for a NUL byte
in the first 8 KiB or invalid UTF-8, and a genuinely binary file is never
lossy-converted into mangled text.

## What the Code viewer deliberately is not

The scope was fixed at the start: this is a convenient file reader, not a git web
UI. Explicitly excluded from v1 and not to be added back without new reasoning:
fuzzy jump for code files, live reload for code files, a raw/binary download
endpoint, and diff views.

Two structural decisions follow from that framing:

**Code is never indexed.** Files are resolved on demand from disk. The index and
FTS5 stay markdown-only. Indexing code would inflate the database on any large
repository and drag in binary detection, exclude semantics, and a search feature
of unclear value.

**Listings are lazy and server-rendered**, one directory at a time. The cost is
therefore bounded no matter how large the repository is, there is no client-side
state to manage, no new JavaScript was added, and it degrades gracefully with
JavaScript disabled.

The Code section lives under the `_code` prefix (`/p/:id/_code/*path`), following
the existing underscore-prefix convention already used by `_search` and `_jump`,
so it does not collide with the namespace of real file paths.

## Why the gate moved from extensions to identity

mdview already had a file-serving path before the Code viewer existed:
`asset_path`, which serves images and PDFs referenced from markdown. That one
*does* use an extension allowlist (`ALLOWED_ASSET_EXTENSIONS`), and it is
correct there — the set of things a markdown document may legitimately embed is
small and closed.

The Code viewer deliberately widens exposure to "most text files". Once the set
of servable things is open-ended, an extension allowlist stops being a gate at
all, which is precisely why the check had to move from *extension* to
*identity*. The two mechanisms coexist in the codebase for that reason, and the
difference between them is the difference in what each one is allowed to serve.

The Code viewer did inherit one thing from `asset_path` unchanged: the sequence
of normalise → canonicalise → verify the path still starts with the project root
→ reject by component name. The comment there already explained why the check
must run on the *canonical* path rather than the URL segments — an
innocent-looking symlink can point anywhere — and the denylist applies that same
reasoning.

## Dotfiles are browsable on purpose

The gate does not filter hidden files as a class. That is a deliberate choice,
not an oversight: `.github/workflows/` is an ordinary thing to want to read, and
so are most dotfile configs at the root of a project. Filtering every dotfile
would break that for no security gain.

The safety property does not come from hiding dotfiles. It comes from the
denylist, which names the dangerous ones directly — and which keeps working when
a project has no `.gitignore` at all, where a hidden-file filter and the
gitignore gate would both be silent.

## Every refusal returns the same error internally

Refusals do not carry a reason through the stack. Every denied case — traversal,
symlink escape, denylist hit, gitignore hit — reuses one error, which lets the
HTTP layer map all of them to a plain 404 without deciding anything.

This is the implementation half of the "refused looks like missing" property
described above. If the reason travelled outward, some layer would eventually be
tempted to render it, and a message reading "blocked because sensitive" would
confirm the file exists. Collapsing the reasons at the source means there is
nothing to leak later.

## When the project is not a git repository

The gitignore machinery normally anchors itself to a git repository. A registered
project need not be one, so the behaviour there was pinned deliberately rather
than left to whatever the library happened to do: with no repository and no
`.gitignore`, no gitignore filtering applies — and the denylist still does. A
test asserts exactly that, because it is the case where the two gates stop being
redundant and the denylist is carrying the whole weight alone.

A related edge is a broken symlink, where canonicalisation fails outright. The
resolver falls back to the joined path, matching what `asset_path` already did,
but the requirement is that such a path then fails at the *read* step rather than
slipping past the gate.

## Listing order

Directory listings are sorted directories first, then files, each group
alphabetically and case-insensitively. This is cosmetic rather than a safety
property, but it is asserted by a test so that the ordering does not drift.

## Sources

Synthesised from the record of `tsk-1hb` (and its children `tsk-1hb-1`,
`tsk-1hb-2`, `tsk-1hb-3`): `docs/history/code-viewer-section/CONTEXT.md`
(decisions D1–D5), the detailed plan under
`plans/260812-1458-code-viewer-section/`, and the shipped gate in
`crates/mdview-core/src/code_source.rs` with its tests. Commits `ca2bac0`,
`eef419e`, `057e138`.

Also synthesised from the record of `tsk-1hb-1` (the `code_source` module
itself), which contributed the threat model's contrast with `asset_path`'s
extension allowlist, the deliberate decision to keep dotfiles browsable, the
single-error refusal policy, the no-git-repository case, and the listing order.
