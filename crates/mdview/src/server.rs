//! Axum daemon: routes, live-reload WebSocket, filesystem watcher.

use crate::runtime::{self, DaemonInfo};
use crate::views;
use anyhow::Result;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Form, Path, Query, State,
    },
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use mdview_core::config::ServerConfig;
use mdview_core::indexer::now_rfc3339;
use mdview_core::render::theme_css;
use mdview_core::Engine;
use serde_json::json;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Engine>,
    pub reload_tx: broadcast::Sender<String>,
    pub highlight_css: Arc<String>,
    /// The token `POST /api/login` compares against. `None`/empty means
    /// login is unusably misconfigured and fails closed — `serve()`
    /// auto-generates one on first start (D3) rather than leaving this
    /// unset.
    pub web_secret: Arc<Option<String>>,
    /// Session ids issued by a successful login. In-memory by design (D7):
    /// a daemon restart invalidates every session, which is acceptable for
    /// a single-operator local tool.
    pub sessions: Arc<Mutex<HashSet<String>>>,
    /// Optional Cloudflare Access verifier. `None` unless the operator set
    /// both `cf_access_team_domain` and `cf_access_aud` (D5); when present,
    /// `auth::AuthSession` also accepts a verified `Cf-Access-Jwt-Assertion`
    /// header as an alternate credential.
    pub cf_access: Arc<Option<crate::cf_access::CfAccessVerifier>>,
}

/// Start the daemon: watcher + HTTP server. Blocks until shutdown.
pub async fn serve() -> Result<()> {
    ensure_web_secret()?;
    let engine = Arc::new(runtime::build_engine()?);
    let (reload_tx, _) = broadcast::channel::<String>(32);
    let highlight_css = Arc::new(build_highlight_css(&engine));
    let cf_access = build_cf_access(&engine.config.server);

    let state = AppState {
        engine: engine.clone(),
        reload_tx: reload_tx.clone(),
        highlight_css,
        web_secret: Arc::new(engine.config.server.web_secret.clone()),
        sessions: Arc::new(Mutex::new(HashSet::new())),
        cf_access: Arc::new(cf_access),
    };

    // Filesystem watcher (kept alive for the process lifetime).
    let _watch = crate::watch::spawn_watchers(engine.clone(), reload_tx.clone())?;

    // Bind with port auto-increment (PRD §10 / mdserve pattern).
    let cfg = &engine.config.server;
    let (listener, addr) = bind_with_retry(&cfg.host, cfg.port).await?;

    runtime::write_lock(&DaemonInfo {
        pid: std::process::id(),
        host: cfg.host.clone(),
        port: addr.port(),
        started_at: now_rfc3339(),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
    })?;
    tracing::info!("mdview serving on http://{addr}");
    // A wildcard bind (`0.0.0.0`) makes `http://0.0.0.0:PORT` a dead link, so
    // list every address that actually reaches this server — one per LAN
    // interface (loopback when none) or the configured hostname override.
    let urls = runtime::display_urls_for(&cfg.host, addr.port());
    if urls.len() == 1 {
        println!("mdview serving on {}", urls[0]);
    } else {
        println!("mdview serving on:");
        for url in &urls {
            println!("  {url}");
        }
    }
    if !is_loopback_host(&cfg.host) {
        eprintln!(
            "warning: mdview is bound to a non-loopback address ({}) — reachable from \
             the LAN. A login token is required (see above/Settings), but review who \
             else can reach this port before relying on that alone. Bind 127.0.0.1 \
             unless you intend LAN exposure.",
            cfg.host
        );
    }

    let app = router(state);
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await;
    runtime::remove_lock();
    result?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

/// First-run provisioning (D3): if no login token is configured, generate
/// one, persist it to `config.toml`, and print it once so the operator can
/// sign in. Idempotent — a config that already has a secret is untouched.
fn ensure_web_secret() -> Result<()> {
    let mut cfg = mdview_core::Config::load();
    if cfg.server.web_secret.as_deref().unwrap_or("").is_empty() {
        let secret = crate::auth::generate_web_secret();
        cfg.server.web_secret = Some(secret.clone());
        cfg.save()?;
        println!("No login token configured — generated one (saved to ~/.mdview/config.toml):");
        println!("  {secret}");
        println!("Sign in at /login with it, or change it later in Settings.");
    }
    Ok(())
}

/// Build the CF Access verifier iff BOTH `cf_access_team_domain` and
/// `cf_access_aud` are configured (D5) — one without the other leaves CF
/// Access fully off, never a partial/half-configured check.
fn build_cf_access(cfg: &ServerConfig) -> Option<crate::cf_access::CfAccessVerifier> {
    let team_domain = cfg
        .cf_access_team_domain
        .as_deref()
        .filter(|s| !s.is_empty())?;
    let aud = cfg.cf_access_aud.as_deref().filter(|s| !s.is_empty())?;
    let client = reqwest::Client::new();
    Some(crate::cf_access::CfAccessVerifier::new(
        client,
        team_domain.to_string(),
        aud.to_string(),
    ))
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index_page))
        .route("/health", get(health))
        .route("/login", get(login_page))
        .route("/api/login", post(crate::auth::login))
        .route("/api/logout", post(crate::auth::logout))
        .route("/api/status", get(status))
        .route("/api/projects", get(api_projects))
        .route("/settings", get(settings_page_handler))
        .route("/api/config", get(api_config).post(update_config))
        .route("/api/projects/:id/unregister", post(unregister_project))
        .route("/static/app.css", get(css_asset))
        .route("/static/app.js", get(js_asset))
        .route("/static/mermaid.min.js", get(mermaid_asset))
        .route("/highlight.css", get(highlight_asset))
        .route("/ws", get(ws_handler))
        .route("/s/:code", get(short_link_redirect))
        .route("/p/:id/", get(project_home))
        .route("/p/:id/_search", get(search_page))
        .route("/p/:id/_jump", get(jump_search))
        .route("/p/:id/_code/", get(code_root))
        .route("/p/:id/_code/*path", get(code_dir_or_file))
        .route("/p/:id/*path", get(project_path))
        .with_state(state)
}

