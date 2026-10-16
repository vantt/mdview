//! Server-rendered HTML views. Self-contained: layout + CSS + JS as consts.
//! Theme is CSS-variable driven (no-flash head script); code colors come from
//! `/highlight.css` (syntect class-based), so themes switch without re-render.

use mdview_core::code_source::DirListing;
use mdview_core::config::Config;
use mdview_core::domain::{IndexedFile, Project, RenderedPage, SearchOutcome, SearchSort};
use mdview_core::render::HighlightedSource;
use mdview_core::snippet;

pub fn layout(title: &str, head_extra: &str, body: &str) -> String {
    let title = esc(title);
    format!(
        r#"<!doctype html>
<html lang="en" data-theme="atelier" class="fg-root">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · mdview</title>
<script>
// No-flash: apply saved scheme before body renders.
(function() {{
  try {{
    var t = localStorage.getItem('mdview-theme') || 'system';
    var dark = t === 'dark' || (t === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
    document.documentElement.setAttribute('data-scheme', dark ? 'dark' : 'light');
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
    let listing = if projects.is_empty() {
        "<p class=\"fg-empty\">Chưa có project nào. Đăng ký: <code>mdview register &lt;dir&gt;</code> hoặc gọi MCP <code>mdview_view_file</code>.</p>".to_string()
    } else {
        // Cards (not a table — cards read better on phones/tablets). Each card is
        // a clickable link to the project plus a delete control that unregisters
        // it. The filesystem path is deliberately omitted — defense in depth
        // beyond the login gate, not a substitute for it.
        let mut cards = String::new();
        for (p, count) in projects {
            cards.push_str(&format!(
                r#"<div class="proj-card">
  <a class="fg-card proj-card__link" href="/p/{id}/">
    <div class="fg-card__title">{name}</div>
    <div class="fg-card__sub">{count} indexed files · <time class="proj-card__time" datetime="{seen}">{seen}</time></div>
  </a>
  <form class="proj-card__delete" method="post" action="/api/projects/{id}/unregister" data-project="{name}">
    <button type="submit" class="proj-card__del" aria-label="Remove {name} from mdview" title="Remove from mdview">✕</button>
  </form>
</div>"#,
                id = esc(&p.id),
                name = esc(&p.name),
                count = count,
                seen = esc(&p.last_seen_at),
            ));
        }
        format!(r#"<div class="proj-cards">{cards}</div>"#, cards = cards)
    };
    let body = format!(
        r#"{topbar}
<main class="fg-page"><h2 class="fg-pagehead__title">Projects</h2>{listing}</main>"#,
        topbar = topbar(""),
        listing = listing
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
    let actions = format!(
        "{}{}{}",
        edit_md_button(),
        copy_md_button(),
        md_editor_bar()
    );
    let breadcrumb = breadcrumb(project, "", &file.rel_path, true, &actions);
    // Raw markdown source for copy-as-markdown and the in-place editor: the
    // client maps a DOM selection (via data-sourcepos line ranges) back to
    // these source lines, and the editor loads its textarea from it. Escape
    // `<` so a source containing "</script>" can't break out of the tag.
    let source_json = escape_json_for_script(&page.source);
    // Hash of that same source: the editor sends it back on save so the
    // daemon can refuse to clobber a file something else wrote meanwhile.
    let source_hash = mdview_core::indexer::content_hash(&page.source);
    let head_extra = if page.has_mermaid {
        // Mermaid is vendored and served locally (/static/mermaid.min.js) rather
        // than loaded from a CDN: the daemon commonly runs on a LAN/offline host
        // where a CDN is unreachable, which would leave diagrams unrendered.
        r#"<script src="/static/mermaid.min.js" defer></script>
<script>
(function () {
  // Surface a render failure ON the page (mobile has no dev console), so a
  // broken diagram shows why instead of silently staying blank.
  function fail(msg) {
    document.querySelectorAll('pre.mermaid').forEach(function (p) {
      if (p.querySelector('svg') || p.dataset.err) return;
      p.dataset.err = '1';
      var d = document.createElement('div');
      d.className = 'mermaid-error';
      d.textContent = 'Mermaid did not render: ' + msg;
      p.parentNode.insertBefore(d, p.nextSibling);
    });
  }
  function renderMermaid() {
    if (!window.mermaid) { fail('library /static/mermaid.min.js did not load'); return; }
    window.__mermaid = window.mermaid;
    var dark = document.documentElement.getAttribute('data-scheme') === 'dark';
    try { window.mermaid.initialize({ startOnLoad: false, theme: dark ? 'dark' : 'default' }); }
    catch (e) { fail('initialize: ' + ((e && e.message) || e)); return; }
    var done = function () { document.dispatchEvent(new Event('mdview:mermaid-done')); };
    var onErr = function (e) { fail((e && e.message) || String(e)); done(); };
    try {
      var r = window.mermaid.run({ querySelector: 'pre.mermaid' });
      if (r && r.then) { r.then(done, onErr); } else { done(); }
    } catch (e) { onErr(e); }
  }
  if (document.readyState === 'loading') {
    window.addEventListener('DOMContentLoaded', renderMermaid);
  } else {
    renderMermaid();
  }
})();
</script>"#
    } else {
        ""
    };
    let body = format!(
        r#"{topbar}
<div class="layout">
  <aside id="sidebar" class="sidebar">{tree}</aside>
  <div class="sidebar-backdrop"></div>
  <main class="content">
    {breadcrumb}
    <div class="fg-reading">
      <article class="fg-prose markdown-body">{html}</article>
    </div>
    {editor}
    <script type="application/json" id="mdsource" data-hash="{source_hash}">{source_json}</script>
  </main>
  {right}
</div>"#,
        topbar = topbar_full(
            sidebar_toggle(),
            &section_switch(project, Section::Docs),
            "",
        ),
        tree = tree,
        breadcrumb = breadcrumb,
        html = page.html,
        editor = md_editor(),
        source_hash = source_hash,
        source_json = source_json,
        right = right,
    );
    layout(&page.title, head_extra, &body)
}

/// Escape `<` in an already-serialized JSON blob so a literal `</script>` in
/// the data cannot break out of the `<script>` tag it is embedded in. Shared by
/// every place that inlines JSON into a page, so the guard can never diverge.
fn escape_script_breakout(json: &str) -> String {
    json.replace('<', "\\u003c")
}

/// Serialize `source` as a JSON string literal safe to embed inside a
/// `<script>` tag: escapes `<` to `<` so a source containing a literal
/// "</script>" can't break out of the tag.
fn escape_json_for_script(source: &str) -> String {
    escape_script_breakout(&serde_json::to_string(source).unwrap_or_else(|_| "\"\"".into()))
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
/// `base` is the section's root prefix under `/p/:id/` — `""` for Docs,
/// `"_code/"` for the Code section — so both sections share this one
/// function instead of a near-duplicate each. Renders as a sticky bar split
/// left|right. `right` is pre-rendered HTML for the right half (e.g. Docs'
/// copy-as-markdown button, or a code file's type/size) — `""` for pages
/// with nothing to put there. `narrow`, true for Docs only, caps the bar's
/// width to match `.fg-reading` (the markdown column below it); Code's bar
/// stays full-width, matching its own full-width main pane.
fn breadcrumb(project: &Project, base: &str, rel_path: &str, narrow: bool, right: &str) -> String {
    let mut crumbs = format!(
        "<a href=\"/p/{pid}/{base}\">{name}</a>",
        pid = esc(&project.id),
        base = base,
        name = esc(&project.name)
    );
    for seg in rel_path.split('/').filter(|s| !s.is_empty()) {
        crumbs.push_str(&format!(" <span class=\"sep\">/</span> {}", esc(seg)));
    }
    let cls = if narrow {
        "breadcrumb breadcrumb--reading"
    } else {
        "breadcrumb"
    };
    format!(
        "<nav class=\"{cls}\"><div class=\"breadcrumb__left\">{crumbs}</div><div class=\"breadcrumb__right\">{right}</div></nav>"
    )
}

/// Which section's page is currently active, for `section_switch`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Docs,
    Code,
}

/// Docs|Code toggle for the top bar. The only change `file_page` makes to
/// the existing Docs pages — Code pages carry it via the same function.
fn section_switch(project: &Project, active: Section) -> String {
    let pid = esc(&project.id);
    let docs_current = if active == Section::Docs {
        " aria-current=\"page\""
    } else {
        ""
    };
    let code_current = if active == Section::Code {
        " aria-current=\"page\""
    } else {
        ""
    };
    format!(
        "<nav class=\"section-switch\"><a href=\"/p/{pid}/\"{docs_current}>Docs</a><a href=\"/p/{pid}/_code/\"{code_current}>Code</a></nav>"
    )
}

/// What the main pane of a Code-section file page shows: highlighted source,
/// or a binary notice (never a garbled render of raw bytes).
pub enum CodeBody<'a> {
    Text {
        highlighted: &'a HighlightedSource,
        truncated: bool,
        size: u64,
    },
    Binary {
        size: u64,
    },
}

/// A single source file in the Code section: line-numbered highlighted
/// source (or a binary notice) plus a sidebar showing its containing
/// directory. Deliberately does not reuse `file_page` — that function is
/// bound to `IndexedFile`/`RenderedPage` and carries a TOC, backlinks, and
/// the copy-as-markdown source blob, none of which exist here.
pub fn code_page(
    project: &Project,
    rel_path: &str,
    body: CodeBody,
    sidebar: &DirListing,
) -> String {
    let active = base_name(rel_path);
    let tree = code_tree(project, sidebar, Some(active));

    let (main, meta) = match body {
        CodeBody::Binary { size } => (
            format!(
                "<div class=\"codeview__binary\">Binary file &middot; {size} — cannot be displayed.</div>",
                size = format_size(size)
            ),
            format!(
                "<span class=\"breadcrumb__meta-lang\">Binary</span> \
                 <span class=\"breadcrumb__meta-size\">{size}</span>",
                size = format_size(size)
            ),
        ),
        CodeBody::Text {
            highlighted,
            truncated,
            size,
        } => {
            let banner = if truncated {
                "<div class=\"codeview__banner\">File truncated — showing the first part only.</div>"
                    .to_string()
            } else {
                String::new()
            };
            let mut rows = String::with_capacity(highlighted.lines.len() * 64);
            for (i, line) in highlighted.lines.iter().enumerate() {
                let n = i + 1;
                rows.push_str(&format!(
                    "<tr id=\"L{n}\"><td class=\"codeview__num\"><a href=\"#L{n}\">{n}</a></td><td class=\"codeview__line\"><code>{line}</code></td></tr>"
                ));
            }
            let main = format!(
                "{banner}<table class=\"codeview__table\">{rows}</table>",
                banner = banner,
                rows = rows,
            );
            let meta = format!(
                "<span class=\"breadcrumb__meta-lang\">{lang}</span> \
                 <span class=\"breadcrumb__meta-size\">{size}</span>",
                lang = esc(&highlighted.syntax_name),
                size = format_size(size),
            );
            (main, meta)
        }
    };
    let breadcrumb = breadcrumb(project, "_code/", rel_path, false, &meta);

    let body_html = format!(
        r#"{topbar}
<div class="layout">
  <aside id="sidebar" class="sidebar">{tree}</aside>
  <div class="sidebar-backdrop"></div>
  <main class="content content--code">
    {breadcrumb}
    <div class="codeview">{main}</div>
  </main>
</div>"#,
        topbar = topbar_full(
            sidebar_toggle(),
            &section_switch(project, Section::Code),
            ""
        ),
        tree = tree,
        breadcrumb = breadcrumb,
        main = main,
    );
    layout(active, "", &body_html)
}

/// A directory in the Code section: the same listing rendered both in the
/// sidebar (compact nav) and the main pane (with sizes and ages) — the two
/// panes serve different roles, same as a file-explorer's tree-plus-detail
/// split. The main pane is a column-aligned table (name / size / modified)
/// under a folder header with counts; the filter box and arrow-key row
/// navigation are progressive enhancements wired by `app.js`, so the
/// listing still works as plain links without scripts.
pub fn code_dir_page(project: &Project, listing: &DirListing) -> String {
    let tree = code_tree(project, listing, None);
    let breadcrumb = breadcrumb(project, "_code/", &listing.rel_path, false, "");
    let pid = esc(&project.id);
    let now = std::time::SystemTime::now();

    let mut rows = String::new();
    if !listing.rel_path.is_empty() {
        let parent = parent_dir(&listing.rel_path);
        let parent_name = if parent.is_empty() {
            project.name.as_str()
        } else {
            base_name(parent)
        };
        rows.push_str(&format!(
            "<li><a class=\"codelist__row codelist__row--up\" href=\"/p/{pid}/_code/{parent}\" data-up>\
             <span class=\"codelist__name\">{icon}<span class=\"codelist__label\">..</span>\
             <span class=\"codelist__hint\">Up to {parent_name}</span></span></a></li>",
            pid = pid,
            parent = esc(parent),
            icon = ICON_UP,
            parent_name = esc(parent_name),
        ));
    }

    let (mut dir_count, mut file_count, mut total) = (0usize, 0usize, 0u64);
    for entry in &listing.entries {
        let rel = child_rel(&listing.rel_path, &entry.name);
        let (cls, icon, size) = if entry.is_dir {
            dir_count += 1;
            (
                "codelist__row codelist__row--dir",
                ICON_FOLDER,
                String::new(),
            )
        } else {
            file_count += 1;
            total += entry.size;
            (
                "codelist__row",
                file_icon(&entry.name),
                format_size(entry.size),
            )
        };
        let modified = entry
            .modified
            .map(|t| {
                let (machine, human) = utc_timestamp(t);
                format!(
                    "<time datetime=\"{machine}\" title=\"{human}\">{age}</time>",
                    age = format_age(now, t),
                )
            })
            .unwrap_or_default();
        rows.push_str(&format!(
            "<li><a class=\"{cls}\" href=\"/p/{pid}/_code/{rel}\" data-name=\"{name}\">\
             <span class=\"codelist__name\">{icon}<span class=\"codelist__label\">{name}</span></span>\
             <span class=\"codelist__size\">{size}</span>\
             <span class=\"codelist__date\">{modified}</span></a></li>",
            cls = cls,
            pid = pid,
            rel = esc(&rel),
            name = esc(&entry.name),
            icon = icon,
            size = size,
            modified = modified,
        ));
    }

    let mut meta = Vec::new();
    if dir_count > 0 {
        meta.push(plural(dir_count, "folder", "folders"));
    }
    if file_count > 0 {
        meta.push(plural(file_count, "file", "files"));
        meta.push(format_size(total));
    }
    let meta = if meta.is_empty() {
        "Empty folder".to_string()
    } else {
        meta.join(" &middot; ")
    };

    let heading = if listing.rel_path.is_empty() {
        project.name.as_str()
    } else {
        base_name(&listing.rel_path)
    };
    let empty = if listing.entries.is_empty() {
        "<p class=\"codelist__empty\">This folder is empty.</p>"
    } else {
        ""
    };
    let table = format!(
            "<div class=\"codelist__table\">\
             <div class=\"codelist__cols\" aria-hidden=\"true\"><span>Name</span><span>Size</span><span>Modified</span></div>\
             <ul class=\"codelist__rows\" id=\"codelist-rows\">{rows}</ul>\
             <p class=\"codelist__nomatch\" hidden>No files or folders match “<span class=\"codelist__q\"></span>”.</p>\
             {empty}</div>",
    );
    let filter = if listing.entries.len() > 1 {
        "<div class=\"codelist__filter\" hidden>\
         <input class=\"fg-input\" type=\"search\" placeholder=\"Filter this folder…\" aria-label=\"Filter this folder\" autocomplete=\"off\" spellcheck=\"false\" aria-controls=\"codelist-rows\"></div>"
    } else {
        ""
    };

    let title = if listing.rel_path.is_empty() {
        project.name.clone()
    } else {
        listing.rel_path.clone()
    };
    let body_html = format!(
        r#"{topbar}
<div class="layout">
  <aside id="sidebar" class="sidebar">{tree}</aside>
  <div class="sidebar-backdrop"></div>
  <main class="content content--code">
    {breadcrumb}
    <div class="codelist"><div class="codelist__inner">
      <header class="codelist__head">
        <div class="codelist__heading">
          <h1 class="codelist__title">{folder_icon}<span>{heading}</span></h1>
          <p class="codelist__meta">{meta}</p>
        </div>
        {filter}
      </header>
      {table}
    </div></div>
  </main>
</div>"#,
        topbar = topbar_full(
            sidebar_toggle(),
            &section_switch(project, Section::Code),
            ""
        ),
        tree = tree,
        breadcrumb = breadcrumb,
        folder_icon = ICON_FOLDER_OPEN,
        heading = esc(heading),
        meta = meta,
        filter = filter,
        table = table,
    );
    layout(&title, "", &body_html)
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

// Listing icons: one outline family (24-unit grid, 1.75 stroke,
// currentColor), sized and tinted by `.codelist__icon` in app.css.
const ICON_FOLDER: &str = r#"<svg class="codelist__icon codelist__icon--dir" viewBox="0 0 24 24" aria-hidden="true"><path d="M3 7.5A1.5 1.5 0 0 1 4.5 6h4.38a1.5 1.5 0 0 1 1.06.44L11.5 8h8A1.5 1.5 0 0 1 21 9.5v8a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z"/></svg>"#;
const ICON_FOLDER_OPEN: &str = r#"<svg class="codelist__icon codelist__icon--dir" viewBox="0 0 24 24" aria-hidden="true"><path d="M3 17.5v-10A1.5 1.5 0 0 1 4.5 6h4.38a1.5 1.5 0 0 1 1.06.44L11.5 8h6A1.5 1.5 0 0 1 19 9.5V11"/><path d="M3 17.5 5.6 12a1.5 1.5 0 0 1 1.36-.9H20a1 1 0 0 1 .92 1.39l-2.3 5.6a1.5 1.5 0 0 1-1.39.91H4.5A1.5 1.5 0 0 1 3 17.5z"/></svg>"#;
const ICON_UP: &str = r#"<svg class="codelist__icon codelist__icon--up" viewBox="0 0 24 24" aria-hidden="true"><path d="m14 9-5-5-5 5"/><path d="M20 20h-7a4 4 0 0 1-4-4V4"/></svg>"#;
const ICON_FILE: &str = r#"<svg class="codelist__icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/></svg>"#;
const ICON_FILE_CODE: &str = r#"<svg class="codelist__icon codelist__icon--code" viewBox="0 0 24 24" aria-hidden="true"><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/><path d="m10 12.5-2 2 2 2"/><path d="m14 12.5 2 2-2 2"/></svg>"#;
const ICON_FILE_TEXT: &str = r#"<svg class="codelist__icon codelist__icon--text" viewBox="0 0 24 24" aria-hidden="true"><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/><path d="M9 13h6"/><path d="M9 17h6"/></svg>"#;
const ICON_FILE_CONFIG: &str = r#"<svg class="codelist__icon codelist__icon--config" viewBox="0 0 24 24" aria-hidden="true"><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/><path d="M9 12h2"/><path d="M13 12h2"/><path d="M9 16h4"/></svg>"#;
const ICON_FILE_IMAGE: &str = r#"<svg class="codelist__icon codelist__icon--image" viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="4" width="16" height="16" rx="2"/><circle cx="9.5" cy="9.5" r="1.5"/><path d="m20 15-4-4-8 9"/></svg>"#;

/// Pick a listing icon from the file name: docs, config, images and source
/// code each get a distinct glyph; anything unrecognised gets the plain one.
fn file_icon(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    match ext {
        "md" | "markdown" | "mdx" | "txt" | "rst" | "adoc" => ICON_FILE_TEXT,
        "json" | "jsonc" | "toml" | "yaml" | "yml" | "ini" | "cfg" | "conf" | "lock" | "env"
        | "xml" | "plist" | "properties" => ICON_FILE_CONFIG,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" | "bmp" | "avif" => ICON_FILE_IMAGE,
        "rs" | "js" | "mjs" | "cjs" | "ts" | "tsx" | "jsx" | "py" | "go" | "rb" | "java" | "kt"
        | "swift" | "c" | "h" | "cc" | "cpp" | "hpp" | "cs" | "php" | "sh" | "bash" | "zsh"
        | "ps1" | "html" | "css" | "scss" | "vue" | "svelte" | "sql" | "lua" | "dart" | "zig"
        | "ex" | "exs" => ICON_FILE_CODE,
        _ if matches!(lower.as_str(), "dockerfile" | "makefile" | "justfile") => ICON_FILE_CODE,
        _ if lower.starts_with(".git") || lower.starts_with(".env") => ICON_FILE_CONFIG,
        _ if lower.starts_with("readme") || lower.starts_with("license") => ICON_FILE_TEXT,
        _ => ICON_FILE,
    }
}

/// Compact relative age of `then` as seen at `now` ("just now", "5 min ago",
/// "3 h ago", "12 days ago"), falling back to a calendar date past ~a month.
/// A timestamp in the future (clock skew) reads as "just now".
fn format_age(now: std::time::SystemTime, then: std::time::SystemTime) -> String {
    let secs = now.duration_since(then).map(|d| d.as_secs()).unwrap_or(0);
    match secs {
        0..=59 => "just now".to_string(),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86_399 => format!("{} h ago", secs / 3600),
        86_400..=172_799 => "yesterday".to_string(),
        172_800..=2_591_999 => format!("{} days ago", secs / 86_400),
        _ => {
            let dt = time::OffsetDateTime::from(then);
            const MONTHS: [&str; 12] = [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ];
            format!(
                "{} {}, {}",
                MONTHS[u8::from(dt.month()) as usize - 1],
                dt.day(),
                dt.year()
            )
        }
    }
}

/// A UTC timestamp twice over: machine-readable for `<time datetime>`
/// (`YYYY-MM-DDTHH:MMZ`) and human-readable for its tooltip.
fn utc_timestamp(t: std::time::SystemTime) -> (String, String) {
    let dt = time::OffsetDateTime::from(t);
    let (date, clock) = (
        format!(
            "{:04}-{:02}-{:02}",
            dt.year(),
            u8::from(dt.month()),
            dt.day()
        ),
        format!("{:02}:{:02}", dt.hour(), dt.minute()),
    );
    (format!("{date}T{clock}Z"), format!("{date} {clock} UTC"))
}

/// Sidebar for the Code section: always exactly one directory's contents
/// (same "one folder, zoomable" model the Docs sidebar uses), server-
/// rendered — no client JS to build the markup, unlike Docs' JSON-payload
/// `file_tree`, because there is no whole-project file list to ship (D1: no
/// index). Structure matches Docs' own sidebar exactly: a `chap-crumbs`
/// path (project root + each ancestor segment) right below the search box,
/// then folders inside the same `chap-folders` disclosure Docs uses
/// (collapsible, chevron, count, folder-icon via `chap-subfolder`), then
/// files. Crumbs/folders render as real `<a>` links here instead of Docs'
/// JS-focus `<button>`s, since Code navigates to a folder/ancestor rather
/// than zooming into it client-side — the crumbs already cover every
/// "go up" case, so there is no separate `..` entry. A small separate
/// script in `app.js` wires the folder bar's click-to-toggle behavior once
/// at load (this HTML needs no JS to render correctly, only to become
/// interactive).
fn code_tree(project: &Project, listing: &DirListing, active_file: Option<&str>) -> String {
    let mut out = String::from(
        "<div class=\"fg-sidebar-search\">\
         <input class=\"fg-input\" placeholder=\"Search…\" autocomplete=\"off\" disabled></div>\
         <nav class=\"chapter\">",
    );

    let mut crumbs = format!(
        "<a class=\"chap-seg\" href=\"/p/{pid}/_code/\">{name}</a>",
        pid = esc(&project.id),
        name = esc(&project.name),
    );
    let mut acc = String::new();
    for seg in listing.rel_path.split('/').filter(|s| !s.is_empty()) {
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(seg);
        crumbs.push_str(&format!(
            "<span class=\"chap-sep\">›</span><a class=\"chap-seg\" href=\"/p/{pid}/_code/{acc}\">{seg}</a>",
            pid = esc(&project.id),
            acc = esc(&acc),
            seg = esc(seg),
        ));
    }
    out.push_str(&format!("<div class=\"chap-crumbs\">{crumbs}</div>"));

    let mut dirs = String::new();
    let mut dir_count = 0usize;
    let mut files = String::new();
    for entry in &listing.entries {
        let rel = child_rel(&listing.rel_path, &entry.name);
        if entry.is_dir {
            dir_count += 1;
            dirs.push_str(&format!(
                "<a class=\"chap-subfolder\" href=\"/p/{pid}/_code/{rel}\">{name}</a>",
                pid = esc(&project.id),
                rel = esc(&rel),
                name = esc(&entry.name),
            ));
        } else {
            let cls = if active_file == Some(entry.name.as_str()) {
                "chap-file active"
            } else {
                "chap-file"
            };
            files.push_str(&format!(
                "<a class=\"{cls}\" href=\"/p/{pid}/_code/{rel}\">{name}</a>",
                cls = cls,
                pid = esc(&project.id),
                rel = esc(&rel),
                name = esc(&entry.name),
            ));
        }
    }
    if dir_count > 0 {
        // Auto-open when this folder has no files of its own (else the
        // sidebar would show nothing until the user expands it) — same
        // rule Docs' own `render()` uses (`foldersOpen || here.length === 0`).
        let open = files.is_empty();
        out.push_str(&format!(
            "<div class=\"chap-folders{open_cls}\">\
             <button class=\"chap-folders__bar\" type=\"button\" aria-expanded=\"{aria}\">\
             <span class=\"chap-folders__chev\">›</span>\
             <span class=\"chap-folders__label\">Subfolders</span>\
             <span class=\"chap-folders__count\">{count}</span>\
             </button>\
             <div class=\"chap-folders__list\"><div class=\"chap-folders__inner\">{dirs}</div></div>\
             </div>",
            open_cls = if open { " is-open" } else { "" },
            aria = open,
            count = dir_count,
            dirs = dirs,
        ));
    }
    if !files.is_empty() {
        out.push_str("<div class=\"chap-sec\">Files</div>");
        out.push_str(&files);
    }
    out.push_str("</nav>");
    out
}

/// Join a directory's `rel_path` with one child name into that child's own
/// `rel_path` (root-level children have no leading slash).
fn child_rel(dir_rel_path: &str, name: &str) -> String {
    if dir_rel_path.is_empty() {
        name.to_string()
    } else {
        format!("{dir_rel_path}/{name}")
    }
}

/// Human-readable byte size (binary units — 1 KB = 1024 B), one decimal
/// place past bytes.
fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
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
    let json =
        escape_script_breakout(&serde_json::to_string(&payload).unwrap_or_else(|_| "[]".into()));

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
        "<form class=\"fg-sidebar-search\" action=\"/p/{pid}/_search\" method=\"get\">\
         <input class=\"fg-input\" name=\"q\" placeholder=\"Search…\" autocomplete=\"off\"></form>\
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
    r#"<button id="theme-toggle" class="theme-toggle fg-btn fg-btn--ghost" title="Toggle theme">◐</button>"#
}

/// Hamburger that opens the file-tree sidebar on mobile (hidden on wide
/// screens via CSS). Only file pages carry a sidebar, so only they render it.
fn sidebar_toggle() -> &'static str {
    r#"<button id="sidebar-toggle" class="sidebar-toggle" type="button" aria-label="Toggle file navigation" aria-controls="sidebar" aria-expanded="false">☰</button>"#
}

/// Edit-this-file action beside the Copy button (file pages only). Toggles
/// the in-place markdown editor (`md_editor`); app.js owns the behaviour.
fn edit_md_button() -> &'static str {
    r#"<button id="edit-md" class="copy-md" type="button" title="Edit this file" aria-label="Edit this file" aria-controls="md-editor" aria-expanded="false"><svg class="copy-md__icon" viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"></path><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"></path></svg><span class="copy-md__txt">Edit</span></button>"#
}

/// The in-place markdown editor: a CodeMirror host (filled by app.js once
/// /static/codemirror.min.js loads) with a plain textarea as the fallback
/// if that script fails, plus a notice for messages too long for the
/// toolbar (conflicts, save errors) — after the editor so it can stick to
/// the viewport bottom however deep in the source the reader is. Rendered hidden on every file page; the
/// Edit button swaps it in for the rendered article. Server-side markup
/// (rather than built by app.js) so the CSS has real elements to target and
/// the page stays one document — but it does nothing without scripting, so it
/// stays hidden when scripting is off.
fn md_editor() -> &'static str {
    r#"<div id="md-editor" class="md-editor" hidden>
      <div class="md-editor__cm" hidden></div>
      <textarea class="md-editor__ta" spellcheck="false" aria-label="Markdown source"></textarea>
      <p class="md-editor__notice" role="status" aria-live="polite" hidden></p>
    </div>"#
}

/// The editor's toolbar. Lives in the sticky breadcrumb (in place of the
/// Edit/Copy buttons while editing) so Save/Cancel and the unsaved-changes
/// state stay in view however far down the source the editor is scrolled.
fn md_editor_bar() -> &'static str {
    r#"<div class="md-editor__bar" hidden>
      <span class="md-editor__state" aria-live="polite"><span class="md-editor__dot" aria-hidden="true"></span><span class="md-editor__state-txt">Editing</span></span>
      <button type="button" class="fg-btn fg-btn--ghost md-editor__cancel">Cancel</button>
      <button type="button" class="fg-btn fg-btn--primary md-editor__save" disabled><span class="md-editor__save-txt">Save</span><kbd class="md-editor__kbd">Ctrl S</kbd></button>
    </div>"#
}

/// Copy-the-whole-page-as-Markdown action for the top bar (file pages only; it
/// reads the `#mdsource` blob). Icon collapses to just the glyph on mobile.
fn copy_md_button() -> &'static str {
    r#"<button id="copy-md" class="copy-md" type="button" title="Copy page as Markdown" aria-label="Copy page as Markdown"><svg class="copy-md__icon" viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg><span class="copy-md__txt">Copy</span></button>"#
}

/// Shared top bar for every page: brand, a page-specific center slot (crumb or
/// empty), the Settings link, and the theme toggle. Keeps the Settings link on
/// all pages and stops each view re-inventing its own header.
fn topbar(center: &str) -> String {
    topbar_full("", center, "")
}

/// Full top bar: an optional `lead` slot (before the brand) and an optional
/// `actions` slot (page-specific buttons before the theme toggle, e.g. the
/// copy-page-as-Markdown button on file pages).
///
/// Three areas — left/center/right — so the bar's width can line up with
/// the sidebar/content/rightbar columns below it on desktop (`.topbar__*`
/// in app.css); on narrower viewports they stay a plain flex row.
fn topbar_full(lead: &str, center: &str, actions: &str) -> String {
    format!(
        r#"<header class="topbar">
  <div class="topbar__left">
    {lead}
    <a href="/" class="home">mdview</a>
  </div>
  <div class="topbar__center">
    {center}
  </div>
  <div class="topbar__right">
    {actions}
    <a class="nav-link" href="/settings" aria-label="Settings"><svg class="nav-link__icon" viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1Z"></path></svg><span class="nav-link__txt">Settings</span></a>
    {toggle}
  </div>
</header>"#,
        lead = lead,
        center = center,
        actions = actions,
        toggle = theme_toggle(),
    )
}

/// Percent-encode everything outside the RFC 3986 unreserved set, so a value
/// such as `a&sort=recent#x` stays one query value.
fn pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A search URL with every value percent-encoded, then HTML-escaped for use
/// inside an attribute.
fn search_href(pid: &str, q: &str, scope: &str, dir: &str, sort: &str) -> String {
    esc(&format!(
        "/p/{}/_search?q={}&scope={}&dir={}&sort={}",
        pct_encode(pid),
        pct_encode(q),
        pct_encode(scope),
        pct_encode(dir),
        pct_encode(sort),
    ))
}

fn toggle_link(label: &str, href: &str, active: bool) -> String {
    let current = if active { " aria-current=\"true\"" } else { "" };
    format!("<a class=\"fg-toggle__opt\" href=\"{href}\"{current}>{label}</a>")
}

pub fn search_page(
    project: &Project,
    query: &str,
    dir: &str,
    scope_is_dir: bool,
    sort: SearchSort,
    outcome: Result<&SearchOutcome, &str>,
) -> String {
    let scope = if scope_is_dir { "dir" } else { "project" };
    let sort_name = match sort {
        SearchSort::Relevance => "relevance",
        SearchSort::Recent => "recent",
    };
    let pid = project.id.as_str();

    let mut toggles = String::from("<div class=\"fg-toggles\">");
    if !dir.is_empty() {
        toggles.push_str("<span class=\"fg-toggle\">");
        toggles.push_str(&toggle_link(
            "Whole project",
            &search_href(pid, query, "project", dir, sort_name),
            !scope_is_dir,
        ));
        toggles.push_str(&toggle_link(
            "This folder",
            &search_href(pid, query, "dir", dir, sort_name),
            scope_is_dir,
        ));
        toggles.push_str("</span>");
    }
    toggles.push_str("<span class=\"fg-toggle\">");
    toggles.push_str(&toggle_link(
        "Relevance",
        &search_href(pid, query, scope, dir, "relevance"),
        sort == SearchSort::Relevance,
    ));
    toggles.push_str(&toggle_link(
        "Newest",
        &search_href(pid, query, scope, dir, "recent"),
        sort == SearchSort::Recent,
    ));
    toggles.push_str("</span></div>");

    let mut status = String::new();
    let mut items = String::new();
    match outcome {
        Err(msg) => {
            items.push_str(&format!(
                "<p class=\"fg-empty fg-search-error\" role=\"alert\">Search failed: {}</p>",
                esc(msg)
            ));
        }
        Ok(out) => {
            let line = if let Some(err) = &out.sync_error {
                Some(format!(
                    "Sync failed: {} — showing indexed results",
                    esc(err)
                ))
            } else if out.sync.skipped_recent {
                Some("Index reused (synced moments ago)".to_string())
            } else if !query.trim().is_empty() {
                Some(format!(
                    "Synced {} files ({} read) in {:.2} s",
                    out.sync.files_seen,
                    out.sync.files_read,
                    out.sync.elapsed_ms as f64 / 1000.0
                ))
            } else {
                None
            };
            if let Some(line) = line {
                status = format!("<p class=\"fg-search-status\">{line}</p>");
            }
            if query.trim().is_empty() {
                items.push_str("<p class=\"fg-empty\">Type a query to search this project.</p>");
            } else if out.results.is_empty() {
                items.push_str(&format!(
                    "<p class=\"fg-empty\">No matches for “{}”.</p>",
                    esc(query)
                ));
            } else {
                for r in &out.results {
                    items.push_str(&format!(
                        "<a class=\"fg-card\" href=\"{url}\"><div class=\"fg-card__title\">{title}</div>\
                         <div class=\"fg-card__sub\">{rel}</div><div class=\"fg-card__sub\">{excerpt}</div></a>",
                        url = esc(&r.url),
                        title = esc(&r.title),
                        rel = esc(&r.rel_path),
                        excerpt = highlight_excerpt(&r.excerpt),
                    ));
                }
            }
        }
    }
    let body = format!(
        r#"{topbar}
<main class="fg-page">
  <form action="/p/{pid}/_search" method="get">
    <input type="hidden" name="scope" value="{scope}">
    <input type="hidden" name="dir" value="{dir}">
    <input type="hidden" name="sort" value="{sort_name}">
    <input class="fg-input" name="q" value="{q}" placeholder="Search…" autofocus autocomplete="off">
  </form>
  {toggles}
  {status}
  {items}
</main>"#,
        topbar = topbar(&format!(
            "<span class=\"crumb\">{name} · search</span>",
            name = esc(&project.name)
        )),
        pid = esc(&project.id),
        dir = esc(dir),
        q = esc(query),
    );
    layout(&format!("search: {query}"), "", &body)
}

/// The excerpt is raw document text with private-use sentinels around
/// matches. Escape everything first, so literal markup in a document stays
/// text, then turn the sentinels into the highlight tags.
fn highlight_excerpt(excerpt: &str) -> String {
    esc(excerpt)
        .replace(snippet::MARK_OPEN, "<mark class=\"fg-mark\">")
        .replace(snippet::MARK_CLOSE, "</mark>")
}

pub fn settings_page(cfg: &Config, saved: bool) -> String {
    let banner = if saved {
        "<div class=\"fg-banner fg-banner--success\"><span class=\"fg-banner__dot\"></span><span class=\"fg-banner__body\">Saved. Server &amp; indexing changes apply after restart (<code>mdview stop &amp;&amp; mdview serve</code>).</span></div>"
    } else {
        ""
    };
    let checked = |b: bool| if b { "checked" } else { "" };
    let sel = |v: &str, opt: &str| if v == opt { "selected" } else { "" };
    let excludes = cfg.indexing.exclude_patterns.join("\n");

    let body = format!(
        r#"{topbar}
<main class="fg-page">
  <h2 class="fg-pagehead__title">Settings <span class="t-caption fg-settings__version">mdview v{version}</span></h2>
  {banner}
  <form class="fg-settings" method="post" action="/api/config">
    <fieldset><legend>Server <span class="fg-chip fg-chip--neutral">restart</span></legend>
      <div class="fg-field-row">
        <div class="fg-field">
          <label class="fg-field__label">Host</label>
          <input class="fg-input" name="host" value="{host}">
          <span class="fg-field__hint">127.0.0.1 (local) or 0.0.0.0 (LAN)</span>
        </div>
        <div class="fg-field">
          <label class="fg-field__label">Port</label>
          <input class="fg-input" type="number" name="port" value="{port}" min="1" max="65535">
        </div>
      </div>
      <div class="fg-field">
        <label class="fg-field__label">Display hostname</label>
        <input class="fg-input" name="hostname" value="{hostname}">
        <span class="fg-field__hint">optional — used in rendered links instead of the IP/host above</span>
      </div>
      <label class="fg-check"><input type="checkbox" name="open_browser" {open}><span class="fg-check__text">Open browser on start</span></label>
    </fieldset>
    <fieldset><legend>MCP <span class="fg-chip fg-chip--neutral">restart</span></legend>
      <label class="fg-check"><input type="checkbox" name="mcp_enabled" {mcp_on}><span class="fg-check__text">Enabled</span></label>
      <div class="fg-field">
        <label class="fg-field__label">Transport</label>
        <div class="fg-select">
          <select name="mcp_transport">
            <option value="stdio" {tr_stdio}>stdio</option>
            <option value="http" {tr_http}>http</option>
          </select>
          <span class="fg-select__chev">▾</span>
        </div>
      </div>
    </fieldset>
    <fieldset><legend>Renderer</legend>
      <div class="fg-field">
        <label class="fg-field__label">Theme</label>
        <div class="fg-select">
          <select name="theme">
            <option value="system" {t_sys}>System</option>
            <option value="light" {t_light}>Light</option>
            <option value="dark" {t_dark}>Dark</option>
          </select>
          <span class="fg-select__chev">▾</span>
        </div>
      </div>
      <div class="fg-field">
        <label class="fg-field__label">Syntax highlight theme</label>
        <input class="fg-input" name="syntax_theme" value="{syntax}">
      </div>
    </fieldset>
    <fieldset><legend>Indexing <span class="fg-chip fg-chip--neutral">restart</span></legend>
      <div class="fg-field-row">
        <div class="fg-field">
          <label class="fg-field__label">Debounce (ms)</label>
          <input class="fg-input" type="number" name="debounce_ms" value="{debounce}" min="0">
        </div>
        <div class="fg-field">
          <label class="fg-field__label">Max file size (MB)</label>
          <input class="fg-input" type="number" name="max_file_size_mb" value="{maxmb}" min="1">
        </div>
      </div>
      <div class="fg-field">
        <label class="fg-field__label">Exclude patterns (one per line)</label>
        <textarea class="fg-input fg-input--area" name="exclude_patterns" rows="5">{excludes}</textarea>
      </div>
    </fieldset>
    <fieldset><legend>Cloudflare Access <span class="fg-chip fg-chip--neutral">restart</span></legend>
      <p class="fg-field__hint">Optional alternate login for requests already authenticated at the edge. Off unless both fields below are set. The login token itself is managed separately — see <code>~/.mdview/config.toml</code> (generated automatically on first start) or delete it there to force a fresh one.</p>
      <div class="fg-field">
        <label class="fg-field__label">Team domain</label>
        <input class="fg-input" name="cf_access_team_domain" value="{cf_team}" placeholder="https://your-team.cloudflareaccess.com">
      </div>
      <div class="fg-field">
        <label class="fg-field__label">Application Audience (aud) tag</label>
        <input class="fg-input" name="cf_access_aud" value="{cf_aud}">
      </div>
    </fieldset>
    <button type="submit" class="fg-btn fg-btn--primary">Save</button>
  </form>
</main>"#,
        topbar = topbar("<span class=\"crumb\">Settings</span>"),
        banner = banner,
        version = env!("CARGO_PKG_VERSION"),
        port = cfg.server.port,
        host = esc(&cfg.server.host),
        hostname = esc(cfg.server.hostname.as_deref().unwrap_or("")),
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
        cf_team = esc(cfg.server.cf_access_team_domain.as_deref().unwrap_or("")),
        cf_aud = esc(cfg.server.cf_access_aud.as_deref().unwrap_or("")),
    );
    layout("Settings", "", &body)
}

/// `GET /login` — a plain form POSTing to `/api/login`. No client JS: a
/// wrong token gets the same opaque 404 `POST /api/login` always returns
/// (D2), so this page carries no error state to render either — there is
/// nothing to distinguish "just arrived" from "just failed".
pub fn login_page() -> String {
    let body = r#"<main class="fg-page">
  <h2 class="fg-pagehead__title">Sign in</h2>
  <form class="fg-settings" method="post" action="/api/login">
    <div class="fg-field">
      <label class="fg-field__label">Token</label>
      <input class="fg-input" type="password" name="token" autofocus autocomplete="current-password">
    </div>
    <button type="submit" class="fg-btn fg-btn--primary">Sign in</button>
  </form>
</main>"#;
    layout("Sign in", "", body)
}

pub fn error_page(status: u16, msg: &str) -> String {
    let body = format!(
        r#"{topbar}
<main class="fg-page"><h2 class="fg-pagehead__title">{status}</h2><p class="fg-empty">{msg}</p></main>"#,
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

pub const APP_CSS: &str = concat!(
    include_str!("../assets/atelier/fonts.css"),
    "\n",
    include_str!("../assets/atelier/contract.css"),
    "\n",
    include_str!("../assets/atelier/components.css"),
    "\n",
    include_str!("../assets/atelier/editorial.css"),
    "\n",
    include_str!("../assets/atelier/atelier.css"),
    "\n",
    include_str!("../assets/app.css"),
);
pub const APP_JS: &str = include_str!("../assets/app.js");
/// Vendored Mermaid (self-contained UMD build) served at /static/mermaid.min.js
/// so diagrams render without a CDN. Only loaded on pages that contain a diagram.
pub const MERMAID_JS: &str = include_str!("../assets/mermaid.min.js");
/// Vendored CodeMirror 6 bundle for the in-place Docs editor; built from
/// `tools/codemirror-bundle/` and lazy-loaded by app.js on the first Edit.
pub const CODEMIRROR_JS: &str = include_str!("../assets/codemirror.min.js");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_age_buckets_relative_times_and_falls_back_to_a_date() {
        use std::time::{Duration, UNIX_EPOCH};
        let now = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let ago = |s: u64| format_age(now, now - Duration::from_secs(s));
        assert_eq!(ago(5), "just now");
        assert_eq!(ago(5 * 60), "5 min ago");
        assert_eq!(ago(3 * 3600), "3 h ago");
        assert_eq!(ago(30 * 3600), "yesterday");
        assert_eq!(ago(12 * 86_400), "12 days ago");
        assert_eq!(format_age(now, UNIX_EPOCH), "Jan 1, 1970");
        // Clock skew: a future mtime never renders a negative age.
        assert_eq!(format_age(now, now + Duration::from_secs(90)), "just now");
    }

    #[test]
    fn file_icon_groups_by_kind() {
        assert_eq!(file_icon("main.RS"), ICON_FILE_CODE);
        assert_eq!(file_icon("Makefile"), ICON_FILE_CODE);
        assert_eq!(file_icon("README.md"), ICON_FILE_TEXT);
        assert_eq!(file_icon("Cargo.toml"), ICON_FILE_CONFIG);
        assert_eq!(file_icon(".gitignore"), ICON_FILE_CONFIG);
        assert_eq!(file_icon("logo.svg"), ICON_FILE_IMAGE);
        assert_eq!(file_icon("blob.bin"), ICON_FILE);
    }

    #[test]
    fn escape_script_breakout_neutralizes_closing_tag_in_array_json() {
        // The sidebar #filelist payload is a JSON array; a file title of
        // "</script>..." must not survive as a raw "<".
        let json = r#"[{"p":"a.md","t":"x</script><script>alert(1)</script>"}]"#;
        let escaped = escape_script_breakout(json);
        assert!(!escaped.contains('<'), "raw '<' leaked: {escaped}");
        assert!(escaped.contains("\\u003c"));
    }

    #[test]
    fn escape_json_for_script_neutralizes_script_breakout() {
        let source = "before </script><script>alert(1)</script> after";
        let escaped = escape_json_for_script(source);
        assert!(
            !escaped.contains('<'),
            "escaped blob must contain no raw '<': {escaped}"
        );
    }

    #[test]
    fn escape_json_for_script_round_trips_to_original_source() {
        let source = "line one\n</script>\nline three with <tag> and \"quotes\"";
        let escaped = escape_json_for_script(source);
        let round_tripped: String =
            serde_json::from_str(&escaped).expect("escaped blob must still be valid JSON");
        assert_eq!(round_tripped, source);
    }

    fn project() -> Project {
        Project {
            id: "p1".into(),
            name: "Proj".into(),
            root_path: std::path::PathBuf::from("/tmp/x"),
            created_at: String::new(),
            last_seen_at: String::new(),
        }
    }

    fn outcome(excerpt: &str) -> SearchOutcome {
        SearchOutcome {
            results: vec![mdview_core::domain::SearchResult {
                project_id: "p1".into(),
                rel_path: "a.md".into(),
                title: "A".into(),
                excerpt: excerpt.into(),
                url: "/p/p1/a.md".into(),
                score: 1.0,
                modified_at: String::new(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn search_query_in_title_cannot_inject_markup_into_head() {
        let out = outcome("");
        let html = search_page(
            &project(),
            "</title><script>alert(1)</script>",
            "",
            false,
            SearchSort::Relevance,
            Ok(&out),
        );
        let head = &html[..html.find("</head>").unwrap()];
        assert!(!head.contains("<script>alert"), "{head}");
        assert!(head.contains("&lt;/title&gt;&lt;script&gt;"));
    }

    #[test]
    fn excerpt_sentinels_become_marks_and_literal_markup_stays_text() {
        let out = outcome("see \u{E000}<mark>x</mark>\u{E001} <b>");
        let html = search_page(&project(), "x", "", false, SearchSort::Relevance, Ok(&out));
        assert!(
            html.contains("<mark class=\"fg-mark\">&lt;mark&gt;x&lt;/mark&gt;</mark>"),
            "{html}"
        );
        assert!(html.contains("&lt;b&gt;"));
        assert!(!html.contains("<b>"));
    }

    #[test]
    fn toggles_show_folder_option_only_with_a_dir_and_mark_the_active_one() {
        let out = outcome("");
        let html = search_page(&project(), "q", "", false, SearchSort::Relevance, Ok(&out));
        assert!(!html.contains("This folder"));
        assert!(html.contains("Newest"));
        let html = search_page(&project(), "q", "docs", true, SearchSort::Recent, Ok(&out));
        assert!(html.contains("This folder"));
        assert!(
            html.contains("sort=recent\" aria-current=\"true\">Newest"),
            "{html}"
        );
        assert!(html.contains("sort=recent\" aria-current=\"true\">Newest"));
        assert_eq!(html.matches("aria-current").count(), 2);
    }

    #[test]
    fn toggle_hrefs_percent_encode_every_value() {
        let out = outcome("");
        let html = search_page(
            &project(),
            "a b&c",
            "a&sort=recent#x",
            true,
            SearchSort::Relevance,
            Ok(&out),
        );
        assert!(html.contains("dir=a%26sort%3Drecent%23x"), "{html}");
        assert!(html.contains("q=a%20b%26c"));
        assert!(!html.contains("dir=a&amp;sort=recent"));
    }

    #[test]
    fn status_lines_cover_synced_reused_and_failed() {
        let mut out = outcome("");
        out.sync.files_seen = 5;
        out.sync.files_read = 2;
        out.sync.elapsed_ms = 1500;
        let html = search_page(&project(), "q", "", false, SearchSort::Relevance, Ok(&out));
        assert!(html.contains("Synced 5 files (2 read) in 1.50 s"), "{html}");
        out.sync.skipped_recent = true;
        let html = search_page(&project(), "q", "", false, SearchSort::Relevance, Ok(&out));
        assert!(html.contains("Index reused (synced moments ago)"));
        out.sync_error = Some("disk <full>".into());
        let html = search_page(&project(), "q", "", false, SearchSort::Relevance, Ok(&out));
        assert!(
            html.contains("Sync failed: disk &lt;full&gt; — showing indexed results"),
            "{html}"
        );
    }

    #[test]
    fn engine_error_renders_a_message_not_an_empty_list() {
        let html = search_page(
            &project(),
            "q",
            "",
            false,
            SearchSort::Relevance,
            Err("boom"),
        );
        assert!(html.contains("Search failed: boom"));
        assert!(!html.contains("No matches"));
    }
}
