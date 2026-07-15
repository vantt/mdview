# Security Review — mdview full-app retrospective

**Focus:** auth/authorization, secrets, injection, permissions, data exposure — emphasis on path traversal, host-binding exposure, process-spawn injection, credential handling.

**Read in full:** the cumulative diff (all 7975 lines), the frozen review-scope record, and the three on-disk plans. Cell traces are verification evidence, not extra product code — every file they cover is in the diff and was reviewed there.

**Threat model (kept honest):** mdview is a **local dev tool** rendering a project's markdown over HTTP. Defaults to `127.0.0.1`; opt-in `--host 0.0.0.0` LAN mode (documented "less secure — no auth"); spawns a detached daemon; opens `$EDITOR`; and — crucially — serves file bytes out of registered project roots. It stores no credentials of its own; the asset it protects is **the contents of the user's project directories** (source, `.env`, `.git/`, keys).

**Net: 1 × P1, 2 × P2, 2 × P3, plus verified non-issues.**

---

### [P1] HTTP asset fallback serves *any* file inside a project root — not just markdown/images — bypassing the index's own exclusions and `.gitignore`   (autofix_class: gated_auto)

**Plain-language:** `/p/<id>/<path>` first tries to render an indexed markdown file; if the path isn't indexed it falls back to reading raw bytes off disk and returning them. That fallback is gated **only** by a "stay inside the project root" traversal check — not by file type, not by the indexer's exclude list, not by `.gitignore`. So the server hands out `.env`, `.git/config`, `id_rsa`, `config/database.yml`, or any file under a registered root.

**What the code does:** `project_path` (server.rs ~5726-5730): `if let Ok(abs) = st.engine.asset_path(&id, &path) { if let Ok(bytes) = std::fs::read(&abs) { ... content_type(&abs), bytes }`. `content_type` returns `application/octet-stream` for unknown extensions, so non-image files are still served. `asset_path` (engine.rs ~1492-1503) only enforces `!canonical.starts_with(&project.root_path)`. The indexer (`scan_markdown_files`, indexer.rs ~1833-1853) honors `.gitignore`/`exclude_patterns` — but those govern indexing/search/render only; the asset path ignores them.

**Why it's a problem:** The stated safety model ("only registered project roots are served … read-only markdown viewer") implies only markdown/referenced-images are exposed. In reality the whole subtree is a read-only web share, including secrets that are `.gitignore`d precisely to stay private. The traversal guard is correct (blocks escape + symlink escape via `canonicalize`+`starts_with`) — this is an over-broad *read surface within* the root, not an escape bug.

**Failure scenario:** Dev registers `~/work/api` (has `.env` with `DATABASE_URL=postgres://user:pw@…` and a gitignored `deploy/id_rsa`) and runs `mdview serve --host 0.0.0.0` (documented LAN workflow). Anyone on the Wi-Fi GETs `http://<dev-ip>:7700/p/api/.env` and `…/deploy/id_rsa` and receives the raw files. Even on default `127.0.0.1`, any local process — or a browser via DNS-rebinding (see P2) — reads the same.

**Evidence:**
- `crates/mdview/src/server.rs` ~5726-5730 (`project_path` asset fallback) — any on-disk path under root, of any type, is read and returned.
- `crates/mdview-core/src/engine.rs` ~1492-1503 (`asset_path`) — only check is containment; no extension allowlist, no exclude/gitignore filter.
- `crates/mdview-core/src/indexer.rs` ~1833-1853 (`scan_markdown_files`) — exclusions exist for indexing but aren't consulted by the asset path.
- Caveat: `asset_path` is in the pre-bee scaffold range (no plan.md, no verification preflight); finding is from the diff alone.

**Fix (smallest credible):** restrict the asset fallback to a small extension allowlist (the image/pdf types `content_type()` already knows), 404 otherwise — keeps referenced-images working, removes arbitrary-file exposure. Stronger: also skip paths matching `exclude_patterns`/`.gitignore` and never serve dotfiles. Tradeoff: allowlist may 404 an unusual legit type (e.g. `.mp4`); trivially extendable.

**Acceptance:**
- [ ] `GET /p/<id>/.env` and `…/.git/config` return 404 (files present under root).
- [ ] `GET /p/<id>/img/logo.png` still returns the image.
- [ ] A file under an `exclude_patterns` dir (e.g. `node_modules/x/y.png`) is not served.
- [ ] Traversal guard unchanged (`../../etc/passwd` still 404s).

---

### [P2] Daemon has no authentication and no `Host`/`Origin` validation; `0.0.0.0` bind (CLI or web-settable) exposes the whole served surface unauthenticated, and enables DNS-rebinding at localhost   (autofix_class: advisory)

**Plain-language:** No endpoint requires a credential and the server never checks `Host`/`Origin`. On `127.0.0.1` that's normal, but (1) `--host 0.0.0.0` (and the web Settings form) put the entire read surface — every doc plus, per P1, every file — on the LAN with no auth; (2) the missing `Host` check is the standard prerequisite for DNS-rebinding against the default localhost daemon.