/// `GET /login` — the only genuinely-open, discoverable route besides
/// `/health`. Deliberately unauthenticated: it has to be, or there would be
/// no way in.
async fn login_page() -> Response {
    Html(views::login_page()).into_response()
}

async fn index_page(_auth: crate::auth::AuthPage, State(st): State<AppState>) -> Response {
    match st.engine.list_projects() {
        Ok(projects) => {
            let with_counts: Vec<_> = projects
                .into_iter()
                .map(|p| {
                    let c = st.engine.file_count(&p.id).unwrap_or(0);
                    (p, c)
                })
                .collect();
            Html(views::project_list_page(&with_counts)).into_response()
        }
        Err(e) => internal_error(&e.to_string()),
    }
}

async fn health() -> impl IntoResponse {
    Json(json!({ "status": "ok", "app": "mdview", "version": env!("CARGO_PKG_VERSION") }))
}

async fn status(_auth: crate::auth::AuthSession, State(st): State<AppState>) -> impl IntoResponse {
    let projects = st.engine.list_projects().unwrap_or_default();
    let files: usize = st.engine.store.total_file_count().unwrap_or(0);
    Json(json!({
        "running": true,
        "app": "mdview",
        "version": env!("CARGO_PKG_VERSION"),
        "project_count": projects.len(),
        "indexed_file_count": files,
    }))
}

async fn api_projects(
    _auth: crate::auth::AuthSession,
    State(st): State<AppState>,
) -> impl IntoResponse {
    let projects = st.engine.list_projects().unwrap_or_default();
    let arr: Vec<_> = projects
        .into_iter()
        .map(|p| {
            let count = st.engine.file_count(&p.id).unwrap_or(0);
            project_summary_json(&p.id, &p.name, count)
        })
        .collect();
    Json(json!({ "projects": arr }))
}

/// One project's public API summary. Deliberately omits the absolute
/// `root_path`: `/api/projects` requires login, but a valid session still
/// has no reason to see local filesystem layout — defense in depth beyond
/// the login gate, not a substitute for it.
fn project_summary_json(id: &str, name: &str, file_count: usize) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "file_count": file_count,
        "url": format!("/p/{id}/"),
    })
}

async fn api_config(
    _auth: crate::auth::AuthSession,
    State(st): State<AppState>,
) -> impl IntoResponse {
    Json(json!(st.engine.config))
}

#[derive(serde::Deserialize)]
struct SavedFlag {
    saved: Option<String>,
}

