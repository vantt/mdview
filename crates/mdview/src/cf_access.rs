//! Cloudflare Access JWT verification — opt-in, additive, fail-closed.
//!
//! When an operator configures `cf_access_team_domain` + `cf_access_aud`
//! (`docs/history/daemon-auth-token-cf-access/CONTEXT.md` D5), a request that
//! arrives already authenticated by Cloudflare Access at the edge carries a
//! `Cf-Access-Jwt-Assertion` header. This module turns that header into a
//! verified identity, or an error — never a partial pass.
//!
//! The single security property this module exists to guarantee: the
//! header's claims are trusted **only** after the JWT's RS256 signature
//! verifies against Cloudflare's published JWKS, and `iss`/`aud`/`exp`/`nbf`
//! all check out (D6). Any single mismatch is an `Err`. Trusting the raw
//! header without a signature check is exactly the hole this feature's
//! threat model warns about.
//!
//! Wired into the request path via `AuthSession` (`crate::auth`): when the
//! operator configures CF Access, a verified assertion is accepted as an
//! alternate credential alongside the session cookie. Some error variants are
//! only reachable on network paths not exercised by the offline test suite,
//! so `dead_code` stays allowed.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;

/// Path (relative to the team domain) of Cloudflare Access's JWKS endpoint.
const CERTS_PATH: &str = "cdn-cgi/access/certs";
/// How long a fetched JWKS is trusted before a refetch. Cloudflare rotates
/// signing keys and keeps the previous ones live well beyond this window, so
/// an hour bounds staleness without refetching on every request.
const JWKS_TTL: Duration = Duration::from_secs(3600);

/// Why a `Cf-Access-Jwt-Assertion` was not accepted. Every variant is a hard
/// "not authenticated" — callers treat all of them identically (fail closed).
#[derive(Debug)]
pub enum CfAccessError {
    /// The JWKS could not be fetched (network, HTTP status, or malformed body).
    Fetch(String),
    /// The token header carried no `kid`, so no signing key can be selected.
    MissingKid,
    /// The token's `kid` is not in the (freshly fetched) JWKS.
    UnknownKid,
    /// Signature, `iss`, `aud`, `exp`, or `nbf` failed validation.
    Invalid(String),
    /// Signature and claims verified, but the token carried no usable identity
    /// (neither `email` nor `sub`).
    NoIdentity,
}

impl std::fmt::Display for CfAccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CfAccessError::Fetch(e) => write!(f, "CF Access JWKS fetch failed: {e}"),
            CfAccessError::MissingKid => write!(f, "CF Access token has no kid header"),
            CfAccessError::UnknownKid => write!(f, "CF Access token kid not in JWKS"),
            CfAccessError::Invalid(e) => write!(f, "CF Access token failed verification: {e}"),
            CfAccessError::NoIdentity => write!(f, "CF Access token carried no identity claim"),
        }
    }
}

impl std::error::Error for CfAccessError {}

/// The subset of a JWKS document this verifier needs. Cloudflare sends more
/// fields per key (`kty`, `alg`, `use`, `x5c`, …); serde ignores the rest.
#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: String,
    /// RSA modulus, base64url (as `DecodingKey::from_rsa_components` expects).
    n: String,
    /// RSA public exponent, base64url.
    e: String,
}

/// Claims read out of a verified token. `exp`/`nbf`/`iss`/`aud` are validated
/// by `jsonwebtoken` itself and so are not re-read here.
#[derive(Debug, Deserialize)]
struct CfClaims {
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    sub: Option<String>,
}

/// In-memory JWKS cache: signing keys by `kid`, plus when they were fetched.
struct JwksCache {
    keys: HashMap<String, DecodingKey>,
    fetched_at: Option<Instant>,
}

/// Verifies `Cf-Access-Jwt-Assertion` headers for one configured CF Access
/// application. Holds the reqwest client, the expected team domain (= JWKS
/// origin and expected `iss`), the expected `aud` tag, and the JWKS cache.
pub struct CfAccessVerifier {
    client: reqwest::Client,
    team_domain: String,
    aud: String,
    cache: Mutex<JwksCache>,
}