**What the code does:** `router()` (server.rs ~5523-5540) ends every route at `.with_state(state)` — no auth/host middleware; `cors` feature is enabled in Cargo but no `CorsLayer` is applied. `bind_with_retry` (~5811-5820) binds `cfg.host` verbatim. `cfg.host` is settable via CLI `serve --host` and the unauthenticated web `update_config`.

**Why it's a problem:** For a localhost dev tool the posture is accepted and docs are honest. The real gap is the combination with P1 (LAN → whole tree readable) plus DNS-rebinding at default localhost: a visited page rebinds its hostname to `127.0.0.1`, then same-origin `fetch`es `:7700` (Host never validated), reading rendered docs and file contents.

**Failure scenario:** Dev leaves default daemon running, browses to `attacker.example` (DNS flips to `127.0.0.1`); page JS fetches `http://attacker.example:7700/p/<id>/.env` and exfiltrates. No LAN exposure required.

**Evidence:**
- `crates/mdview/src/server.rs` ~5523-5540 (`router`) — every handler unauthenticated, no host middleware.
- `crates/mdview/src/server.rs` ~5811-5820 (`bind_with_retry`) — wildcard binds honored, no compensating control.
- `docs/usage.md` ~7751-7755 — "expose it on the LAN (less secure — no auth)".

**Fix (smallest credible, defends default localhost without adding auth):** validate `Host` against an allowlist (`localhost`, `127.0.0.1`, `::1`, configured `host`/`host_name`), 403 otherwise — neutralizes DNS-rebinding regardless of P1. Optional: loud warning (or `--allow-lan`) on `0.0.0.0` startup. Full token auth is a product/user decision (flagged, not prescribed).

**Acceptance:**
- [ ] `Host: evil.example` rejected (403); `Host: localhost`/`127.0.0.1` succeed.
- [ ] `0.0.0.0` bind still serves configured/loopback hosts and surfaces a "no auth on LAN" warning.
- [ ] No regression for `http://localhost:7700` / `http://<configured-host>:7700`.

**User-decision boundary:** "No auth" is an explicit documented choice — not reversing it; the Host-validation fix defends the default case without imposing auth.

---

### [P2] Config is mutated by an unauthenticated, CSRF-able `POST /api/config`   (autofix_class: manual)

**Plain-language:** The Settings page saves via a plain HTML form POST to `/api/config` with no CSRF token and no auth. Any page the dev visits can silently submit it cross-origin and rewrite `~/.mdview/config.toml` — flip `host` to `0.0.0.0`, change `exclude_patterns`, repoint `host_name`.

**What the code does:** `update_config(Form(form): Form<SettingsForm>)` (server.rs ~5620-5672) reads a urlencoded form and `cfg.save()`s to `~/.mdview/config.toml`, then redirects. Simple-request-eligible form POST, no token, no `Origin`/`Referer` check, no auth. The form (views.rs ~6234) carries no CSRF field.

**Why it's a problem:** CSRF against a localhost dev server is a known class. Host/server changes take effect on restart (capping immediacy), but a persisted `host=0.0.0.0` or emptied `exclude_patterns` becomes effective on the next `mdview restart` — quietly widening exposure (compounds P1/P2).

**Failure scenario:** Dev with daemon running visits a malicious page; hidden form auto-POSTs `host=0.0.0.0` to `localhost:7700/api/config`. Nothing visible. Next restart comes up on the LAN.

**Evidence:**
- `crates/mdview/src/server.rs` ~5620-5672 (`update_config`) — state-changing form POST, no token/origin/auth.
- `crates/mdview/src/views.rs` ~6234 (`settings_page` form) — `<form method="post" action="/api/config">`, no CSRF field.

**Fix (smallest credible):** reject `POST /api/config` unless `Origin` (or `Referer`) matches the daemon's own host — a few lines, blocks cross-site POSTs, pairs with the Host allowlist above. Tradeoff: none for a same-origin form.

**Acceptance:**
- [ ] Cross-origin `POST /api/config` rejected; same-origin Settings form still saves.
- [ ] `~/.mdview/config.toml` unchanged after a simulated cross-origin POST.

---

### [P3] Mermaid loaded from a public CDN, unpinned, no SRI/CSP — third-party script executes against the unauthenticated local server   (autofix_class: advisory)

**Plain-language:** Pages with Mermaid diagrams `import mermaid@11` from `cdn.jsdelivr.net` at load. Floating major (`@11`), no Subresource Integrity, no CSP. Whatever that URL returns runs with full access to the unauthenticated `localhost:7700` origin.

**What the code does:** `file_page` (views.rs ~5991-5992) injects, when `has_mermaid`: `import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';`. No `integrity`; `layout` sets no CSP.

**Why it's a problem:** The page is same-origin with an API that (P1/P2) can read project files. CDN compromise/MITM, or a malicious publish under the floating `@11`, yields arbitrary JS in the mdview origin able to read docs and hit local endpoints. Likelihood low for a local tool, but blast radius (local file read via same-origin) warrants pinning. Also breaks offline (reliability overlap).

