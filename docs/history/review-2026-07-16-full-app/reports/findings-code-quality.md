# Code-Quality Review — mdview (full-app retrospective)

Focus: correctness, readability, type safety, error handling. Inputs read in full: the cumulative diff (all 7975 lines), the frozen review-scope record, the three on-disk `plan.md` files (daemon-auto-spawn-detach, file-nav-ux, mdview-hostname-doctor-fix), and cross-checked cell traces for `multi-ip-urls-1` and `mdview-restart-1`.

**Attribution:** findings in `daemon.rs`, `engine.rs`, `render.rs`, `repository.rs`, `link_resolver.rs`, `indexer.rs`, `server.rs` fall in the **pre-bee scaffold range** (`d52b4969..5c8d5e6`) with **no plan.md and no verification-evidence preflight** — reviewed from the diff alone and flagged as such. `cli.rs`/`runtime.rs` findings map to tracked cells; the traces don't contradict them (notably `multi-ip-urls-1`'s `must_haves` explicitly *prohibits* touching `health_check`, which is exactly where P2-3 lives — so the gap was knowingly locked in).

**Severity counts: P1 = 0, P2 = 5, P3 = 8.**

---

### [P2] TOC anchor slugs computed by a second, different algorithm than comrak's heading ids — drift on duplicate/punctuated headings   (autofix_class: manual)

**Plain-language:** The "On this page" TOC builds `#anchor` links with a hand-written `slugify()`, while heading `id=` attributes are generated independently by comrak. For duplicate or punctuation-heavy headings the two disagree, so TOC clicks jump to the wrong heading or nowhere.

**Today:** `render.rs::comrak_options()` sets `o.extension.header_ids = Some(String::new())` (comrak emits ids with its own slugifier, de-duping via `-1`/`-2`). Separately `render.rs::walk()` computes `slug = slugify(&text)` into `Heading`, and `views.rs::right_panel()` renders `href="#{slug}"`.

**Problem:** The two slug sources differ. Two headings with identical text: comrak → `intro`, `intro-1`; `slugify()` → `intro`, `intro`. The second TOC link then targets the first heading. Underscore/`--` handling also differs.

**Failure scenario:** A doc with two `## Setup` sections (common in AI-generated docs). Both TOC "Setup" links scroll to the first.

**Evidence:** `crates/mdview-core/src/render.rs` — `comrak_options` (`header_ids = Some(String::new())`), `walk` (`let slug = slugify(&text); headings.push(Heading { level, text, slug })`), and `slugify`. Consumed at `crates/mdview/src/views.rs::right_panel` (`href="#{slug}"`). Two independent slug sources with different de-dup prove the mismatch. (Pre-bee scaffold — no verification evidence.)

**Fix:** Single source of truth — read comrak's assigned id back off each heading node during the walk, or disable comrak header ids and emit ids from the same `slugify()` (with matching `-N` de-dup).

**Acceptance:**
- [ ] Two identically-titled headings produce two TOC links resolving to the first and second heading respectively.
- [ ] A punctuated/underscored heading's TOC href exactly equals its rendered `id`.

---

### [P2] `mdview_view_file` returns a "Viewable at" URL even when the file was never indexed (missing/unreadable/oversized)   (autofix_class: manual)

**Plain-language:** The MCP tool and `mdview open` report success + URL even when the target doesn't exist or couldn't be read; the URL then 404s, with no error surfaced.

**Today:** `engine.rs::view_file()` calls `index_file_incremental(&project, &abs)?`, but `indexer.rs::index_file()` returns `Ok(None)` silently on `metadata`/`read_to_string` failure or `len > max_bytes`. `view_file` discards that `None`, recomputes `rel`, and returns `ViewFile { url: "/p/{id}/{rel}", … }` as long as `rel` is non-empty.

**Problem:** `view_file` is the single use case behind both the MCP tool and CLI `open`. A wrong `relative_path` (typo/case/not-yet-written) yields a confident success; the failure is only discovered by a human clicking → 404.

**Failure scenario:** Agent calls `mdview_view_file(root, "docs/architecture.md")` but the file is `docs/Architecture.md`. Tool responds "Viewable at: …/docs/architecture.md" → 404.

**Evidence:** `crates/mdview-core/src/engine.rs::view_file` (`self.index_file_incremental(&project, &abs)?;` result discarded) + `indexer.rs::index_file` (`Err(_) => return Ok(None)`). The success path never asserts a row was inserted. (Pre-bee scaffold — no verification evidence.)

