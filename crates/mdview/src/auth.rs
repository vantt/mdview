//! Authentication — token + session cookie, fail-closed and silent.
//!
//! Single operator (`docs/history/daemon-auth-token-cf-access/CONTEXT.md`
//! D7/D8): a login presents the configured secret (compared in constant
//! time); on success the server issues a random session id as an httpOnly,
//! SameSite=Strict cookie. Every protected route requires a valid session
//! cookie. Two extractors share the same credential check but differ in
//! what an unauthenticated request gets back: `AuthSession` (API/websocket
//! routes) returns an **opaque 404** (D2) — no descriptive 401, nothing
//! that confirms the route exists; `AuthPage` (browser page/form routes)
//! redirects to `/login` instead, since a browser navigation has nowhere
//! useful to go on a bare 404.

use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;

use crate::server::AppState;

const COOKIE_NAME: &str = "mdv_session";

/// Short-lived cookie holding the page a visitor was denied, set by
/// `AuthPage` on rejection and consumed once by `login` so a successful sign-in
/// lands back where the visitor started instead of always at `/`.
const NEXT_COOKIE_NAME: &str = "mdv_next";

/// Header Cloudflare Access sets on requests it has already authenticated at
/// the edge. Read case-insensitively (axum lowercases header names).
const CF_ACCESS_HEADER: &str = "cf-access-jwt-assertion";

#[derive(Deserialize)]
pub struct LoginForm {
    token: String,
}

/// `POST /api/login` — validate the token, issue a session cookie, redirect
/// to `/`. A wrong or missing token gets the same opaque 404 as any other
/// auth failure (D2), so the endpoint leaks nothing about whether it exists
/// or what it wants — including no distinguishing error on the login page
/// itself.
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> Response {
    let configured = match state.web_secret.as_ref() {
        Some(s) if !s.is_empty() => s,
        // Misconfigured (no secret) → fail closed, leak nothing.
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    if !constant_time_eq(form.token.as_bytes(), configured.as_bytes()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let session_id = new_session_id();
    state.sessions.lock().await.insert(session_id.clone());

    // Where AuthPage sent the visitor from, if anywhere and still safe —
    // consumed once, then cleared below regardless of whether it was used.
    let target = next_cookie(&headers)
        .as_deref()
        .and_then(safe_redirect_target)
        .unwrap_or("/")
        .to_string();

    let session_cookie =
        format!("{COOKIE_NAME}={session_id}; HttpOnly; SameSite=Strict; Path=/; Max-Age=604800");
    let expired_next = format!("{NEXT_COOKIE_NAME}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0");
    let mut out = HeaderMap::new();
    out.append(header::SET_COOKIE, session_cookie.parse().unwrap());
    out.append(header::SET_COOKIE, expired_next.parse().unwrap());
    (out, Redirect::to(&target)).into_response()
}

/// `POST /api/logout` — invalidate the current session, redirect to
/// `/login`. Always succeeds (idempotent).
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(sid) = session_cookie(&headers) {
        state.sessions.lock().await.remove(&sid);
    }
    let expired = format!("{COOKIE_NAME}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0");
    let mut out = HeaderMap::new();
    out.insert(header::SET_COOKIE, expired.parse().unwrap());
    (out, Redirect::to("/login")).into_response()
}

/// Shared credential check behind both extractors below: a valid
/// `mdv_session` cookie is the primary credential; when the operator has
/// configured Cloudflare Access, a verified `Cf-Access-Jwt-Assertion`
/// header is accepted as an equivalent alternate (D1). The raw CF header is
/// never trusted — `verify` runs the full JWKS/signature/iss/aud/exp check;
/// any failure is treated exactly like no credential at all.
async fn authenticated(parts: &Parts, state: &AppState) -> bool {
    if let Some(sid) = session_cookie(&parts.headers) {
        if state.sessions.lock().await.contains(&sid) {
            return true;
        }
    }

    // Additive fallback (D5): only when the operator configured CF Access
    // does a request without a valid cookie get a second chance via an
    // edge-verified JWT.
    if let Some(verifier) = state.cf_access.as_ref() {
        if let Some(assertion) = parts
            .headers
            .get(CF_ACCESS_HEADER)
            .and_then(|v| v.to_str().ok())
        {
            if verifier.verify(assertion).await.is_ok() {
                return true;
            }
        }
    }

    false
}

/// Extractor proving a request is authenticated, for API and websocket
/// routes. On failure it short-circuits with an opaque 404 — the caller
/// never distinguishes "no cookie", "bad CF token", or "unknown route".
pub struct AuthSession;

#[async_trait::async_trait]
impl FromRequestParts<AppState> for AuthSession {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if authenticated(parts, state).await {
            Ok(AuthSession)
        } else {
            Err(StatusCode::NOT_FOUND.into_response())
        }
    }
}

