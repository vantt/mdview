//! Server-rendered HTML views. Self-contained: layout + CSS + JS as consts.
//! Theme is CSS-variable driven (no-flash head script); code colors come from
//! `/highlight.css` (syntect class-based), so themes switch without re-render.

use mdview_core::config::Config;
use mdview_core::domain::{IndexedFile, Project, RenderedPage, SearchResult};

pub fn layout(title: &str, head_extra: &str, body: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · mdview</title>
<script>
// No-flash: apply saved theme before body renders.
(function() {{
  try {{
    var t = localStorage.getItem('mdview-theme') || 'system';
    var dark = t === 'dark' || (t === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
    document.documentElement.setAttribute('data-theme', dark ? 'dark' : 'light');
  }} catch (e) {{}}
}})();
</script>
<link rel="stylesheet" href="/static/app.css">
<link rel="stylesheet" href="/highlight.css">
{head_extra}
</head>
<body>
{body}
<script src="/static/app.js"></script>
</body>
</html>"#
    )
}

pub fn project_list_page(projects: &[(Project, usize)]) -> String {
    let mut rows = String::new();
    if projects.is_empty() {
        rows.push_str("<p class=\"muted\">Chưa có project nào. Đăng ký: <code>mdview register &lt;dir&gt;</code> hoặc gọi MCP <code>mdview_view_file</code>.</p>");
    }
    for (p, count) in projects {
        rows.push_str(&format!(
            r#"<a class="card" href="/p/{id}/">
  <div class="card-title">{name}</div>
  <div class="muted">{root}</div>
  <div class="muted">{count} markdown files · {seen}</div>
</a>"#,
            id = esc(&p.id),
            name = esc(&p.name),
            root = esc(&p.root_path.to_string_lossy()),
            count = count,
            seen = esc(&p.last_seen_at),
        ));
    }
    let body = format!(
        r#"{topbar}
<main class="container"><h2>Projects</h2>{rows}</main>"#,
        topbar = topbar(""),
        rows = rows
    );
    layout("Projects", "", &body)
}

pub fn file_page(
    project: &Project,
    file: &IndexedFile,
    page: &RenderedPage,
    files: &[IndexedFile],
    backlinks: &[(String, String)],
) -> String {
    let tree = file_tree(project, files, &file.rel_path);
    let right = right_panel(project, page, backlinks);
    let breadcrumb = breadcrumb(project, &file.rel_path);
    // Raw markdown source for copy-as-markdown: the client maps a DOM selection
    // (via data-sourcepos line ranges) back to these source lines. Escape `<`
    // so a source containing "</script>" can't break out of the tag.
    let source_json = serde_json::to_string(&page.source)
        .unwrap_or_else(|_| "\"\"".into())
        .replace('<', "\\u003c");
    let head_extra = if page.has_mermaid {
        // PRD §9: Mermaid via CDN for AI-generated docs.
        r#"<script type="module">
import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';
window.__mermaid = mermaid;
function renderMermaid() {
  var dark = document.documentElement.getAttribute('data-theme') === 'dark';
  mermaid.initialize({ startOnLoad: false, theme: dark ? 'dark' : 'default' });
  mermaid.run({ querySelector: 'pre.mermaid' });
}
window.addEventListener('DOMContentLoaded', renderMermaid);
</script>"#
    } else {
        ""
    };
    let body = format!(
        r#"{topbar}
<div class="layout">
  <aside class="sidebar">{tree}</aside>
  <main class="content">
    {breadcrumb}
    <article class="markdown-body">{html}</article>
    <script type="application/json" id="mdsource">{source_json}</script>
  </main>
  {right}
</div>"#,
        topbar = topbar(&format!(
            "<span class=\"crumb\">{pname} / {rel}</span>",
            pname = esc(&project.name),
            rel = esc(&file.rel_path),
        )),
        tree = tree,
        breadcrumb = breadcrumb,
        html = page.html,
        source_json = source_json,
        right = right,
    );
    layout(&page.title, head_extra, &body)
}

/// Right sidebar: table of contents + backlinks (FR-18). Empty string if neither.
fn right_panel(project: &Project, page: &RenderedPage, backlinks: &[(String, String)]) -> String {
    let mut inner = String::new();
    let toc: Vec<_> = page
        .headings
        .iter()
        .filter(|h| h.level >= 1 && h.level <= 4)
        .collect();
    if !toc.is_empty() {
        inner.push_str("<div class=\"panel-head\">On this page</div><ul class=\"toc\">");
        for h in toc {
            inner.push_str(&format!(
                "<li class=\"toc-l{lvl}\"><a href=\"#{slug}\">{text}</a></li>",
                lvl = h.level,
                slug = esc(&h.slug),
                text = esc(&h.text),
            ));
        }
        inner.push_str("</ul>");
    }
    if !backlinks.is_empty() {
        inner.push_str("<div class=\"panel-head\">Linked from</div><ul class=\"backlinks\">");
        for (rel, title) in backlinks {
            inner.push_str(&format!(
                "<li><a href=\"/p/{pid}/{rel}\">{title}</a></li>",
                pid = esc(&project.id),
                rel = esc(rel),
                title = esc(title),
            ));
        }
        inner.push_str("</ul>");
    }
    if inner.is_empty() {
        String::new()
    } else {
        format!("<aside class=\"rightbar\">{inner}</aside>")
    }
}