**Fix:** Have `index_file_incremental`/`view_file` surface the `Ok(None)` as `Error::FileNotFound(rel)`. Tradeoff: an oversized-but-present file now errors instead of 404 (arguably the correct signal, but a behavior change for that case).

**Acceptance:**
- [ ] `view_file(root, "does-not-exist.md")` returns `Err`, not `Ok(ViewFile)`.
- [ ] MCP `tools/call` for a missing relative path returns an `isError` result.

---

### [P2] Daemon health probe can't detect a `0.0.0.0`-bound daemon on non-Linux hosts   (autofix_class: manual)

**Plain-language:** Started with `--host 0.0.0.0`, the liveness check dials the literal `0.0.0.0`. Linux tolerates it (loopback); macOS/Windows reject it, so a healthy wildcard-bound daemon reads as "not running."

**Today:** `server.rs::serve()` writes the lock with `host: cfg.host.clone()` (e.g. `"0.0.0.0"`). `daemon.rs::running_daemon()` → `health_check(&info.host, info.port)` → `TcpStream::connect("0.0.0.0:{port}")`.

**Problem:** `connect()` to `0.0.0.0` is rejected on macOS/Windows (`WSAEADDRNOTAVAIL`). There `running_daemon()` returns `None` for a live daemon → every `open`/MCP spawns a duplicate (which port-increments), `mdview status` prints "running: no", and `mdview restart`'s wait loop mis-detects state. This is the exact wildcard scenario the multi-ip feature targets; `multi-ip-urls-1`'s `must_haves` prohibit touching `health_check`, so it was locked in unaddressed.

**Failure scenario:** macOS: `mdview serve --host 0.0.0.0` then `mdview status` → "running: no"; `mdview open x.md` spawns a second daemon on 7701.

**Evidence:** `crates/mdview-core/src/daemon.rs::health_check` (`TcpStream::connect(format!("{host}:{port}"))`) fed by `server.rs::serve()` `host: cfg.host.clone()`. The probe dials the bind address verbatim, which can be the unroutable wildcard. (Pre-bee scaffold `daemon.rs`; confirmed against `multi-ip-urls-1` trace.)

**Fix:** Map wildcard → loopback before probing: `let probe_host = if is_wildcard(host) { "127.0.0.1" } else { host };` (`is_wildcard` already exists in `runtime.rs`; lift to core).

**Acceptance:**
- [ ] With the daemon bound to `0.0.0.0`, `running_daemon()` returns `Some` on macOS/Windows.
- [ ] `mdview open` doesn't spawn a second daemon when a wildcard-bound one is up.

---

### [P2] `project_path` does `get_file(...).unwrap().unwrap()` after a separate existence check — a mid-request delete panics, and `panic = "abort"` takes the whole daemon down   (autofix_class: manual)

**Plain-language:** The file-page handler re-queries the DB and force-unwraps after an earlier check. If the watcher deletes the file between the two queries, the unwrap panics; with `panic = "abort"`, the entire daemon dies for all clients.

**Today:** `server.rs::project_path()`:
```
if st.engine.store.get_file(&id, &path).ok().flatten().is_some() {
    return match st.engine.render_file(&id, &path) {
        Ok(page) => {
            let file = st.engine.store.get_file(&id, &path).unwrap().unwrap();
```
`Cargo.toml`: `[profile.release] panic = "abort"`.

**Problem:** TOCTOU between the guard, `render_file`, and the final `get_file(...).unwrap().unwrap()`. The watcher's `reindex_paths` calls `engine.remove_file` on deletion (incl. editor atomic-save transients). If the row is gone by the final `get_file`, `.unwrap()` on `Ok(None)` panics → under `panic=abort` the whole process aborts. Even without the race, the double-unwrap re-runs a query already known redundant.

**Failure scenario:** Open a file page and delete that `.md` at the same moment → handler panics between `render_file` (Ok) and the trailing `get_file`, aborting the daemon; every tab's live-reload socket drops.

**Evidence:** `crates/mdview/src/server.rs::project_path` (`.get_file(&id, &path).unwrap().unwrap()`) + `Cargo.toml` `panic = "abort"`. The value was already fetched; re-fetching + double-unwrapping assumes an invariant the concurrent watcher can violate. (Pre-bee scaffold `server.rs`.)

