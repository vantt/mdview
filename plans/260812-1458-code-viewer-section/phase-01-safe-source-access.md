# Phase 01 — Safe source access

New module `crates/mdview-core/src/code_source.rs`. Everything the code section
is allowed to see or read passes through here. Pure domain logic, no HTTP.

## Threat model

The daemon has **no authentication** and can bind wildcard on a LAN
(`build_display_urls` in `runtime.rs` exists precisely because it does). Today
`asset_path` (`engine.rs:252`) limits exposure with an extension allowlist
(`ALLOWED_ASSET_EXTENSIONS`, `engine.rs:288` — images and PDF only). The code
section deliberately widens that to "most text files", so the guard has to move
from *extension* to *identity of the file*.

What must never be servable, regardless of URL typed:

- anything gitignored (`.env`, local config, build output);
- anything matching the sensitive-name denylist below, gitignored or not;
- anything under `.git/` — **enforced here unconditionally**, never by relying on
  `config.indexing.exclude_patterns`, which the user can edit via `/settings`;
- anything resolving outside the project root after symlink canonicalisation.

## Reference points

- `engine.rs:252-281` — `asset_path`: the normalize → canonicalize →
  `starts_with(root)` → exclude-components sequence to mirror. Its comments
  explain why the extension check runs on the canonical path, not the URL
  segment; the same reasoning applies to the denylist.
- `engine.rs:302` — `is_excluded_path`: exact component-name equality (not glob).
- `indexer.rs:92` — `scan_markdown_files`: existing `ignore` crate usage.
- `config.rs:87` — default `exclude_patterns` (`.git`, `node_modules`, `.venv`, `target`, `dist`).

## Public surface

```rust
pub struct DirEntry { pub name: String, pub is_dir: bool, pub size: u64 }

pub struct DirListing { pub rel_path: String, pub entries: Vec<DirEntry> }

pub enum SourceContent {
    Text { text: String, truncated: bool },
    Binary { size: u64 },
}

/// Canonicalise + authorise `rel` under the project root. Err on anything the
/// threat model forbids. Callers must not touch the filesystem without it.
pub fn resolve_source_path(root: &Path, rel: &str, exclude: &[String]) -> Result<PathBuf>;

/// One directory level, already filtered and sorted (dirs first, then files,
/// both case-insensitive alphabetical).
pub fn list_dir(root: &Path, rel: &str, exclude: &[String]) -> Result<DirListing>;

/// Read an authorised path, applying the binary sniff and the size cap.
pub fn read_source(abs: &Path) -> Result<SourceContent>;
```

## Rules

**Denylist** (case-insensitive, matched against every path component of the
canonical path, so a directory match kills everything beneath it):

```
.git  .ssh  .aws  .gnupg
.env  .env.*
*.pem  *.key  *.p12  *.pfx  *.keystore  *.jks
id_rsa  id_dsa  id_ecdsa  id_ed25519
.netrc  .npmrc  .pypirc  .git-credentials
credentials  credentials.*  secrets  secrets.*
```

Keep it a `const` slice with a small matcher (exact name, or `prefix.*` /
`*.ext` forms) — do not pull in a glob crate for this.

**gitignore**: `ignore::WalkBuilder::new(dir).max_depth(1)` with
`git_ignore(true)`, `parents(true)` (so a parent `.gitignore` still applies) and
**`hidden(false)`**.

> `hidden(false)` is deliberate: dotfiles stay browsable so `.github/workflows/`
> is reachable, which is a normal thing to want to read. The denylist, not the
> hidden filter, is what protects `.env` and friends — and it protects them even
> when the repo has no `.gitignore` at all.

For `resolve_source_path` (single file, no walk), check ignore status with a
`ignore::gitignore::GitignoreBuilder` rooted at the project root, or by walking
the parent dir once — whichever reads cleaner; the listing and the direct-URL
path must agree, so factor the decision into one `fn is_ignored(...)`.

**Size / binary** (`read_source`):

- Cap `MAX_SOURCE_BYTES = 2 * 1024 * 1024`. Over cap → read the first cap bytes,
  cut at the last complete line, `truncated: true`.
- Binary if the first 8 KiB contains a NUL byte, or the bytes are not valid
  UTF-8. Return `Binary { size }`; never lossy-convert into the viewer.

## Files

- create `crates/mdview-core/src/code_source.rs`
- modify `crates/mdview-core/src/lib.rs` — declare the module
- modify `crates/mdview-core/src/error.rs` only if a distinct error variant is
  warranted; reusing `Error::PathOutsideProject` for every deny is acceptable
  and keeps the HTTP layer's 404 mapping trivial (do not leak *why* in the
  response — a "denied because sensitive" message is itself a disclosure).

## Tests (in-module, `tempfile` per existing style)

1. traversal `../../etc/passwd` → Err
2. symlink inside root pointing outside → Err (canonical check)
3. `.git/config` → Err **with `exclude_patterns` emptied** (proves independence)
4. `.env` present and gitignored → not listed, and direct resolve → Err
5. `.env` present with **no** `.gitignore` → still Err (denylist alone)
6. `.github/workflows/ci.yml` → listed and resolvable (hidden(false) works)
7. `node_modules` excluded via config → not listed
8. listing order: dirs before files, case-insensitive alphabetical
9. binary (embedded NUL) → `Binary`
10. over-cap file → `truncated: true`, ends on a line boundary

## Risks

- `ignore`'s parent-gitignore lookup needs a `.git` dir to anchor; verify
  behaviour for a project root that is not a git repo (expect: no gitignore
  filtering, denylist still applies — assert this in a test).
- `canonicalize` fails on broken symlinks; `asset_path` falls back to the joined
  path (`engine.rs:258`). Match that, but ensure a broken symlink then fails the
  subsequent read rather than escaping the guard.