async fn settings_page_handler(
    _auth: crate::auth::AuthPage,
    Query(flag): Query<SavedFlag>,
) -> Response {
    // Read fresh from disk so the form reflects the last save (the running daemon
    // still uses its startup config until restarted — noted in the UI).
    let cfg = mdview_core::Config::load();
    Html(views::settings_page(&cfg, flag.saved.is_some())).into_response()
}

#[derive(serde::Deserialize)]
struct SettingsForm {
    port: Option<u16>,
    host: Option<String>,
    hostname: Option<String>,
    open_browser: Option<String>,
    theme: Option<String>,
    syntax_theme: Option<String>,
    debounce_ms: Option<u64>,
    max_file_size_mb: Option<u64>,
    exclude_patterns: Option<String>,
    mcp_enabled: Option<String>,
    mcp_transport: Option<String>,
    cf_access_team_domain: Option<String>,
    cf_access_aud: Option<String>,
}

async fn update_config(_auth: crate::auth::AuthPage, Form(form): Form<SettingsForm>) -> Response {
    let mut cfg = mdview_core::Config::load();
    if let Some(p) = form.port {
        if p >= 1 {
            cfg.server.port = p;
        }
    }
    if let Some(h) = form.host {
        let h = h.trim();
        if !h.is_empty() {
            cfg.server.host = h.to_string();
        }
    }
    cfg.server.hostname = normalize_hostname(form.hostname);
    cfg.server.open_browser_on_start = form.open_browser.is_some();
    if let Some(t) = form.theme {
        if ["light", "dark", "system"].contains(&t.as_str()) {
            cfg.renderer.theme = t;
        }
    }
    if let Some(s) = form.syntax_theme {
        let s = s.trim();
        if !s.is_empty() {
            cfg.renderer.syntax_highlight_theme = s.to_string();
        }
    }
    if let Some(d) = form.debounce_ms {
        cfg.indexing.debounce_ms = d;
    }
    if let Some(m) = form.max_file_size_mb {
        if m >= 1 {
            cfg.indexing.max_file_size_mb = m;
        }
    }
    if let Some(ex) = form.exclude_patterns {
        cfg.indexing.exclude_patterns = ex
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
    }
    cfg.mcp.enabled = form.mcp_enabled.is_some();
    if let Some(tr) = form.mcp_transport {
        if ["stdio", "http"].contains(&tr.as_str()) {
            cfg.mcp.transport = tr;
        }
    }
    cfg.server.cf_access_team_domain = form
        .cf_access_team_domain
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    cfg.server.cf_access_aud = form
        .cf_access_aud
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let _ = cfg.save();
    Redirect::to("/settings?saved=1").into_response()
}

/// Remove a project from the registry, then return to the project list. This
/// only deletes the registry entry and index — the project's files on disk are
/// untouched, and re-registering re-scans them.
async fn unregister_project(
    _auth: crate::auth::AuthPage,
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let _ = st.engine.unregister(&id);
    Redirect::to("/").into_response()
}

// The CSS/JS assets are compiled into the binary and change whenever the daemon
// is upgraded, but their URLs never change. Without a cache directive a browser
// (mobile especially) may keep serving a stale copy after an upgrade, so UI
// fixes silently never arrive. `no-cache` forces a revalidation each load; the
// files are tiny and served locally, so the cost is negligible.
const NO_CACHE: (header::HeaderName, &str) = (header::CACHE_CONTROL, "no-cache");

async fn css_asset() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css"), NO_CACHE],
        views::APP_CSS,
    )
}
async fn js_asset() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript"), NO_CACHE],
        views::APP_JS,
    )
}
async fn highlight_asset(State(st): State<AppState>) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css"), NO_CACHE],
        st.highlight_css.to_string(),
    )
}
/// Vendored Mermaid bundle. It is large (~3.4 MB) but static across a daemon
/// version, so it may be cached hard — unlike the app's own CSS/JS.
async fn mermaid_asset() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript"),
            (header::CACHE_CONTROL, "public, max-age=604800"),
        ],
        views::MERMAID_JS,
    )
}

/// `/s/<code>` → the file's real page.
///
/// A redirect rather than a second way to render the page: everything about
/// rendering, including how relative links inside a document resolve, stays in
/// `project_path` with no duplicate. The long URL keeps working unchanged; this
/// is an extra door, not a replacement. A code whose file has left the index is a
/// plain 404 — there is nothing correct left to show, and guessing would open the
/// wrong file.
async fn short_link_redirect(
    _auth: crate::auth::AuthPage,
    State(st): State<AppState>,
    Path(code): Path<String>,
) -> Response {
    match st.engine.store.find_by_hash_prefix(&code) {
        Ok(Some((project_id, rel_path))) => {
            Redirect::to(&format!("/p/{project_id}/{rel_path}")).into_response()
        }
        Ok(None) => not_found("no file for that short link"),
        Err(e) => internal_error(&e.to_string()),
    }
}

