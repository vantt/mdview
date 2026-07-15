//! Markdown → HTML rendering (PRD §5.4 / §9).
//!
//! Pipeline: comrak (GFM, frontmatter-strip) → AST walk (link rewrite via
//! [`link_resolver`], broken-link marking, syntect class-based code highlight,
//! mermaid marking) → ammonia sanitize. Highlight is server-side and
//! class-based, so themes switch via CSS with no re-render (PRD §9, FR-21).

use crate::domain::{Heading, RenderedPage};
use crate::link_resolver::{self, IndexLookup};
use comrak::nodes::{AstNode, NodeHtmlBlock, NodeValue};
use comrak::{parse_document, Arena, Options};
use std::path::Path;
use syntect::html::{css_for_theme_with_class_style, ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

const CLASS_STYLE: ClassStyle = ClassStyle::Spaced;

pub struct RenderService {
    syntaxes: SyntaxSet,
}

impl Default for RenderService {
    fn default() -> Self {
        Self { syntaxes: SyntaxSet::load_defaults_newlines() }
    }
}

/// What to do with a node after inspecting it (computed outside the AST borrow).
enum Action {
    None,
    Replace(NodeValue),
    /// Replace with inline raw HTML and detach the node's children.
    InlineHtml(String),
}

impl RenderService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render `source` (the markdown of `source_abs`) to a sanitized page.
    pub fn render(
        &self,
        source: &str,
        source_abs: &Path,
        project_id: &str,
        project_root: &Path,
        index: &dyn IndexLookup,
    ) -> RenderedPage {
        let arena = Arena::new();
        let opts = comrak_options();
        let root = parse_document(&arena, source, &opts);

        let mut headings = Vec::new();
        let mut has_mermaid = false;
        let mut title = String::new();

        self.walk(root, source_abs, project_id, project_root, index, &mut headings, &mut has_mermaid, &mut title);

        let mut html_bytes = Vec::new();
        comrak::format_html(root, &opts, &mut html_bytes).ok();
        let html = sanitize(&String::from_utf8_lossy(&html_bytes));

        if title.is_empty() {
            title = source_abs.file_name().and_then(|s| s.to_str()).unwrap_or("untitled").to_string();
        }
        RenderedPage { html, title, headings, has_mermaid }
    }

    #[allow(clippy::too_many_arguments)]
    fn walk<'a>(
        &self,
        node: &'a AstNode<'a>,
        source_abs: &Path,
        project_id: &str,
        project_root: &Path,
        index: &dyn IndexLookup,
        headings: &mut Vec<Heading>,
        has_mermaid: &mut bool,
        title: &mut String,
    ) {
        // Phase 1: read the value kind without holding the borrow across the
        // work below (collect_text re-borrows descendants).
        enum Kind {
            Link(String),
            Image(String),
            Code(String, String),
            Mermaid(String),
            Heading(u8),
            Other,
        }
        let kind = {
            let data = node.data.borrow();
            match &data.value {
                NodeValue::Link(l) => Kind::Link(l.url.clone()),
                NodeValue::Image(l) => Kind::Image(l.url.clone()),
                NodeValue::CodeBlock(cb) => {
                    let lang = cb.info.split_whitespace().next().unwrap_or("").to_string();
                    if lang == "mermaid" {
                        Kind::Mermaid(cb.literal.clone())
                    } else {
                        Kind::Code(lang, cb.literal.clone())
                    }
                }
                NodeValue::Heading(h) => Kind::Heading(h.level),
                _ => Kind::Other,
            }
        };

        // Phase 2: compute the action (may call collect_text safely now).
        let action = match kind {
            Kind::Link(url) => {
                if link_resolver::is_external(&url) {
                    Action::None
                } else {
                    let r = link_resolver::resolve_link(source_abs, &url, project_id, project_root, index);
                    match r.url {
                        Some(new_url) => Action::Replace(link_node(&new_url)),
                        None => {
                            let text = collect_text(node);
                            Action::InlineHtml(format!(
                                "<a href=\"{}\" class=\"broken-link\" title=\"unresolved link\">{}</a>",
                                html_escape(&url),
                                html_escape(&text)
                            ))
                        }
                    }
                }
            }
            Kind::Image(url) => {
                if link_resolver::is_external(&url) {
                    Action::None
                } else if let Some(new) = resolve_asset(&url, source_abs, project_id, project_root) {
                    Action::Replace(image_node(&new))
                } else {
                    Action::None
                }
            }
            Kind::Code(lang, lit) => Action::Replace(html_block(self.highlight(&lang, &lit))),
            Kind::Mermaid(lit) => {
                *has_mermaid = true;
                Action::Replace(html_block(format!("<pre class=\"mermaid\">{}</pre>", html_escape(&lit))))
            }
            Kind::Heading(level) => {
                let text = collect_text(node);
                let slug = slugify(&text);
                if level == 1 && title.is_empty() {
                    *title = text.clone();
                }
                headings.push(Heading { level, text, slug });
                Action::None
            }
            Kind::Other => Action::None,
        };

        match action {
            Action::None => {}
            Action::Replace(v) => node.data.borrow_mut().value = v,
            Action::InlineHtml(html) => {
                let kids: Vec<_> = node.children().collect();
                for k in kids {
                    k.detach();
                }
                node.data.borrow_mut().value = NodeValue::HtmlInline(html);
            }
        }

        for child in node.children() {
            self.walk(child, source_abs, project_id, project_root, index, headings, has_mermaid, title);
        }
    }

    /// syntect class-based highlight → `<pre class="code"><code>…</code></pre>`.
    fn highlight(&self, lang: &str, code: &str) -> String {
        let syntax = self
            .syntaxes
            .find_syntax_by_token(lang)
            .or_else(|| self.syntaxes.find_syntax_by_extension(lang))
            .unwrap_or_else(|| self.syntaxes.find_syntax_plain_text());
        let mut gen = ClassedHTMLGenerator::new_with_class_style(syntax, &self.syntaxes, CLASS_STYLE);
        for line in LinesWithEndings::from(code) {
            let _ = gen.parse_html_for_line_which_includes_newline(line);
        }
        let inner = gen.finalize();
        let lang_class = if lang.is_empty() { String::new() } else { format!(" language-{}", html_escape(lang)) };
        format!("<pre class=\"code\"><code class=\"code{lang_class}\">{inner}</code></pre>")
    }
}