**Fix:** Fetch once and thread through: `let Some(file) = st.engine.store.get_file(&id,&path).ok().flatten() else { … }`; pass `&file` to `file_page`; `render_file` already returns `Err(FileNotFound)` for the error page. Remove the trailing `get_file`; no unwrap remains.

**Acceptance:**
- [ ] Deleting a file while its page is served returns a 404/500 page, not a process abort.
- [ ] `project_path` calls `get_file` at most once and contains no `.unwrap()`.

---

### [P2] `stop`/`restart` kill the lock's pid via bare `kill <pid>` with no liveness/identity check — a recycled pid can kill an unrelated process   (autofix_class: manual)

**Plain-language:** Stopping the daemon runs `kill <pid>` on whatever pid the lock records, without confirming it's still the mdview daemon. If the daemon crashed and the OS reused its pid, `stop`/`restart` signals an innocent process.

**Today:** `cli.rs::stop_daemon()` reads `runtime::read_lock()` (which does **not** health-check, unlike `running_daemon()`), then unconditionally `Command::new("kill").arg(info.pid.to_string())` (Unix) / `taskkill /PID … /F` (Windows), then removes the lock.

**Problem:** `read_lock()` can return a stale `DaemonInfo`. `running_daemon()` exists precisely to gate on a health check, but `stop_daemon` bypasses it and kills the raw pid. Between a daemon crash (stale lock) and `mdview stop`, the OS may have recycled the pid. `mdview restart` reuses this helper.

**Failure scenario:** Daemon is SIGKILLed (or aborts via P2-4), leaving a stale lock with pid 4242. OS reassigns 4242 to the user's editor. `mdview restart` → `kill 4242` kills the editor, then starts a fresh daemon.

**Evidence:** `crates/mdview/src/cli.rs::stop_daemon` (`let info = runtime::read_lock()?; … Command::new("kill").arg(info.pid.to_string())…`). Acts on `read_lock()` (no liveness) and the pid alone. (`mdview-restart-1`: "cmd_stop behavior/messages unchanged (shared helper)" — reused as-is.)

**Fix:** Gate the kill on `running_daemon()` (health-checked); if it fails, treat as "already gone" and just remove the stale lock without signalling. Optionally store a start-time/identity token in the lock and verify it.

**Acceptance:**
- [ ] `stop`/`restart` never signal a pid whose port fails the mdview health check.
- [ ] A stale lock (dead pid) is removed with "no daemon running" and no `kill` issued.

---

### [P3] `indexer::extract_title` (line-scan) diverges from the render-path title and can pick a `#` inside a code fence   (autofix_class: advisory)

Sidebar/search/jump titles come from `indexer.rs::extract_title` (first line with `# ` prefix), while the rendered page title comes from the comrak AST first-H1. A file whose first content is a fenced code block containing `# comment` yields that comment as the indexed title; setext H1s are also missed. Evidence: `crates/mdview-core/src/indexer.rs::extract_title` (naive `strip_prefix("# ")` scan, no fence awareness), vs `render.rs::walk` AST title. (Pre-bee scaffold.) Fix: derive the indexed title from the same comrak parse (one title source). Acceptance: a `# ` line inside a code fence is not used as title; indexed title == rendered title.

---

### [P3] `view_file`'s incremental index path indexes any file regardless of extension, unlike the scan path   (autofix_class: advisory)

`scan_markdown_files` filters via `is_markdown`, but `IndexService::index_file` (used by `view_file` → `index_file_incremental`) has no extension check — it upserts any readable file. `mdview_view_file(root, "notes.txt")` indexes and later renders a `.txt` through the markdown pipeline. Evidence: `crates/mdview-core/src/indexer.rs::index_file` vs `scan_markdown_files`. (Pre-bee scaffold.) Fix: apply `is_markdown` in `index_file` (`Ok(None)` for non-markdown). Acceptance: `view_file(root, "x.txt")` doesn't create a markdown-rendered entry.

---

### [P3] `daemon::health_check` treats any `200 OK` as "mdview is here"   (autofix_class: advisory)

`health_check` returns `buf.contains("\"mdview\"") || buf.contains("200 OK")` — any unrelated HTTP server answering 200 on the port is mistaken for mdview, so mdview returns URLs pointed at the foreign server. Evidence: `crates/mdview-core/src/daemon.rs::health_check` final expression. (Pre-bee scaffold.) Fix: drop the `|| buf.contains("200 OK")` disjunct; the `"mdview"` marker (from `/health` JSON `"app":"mdview"`) is specific and sufficient. Acceptance: a non-mdview 200 server on the port fails the check.

