# Why the daemon answers 404 when you are not logged in

The mdview daemon now requires authentication. If you are not logged in, or you
submit the wrong token, you do not get a login prompt or an error message — you
get an empty 404, as though the page simply did not exist. This page explains
that choice and the rest of the auth design, including why your sessions
disappear when the daemon restarts.

## This deliberately reverses an earlier decision

"No auth" used to be a documented, intentional choice for mdview, recorded in a
security review. It is being reversed on purpose, at the user's direct request —
not because the earlier reasoning was overlooked.

That history matters for anyone reading the older review: it is not stale by
accident. The daemon can be bound to a wildcard address and reached across a LAN,
and the Code viewer widened what a reachable daemon can serve, which is the
context that changed.

## Silence is the point

A wrong token at login returns an empty 404. So does any request to a protected
route without a session. The daemon does not distinguish "wrong credentials" from
"no such page".

The reasoning is the same one that governs the Code viewer's refusals: a distinct
error message is itself information. A response that says "wrong token" confirms
there is something here worth guessing a token for. A 404 tells a scanner nothing
and leaves it with no signal to work against.

This extends to Cloudflare Access. When CF Access is *not* configured, a request
carrying a `Cf-Access-Jwt-Assertion` header is refused exactly like a request
without one — the daemon does not reveal that it knows what that header is. A
test asserts precisely that.

The tradeoff was weighed and accepted: a friendlier error was available and
was explicitly declined in favour of keeping the silence.

## Two ways in

**A token, exchanged for a session cookie.** You post the token to the login
form; on success the daemon sets an `mdv_session` cookie and remembers the
session:

```
mdv_session=<id>; HttpOnly; SameSite=Strict; Path=/; Max-Age=604800
```

`HttpOnly` keeps it out of JavaScript's reach, `SameSite=Strict` stops another
site from riding it, and the session id is 24 random bytes.

**A Cloudflare Access JWT**, when configured, as an alternative rather than a
replacement. It is tried when the cookie is missing or invalid, and cookie login
keeps working alongside it.

The whole design is a near-direct port of an existing, already-exercised
implementation from a sibling project on the same Axum stack — including its
tests. That is why it looks conventional: it is proven code, moved, not a fresh
security design.

## The token generates itself on first run

If no `web_secret` is configured, the daemon does not refuse to start. It
generates a random token on first startup, writes it into `config.toml`, and
prints it to stdout once.

This is the Jupyter / code-server model, and it was chosen over the strict
fail-closed alternative — where the operator must supply a secret before anything
runs — because it keeps mdview startable without a setup ritual. The token still
exists and is still required; it is simply created for you.

The settings UI never shows `web_secret` in plain text. Rendering it into the
page would leak it into the DOM and into browser history, so the UI offers
regeneration rather than display.

## What is open, and why exactly those

| Open (no session needed) | Protected |
|---|---|
| `/health` | `/` |
| `GET /login` | `/settings`, `/api/config` |
| `POST /api/login`, `POST /api/logout` | `/api/status`, `/api/projects`, unregister |
| `/static/app.css`, `/static/app.js`, `/static/mermaid.min.js` | `/ws` |
| `/highlight.css` | `/s/:code` and every `/p/:id/*` route (Docs and Code) |

Each open route has a reason rather than an exemption. The login page must be
reachable or there is no way in. Its static assets must be reachable too —
protecting them would make the login page unable to render, producing a loop with
no exit. Health is low-sensitivity and operational tooling needs to call it.

Everything else is closed, including `/ws`. WebSocket upgrades are same-origin
requests that carry cookies, so the same session gates live reload. Because that
relies on real browser behaviour rather than theory, it was verified with a real
end-to-end test rather than trusted from the spec.

Note that the short-link route `/s/:code` is protected too — a short link is a
convenience for addressing a file, never a way around the gate.

## Cloudflare Access is all-or-nothing

CF Access turns on only when **both** `cf_access_team_domain` and
`cf_access_aud` are configured. With only one set, the feature is entirely off.

This avoids a half-configured state that silently enables part of a protection.
Partial security configuration is worse than none, because it reads as done.

When it is on, the JWT rules are deliberately strict:

- **RS256 only, named explicitly.** This blocks the two classic bypasses:
  `alg:none`, and RS256→HS256 key confusion, where a verifier is tricked into
  treating a public key as an HMAC secret. Both have dedicated tests, ported
  along with the code.
- **`exp`, `iss`, `aud`, and `nbf` are required claims**, not merely validated
  when present. A token that omits a claim fails rather than skipping that check.
- **`iss` must match the configured team domain** after normalising a trailing
  slash, and `aud` must contain the configured tag.

JWKS keys are fetched and cached with a one-hour TTL. One consequence is worth
knowing: this needs outbound network access to your team domain. On an offline or
air-gapped host with CF Access enabled, that branch fails — but cookie login
continues to work, so it degrades rather than locking you out.

## Sessions live in memory, and that is intentional

Sessions are held in an in-memory set inside the running daemon. Restarting it
logs everyone out.

That is a deliberate property of the original design, not a missing feature. It
needs no database, no expiry sweeper, and no persistence format, and it fits the
single-operator model both applications were built for. The cost is real —
upgrading or restarting the daemon means logging in again — and it was accepted
rather than overlooked.

## Two implementation choices worth knowing

**Token comparison is constant-time.** A comparison that returns as soon as two
bytes differ leaks, through timing, how much of a guess was correct — which turns
guessing a token from infeasible into incremental. The comparison is hand-written
to always examine the whole input, with its own test.

**None of this lives in `mdview-core`.** That crate documents an architectural
rule: it never depends on Axum or Tauri. So the extractor, cookie handling,
routes, and JWT verifier all live in the `mdview` binary crate. Only the
configuration *fields* — `web_secret`, `cf_access_team_domain`, `cf_access_aud` —
sit in core, alongside `host` and `hostname`, because plain data breaks no
boundary.

## What did not change

The CLI (`mdview open`) and the MCP tool (`mdview_view_file`) are unaffected.
Both talk to the shared SQLite store directly rather than making HTTP calls into
the daemon, so neither needs a token. Auth guards the web surface, not the local
tools.

The work was also kept as a single item rather than split into children: the
verifier, the auth module, and the route wiring depend on each other strictly in
sequence, so splitting would have added worktree and merge overhead with no
parallelism to gain — unlike the Code viewer work, which did split.

## Sources

Synthesised from the record of `tsk-1j4`:
`docs/history/daemon-auth-token-cf-access/CONTEXT.md` (decisions D1–D10, the
protected-route table, the required test list, and the recorded risks) and the
shipped implementation in `crates/mdview/src/auth.rs`,
`crates/mdview/src/cf_access.rs`, and `crates/mdview/src/server.rs`. Commit
`d6fb226`.