impl CfAccessVerifier {
    /// Build a verifier. `team_domain` is the full origin
    /// (e.g. `https://team.cloudflareaccess.com`); `aud` is the Access
    /// Application Audience tag.
    pub fn new(client: reqwest::Client, team_domain: String, aud: String) -> Self {
        // Normalize once so the JWKS fetch and the `iss` check always agree: a
        // trailing slash on the configured domain must not let the fetch
        // succeed while the `iss` comparison silently never matches.
        let team_domain = team_domain.trim_end_matches('/').to_string();
        CfAccessVerifier {
            client,
            team_domain,
            aud,
            cache: Mutex::new(JwksCache {
                keys: HashMap::new(),
                fetched_at: None,
            }),
        }
    }

    fn certs_url(&self) -> String {
        format!("{}/{}", self.team_domain, CERTS_PATH)
    }

    /// Verify a raw `Cf-Access-Jwt-Assertion` value. Returns the identity
    /// (`email` if present, else `sub`) only when the signature verifies
    /// against the cached JWKS and `iss`/`aud`/`exp`/`nbf` all pass.
    pub async fn verify(&self, assertion: &str) -> Result<String, CfAccessError> {
        let kid = decode_header(assertion)
            .map_err(|e| CfAccessError::Invalid(e.to_string()))?
            .kid
            .ok_or(CfAccessError::MissingKid)?;
        let key = self.decoding_key(&kid).await?;
        verify_with_key(assertion, &key, &self.team_domain, &self.aud)
    }

    /// Resolve the signing key for `kid` from the cache, refetching the JWKS
    /// once if the cache is stale or unseeded. A *fresh* cache that lacks the
    /// `kid` is `UnknownKid` without a refetch — so a stream of bogus `kid`s
    /// cannot force a fetch per request.
    async fn decoding_key(&self, kid: &str) -> Result<DecodingKey, CfAccessError> {
        {
            let cache = self.cache.lock().unwrap();
            if let Some(at) = cache.fetched_at {
                if at.elapsed() < JWKS_TTL {
                    return cache
                        .keys
                        .get(kid)
                        .cloned()
                        .ok_or(CfAccessError::UnknownKid);
                }
            }
        }
        let jwks = self.fetch_jwks().await?;
        let mut keys = HashMap::new();
        for jwk in jwks.keys {
            if let Ok(dk) = DecodingKey::from_rsa_components(&jwk.n, &jwk.e) {
                keys.insert(jwk.kid, dk);
            }
        }
        let mut cache = self.cache.lock().unwrap();
        cache.keys = keys;
        cache.fetched_at = Some(Instant::now());
        cache
            .keys
            .get(kid)
            .cloned()
            .ok_or(CfAccessError::UnknownKid)
    }

    async fn fetch_jwks(&self) -> Result<Jwks, CfAccessError> {
        self.client
            .get(self.certs_url())
            .send()
            .await
            .map_err(|e| CfAccessError::Fetch(e.to_string()))?
            .error_for_status()
            .map_err(|e| CfAccessError::Fetch(e.to_string()))?
            .json::<Jwks>()
            .await
            .map_err(|e| CfAccessError::Fetch(e.to_string()))
    }
}

/// The pure verification step, isolated from the network so it can be tested
/// against a locally-generated key + hand-built JWT. RS256 only; `iss` must
/// equal `team_domain`; `aud` must contain `aud`; `exp`/`nbf` must be valid.
fn verify_with_key(
    assertion: &str,
    key: &DecodingKey,
    team_domain: &str,
    aud: &str,
) -> Result<String, CfAccessError> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[team_domain]);
    validation.set_audience(&[aud]);
    validation.validate_nbf = true;
    // `Validation::new` only seeds `exp` as required, so `iss`/`aud` would be
    // checked *if present* but a token omitting either would still pass.
    // Require all three so a missing `iss` or `aud` is an outright rejection.
    validation.set_required_spec_claims(&["exp", "iss", "aud"]);
    let claims = decode::<CfClaims>(assertion, key, &validation)
        .map_err(|e| CfAccessError::Invalid(e.to_string()))?
        .claims;
    claims
        .email
        .filter(|s| !s.is_empty())
        .or_else(|| claims.sub.filter(|s| !s.is_empty()))
        .ok_or(CfAccessError::NoIdentity)
}