async fn project_home(
    _auth: crate::auth::AuthPage,
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    match st.engine.list_files(&id) {
        Ok(files) if !files.is_empty() => {
            let entry = pick_entry_file(&files).unwrap_or(&files[0]);
            Redirect::to(&format!("/p/{}/{}", id, entry.rel_path)).into_response()
        }
        Ok(_) => not_found("project has no markdown files"),
        Err(_) => not_found("project not found"),
    }
}

/// Which file a project opens to — a fixed, predictable rule instead of
/// "whatever the index lists first". Precedence: a `README.md` wins over
/// everything, then an `index.md`, then any other file; within the same rank the
/// shallowest path wins, then case-insensitive alphabetical order. So a
/// project's README is the landing page when it has one, and the choice never
/// looks random.
fn pick_entry_file(
    files: &[mdview_core::domain::IndexedFile],
) -> Option<&mdview_core::domain::IndexedFile> {
    fn rank(rel: &str) -> u8 {
        match rel
            .rsplit('/')
            .next()
            .unwrap_or(rel)
            .to_ascii_lowercase()
            .as_str()
        {
            "readme.md" => 0,
            "index.md" => 1,
            _ => 2,
        }
    }
    fn depth(rel: &str) -> usize {
        rel.bytes().filter(|&b| b == b'/').count()
    }
    files.iter().min_by(|a, b| {
        rank(&a.rel_path)
            .cmp(&rank(&b.rel_path))
            .then_with(|| depth(&a.rel_path).cmp(&depth(&b.rel_path)))
            .then_with(|| {
                a.rel_path
                    .to_ascii_lowercase()
                    .cmp(&b.rel_path.to_ascii_lowercase())
            })
    })
}

async fn project_path(
    _auth: crate::auth::AuthPage,
    State(st): State<AppState>,
    Path((id, path)): Path<(String, String)>,
) -> Response {
    // Markdown file in the index → render it.
    if let Ok(Some(project)) = st.engine.get_project(&id) {
        if st
            .engine
            .store
            .get_file(&id, &path)
            .ok()
            .flatten()
            .is_some()
        {
            return match st.engine.render_file(&id, &path) {
                Ok(page) => {
                    let file = st.engine.store.get_file(&id, &path).unwrap().unwrap();
                    let files = st.engine.list_files(&id).unwrap_or_default();
                    let backlinks = st.engine.backlinks(&id, &path).unwrap_or_default();
                    Html(views::file_page(&project, &file, &page, &files, &backlinks))
                        .into_response()
                }
                Err(e) => internal_error(&e.to_string()),
            };
        }
        // Otherwise serve as a static asset (image, etc.) with traversal guard.
        if let Ok(abs) = st.engine.asset_path(&id, &path) {
            if let Ok(bytes) = std::fs::read(&abs) {
                return asset_response(&abs, bytes);
            }
        }
    }
    not_found("file not found")
}

#[derive(serde::Deserialize)]
struct SearchQuery {
    #[serde(default)]
    q: String,
}

async fn search_page(
    _auth: crate::auth::AuthPage,
    State(st): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<SearchQuery>,
) -> Response {
    let Ok(Some(project)) = st.engine.get_project(&id) else {
        return not_found("project not found");
    };
    let results = if query.q.trim().is_empty() {
        Vec::new()
    } else {
        st.engine
            .search(&query.q, Some(&id), 30)
            .unwrap_or_default()
    };
    Html(views::search_page(&project, &query.q, &results)).into_response()
}

#[derive(serde::Deserialize)]
struct JumpQuery {
    #[serde(default)]
    q: String,
    #[serde(default = "default_jump_limit")]
    limit: usize,
}

fn default_jump_limit() -> usize {
    20
}

/// Fuzzy file-jump endpoint: ranks the project's files by a fuzzy match of `q`
/// against their relative paths (complements the `_search` content search) and
/// returns the hits as JSON for the client jump palette.
async fn jump_search(
    _auth: crate::auth::AuthSession,
    State(st): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<JumpQuery>,
) -> Response {
    if matches!(st.engine.get_project(&id), Ok(None) | Err(_)) {
        return not_found("project not found");
    }
    let hits = st
        .engine
        .fuzzy_files(&id, &query.q, query.limit)
        .unwrap_or_default();
    Json(hits).into_response()
}

