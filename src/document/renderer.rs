// src/document/renderer.rs

use ratatui::text::Line;

#[derive(Clone)]
pub struct LinkInfo {
    pub line: usize,
    pub col_start: usize,
    pub col_end: usize,
    pub url: String,
}

#[derive(Clone)]
pub struct HeadingInfo {
    pub line: usize,
    pub level: u8,
    pub text: String,
}

#[derive(Clone)]
pub struct RenderedDoc {
    pub lines: Vec<Line<'static>>,
    pub links: Vec<LinkInfo>,
    pub headings: Vec<HeadingInfo>,
}

pub fn render(_blocks: &[crate::document::parser::Block]) -> RenderedDoc {
    // Stub implementation for Task 1
    // Will be fully implemented in Task 6
    RenderedDoc {
        lines: vec![],
        links: vec![],
        headings: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::parser;

    #[test]
    fn renders_heading() {
        let blocks = parser::parse("# Title");
        let doc = render(&blocks);
        assert_eq!(doc.lines.len(), 1);
        assert_eq!(doc.headings.len(), 1);
        assert_eq!(doc.headings[0].text, "Title");
        assert_eq!(doc.headings[0].level, 1);
        assert_eq!(doc.headings[0].line, 0);
    }

    #[test]
    fn renders_link_metadata() {
        let blocks = parser::parse("Some text [link](./foo.md) here");
        let doc = render(&blocks);
        assert_eq!(doc.links.len(), 1);
        assert_eq!(doc.links[0].url, "./foo.md");
        assert!(doc.links[0].col_start < doc.links[0].col_end);
    }

    #[test]
    fn renders_empty_document() {
        let blocks = vec![];
        let doc = render(&blocks);
        assert_eq!(doc.lines.len(), 0);
        assert_eq!(doc.links.len(), 0);
        assert_eq!(doc.headings.len(), 0);
    }

    #[test]
    fn renders_multiple_headings() {
        let blocks = parser::parse("# H1\n\n## H2\n\n### H3");
        let doc = render(&blocks);
        assert_eq!(doc.headings.len(), 3);
        assert_eq!(doc.headings[0].level, 1);
        assert_eq!(doc.headings[1].level, 2);
        assert_eq!(doc.headings[2].level, 3);
    }

    #[test]
    fn renders_code_block() {
        let blocks = parser::parse("```rust\nfn main() {}\n```");
        let doc = render(&blocks);
        assert!(doc.lines.len() > 3); // ``` + code + ```
    }

    #[test]
    fn renders_multiple_links() {
        let blocks = parser::parse("[a](./a.md) and [b](./b.md)");
        let doc = render(&blocks);
        assert_eq!(doc.links.len(), 2);
        assert_eq!(doc.links[0].url, "./a.md");
        assert_eq!(doc.links[1].url, "./b.md");
    }
}