/// Extractor proving a request is authenticated, for browser page and form
/// routes. Same credential check as `AuthSession`, but on failure it
/// redirects to `/login` instead of an opaque 404 — a top-level browser
/// navigation has nowhere useful to go on a bare 404, and `/login` is
/// itself unauthenticated (`GET /login` needs no session, or there would be
/// no way in).
pub struct AuthPage;

#[async_trait::async_trait]
impl FromRequestParts<AppState> for AuthPage {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if authenticated(parts, state).await {
            Ok(AuthPage)
        } else {
            let requested = parts
                .uri
                .path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or("/");
            let mut headers = HeaderMap::new();
            if let Some(target) = safe_redirect_target(requested) {
                let cookie = format!(
                    "{NEXT_COOKIE_NAME}={target}; HttpOnly; SameSite=Strict; Path=/; Max-Age=300"
                );
                if let Ok(v) = cookie.parse() {
                    headers.insert(header::SET_COOKIE, v);
                }
            }
            Err((headers, Redirect::to("/login")).into_response())
        }
    }
}

/// Only an internal, single-slash path is a safe `next` redirect target.
/// `//host/evil` (protocol-relative) and `/\host/evil` (browsers normalize
/// the backslash to a second slash) both fail this, closing the open-redirect
/// hole a raw `next` value would otherwise open.
fn safe_redirect_target(next: &str) -> Option<&str> {
    if next.starts_with('/') && !next.starts_with("//") && !next.starts_with("/\\") {
        Some(next)
    } else {
        None
    }
}

/// Extract a named cookie's value from the Cookie header, if present.
fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for pair in raw.split(';') {
        let pair = pair.trim();
        if let Some(v) = pair.strip_prefix(&format!("{name}=")) {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Extract the `mdv_session` value from the Cookie header, if present.
pub fn session_cookie(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, COOKIE_NAME)
}

/// Extract the `mdv_next` value from the Cookie header, if present.
fn next_cookie(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, NEXT_COOKIE_NAME)
}

fn new_session_id() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Constant-time byte comparison (D8) — no early return on first mismatch,
/// so timing does not leak how much of the token was correct.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// A random 32-character hex token, generated on first start when the
/// operator has not configured one (D3): `POST /api/login`'s constant-time
/// compare only needs matching lengths, so 32 hex chars (128 bits) is ample.
pub fn generate_web_secret() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_matches_semantics() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }

    #[test]
    fn generated_web_secret_is_32_hex_chars_and_varies() {
        let a = generate_web_secret();
        let b = generate_web_secret();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn safe_redirect_target_rejects_open_redirects() {
        assert_eq!(safe_redirect_target("/p/abc/docs/architect"), Some("/p/abc/docs/architect"));
        assert_eq!(safe_redirect_target("/"), Some("/"));
        assert_eq!(safe_redirect_target("//evil.example"), None);
        assert_eq!(safe_redirect_target("/\\evil.example"), None);
        assert_eq!(safe_redirect_target("https://evil.example"), None);
        assert_eq!(safe_redirect_target("evil.example"), None);
    }

    #[test]
    fn session_cookie_extraction() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            "other=x; mdv_session=abc123; another=y".parse().unwrap(),
        );
        assert_eq!(session_cookie(&headers), Some("abc123".to_string()));

        let empty = HeaderMap::new();
        assert_eq!(session_cookie(&empty), None);
    }
}