async fn code_root(
    _auth: crate::auth::AuthPage,
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    code_response(&st, &id, "").await
}

async fn code_dir_or_file(
    _auth: crate::auth::AuthPage,
    State(st): State<AppState>,
    Path((id, path)): Path<(String, String)>,
) -> Response {
    code_response(&st, &id, &path).await
}

/// Shared body for both Code-section routes. Every filesystem access goes
/// through `Engine::code_path`, the only thing allowed to touch a file for
/// this section — this function never computes a path itself. A denied path
/// and a missing path return the identical 404 body: a distinguishing
/// message would itself disclose that a denied file exists.
async fn code_response(st: &AppState, id: &str, path: &str) -> Response {
    let Ok(Some(project)) = st.engine.get_project(id) else {
        return not_found("file not found");
    };
    match st.engine.code_path(id, path) {
        Ok(mdview_core::engine::CodeView::Dir(listing)) => {
            Html(views::code_dir_page(&project, &listing)).into_response()
        }
        Ok(mdview_core::engine::CodeView::File {
            highlighted,
            truncated,
            size,
        }) => {
            let sidebar = code_sidebar_listing(st, id, path);
            Html(views::code_page(
                &project,
                path,
                views::CodeBody::Text {
                    highlighted: &highlighted,
                    truncated,
                    size,
                },
                &sidebar,
            ))
            .into_response()
        }
        Ok(mdview_core::engine::CodeView::Binary { size }) => {
            let sidebar = code_sidebar_listing(st, id, path);
            Html(views::code_page(
                &project,
                path,
                views::CodeBody::Binary { size },
                &sidebar,
            ))
            .into_response()
        }
        Err(_) => not_found("file not found"),
    }
}

/// The directory listing for a file's sidebar (its containing folder). Falls
/// back to an empty listing on lookup failure — the sidebar is navigation
/// chrome, not the reason the request itself would fail.
fn code_sidebar_listing(
    st: &AppState,
    id: &str,
    file_path: &str,
) -> mdview_core::code_source::DirListing {
    let parent = match file_path.rfind('/') {
        Some(i) => &file_path[..i],
        None => "",
    };
    match st.engine.code_path(id, parent) {
        Ok(mdview_core::engine::CodeView::Dir(listing)) => listing,
        _ => mdview_core::code_source::DirListing {
            rel_path: parent.to_string(),
            entries: Vec::new(),
        },
    }
}

async fn ws_handler(
    _auth: crate::auth::AuthSession,
    ws: WebSocketUpgrade,
    State(st): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws(socket, st.reload_tx.subscribe()))
}

async fn handle_ws(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    loop {
        tokio::select! {
            r = rx.recv() => match r {
                Ok(msg) => {
                    if socket.send(Message::Text(msg)).await.is_err() { break; }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            },
            r = socket.recv() => match r {
                Some(Ok(_)) => {}
                _ => break,
            },
        }
    }
}

async fn bind_with_retry(host: &str, port: u16) -> Result<(tokio::net::TcpListener, SocketAddr)> {
    for p in port..port.saturating_add(10) {
        let addr = format!("{host}:{p}");
        if let Ok(l) = tokio::net::TcpListener::bind(&addr).await {
            let local = l.local_addr()?;
            return Ok((l, local));
        }
    }
    anyhow::bail!("no free port in {port}..{}", port + 10);
}

fn build_highlight_css(engine: &Engine) -> String {
    // Atelier renders code blocks (`.fg-prose pre`) on a fixed dark "signature"
    // panel in both page schemes (D5), so syntect must emit a dark palette that
    // stays readable on that panel whether the page is in light or dark scheme.
    // Scope the same dark theme under both data-scheme values rather than
    // pairing a light theme with the light scheme.
    let dark = theme_css("base16-ocean.dark").unwrap_or_default();
    let _ = &engine.config.renderer.syntax_highlight_theme; // reserved for user override
    format!(
        "{}\n{}",
        scope_css(&dark, ":root[data-scheme=\"light\"]"),
        scope_css(&dark, ":root[data-scheme=\"dark\"]")
    )
}

/// Prefix every selector in `css` with `prefix` so two theme sheets coexist.
fn scope_css(css: &str, prefix: &str) -> String {
    let css = strip_comments(css);
    let mut out = String::new();
    for block in css.split_inclusive('}') {
        if let Some(idx) = block.find('{') {
            let (sel, rest) = block.split_at(idx);
            let scoped = sel
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| format!("{prefix} {s}"))
                .collect::<Vec<_>>()
                .join(", ");
            if !scoped.is_empty() {
                out.push_str(&scoped);
                out.push(' ');
                out.push_str(rest);
            }
        }
    }
    out
}