/// Breadcrumb of path segments (orientation only; folders are not pages).
fn breadcrumb(project: &Project, rel_path: &str) -> String {
    let mut crumbs = format!(
        "<a href=\"/p/{pid}/\">{name}</a>",
        pid = esc(&project.id),
        name = esc(&project.name)
    );
    for seg in rel_path.split('/') {
        crumbs.push_str(&format!(" <span class=\"sep\">/</span> {}", esc(seg)));
    }
    format!("<nav class=\"breadcrumb\">{crumbs}</nav>")
}

/// The parent folder of a relative path (`""` for a root-level file).
fn parent_dir(rel: &str) -> &str {
    match rel.rfind('/') {
        Some(i) => &rel[..i],
        None => "",
    }
}

/// The last path segment of a relative path.
fn base_name(rel: &str) -> &str {
    match rel.rfind('/') {
        Some(i) => &rel[i + 1..],
        None => rel,
    }
}

/// Chapter sidebar (C2, per D 99e8df73): the search box, plus a `#chapter`
/// container the client script renders into — always one folder's contents with
/// a zoomable breadcrumb. The full file list ships as JSON so the zoom is
/// client-side (no extra routes); a minimal current-folder list is server-
/// rendered inside `#chapter` as a no-JS fallback.
fn file_tree(project: &Project, files: &[IndexedFile], active: &str) -> String {
    // JSON payload for the client renderer: one {p: rel_path, t: title} per file.
    let payload: Vec<_> = files
        .iter()
        .map(|f| serde_json::json!({ "p": f.rel_path, "t": f.title }))
        .collect();
    // Escape `<` so a title containing "</script>" can't break out of the tag.
    let json = serde_json::to_string(&payload)
        .unwrap_or_else(|_| "[]".into())
        .replace('<', "\\u003c");

    // No-JS fallback: the files directly in the active file's folder, by title.
    let active_dir = parent_dir(active);
    let mut fallback = String::new();
    for f in files
        .iter()
        .filter(|f| parent_dir(&f.rel_path) == active_dir)
    {
        let label = if f.title.is_empty() {
            base_name(&f.rel_path)
        } else {
            &f.title
        };
        let cls = if f.rel_path == active {
            "chap-file active"
        } else {
            "chap-file"
        };
        fallback.push_str(&format!(
            "<a class=\"{cls}\" href=\"/p/{pid}/{rel}\">{label}</a>",
            pid = esc(&project.id),
            rel = esc(&f.rel_path),
            label = esc(label),
        ));
    }

    format!(
        "<form class=\"search\" action=\"/p/{pid}/_search\" method=\"get\">\
         <input name=\"q\" placeholder=\"Search…\" autocomplete=\"off\"></form>\
         <nav class=\"chapter\" id=\"chapter\" data-pid=\"{pid}\" data-root=\"{root}\" \
         data-current=\"{cur}\">{fallback}</nav>\
         <script type=\"application/json\" id=\"filelist\">{json}</script>",
        pid = esc(&project.id),
        root = esc(&project.name),
        cur = esc(active),
        fallback = fallback,
        json = json,
    )
}

fn theme_toggle() -> &'static str {
    r#"<button id="theme-toggle" class="theme-toggle" title="Toggle theme">◐</button>"#
}

/// Shared top bar for every page: brand, a page-specific center slot (crumb or
/// empty), the Settings link, and the theme toggle. Keeps the Settings link on
/// all pages and stops each view re-inventing its own header.
fn topbar(center: &str) -> String {
    format!(
        r#"<header class="topbar">
  <a href="/" class="home">mdview</a>
  {center}
  <a class="nav-link" href="/settings">Settings</a>
  {toggle}
</header>"#,
        center = center,
        toggle = theme_toggle(),
    )
}

pub fn search_page(project: &Project, query: &str, results: &[SearchResult]) -> String {
    let mut items = String::new();
    if query.trim().is_empty() {
        items.push_str("<p class=\"muted\">Type a query to search this project.</p>");
    } else if results.is_empty() {
        items.push_str(&format!(
            "<p class=\"muted\">No matches for “{}”.</p>",
            esc(query)
        ));
    } else {
        for r in results {
            items.push_str(&format!(
                "<a class=\"result\" href=\"{url}\"><div class=\"result-title\">{title}</div>\
                 <div class=\"muted\">{rel}</div><div class=\"excerpt\">{excerpt}</div></a>",
                url = esc(&r.url),
                title = esc(&r.title),
                rel = esc(&r.rel_path),
                excerpt = highlight_excerpt(&r.excerpt),
            ));
        }
    }
    let body = format!(
        r#"{topbar}
<main class="container">
  <form class="search wide" action="/p/{pid}/_search" method="get">
    <input name="q" value="{q}" placeholder="Search…" autofocus autocomplete="off">
  </form>
  {items}
</main>"#,
        topbar = topbar(&format!(
            "<span class=\"crumb\">{name} · search</span>",
            name = esc(&project.name)
        )),
        pid = esc(&project.id),
        q = esc(query),
        items = items,
    );
    layout(&format!("search: {query}"), "", &body)
}