// Test key material, shared with `crate::auth`'s auth-wiring tests so the
// signing key and its published JWK live in exactly one place.
#[cfg(test)]
pub(crate) const TEST_KID: &str = "test-kid";
#[cfg(test)]
pub(crate) const TEST_TEAM: &str = "https://team.cloudflareaccess.com";
#[cfg(test)]
pub(crate) const TEST_AUD: &str = "aud-tag-123";
// Public-key components (base64url) of the test keypair below — the JWK a
// real JWKS would publish for `kid=test-kid`. Generated once via openssl.
#[cfg(test)]
pub(crate) const TEST_N: &str = "rzzQdFQLD3rIpAMBGDrOeW9IO06xBImw648LmM03KtjBqBOo5WSW8sJNKiZdXECtCsoe19uc8c4eJioAIBtKKxlcfdEyFL-85uf0kXOrFCLrU5Lm_1BpPLkBJho9ObiyarBJLcUVtw5twmWd0BNjwP5EBjOeeTAcInZe513cKaErWL-adjbpSQZLBvw64A_zlDt4GsMKSdShgn2j4We_SkiWww3t7NbfaAmKmbjYAJ8niZzddvtKBLvUA7aAPyzaCVFFJoUHG0yaxw6z3hyKYMiCTwH9d6BP2tM65jCbBchbMJ4QyPILr7BYwSrT7K9DY6Vh5XEfesABgA3qVZdGWw";
#[cfg(test)]
pub(crate) const TEST_E: &str = "AQAB";
// The matching RSA-2048 private key (PKCS#8), used only to sign test JWTs.
// This is a throwaway key that exists solely for these tests.
#[cfg(test)]
pub(crate) const TEST_PRIV_PEM: &str = "-----BEGIN PRIVATE KEY-----\n\
MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQCvPNB0VAsPesik\n\
AwEYOs55b0g7TrEEibDrjwuYzTcq2MGoE6jlZJbywk0qJl1cQK0Kyh7X25zxzh4m\n\
KgAgG0orGVx90TIUv7zm5/SRc6sUIutTkub/UGk8uQEmGj05uLJqsEktxRW3Dm3C\n\
ZZ3QE2PA/kQGM555MBwidl7nXdwpoStYv5p2NulJBksG/DrgD/OUO3gawwpJ1KGC\n\
faPhZ79KSJbDDe3s1t9oCYqZuNgAnyeJnN12+0oEu9QDtoA/LNoJUUUmhQcbTJrH\n\
DrPeHIpgyIJPAf13oE/a0zrmMJsFyFswnhDI8guvsFjBKtPsr0NjpWHlcR96wAGA\n\
DepVl0ZbAgMBAAECggEAQdHt2kPXA7Fyu2gFeTxdYW9TCjO8eZ/ePBw2luY92nIg\n\
CymXPtJRgE1K/pH6bzQ7ytmwTBPJF2n8GQmdknGtohKckwWIchKyuWhWjGuNzrpu\n\
+a0r5qolXRGARPeGF7AwE7KzSeXFCkT5JRNV+1nNFihrFIk+9PYFG0yqyOyQTXIC\n\
9whuJmT5u2706tgzmS7WakVx5WGzAQpvkecfvAfhtQxBc7SiFpzjcBbQosKNiJos\n\
tvvol6xAUxf0ulYvQFVy3wbNaWA/R1IGu11Y2X0BMPOIbwF9qDWW1DuhG5UKTzMM\n\
i6d2nIVOwguzhfl9FsfiNj98vfHY6F+GiCUhspsbAQKBgQC1YqqEQxKMgJm2xF3U\n\
2iVEb28Q9iTHRTUa7XuBm+T+4r7efZsIklSDsesGCtNtTNXQh4FP/cDOpgLFxT/b\n\
6vUHbbqTufDt1wDqmCICzXeL+2riFuupbLdFiAxTs1KCf0OX87QUDwdzE1NlN7DP\n\
dkL+jwZuMGxIzfp+YfN4D5WvMQKBgQD3UrrDWMxWdKL8p6amvJgSow2A564QvO7j\n\
4o/4I/xpFByUadOC+tedYtCNxtruSeDprJLc4+vG1lvK6bzYLTuIg9wd/t1jpnSk\n\
UClMkvRrfVxdDU2wioCNwdIUTNg17vWfjzz+w8a0J/GRbeLQoKLA0JwXl5TduT8o\n\
r1QqpI1jSwKBgAPqkHHwnMrpz+fRT5FT8HAM0+IS3nJq/R2KuRrwSb5zGNnm7lz+\n\
A9MgGUn1G+GFQiyRcGpQuUP885xfiORvq0CwztF3t0r7VGq8RCe5VfZwxDsDca0j\n\
ysU2jcWU3pgwtT3npiC0vl1usmNCE5A3JnUmk2X3p67eu6TU6pPSClJRAoGAfk4c\n\
nPizWg+00OzZedtkmlf05Hjs9xVVtsGUnrfaBtvDgLPO1dw+0tyM/2qnkfvexddh\n\
JTesyF3egPD/hTMMbTpR5murKmHuvZ9GiBmgg2iBC/BoVZlV748lN0LLRDfl7neb\n\
Qcw/pO+lOYzxwXPXyjp/DLlXyCf7rk5j4Gcq4aMCgYBqH86pNyrWB52kRXJ916Lo\n\
mmUTnYeTgT53Ql27+srWuB3yeOSVuiIMnXvcDOA/BgNqX7juSOCvArzxn3nrF+6V\n\
eu5cvjXmdt7uePnBiwKSysr/Z6eZpHKR/TKW2IKOONv2fM4EGbzdoDP1OvbhvD7v\n\
XGaRodbsp22wOVJ9iado4w==\n\
-----END PRIVATE KEY-----\n";