fn strip_comments(css: &str) -> String {
    let mut out = String::new();
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        if let Some(end) = rest[start..].find("*/") {
            rest = &rest[start + end + 2..];
        } else {
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

fn content_type(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("bmp") => "image/bmp",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// Build the HTTP response for a static project asset.
///
/// Assets are project-supplied bytes served on a no-auth origin and do NOT pass
/// through the markdown sanitizer. `X-Content-Type-Options: nosniff` plus a
/// fully-restrictive `Content-Security-Policy: sandbox` stop a project-supplied
/// `.svg` (served as `image/svg+xml`) from executing script when navigated to
/// directly, while still letting it render inside an `<img>`.
fn asset_response(path: &std::path::Path, bytes: Vec<u8>) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type(path)),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::CONTENT_SECURITY_POLICY, "sandbox"),
        ],
        bytes,
    )
        .into_response()
}

/// Normalize a submitted `hostname`: trim it and treat blank/whitespace-only as
/// unset. The settings form always sends the field (empty when cleared), so this
/// maps `""`/`"  "` → `None` and keeps the display override off `http://:PORT`.
fn normalize_hostname(raw: Option<String>) -> Option<String> {
    raw.map(|h| h.trim().to_string()).filter(|h| !h.is_empty())
}

/// True when `host` is a loopback bind (safe default). A wildcard (`0.0.0.0`/`::`)
/// or a concrete LAN IP is not loopback and exposes the no-auth server to the
/// network — the trigger for the startup warning.
fn is_loopback_host(host: &str) -> bool {
    matches!(host, "localhost")
        || host
            .parse::<std::net::IpAddr>()
            .map(|ip| ip.is_loopback())
            .unwrap_or(false)
}

fn not_found(msg: &str) -> Response {
    (StatusCode::NOT_FOUND, Html(views::error_page(404, msg))).into_response()
}
fn internal_error(msg: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Html(views::error_page(500, msg)),
    )
        .into_response()
}

#[cfg(test)]
mod highlight_css_tests {
    use super::*;

    #[test]
    fn dark_theme_is_scoped_to_both_schemes_without_page_wide_background() {
        let dark = theme_css("base16-ocean.dark").unwrap_or_default();
        let scoped = format!(
            "{}\n{}",
            scope_css(&dark, ":root[data-scheme=\"light\"]"),
            scope_css(&dark, ":root[data-scheme=\"dark\"]")
        );
        assert!(scoped.contains(":root[data-scheme=\"light\"]"));
        assert!(scoped.contains(":root[data-scheme=\"dark\"]"));
        // Every scoped selector must target something under the prefix, never
        // the bare :root itself, or the theme's background would leak page-wide.
        assert!(!scoped.contains(":root[data-scheme=\"light\"] {"));
        assert!(!scoped.contains(":root[data-scheme=\"dark\"] {"));
    }
}

#[cfg(test)]
mod asset_response_tests {
    use super::*;

    #[test]
    fn svg_asset_is_sandboxed_and_nosniff() {
        // A project-supplied .svg must be served with headers that neutralize
        // script execution on direct navigation (the XSS vector).
        let resp = asset_response(std::path::Path::new("diagram.svg"), b"<svg/>".to_vec());
        let h = resp.headers();
        assert_eq!(h.get(header::CONTENT_TYPE).unwrap(), "image/svg+xml");
        assert_eq!(h.get(header::CONTENT_SECURITY_POLICY).unwrap(), "sandbox");
        assert_eq!(h.get(header::X_CONTENT_TYPE_OPTIONS).unwrap(), "nosniff");
    }