/// FTS snippets contain `<mark>…</mark>`. Escape everything, then restore marks.
fn highlight_excerpt(excerpt: &str) -> String {
    esc(excerpt)
        .replace("&lt;mark&gt;", "<mark>")
        .replace("&lt;/mark&gt;", "</mark>")
}

pub fn settings_page(cfg: &Config, saved: bool) -> String {
    let banner = if saved {
        "<div class=\"banner\">Saved. Server &amp; indexing changes apply after restart (<code>mdview stop &amp;&amp; mdview serve</code>).</div>"
    } else {
        ""
    };
    let checked = |b: bool| if b { "checked" } else { "" };
    let sel = |v: &str, opt: &str| if v == opt { "selected" } else { "" };
    let excludes = cfg.indexing.exclude_patterns.join("\n");

    let body = format!(
        r#"{topbar}
<main class="container">
  <h2>Settings</h2>
  {banner}
  <form class="settings" method="post" action="/api/config">
    <fieldset><legend>Server <span class="tag">restart</span></legend>
      <label>Port <input type="number" name="port" value="{port}" min="1" max="65535"></label>
      <label>Host <input name="host" value="{host}"> <span class="hint">127.0.0.1 (local) or 0.0.0.0 (LAN)</span></label>
      <label>Display hostname <input name="host_name" value="{host_name}"> <span class="hint">optional — used in rendered links instead of the IP/host above</span></label>
      <label class="cb"><input type="checkbox" name="open_browser" {open}> Open browser on start</label>
    </fieldset>
    <fieldset><legend>Renderer</legend>
      <label>Theme
        <select name="theme">
          <option value="system" {t_sys}>System</option>
          <option value="light" {t_light}>Light</option>
          <option value="dark" {t_dark}>Dark</option>
        </select>
      </label>
      <label>Syntax highlight theme <input name="syntax_theme" value="{syntax}"></label>
    </fieldset>
    <fieldset><legend>Indexing <span class="tag">restart</span></legend>
      <label>Debounce (ms) <input type="number" name="debounce_ms" value="{debounce}" min="0"></label>
      <label>Max file size (MB) <input type="number" name="max_file_size_mb" value="{maxmb}" min="1"></label>
      <label>Exclude patterns (one per line)
        <textarea name="exclude_patterns" rows="5">{excludes}</textarea>
      </label>
    </fieldset>
    <fieldset><legend>MCP <span class="tag">restart</span></legend>
      <label class="cb"><input type="checkbox" name="mcp_enabled" {mcp_on}> Enabled</label>
      <label>Transport
        <select name="mcp_transport">
          <option value="stdio" {tr_stdio}>stdio</option>
          <option value="http" {tr_http}>http</option>
        </select>
      </label>
    </fieldset>
    <button type="submit">Save</button>
  </form>
</main>"#,
        topbar = topbar("<span class=\"crumb\">Settings</span>"),
        banner = banner,
        port = cfg.server.port,
        host = esc(&cfg.server.host),
        host_name = esc(cfg.server.host_name.as_deref().unwrap_or("")),
        open = checked(cfg.server.open_browser_on_start),
        t_sys = sel(&cfg.renderer.theme, "system"),
        t_light = sel(&cfg.renderer.theme, "light"),
        t_dark = sel(&cfg.renderer.theme, "dark"),
        syntax = esc(&cfg.renderer.syntax_highlight_theme),
        debounce = cfg.indexing.debounce_ms,
        maxmb = cfg.indexing.max_file_size_mb,
        excludes = esc(&excludes),
        mcp_on = checked(cfg.mcp.enabled),
        tr_stdio = sel(&cfg.mcp.transport, "stdio"),
        tr_http = sel(&cfg.mcp.transport, "http"),
    );
    layout("Settings", "", &body)
}

pub fn error_page(status: u16, msg: &str) -> String {
    let body = format!(
        r#"{topbar}
<main class="container"><h2>{status}</h2><p class="muted">{msg}</p></main>"#,
        topbar = topbar(""),
        status = status,
        msg = esc(msg)
    );
    layout(&status.to_string(), "", &body)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub const APP_CSS: &str = include_str!("../assets/app.css");
pub const APP_JS: &str = include_str!("../assets/app.js");