---

### [P3] `fuzzy::rank_files` re-scans the whole file list for every match (O(n²))   (autofix_class: advisory)

After ranking, each hit is mapped back via `files.iter().find(|f| f.rel_path == rel_path)` inside a per-hit closure, making the jump endpoint quadratic (bounded by `limit`, hence P3). Evidence: `crates/mdview-core/src/fuzzy.rs::rank_files` `filter_map` body. Fix: build a `HashMap<&str, &IndexedFile>` once. Acceptance: no per-hit linear scan of `files`.

---

### [P3] `project_path` and the URL builders issue redundant DB reads / config loads   (autofix_class: advisory)

Serving one file page runs `get_file` up to three times plus `get_project` twice; building one display URL reads `~/.mdview/config.toml` more than once (`ensure_daemon_base` → `ensure_bind` may `Config::load` + `display_base_url` loads again; `ensure_daemon_bases` a third). Evidence: `crates/mdview/src/server.rs::project_path`; `crates/mdview/src/runtime.rs` (`display_base_url`, `ensure_daemon_bases` each `Config::load()`). Fix: fetch each once and pass by reference (ties into P2-4's single `get_file`); thread a single `Config`. Acceptance: `project_path` does one `get_file`+one `get_project`; URL build loads config once.

---

### [P3] `scope_css` is a brace-splitting CSS "parser" that silently corrupts any rule with nested braces   (autofix_class: advisory)

`server.rs::scope_css` does `css.split_inclusive('}')` then `block.find('{')`; `strip_comments` is a manual scanner. Correct only because current syntect output is flat — an `@media`/`@font-face` block would produce malformed CSS with no error. Evidence: `crates/mdview/src/server.rs::scope_css`/`strip_comments`. Fix: document/enforce the flat-input assumption (assertion) or wrap in a single scoping selector. Acceptance: nested-brace input doesn't silently corrupt output.

---

### [P3] Settings "Saved" banner still says restart via `mdview stop && mdview serve` after `mdview restart` shipped   (autofix_class: advisory)

`views.rs::settings_page` banner instructs `mdview stop && mdview serve`, while `cli.rs::cmd_config_edit` prints "Restart the daemon to apply: mdview restart" — divergent guidance for the same action. Evidence: `crates/mdview/src/views.rs::settings_page` banner vs `cli.rs::cmd_config_edit`. Fix: reference `mdview restart` in the banner. Acceptance: banner references `mdview restart`.

---

### [P3] Mixed-language UI copy: project-list empty state is Vietnamese amid an English UI   (autofix_class: advisory)

`views.rs::project_list_page` empty state: "Chưa có project nào. Đăng ký: …" while topbar/settings/search/errors are English. Evidence: `crates/mdview/src/views.rs::project_list_page`. Fix: translate to English (or introduce intentional i18n, out of scope). Acceptance: empty-state string matches the UI language.

---

## Areas checked, no material finding
- `link_resolver` normalize/resolve: `..` clamping and traversal handling correct and well-tested; external-link and dir→README fallbacks sound.
- `config` load/save: corrupt→default + atomic write is intentional and tested.
- `write_atomic`: minor only — temp name keyed solely on `process::id()` (two same-process threads writing the same target could collide) and no temp cleanup on rename failure; low likelihood, not filed separately.
- `fts_sanitize`: Unicode-aware, avoids FTS syntax injection.
- `handle_ws` broadcast-lag handling: correct.
- MCP JSON-RPC surface: notification/id handling and error codes reasonable.
- app.js copy-as-markdown, jump-palette sequence guarding, mermaid zoom: sound (minor: mermaid `enhance` attaches window-level mousemove/mouseup per diagram — small listener duplication, not filed).

Status: DONE
Summary: 13 code-quality findings — 0 P1, 5 P2 (TOC slug drift, silent-success view_file, wildcard health-check blind spot on non-Linux, project_path double-unwrap panic under panic=abort, bare-pid kill on stale lock), 8 P3 (title divergence, non-markdown indexing, over-broad health match, O(n²) fuzzy map-back, redundant DB/config reads, fragile scope_css, stale settings banner, mixed-language copy).