    #[test]
    fn png_asset_also_carries_security_headers() {
        let resp = asset_response(std::path::Path::new("logo.png"), b"x".to_vec());
        let h = resp.headers();
        assert_eq!(h.get(header::CONTENT_TYPE).unwrap(), "image/png");
        assert_eq!(h.get(header::CONTENT_SECURITY_POLICY).unwrap(), "sandbox");
        assert_eq!(h.get(header::X_CONTENT_TYPE_OPTIONS).unwrap(), "nosniff");
    }

    #[test]
    fn project_summary_json_omits_filesystem_path() {
        let v = project_summary_json("abc", "My Proj", 3);
        assert!(
            v.get("root_path").is_none(),
            "unauthenticated API must not leak the project filesystem path"
        );
        assert_eq!(v["id"], "abc");
        assert_eq!(v["name"], "My Proj");
        assert_eq!(v["file_count"], 3);
        assert_eq!(v["url"], "/p/abc/");
    }

    #[test]
    fn hostname_form_value_normalizes_blank_to_none() {
        assert_eq!(normalize_hostname(None), None);
        assert_eq!(normalize_hostname(Some(String::new())), None);
        assert_eq!(normalize_hostname(Some("   ".into())), None);
        assert_eq!(
            normalize_hostname(Some("  host.local ".into())),
            Some("host.local".to_string())
        );
    }

    fn f(rel: &str) -> mdview_core::domain::IndexedFile {
        mdview_core::domain::IndexedFile {
            project_id: "p".into(),
            abs_path: std::path::PathBuf::from(rel),
            rel_path: rel.into(),
            title: rel.into(),
            size_bytes: 0,
            modified_at: String::new(),
        }
    }

    #[test]
    fn entry_file_prefers_readme_then_index_then_shallow_alpha() {
        // README wins even when a non-README sorts earlier alphabetically.
        let files = vec![f("architecture.md"), f("README.md"), f("guide.md")];
        assert_eq!(pick_entry_file(&files).unwrap().rel_path, "README.md");

        // README anywhere beats a root-level non-README (README is the rule).
        let files = vec![f("guide.md"), f("docs/README.md")];
        assert_eq!(pick_entry_file(&files).unwrap().rel_path, "docs/README.md");

        // A shallower README beats a deeper one.
        let files = vec![f("docs/README.md"), f("README.md")];
        assert_eq!(pick_entry_file(&files).unwrap().rel_path, "README.md");

        // No README → index.md wins.
        let files = vec![f("zoo.md"), f("index.md"), f("apple.md")];
        assert_eq!(pick_entry_file(&files).unwrap().rel_path, "index.md");

        // Neither → shallowest, then alphabetical.
        let files = vec![f("docs/a.md"), f("beta.md"), f("alpha.md")];
        assert_eq!(pick_entry_file(&files).unwrap().rel_path, "alpha.md");

        // Case-insensitive basename match.
        let files = vec![f("intro.md"), f("ReadMe.md")];
        assert_eq!(pick_entry_file(&files).unwrap().rel_path, "ReadMe.md");
    }

    #[test]
    fn loopback_detection_flags_wildcard_and_lan_as_exposed() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("::1"));
        assert!(!is_loopback_host("0.0.0.0"));
        assert!(!is_loopback_host("192.168.1.10"));
        assert!(!is_loopback_host("::"));
    }
}