/// Extract the target project-relative paths a file links to (resolved only).
/// Used to build backlinks (FR-18).
pub fn extract_internal_links(
    source: &str,
    source_abs: &Path,
    project_root: &Path,
    index: &dyn IndexLookup,
) -> Vec<String> {
    let arena = Arena::new();
    let opts = comrak_options();
    let root = parse_document(&arena, source, &opts);
    let mut out = Vec::new();
    for node in root.descendants() {
        if let NodeValue::Link(l) = &node.data.borrow().value {
            if let Some(rel) = link_resolver::resolve_to_rel(source_abs, &l.url, project_root, index) {
                out.push(rel);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Generate the CSS for a syntect theme, matched to the class-based output.
pub fn theme_css(theme_name: &str) -> Option<String> {
    let ts = syntect::highlighting::ThemeSet::load_defaults();
    let theme = ts.themes.get(theme_name)?;
    css_for_theme_with_class_style(theme, CLASS_STYLE).ok()
}

fn comrak_options() -> Options<'static> {
    let mut o = Options::default();
    o.extension.table = true;
    o.extension.strikethrough = true;
    o.extension.tasklist = true;
    o.extension.autolink = true;
    o.extension.footnotes = true;
    o.extension.front_matter_delimiter = Some("---".to_string());
    // Add id="slug" to headings so the TOC (FR-18) can anchor to them.
    o.extension.header_ids = Some(String::new());
    o.render.unsafe_ = true;
    o
}

fn link_node(new_url: &str) -> NodeValue {
    NodeValue::Link(comrak::nodes::NodeLink { url: new_url.to_string(), title: String::new() })
}
fn image_node(new_url: &str) -> NodeValue {
    NodeValue::Image(comrak::nodes::NodeLink { url: new_url.to_string(), title: String::new() })
}

fn html_block(literal: String) -> NodeValue {
    NodeValue::HtmlBlock(NodeHtmlBlock { literal, block_type: 0 })
}

fn collect_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut s = String::new();
    for d in node.descendants() {
        match &d.data.borrow().value {
            NodeValue::Text(t) => s.push_str(t),
            NodeValue::Code(c) => s.push_str(&c.literal),
            _ => {}
        }
    }
    s.trim().to_string()
}

/// GitHub-ish slug: lowercase, drop non-alphanumeric (keep spaces/hyphens).
pub fn slugify(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
        } else if ch == ' ' || ch == '-' || ch == '_' {
            out.push('-');
        }
    }
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    out.trim_matches('-').to_string()
}

