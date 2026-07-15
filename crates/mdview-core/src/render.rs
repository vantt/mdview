//! Markdown → HTML rendering (PRD §5.4 / §9).
//!
//! Pipeline: comrak (GFM, frontmatter-strip) → AST walk (link rewrite via
//! [`link_resolver`], syntect class-based code highlight, mermaid marking) →
//! ammonia sanitize. Highlight is server-side and **class-based**, so themes
//! switch via CSS with no re-render (PRD §9, FR-21).

use crate::domain::{Heading, RenderedPage};
use crate::link_resolver::{self, IndexLookup};
use comrak::nodes::{AstNode, NodeHtmlBlock, NodeValue};
use comrak::{parse_document, Arena, Options};
use std::path::Path;
use syntect::html::{css_for_theme_with_class_style, ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

const CLASS_STYLE: ClassStyle = ClassStyle::Spaced;

/// Renders markdown for a specific file inside a project, rewriting internal
/// links into the app URL namespace.
pub struct RenderService {
    syntaxes: SyntaxSet,
}

impl Default for RenderService {
    fn default() -> Self {
        Self { syntaxes: SyntaxSet::load_defaults_newlines() }
    }
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

        self.walk(
            root,
            source_abs,
            project_id,
            project_root,
            index,
            &mut headings,
            &mut has_mermaid,
            &mut title,
        );

        let mut html_bytes = Vec::new();
        comrak::format_html(root, &opts, &mut html_bytes).ok();
        let raw_html = String::from_utf8_lossy(&html_bytes).into_owned();
        let html = sanitize(&raw_html);

        if title.is_empty() {
            title = source_abs
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("untitled")
                .to_string();
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
        // Snapshot the value; mutate below where needed.
        let replace: Option<NodeValue> = {
            let data = node.data.borrow();
            match &data.value {
                NodeValue::Link(link) => Some(NodeValue::Link(self.rewrite_link(
                    link,
                    source_abs,
                    project_id,
                    project_root,
                    index,
                    false,
                ))),
                NodeValue::Image(link) => Some(NodeValue::Image(self.rewrite_link(
                    link,
                    source_abs,
                    project_id,
                    project_root,
                    index,
                    true,
                ))),
                NodeValue::CodeBlock(cb) => {
                    let lang = cb.info.split_whitespace().next().unwrap_or("").to_string();
                    if lang == "mermaid" {
                        *has_mermaid = true;
                        let esc = html_escape(&cb.literal);
                        Some(html_block(format!("<pre class=\"mermaid\">{esc}</pre>")))
                    } else {
                        Some(html_block(self.highlight(&lang, &cb.literal)))
                    }
                }
                NodeValue::Heading(h) => {
                    let text = collect_text(node);
                    let slug = slugify(&text);
                    if h.level == 1 && title.is_empty() {
                        *title = text.clone();
                    }
                    headings.push(Heading { level: h.level, text, slug });
                    None
                }
                _ => None,
            }
        };
        if let Some(v) = replace {
            node.data.borrow_mut().value = v;
        }

        // CodeBlock/Image/Link have no children we need to recurse for rewriting,
        // but headings/paragraphs do. Recurse over children generally.
        for child in node.children() {
            self.walk(
                child, source_abs, project_id, project_root, index, headings, has_mermaid, title,
            );
        }
    }

    fn rewrite_link(
        &self,
        link: &comrak::nodes::NodeLink,
        source_abs: &Path,
        project_id: &str,
        project_root: &Path,
        index: &dyn IndexLookup,
        is_image: bool,
    ) -> comrak::nodes::NodeLink {
        let mut out = link.clone();
        if link_resolver::is_external(&link.url) {
            return out;
        }
        if is_image {
            if let Some(url) = resolve_asset(&link.url, source_abs, project_id, project_root) {
                out.url = url;
            }
        } else {
            let r =
                link_resolver::resolve_link(source_abs, &link.url, project_id, project_root, index);
            if let Some(url) = r.url {
                out.url = url;
            }
            // broken internal links keep their original href (styling: PRD Phase 3).
        }
        out
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
        let lang_class = if lang.is_empty() {
            String::new()
        } else {
            format!(" language-{}", html_escape(lang))
        };
        format!("<pre class=\"code\"><code class=\"code{lang_class}\">{inner}</code></pre>")
    }
}

/// Generate the CSS for a syntect theme, matched to the class-based output.
/// Served by the web UI so code-block colors follow the active theme.
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
    o.render.unsafe_ = true; // agent markdown embeds raw HTML; ammonia guards the output
    o
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

/// GitHub-ish slug: lowercase, drop non-alphanumeric (keep spaces/hyphens),
/// spaces → hyphens.
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

/// Resolve an image/asset href to an app URL if it lives inside the project root.
/// Assets are not in the markdown index, so containment (not membership) decides.
fn resolve_asset(
    href: &str,
    source_abs: &Path,
    project_id: &str,
    project_root: &Path,
) -> Option<String> {
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
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Final sanitize pass — safe to view untrusted agent markdown.
fn sanitize(html: &str) -> String {
    let mut b = ammonia::Builder::default();
    b.add_tags(&["pre", "code", "span", "section"]);
    b.add_generic_attributes(&["class", "id"]);
    b.add_generic_attribute_prefixes(&["data-"]);
    b.add_tag_attributes("a", &["href", "title", "target", "class"]);
    b.add_tag_attributes("img", &["src", "alt", "title", "class"]);
    // Keep app-relative URLs like /p/{id}/... intact.
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

    #[test]
    fn rewrites_internal_link_and_extracts_title() {
        let root = PathBuf::from("/proj");
        let mut idx: HashSet<PathBuf> = HashSet::new();
        idx.insert(root.join("api/README.md"));
        let src = root.join("docs/x.md");
        let md = "# Hello World\n\nsee [api](../api/README.md)";
        let page = svc().render(md, &src, "p1", &root, &idx);
        assert_eq!(page.title, "Hello World");
        assert!(page.html.contains("/p/p1/api/README.md"), "html: {}", page.html);
        assert_eq!(page.headings.first().map(|h| h.slug.as_str()), Some("hello-world"));
    }

    #[test]
    fn external_link_survives_sanitize() {
        let root = PathBuf::from("/proj");
        let idx: HashSet<PathBuf> = HashSet::new();
        let page = svc().render("[x](https://a.com)", &root.join("a.md"), "p1", &root, &idx);
        assert!(page.html.contains("https://a.com"));
    }

    #[test]
    fn mermaid_block_marked_and_wrapped() {
        let root = PathBuf::from("/proj");
        let idx: HashSet<PathBuf> = HashSet::new();
        let md = "```mermaid\ngraph TD; A-->B;\n```";
        let page = svc().render(md, &root.join("a.md"), "p1", &root, &idx);
        assert!(page.has_mermaid);
        assert!(page.html.contains("class=\"mermaid\""), "html: {}", page.html);
    }

    #[test]
    fn code_block_gets_class_based_highlight() {
        let root = PathBuf::from("/proj");
        let idx: HashSet<PathBuf> = HashSet::new();
        let md = "```rust\nfn main() {}\n```";
        let page = svc().render(md, &root.join("a.md"), "p1", &root, &idx);
        assert!(page.html.contains("<pre class=\"code\">"), "html: {}", page.html);
        // class-based: no inline color styles
        assert!(!page.html.contains("style=\"color"));
    }

    #[test]
    fn strips_script_xss() {
        let root = PathBuf::from("/proj");
        let idx: HashSet<PathBuf> = HashSet::new();
        let md = "<script>alert(1)</script>\n\nhi";
        let page = svc().render(md, &root.join("a.md"), "p1", &root, &idx);
        assert!(!page.html.contains("<script"));
    }
}