**Evidence:** `crates/mdview/src/views.rs` ~5991-5992 — unpinned third-party module, no SRI, in the local origin.

**Fix:** pin to an exact version at minimum; ideally vendor the Mermaid ESM into `assets/` and serve from `/static/` (the project already embeds its own JS via `include_str!`), removing the dependency and fixing offline; add a restrictive CSP once self-hosted. Tradeoff: vendoring grows the binary (Mermaid is large); exact-version pin is the low-cost middle ground.

**Acceptance:**
- [ ] Mermaid source is exact-pinned or served from the app's own origin.
- [ ] Diagrams render with no external request (if vendored) — verifiable offline.

---

### [P3] Dangerous URL schemes rely solely on the ammonia pass; no CSP; `is_external` allowlist omits `javascript:`/`vbscript:`   (autofix_class: advisory)

**Plain-language:** XSS defense is anchored entirely on the final ammonia sanitize pass (the right tool, correctly configured). But there's no defense-in-depth: `is_external` lists `http/https/mailto/tel///data/#` but not `javascript:`/`vbscript:`. Those are treated as *internal*, become `broken-link` anchors carrying the raw scheme, then ammonia strips them. Safe today, but single-layered, and there's no CSP.

**What the code does:** `is_external` (link_resolver.rs ~2038-2048) → `javascript:alert(1)` classified internal → `render.rs` emits `<a href="javascript:alert(1)" class="broken-link">` → whole doc runs through `sanitize()` (ammonia), which drops the scheme. `layout` (views.rs ~5915-5943) has no CSP. Note `data:` *is* in `is_external`, so `data:` URLs pass untouched to ammonia too.

**Why it's a problem:** No live XSS found — ammonia is authoritative (`strips_script_xss`, `broken_internal_link_gets_class` tests confirm). Risk is regression: a future refactor that trusts `is_external`'s classification for output safety, or reorders the sanitize pass, turns a latent gap real. CSP is a cheap second layer that also mitigates the CDN finding.

**Evidence:**
- `crates/mdview-core/src/link_resolver.rs` ~2038-2048 (`is_external`) — dangerous schemes classified internal, only neutralized downstream by ammonia.
- `crates/mdview/src/views.rs` ~5915-5943 (`layout`) — no CSP header.

**Fix:** add a CSP to `layout` (`default-src 'self'` + Mermaid's needs until self-hosted). Optionally add `javascript`/`vbscript`/`data` to an explicit "force broken, drop href" branch so safety doesn't depend only on ammonia. Tradeoff: CSP must be reconciled with the external Mermaid import (another reason to self-host it).

**Acceptance:**
- [ ] Every served HTML page carries a CSP.
- [ ] `[x](javascript:alert(1))` renders with no executable `href` (add a regression test).

---

## Verified non-issues (checked, no finding)
- **`config edit` `$EDITOR`** (cli.rs): editor string is `split_whitespace()` → `Command::new(program).args(parts)` — **no shell**, no injection; env-var trust is the local user's own. Safe.
- **`kill`/`taskkill` by pid** (cli.rs `stop_daemon`): pid is `u32` from `~/.mdview/daemon.lock`, passed as a string arg — no shell, integer can't inject; lock file is in the user's own trust domain. Safe.
- **Detached daemon spawn** (runtime.rs): `Command::new(current_exe())` + setsid/creation-flags — no injection surface. Safe.
- **SQL** (repository.rs): all queries parameterized; `fts_sanitize` strips non-alphanumerics before FTS5 MATCH. No SQL/FTS injection.
- **Path traversal proper** (engine/link_resolver): `..` lexically clamped + `canonicalize`+`starts_with` blocks directory and symlink escape. Guard is correct (test `does_not_escape_project_root`); P1 is breadth within root, not escape.
- **HTML escaping** (`esc`): double-quoted attributes, escapes `& < > "`; JSON payloads `<`-escape against `</script>` breakout. No XSS found here.
- **Secrets**: no hardcoded credentials/tokens/keys; `tracing` to stderr logs no secrets; `doctor` backs up `~/.claude.json` and writes only `{command,args}`.

## Coverage caveats
- `engine.asset_path`, `link_resolver`, `render`, `repository`, `indexer` are in the pre-bee scaffold range (no plan.md, no verification preflight); P1 and related non-issues are from the diff alone.
- `mdview-desktop` is excluded from the workspace build; its duplicate `spawn_mdview_serve` is not detached (accepted low-risk per the daemon-auto-spawn plan) — not security; cross-ref reliability reviewer.

Status: DONE
Summary: 5 security findings — 1 P1 (asset endpoint serves arbitrary files within project root, incl. secrets/.git, bypassing index exclusions), 2 P2 (no auth/Host-validation with 0.0.0.0+DNS-rebinding exposure; unauthenticated CSRF-able config mutation), 2 P3 (unpinned CDN Mermaid script; scheme-filtering relies solely on ammonia + no CSP); plus verified non-issues (editor/kill spawns, SQL, traversal guard, secrets).