fn resolve_asset(href: &str, source_abs: &Path, project_id: &str, project_root: &Path) -> Option<String> {
    let path_part = href.split('#').next().unwrap_or(href);
    let path_part = path_part.split('?').next().unwrap_or(path_part);
    let abs = if let Some(rest) = path_part.strip_prefix('/') {
        link_resolver::normalize(&project_root.join(rest))
    } else {
        let dir = source_abs.parent().unwrap_or(project_root);
        link_resolver::normalize(&dir.join(path_part))
    };
    let rel = abs.strip_prefix(project_root).ok()?;
    let rel_url = rel
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    if rel_url.is_empty() {
        return None;
    }
    Some(format!("/p/{project_id}/{rel_url}"))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn sanitize(html: &str) -> String {
    let mut b = ammonia::Builder::default();
    b.add_tags(&["pre", "code", "span", "section"]);
    b.add_generic_attributes(&["class", "id"]);
    b.add_generic_attribute_prefixes(&["data-"]);
    b.add_tag_attributes("a", &["href", "title", "target", "class"]);
    b.add_tag_attributes("img", &["src", "alt", "title", "class"]);
    b.url_relative(ammonia::UrlRelative::PassThrough);
    b.clean(html).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn svc() -> RenderService {
        RenderService::new()
    }
    fn idx(root: &Path, paths: &[&str]) -> HashSet<PathBuf> {
        paths.iter().map(|p| root.join(p)).collect()
    }

    #[test]
    fn rewrites_internal_link_and_extracts_title() {
        let root = PathBuf::from("/proj");
        let index = idx(&root, &["api/README.md"]);
        let md = "# Hello World\n\nsee [api](../api/README.md)";
        let page = svc().render(md, &root.join("docs/x.md"), "p1", &root, &index);
        assert_eq!(page.title, "Hello World");
        assert!(page.html.contains("/p/p1/api/README.md"), "{}", page.html);
        assert_eq!(page.headings.first().map(|h| h.slug.as_str()), Some("hello-world"));
    }

    #[test]
    fn broken_internal_link_gets_class() {
        let root = PathBuf::from("/proj");
        let index: HashSet<PathBuf> = HashSet::new();
        let page = svc().render("[gone](./missing.md)", &root.join("a.md"), "p1", &root, &index);
        assert!(page.html.contains("broken-link"), "{}", page.html);
        assert!(page.html.contains(">gone<"), "{}", page.html);
    }

    #[test]
    fn external_link_survives_sanitize() {
        let root = PathBuf::from("/proj");
        let index: HashSet<PathBuf> = HashSet::new();
        let page = svc().render("[x](https://a.com)", &root.join("a.md"), "p1", &root, &index);
        assert!(page.html.contains("https://a.com"));
        assert!(!page.html.contains("broken-link"));
    }

    #[test]
    fn mermaid_block_marked_and_wrapped() {
        let root = PathBuf::from("/proj");
        let index: HashSet<PathBuf> = HashSet::new();
        let page = svc().render("```mermaid\ngraph TD; A-->B;\n```", &root.join("a.md"), "p1", &root, &index);
        assert!(page.has_mermaid);
        assert!(page.html.contains("class=\"mermaid\""), "{}", page.html);
    }

    #[test]
    fn code_block_gets_class_based_highlight() {
        let root = PathBuf::from("/proj");
        let index: HashSet<PathBuf> = HashSet::new();
        let page = svc().render("```rust\nfn main() {}\n```", &root.join("a.md"), "p1", &root, &index);
        assert!(page.html.contains("<pre class=\"code\">"), "{}", page.html);
        assert!(!page.html.contains("style=\"color"));
    }

    #[test]
    fn strips_script_xss() {
        let root = PathBuf::from("/proj");
        let index: HashSet<PathBuf> = HashSet::new();
        let page = svc().render("<script>alert(1)</script>\n\nhi", &root.join("a.md"), "p1", &root, &index);
        assert!(!page.html.contains("<script"));
    }

    #[test]
    fn extract_links_resolves_targets() {
        let root = PathBuf::from("/proj");
        let index = idx(&root, &["api/README.md", "docs/other.md"]);
        let md = "[a](../api/README.md) and [b](./other.md) and [ext](https://x.com)";
        let links = extract_internal_links(md, &root.join("docs/x.md"), &root, &index);
        assert!(links.contains(&"api/README.md".to_string()));
        assert!(links.contains(&"docs/other.md".to_string()));
        assert_eq!(links.len(), 2); // external excluded
    }
}
