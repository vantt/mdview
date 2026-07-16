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
    routing::get,
    Json, Router,
};
use mdview_core::indexer::now_rfc3339;
use mdview_core::render::theme_css;
use mdview_core::Engine;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Engine>,
    pub reload_tx: broadcast::Sender<String>,
    pub highlight_css: Arc<String>,
}

/// Start the daemon: watcher + HTTP server. Blocks until shutdown.
pub async fn serve() -> Result<()> {
    let engine = Arc::new(runtime::build_engine()?);
    let (reload_tx, _) = broadcast::channel::<String>(32);
    let highlight_css = Arc::new(build_highlight_css(&engine));

    let state = AppState {
        engine: engine.clone(),
        reload_tx: reload_tx.clone(),
        highlight_css,
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
    })?;
    tracing::info!("mdview serving on http://{addr}");
    println!("mdview serving on http://{addr}");
    if !is_loopback_host(&cfg.host) {
        eprintln!(
            "warning: mdview is bound to a non-loopback address ({}) and has NO \
             authentication — anyone who can reach this port can read every \
             indexed file and each project's filesystem path. Bind 127.0.0.1 \
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

fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index_page))
        .route("/health", get(health))
        .route("/api/status", get(status))
        .route("/api/projects", get(api_projects))
        .route("/settings", get(settings_page_handler))
        .route("/api/config", get(api_config).post(update_config))
        .route("/static/app.css", get(css_asset))
        .route("/static/app.js", get(js_asset))
        .route("/highlight.css", get(highlight_asset))
        .route("/ws", get(ws_handler))
        .route("/p/:id/", get(project_home))
        .route("/p/:id/_search", get(search_page))
        .route("/p/:id/_jump", get(jump_search))
        .route("/p/:id/*path", get(project_path))
        .with_state(state)
}

async fn index_page(State(st): State<AppState>) -> Response {
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

async fn status(State(st): State<AppState>) -> impl IntoResponse {
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

async fn api_projects(State(st): State<AppState>) -> impl IntoResponse {
    let projects = st.engine.list_projects().unwrap_or_default();
    let arr: Vec<_> = projects
        .into_iter()
        .map(|p| {
            let count = st.engine.file_count(&p.id).unwrap_or(0);
            json!({
                "id": p.id, "name": p.name, "root_path": p.root_path,
                "file_count": count, "url": format!("/p/{}/", p.id),
            })
        })
        .collect();
    Json(json!({ "projects": arr }))
}

async fn api_config(State(st): State<AppState>) -> impl IntoResponse {
    Json(json!(st.engine.config))
}

#[derive(serde::Deserialize)]
struct SavedFlag {
    saved: Option<String>,
}

async fn settings_page_handler(Query(flag): Query<SavedFlag>) -> Response {
    // Read fresh from disk so the form reflects the last save (the running daemon
    // still uses its startup config until restarted — noted in the UI).
    let cfg = mdview_core::Config::load();
    Html(views::settings_page(&cfg, flag.saved.is_some())).into_response()
}

#[derive(serde::Deserialize)]
struct SettingsForm {
    port: Option<u16>,
    host: Option<String>,
    host_name: Option<String>,
    open_browser: Option<String>,
    theme: Option<String>,
    syntax_theme: Option<String>,
    debounce_ms: Option<u64>,
    max_file_size_mb: Option<u64>,
    exclude_patterns: Option<String>,
    mcp_enabled: Option<String>,
    mcp_transport: Option<String>,
}

async fn update_config(Form(form): Form<SettingsForm>) -> Response {
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
    cfg.server.host_name = form
        .host_name
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty());
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
    let _ = cfg.save();
    Redirect::to("/settings?saved=1").into_response()
}

async fn css_asset() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], views::APP_CSS)
}
async fn js_asset() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript")],
        views::APP_JS,
    )
}
async fn highlight_asset(State(st): State<AppState>) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        st.highlight_css.to_string(),
    )
}

async fn project_home(State(st): State<AppState>, Path(id): Path<String>) -> Response {
    match st.engine.list_files(&id) {
        Ok(files) if !files.is_empty() => {
            Redirect::to(&format!("/p/{}/{}", id, files[0].rel_path)).into_response()
        }
        Ok(_) => not_found("project has no markdown files"),
        Err(_) => not_found("project not found"),
    }
}

async fn project_path(
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

async fn ws_handler(ws: WebSocketUpgrade, State(st): State<AppState>) -> Response {
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
    fn loopback_detection_flags_wildcard_and_lan_as_exposed() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("::1"));
        assert!(!is_loopback_host("0.0.0.0"));
        assert!(!is_loopback_host("192.168.1.10"));
        assert!(!is_loopback_host("::"));
    }
}
