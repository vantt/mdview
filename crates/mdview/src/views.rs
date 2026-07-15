//! Server-rendered HTML views. Self-contained: layout + CSS + JS as consts.
//! Theme is CSS-variable driven (no-flash head script); code colors come from
//! `/highlight.css` (syntect class-based), so themes switch without re-render.

use mdview_core::domain::{IndexedFile, Project, RenderedPage};

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
        r#"<header class="topbar"><h1>mdview</h1>{toggle}</header>
<main class="container"><h2>Projects</h2>{rows}</main>"#,
        toggle = theme_toggle(),
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
        r#"<header class="topbar">
  <a href="/" class="home">mdview</a>
  <span class="crumb">{pname} / {rel}</span>
  {toggle}
</header>
<div class="layout">
  <aside class="sidebar">{tree}</aside>
  <main class="content">
    {breadcrumb}
    <article class="markdown-body">{html}</article>
  </main>
  {right}
</div>"#,
        pname = esc(&project.name),
        rel = esc(&file.rel_path),
        toggle = theme_toggle(),
        tree = tree,
        breadcrumb = breadcrumb,
        html = page.html,
        right = right,
    );
    layout(&page.title, head_extra, &body)
}

/// Right sidebar: table of contents + backlinks (FR-18). Empty string if neither.
fn right_panel(project: &Project, page: &RenderedPage, backlinks: &[(String, String)]) -> String {
    let mut inner = String::new();
    let toc: Vec<_> = page.headings.iter().filter(|h| h.level >= 1 && h.level <= 4).collect();
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

fn file_tree(project: &Project, files: &[IndexedFile], active: &str) -> String {
    let mut out = format!("<div class=\"tree-head\">{}</div><ul class=\"tree\">", esc(&project.name));
    for f in files {
        let cls = if f.rel_path == active { "tree-item active" } else { "tree-item" };
        out.push_str(&format!(
            "<li class=\"{cls}\"><a href=\"/p/{pid}/{rel}\">{label}</a></li>",
            pid = esc(&project.id),
            rel = esc(&f.rel_path),
            label = esc(&f.rel_path),
        ));
    }
    out.push_str("</ul>");
    out
}

fn theme_toggle() -> &'static str {
    r#"<button id="theme-toggle" class="theme-toggle" title="Toggle theme">◐</button>"#
}

pub fn error_page(status: u16, msg: &str) -> String {
    let body = format!(
        r#"<header class="topbar"><a href="/" class="home">mdview</a></header>
<main class="container"><h2>{status}</h2><p class="muted">{msg}</p></main>"#,
        status = status,
        msg = esc(msg)
    );
    layout(&status.to_string(), "", &body)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub const APP_CSS: &str = include_str!("../assets/app.css");
pub const APP_JS: &str = include_str!("../assets/app.js");