/// Route-level auth wiring, exercised through the real router (not the
/// individual handlers) so a protected route accidentally left off the list
/// would show up here as a false "200" — the same shape of test herdr-gateway
/// itself uses to prove its own wiring
/// (`docs/history/daemon-auth-token-cf-access/CONTEXT.md`).
#[cfg(test)]
mod auth_wiring_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    const TOKEN: &str = "s3cret-token";

    fn test_state() -> AppState {
        let engine = Arc::new(mdview_core::Engine::new(
            mdview_core::SqliteStore::open_in_memory().unwrap(),
            mdview_core::Config::default(),
        ));
        let (reload_tx, _) = broadcast::channel::<String>(32);
        AppState {
            engine,
            reload_tx,
            highlight_css: Arc::new(String::new()),
            web_secret: Arc::new(Some(TOKEN.to_string())),
            sessions: Arc::new(Mutex::new(HashSet::new())),
            cf_access: Arc::new(None),
        }
    }

    async fn get(app: Router, uri: &str) -> StatusCode {
        app.oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    async fn login(app: Router, token: &str) -> Response {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("token={token}")))
                .unwrap(),
        )
        .await
        .unwrap()
    }

    fn cookie_from(res: &Response) -> String {
        res.headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string()
    }

    /// tsk-46t: the two extractors diverge on an unauthenticated request —
    /// `AuthSession` (API/websocket routes) stays an opaque 404, `AuthPage`
    /// (browser page routes) redirects to `/login`.
    #[tokio::test]
    async fn unauth_request_to_protected_api_route_is_opaque_404() {
        assert_eq!(
            get(router(test_state()), "/api/status").await,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn unauth_request_to_protected_page_route_redirects_to_login() {
        let res = router(test_state())
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        assert_eq!(res.headers().get(header::LOCATION).unwrap(), "/login");
    }

    #[tokio::test]
    async fn health_login_page_and_static_assets_stay_open() {
        let state = test_state();
        assert_eq!(get(router(state.clone()), "/health").await, StatusCode::OK);
        assert_eq!(get(router(state.clone()), "/login").await, StatusCode::OK);
        assert_eq!(
            get(router(state.clone()), "/static/app.css").await,
            StatusCode::OK
        );
        assert_eq!(get(router(state), "/highlight.css").await, StatusCode::OK);
    }

    #[tokio::test]
    async fn wrong_token_login_is_opaque_404() {
        let res = login(router(test_state()), "wrong").await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn missing_web_secret_fails_closed_even_with_right_looking_token() {
        let mut state = test_state();
        state.web_secret = Arc::new(None);
        let res = login(router(state), TOKEN).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn correct_token_sets_cookie_and_grants_access() {
        let state = test_state();
        let res = login(router(state.clone()), TOKEN).await;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let cookie = res
            .headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Strict"));

        let sid = cookie.split(';').next().unwrap().to_string();
        let res2 = router(state)
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(header::COOKIE, sid)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res2.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn logout_invalidates_the_session() {
        let state = test_state();
        let login_res = login(router(state.clone()), TOKEN).await;
        let sid = cookie_from(&login_res);

        let logout_res = router(state.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/logout")
                    .header(header::COOKIE, sid.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(logout_res.status(), StatusCode::SEE_OTHER);

        // "/" is a page route (AuthPage): logging out sends a subsequent
        // request back to /login rather than an opaque 404.
        let after = router(state)
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(header::COOKIE, sid)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(after.status(), StatusCode::SEE_OTHER);
        assert_eq!(after.headers().get(header::LOCATION).unwrap(), "/login");
    }

    /// Regression guard: with CF Access unconfigured (the default
    /// `test_state`), an unauthenticated request to a guarded API route is
    /// byte-identical to today — an opaque 404 — even if it carries a CF
    /// Access header, which must be completely ignored when no verifier is
    /// configured.
    #[tokio::test]
    async fn cf_access_unconfigured_ignores_header_and_stays_opaque_404() {
        let res = router(test_state())
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .header("Cf-Access-Jwt-Assertion", "anything.at.all")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    /// With CF Access configured and a validly-signed assertion for the
    /// expected team/aud, a guarded route succeeds with NO session cookie
    /// present.
    #[tokio::test]
    async fn cf_access_configured_valid_header_authenticates_without_cookie() {
        let (verifier, token) = crate::cf_access::test_verifier_with_valid_token();
        let mut state = test_state();
        state.cf_access = Arc::new(Some(verifier));
        let res = router(state)
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("Cf-Access-Jwt-Assertion", token)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    /// With CF Access configured but the assertion unverifiable (not a real
    /// signed token), the request gets exactly the same opaque 404 as any
    /// unauthenticated one on an API route — the raw header is never
    /// trusted.
    #[tokio::test]
    async fn cf_access_configured_bogus_header_is_opaque_404() {
        let (verifier, _valid) = crate::cf_access::test_verifier_with_valid_token();
        let mut state = test_state();
        state.cf_access = Arc::new(Some(verifier));
        let res = router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .header("Cf-Access-Jwt-Assertion", "not.a.jwt")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    /// With CF Access configured, a valid session cookie still authenticates
    /// — the cookie path is preserved unchanged alongside the new branch.
    #[tokio::test]
    async fn cf_access_configured_cookie_still_authenticates() {
        let (verifier, _token) = crate::cf_access::test_verifier_with_valid_token();
        let mut state = test_state();
        state.cf_access = Arc::new(Some(verifier));
        let login_res = login(router(state.clone()), TOKEN).await;
        let sid = cookie_from(&login_res);
        let res = router(state)
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(header::COOKIE, sid)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