#[cfg(test)]
impl CfAccessVerifier {
    /// Test seam: seed the JWKS cache with a signing key so `verify` runs
    /// fully offline (what a real fetch would populate).
    pub(crate) fn seed_test_key(&self, kid: &str, key: DecodingKey) {
        let mut cache = self.cache.lock().unwrap();
        cache.keys.insert(kid.to_string(), key);
        cache.fetched_at = Some(Instant::now());
    }
}

/// Build a verifier whose cache is pre-seeded with the test signing key, plus
/// a freshly signed, fully valid token it will accept — the pair
/// `crate::auth`'s wiring tests need to exercise the CF Access branch of
/// `AuthSession` offline.
#[cfg(test)]
pub(crate) fn test_verifier_with_valid_token() -> (CfAccessVerifier, String) {
    use jsonwebtoken::{encode, EncodingKey, Header};
    use std::time::{SystemTime, UNIX_EPOCH};

    let verifier = CfAccessVerifier::new(
        reqwest::Client::new(),
        TEST_TEAM.to_string(),
        TEST_AUD.to_string(),
    );
    verifier.seed_test_key(
        TEST_KID,
        DecodingKey::from_rsa_components(TEST_N, TEST_E).unwrap(),
    );

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let claims = serde_json::json!({
        "iss": TEST_TEAM,
        "aud": [TEST_AUD],
        "sub": "user-sub-123",
        "email": "user@example.com",
        "exp": now + 3600,
        "nbf": now - 10,
        "iat": now - 10,
    });
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(TEST_KID.to_string());
    let token = encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(TEST_PRIV_PEM.as_bytes()).unwrap(),
    )
    .unwrap();

    (verifier, token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde::Serialize;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Serialize)]
    struct TestClaims {
        iss: String,
        aud: Vec<String>,
        sub: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
        exp: usize,
        nbf: usize,
        iat: usize,
    }

    fn now() -> usize {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize
    }

    fn test_key() -> DecodingKey {
        DecodingKey::from_rsa_components(TEST_N, TEST_E).unwrap()
    }

    /// Sign a JWT (RS256, `kid=test-kid`) with the test private key. Generic
    /// so tests can sign either a `TestClaims` or a hand-built
    /// `serde_json::Value` that deliberately omits a claim.
    fn sign<T: Serialize>(claims: &T) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(TEST_KID.to_string());
        let key = EncodingKey::from_rsa_pem(TEST_PRIV_PEM.as_bytes()).unwrap();
        encode(&header, claims, &key).unwrap()
    }

    fn base_claims() -> TestClaims {
        let n = now();
        TestClaims {
            iss: TEST_TEAM.to_string(),
            aud: vec![TEST_AUD.to_string()],
            sub: "user-sub-123".to_string(),
            email: Some("user@example.com".to_string()),
            exp: n + 3600,
            nbf: n - 10,
            iat: n - 10,
        }
    }

    #[test]
    fn valid_token_returns_email_identity() {
        let token = sign(&base_claims());
        let id = verify_with_key(&token, &test_key(), TEST_TEAM, TEST_AUD).unwrap();
        assert_eq!(id, "user@example.com");
    }

    #[test]
    fn valid_token_without_email_falls_back_to_sub() {
        let mut claims = base_claims();
        claims.email = None;
        let token = sign(&claims);
        let id = verify_with_key(&token, &test_key(), TEST_TEAM, TEST_AUD).unwrap();
        assert_eq!(id, "user-sub-123");
    }

    #[test]
    fn tampered_signature_is_rejected() {
        let token = sign(&base_claims());
        // Flip one character of the signature segment: well-formed shape,
        // wrong signature. The verifier must not trust the (unchanged) claims.
        let mut parts: Vec<&str> = token.split('.').collect();
        let sig = parts[2].to_string();
        let flipped: String = sig
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if i == 0 && c != 'A' {
                    'A'
                } else if i == 0 {
                    'B'
                } else {
                    c
                }
            })
            .collect();
        parts[2] = &flipped;
        let tampered = parts.join(".");
        assert!(verify_with_key(&tampered, &test_key(), TEST_TEAM, TEST_AUD).is_err());
    }

    #[test]
    fn wrong_aud_is_rejected() {
        let mut claims = base_claims();
        claims.aud = vec!["some-other-aud".to_string()];
        let token = sign(&claims);
        assert!(verify_with_key(&token, &test_key(), TEST_TEAM, TEST_AUD).is_err());
    }

    #[test]
    fn wrong_iss_is_rejected() {
        let mut claims = base_claims();
        claims.iss = "https://attacker.cloudflareaccess.com".to_string();
        let token = sign(&claims);
        assert!(verify_with_key(&token, &test_key(), TEST_TEAM, TEST_AUD).is_err());
    }

    #[test]
    fn expired_token_is_rejected() {
        let mut claims = base_claims();
        let n = now();
        claims.iat = n - 100_000;
        claims.nbf = n - 100_000;
        claims.exp = n - 90_000;
        let token = sign(&claims);
        assert!(verify_with_key(&token, &test_key(), TEST_TEAM, TEST_AUD).is_err());
    }

    #[test]
    fn not_yet_valid_token_is_rejected() {
        let mut claims = base_claims();
        let n = now();
        claims.nbf = n + 100_000;
        claims.exp = n + 200_000;
        let token = sign(&claims);
        assert!(verify_with_key(&token, &test_key(), TEST_TEAM, TEST_AUD).is_err());
    }

    #[test]
    fn missing_aud_claim_is_rejected() {
        // A validly-signed token that omits `aud` entirely must be rejected —
        // `aud` is a required claim, not merely checked when present.
        let n = now();
        let claims = serde_json::json!({
            "iss": TEST_TEAM,
            "sub": "user-sub-123",
            "email": "user@example.com",
            "exp": n + 3600,
            "nbf": n - 10,
            "iat": n - 10,
        });
        let token = sign(&claims);
        assert!(verify_with_key(&token, &test_key(), TEST_TEAM, TEST_AUD).is_err());
    }

    #[test]
    fn missing_iss_claim_is_rejected() {
        // A validly-signed token that omits `iss` entirely must be rejected —
        // `iss` is a required claim, not merely checked when present.
        let n = now();
        let claims = serde_json::json!({
            "aud": [TEST_AUD],
            "sub": "user-sub-123",
            "email": "user@example.com",
            "exp": n + 3600,
            "nbf": n - 10,
            "iat": n - 10,
        });
        let token = sign(&claims);
        assert!(verify_with_key(&token, &test_key(), TEST_TEAM, TEST_AUD).is_err());
    }

    #[test]
    fn alg_none_token_is_rejected() {
        // Classic JWT bypass: an unsigned `alg: none` token whose claims are
        // all valid. RS256 is pinned, so decoding must reject it outright
        // rather than trust the unsigned claims. Built by hand because
        // `jsonwebtoken` will not encode `none`.
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
        let n = now();
        let header = serde_json::json!({ "alg": "none", "typ": "JWT", "kid": TEST_KID });
        let claims = serde_json::json!({
            "iss": TEST_TEAM,
            "aud": [TEST_AUD],
            "sub": "user-sub-123",
            "email": "user@example.com",
            "exp": n + 3600,
            "nbf": n - 10,
            "iat": n - 10,
        });
        let h = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap());
        let p = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
        // `alg: none` carries an empty signature segment.
        let token = format!("{h}.{p}.");
        assert!(verify_with_key(&token, &test_key(), TEST_TEAM, TEST_AUD).is_err());
    }

    #[test]
    fn hs256_key_confusion_is_rejected() {
        // Classic RS256->HS256 confusion: forge an HS256 token using the RSA
        // public key's own modulus bytes as the HMAC secret. If the verifier
        // ever accepted HS256, this would validate against the public key.
        // RS256 is pinned, so it must be rejected.
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
        let n = now();
        let claims = base_claims_json(n);
        let secret = URL_SAFE_NO_PAD.decode(TEST_N).unwrap();
        let mut header = Header::new(Algorithm::HS256);
        header.kid = Some(TEST_KID.to_string());
        let token = encode(&header, &claims, &EncodingKey::from_secret(&secret)).unwrap();
        assert!(verify_with_key(&token, &test_key(), TEST_TEAM, TEST_AUD).is_err());
    }

    /// Fully-valid claim set as a JSON value, for tests that must control the
    /// header (and so cannot go through `sign`/`TestClaims`).
    fn base_claims_json(n: usize) -> serde_json::Value {
        serde_json::json!({
            "iss": TEST_TEAM,
            "aud": [TEST_AUD],
            "sub": "user-sub-123",
            "email": "user@example.com",
            "exp": n + 3600,
            "nbf": n - 10,
            "iat": n - 10,
        })
    }

    #[tokio::test]
    async fn trailing_slash_team_domain_still_matches_iss() {
        // An operator who configures the team domain with a trailing slash
        // must still authenticate tokens whose `iss` is the domain without
        // the slash: the JWKS fetch and the `iss` check both use the
        // normalized value.
        let verifier = CfAccessVerifier::new(
            reqwest::Client::new(),
            format!("{TEST_TEAM}/"),
            TEST_AUD.to_string(),
        );
        verifier.seed_test_key(TEST_KID, test_key());
        let token = sign(&base_claims());
        let id = verifier.verify(&token).await.unwrap();
        assert_eq!(id, "user@example.com");
    }

    #[tokio::test]
    async fn verify_uses_fresh_cache_without_network() {
        // Seed a fresh cache, then verify end-to-end. A cache hit means no
        // JWKS fetch happens, so this exercises the full async path offline
        // and proves the cache is consulted rather than refetched every call.
        let verifier = CfAccessVerifier::new(
            reqwest::Client::new(),
            TEST_TEAM.to_string(),
            TEST_AUD.to_string(),
        );
        {
            let mut cache = verifier.cache.lock().unwrap();
            cache.keys.insert(TEST_KID.to_string(), test_key());
            cache.fetched_at = Some(Instant::now());
        }
        let token = sign(&base_claims());
        let id = verifier.verify(&token).await.unwrap();
        assert_eq!(id, "user@example.com");
    }

    #[tokio::test]
    async fn fresh_cache_missing_kid_is_unknown_kid_without_network() {
        // Fresh cache that lacks the token's kid must fail closed as
        // UnknownKid without attempting a fetch (the client points nowhere
        // useful here).
        let verifier = CfAccessVerifier::new(
            reqwest::Client::new(),
            TEST_TEAM.to_string(),
            TEST_AUD.to_string(),
        );
        {
            let mut cache = verifier.cache.lock().unwrap();
            cache.fetched_at = Some(Instant::now());
        }
        let token = sign(&base_claims());
        assert!(matches!(
            verifier.verify(&token).await,
            Err(CfAccessError::UnknownKid)
        ));
    }
}
