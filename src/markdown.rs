use std::collections::{HashMap, HashSet};

use ammonia::Builder;
use pulldown_cmark::{Options, Parser, html};

#[derive(Clone, Debug, Default)]
pub struct MarkdownRenderer;

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, markdown: &str) -> String {
        let parser = Parser::new_ext(markdown, markdown_options());
        let mut rendered_html = String::new();
        html::push_html(&mut rendered_html, parser);

        sanitizer().clean(&rendered_html).to_string()
    }
}

fn markdown_options() -> Options {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    options
}

fn sanitizer() -> Builder<'static> {
    let mut tags = HashSet::new();
    for tag in [
        "a",
        "blockquote",
        "br",
        "code",
        "dd",
        "del",
        "div",
        "dl",
        "dt",
        "em",
        "figcaption",
        "figure",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "hr",
        "img",
        "li",
        "ol",
        "p",
        "pre",
        "strong",
        "sup",
        "table",
        "tbody",
        "td",
        "th",
        "thead",
        "tr",
        "ul",
    ] {
        tags.insert(tag);
    }

    let mut tag_attributes: HashMap<&'static str, HashSet<&'static str>> = HashMap::new();
    tag_attributes.insert("a", HashSet::from(["href", "title"]));
    tag_attributes.insert("img", HashSet::from(["src", "alt", "title"]));
    tag_attributes.insert("code", HashSet::from(["class"]));
    tag_attributes.insert("pre", HashSet::from(["class"]));
    tag_attributes.insert("th", HashSet::from(["align"]));
    tag_attributes.insert("td", HashSet::from(["align"]));

    let mut builder = Builder::default();
    builder.tags(tags);
    builder.tag_attributes(tag_attributes);
    builder.link_rel(Some("noopener noreferrer"));
    builder
}

#[cfg(test)]
mod tests {
    use super::MarkdownRenderer;

    #[test]
    fn renders_common_markdown_to_html() {
        let renderer = MarkdownRenderer::new();
        let html = renderer.render(
            r#"# Hello World

Paragraph with **bold** text and a [link](https://example.com).

- first
- second
"#,
        );

        assert!(html.contains("<h1>Hello World</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains(r#"<a href="https://example.com" rel="noopener noreferrer">link</a>"#));
        assert!(html.contains("<li>first</li>"));
    }

    #[test]
    fn strips_unsafe_html_and_javascript_urls() {
        let renderer = MarkdownRenderer::new();
        let html = renderer.render(
            r#"<script>alert("xss")</script>

[bad link](javascript:alert('xss'))

<img src="https://example.com/image.png" onerror="alert('xss')" alt="safe">
"#,
        );

        assert!(!html.contains("<script>"));
        assert!(!html.contains("javascript:alert"));
        assert!(!html.contains("onerror="));
        assert!(html.contains(r#"<img src="https://example.com/image.png" alt="safe">"#));
    }
}
