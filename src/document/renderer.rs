// src/document/renderer.rs

use super::highlight;
use ratatui::text::{Line, Span};
use ratatui::style::{Color, Modifier, Style};

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

pub fn render(blocks: &[crate::document::parser::Block]) -> RenderedDoc {
    render_with(blocks, true)
}

/// With `highlight == false`, code bodies are unstyled (same line count), so
/// no grammar is loaded; highlighting can be applied later via `render_with(.., true)`.
pub fn render_with(blocks: &[crate::document::parser::Block], highlight: bool) -> RenderedDoc {
    let mut doc = RenderedDoc {
        lines: vec![],
        links: vec![],
        headings: vec![],
    };

    for block in blocks {
        render_block(block, &mut doc, 0, highlight);
    }

    doc
}

fn render_block(block: &crate::document::parser::Block, doc: &mut RenderedDoc, indent_level: usize, highlight: bool) {
    use crate::document::parser::Block;

    match block {
        Block::Heading { level, inlines } => {
            let line_index = doc.lines.len();
            let style = match level {
                1 => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                2 => Style::default().fg(Color::Cyan),
                3 => Style::default().fg(Color::Blue),
                _ => Style::default().fg(Color::Blue).add_modifier(Modifier::DIM),
            };
            let spans = render_inlines(inlines, style, doc, line_index);
            if !spans.is_empty() {
                doc.lines.push(Line::from(spans));
            }
            let text = inlines_to_text(inlines);
            doc.headings.push(HeadingInfo {
                line: line_index,
                level: *level,
                text,
            });
        }
        Block::Paragraph { inlines } => {
            let line_index = doc.lines.len();
            let style = Style::default();
            let spans = render_inlines(inlines, style, doc, line_index);
            if !spans.is_empty() {
                doc.lines.push(Line::from(spans));
            }
        }
        Block::CodeBlock { lang, code } => {
            let fence_style = Style::default().add_modifier(Modifier::DIM);
            let token = lang.as_deref().and_then(highlight::language_token);
            doc.lines.push(Line::from("")); // Empty line before code block
            doc.lines.push(Line::from(Span::styled(
                format!("```{}", token.as_deref().unwrap_or("")),
                fence_style,
            )));
            if highlight {
                doc.lines.extend(highlight::highlight_code(code, lang.as_deref()));
            } else {
                doc.lines.extend(highlight::plain(code));
            }
            doc.lines.push(Line::from(Span::styled("```".to_string(), fence_style)));
            doc.lines.push(Line::from("")); // Empty line after code block
        }
        Block::BlockQuote { blocks: inner } => {
            let start_idx = doc.lines.len();
            for block in inner {
                render_block(block, doc, indent_level + 1, highlight);
            }
            // Add blockquote prefix to all lines added
            for i in start_idx..doc.lines.len() {
                let mut line = doc.lines[i].clone();
                let prefix_span = Span::styled("│ ", Style::default().fg(Color::Gray));
                line.spans.insert(0, prefix_span);
                doc.lines[i] = line;
            }
        }
        Block::List { ordered, items } => {
            for (item_idx, item_blocks) in items.iter().enumerate() {
                let marker = if *ordered {
                    format!("{}. ", item_idx + 1)
                } else {
                    "• ".to_string()
                };
                let start_idx = doc.lines.len();
                for block in item_blocks {
                    render_block(block, doc, indent_level + 1, highlight);
                }
                // Add marker to first line of item
                if start_idx < doc.lines.len() {
                    let mut line = doc.lines[start_idx].clone();
                    let indent = "  ".repeat(indent_level);
                    line.spans.insert(0, Span::raw(format!("{}{}", indent, marker)));
                    doc.lines[start_idx] = line;
                }
                // Indent remaining lines
                for i in (start_idx + 1)..doc.lines.len() {
                    let mut line = doc.lines[i].clone();
                    let indent = "  ".repeat(indent_level + 1);
                    line.spans.insert(0, Span::raw(indent));
                    doc.lines[i] = line;
                }
            }
        }
        Block::Rule => {
            let style = Style::default().fg(Color::DarkGray);
            doc.lines.push(Line::from(Span::styled("─".repeat(40), style)));
        }
        Block::Table { .. } => {
            // Simplified table handling
            doc.lines.push(Line::from(Span::raw("[Table]")));
        }
    }
}

fn render_inlines(
    inlines: &[crate::document::parser::Inline],
    base_style: Style,
    doc: &mut RenderedDoc,
    line_index: usize,
) -> Vec<Span<'static>> {
    use crate::document::parser::Inline;
    let mut spans = vec![];

    for inline in inlines {
        match inline {
            Inline::Text(s) => {
                spans.push(Span::styled(s.clone(), base_style));
            }
            Inline::Bold(inner) => {
                let bold_style = base_style.add_modifier(Modifier::BOLD);
                for span in render_inlines(inner, bold_style, doc, line_index) {
                    spans.push(span);
                }
            }
            Inline::Italic(inner) => {
                let italic_style = base_style.add_modifier(Modifier::ITALIC);
                for span in render_inlines(inner, italic_style, doc, line_index) {
                    spans.push(span);
                }
            }
            Inline::Code(s) => {
                let code_style = base_style.fg(Color::Yellow);
                spans.push(Span::styled(s.clone(), code_style));
            }
            Inline::Link { text, url } => {
                let link_style = base_style
                    .fg(Color::Green)
                    .add_modifier(Modifier::UNDERLINED);
                let text_str = inlines_to_text(text);
                let col_start: usize = spans.iter().map(|s| s.content.len()).sum();
                let col_end = col_start + text_str.len();

                spans.push(Span::styled(text_str, link_style));

                doc.links.push(LinkInfo {
                    line: line_index,
                    col_start,
                    col_end,
                    url: url.clone(),
                });
            }
            Inline::SoftBreak => {
                spans.push(Span::raw(" "));
            }
            Inline::HardBreak => {
                // End the line; next item on new line
                spans.push(Span::raw("\n"));
            }
        }
    }

    spans
}

fn inlines_to_text(inlines: &[crate::document::parser::Inline]) -> String {
    use crate::document::parser::Inline;
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(s) => text.push_str(s),
            Inline::Bold(inner) | Inline::Italic(inner) => {
                text.push_str(&inlines_to_text(inner));
            }
            Inline::Code(s) => text.push_str(s),
            Inline::Link { text: link_text, .. } => {
                text.push_str(&inlines_to_text(link_text));
            }
            Inline::SoftBreak => text.push(' '),
            Inline::HardBreak => text.push('\n'),
        }
    }
    text
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
    fn code_block_body_is_highlighted() {
        let doc = render(&parser::parse("```rust,ignore\nfn main() {}\n```"));
        // [blank, fence, body, fence, blank]
        assert_eq!(doc.lines.len(), 5);
        let fence: String = doc.lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(fence, "```rust");
        assert!(doc.lines[2].spans.iter().any(|s| s.style.fg.is_some()));
        assert!(doc.lines.iter().flat_map(|l| &l.spans).all(|s| s.style.bg.is_none()));
    }

    #[test]
    fn renders_multiple_links() {
        let blocks = parser::parse("[a](./a.md) and [b](./b.md)");
        let doc = render(&blocks);
        assert_eq!(doc.links.len(), 2);
        assert_eq!(doc.links[0].url, "./a.md");
        assert_eq!(doc.links[1].url, "./b.md");
    }

    #[test]
    fn plain_and_highlighted_have_same_shape() {
        let md = "# T\n\n```rust\nfn a() {}\n\n\tlet x = 1;\n```\n\n[l](./x.md)";
        let blocks = crate::document::parser::parse(md);
        let plain = render_with(&blocks, false);
        let hl = render_with(&blocks, true);
        assert_eq!(plain.lines.len(), hl.lines.len());
        assert_eq!(plain.links.len(), hl.links.len());
        assert_eq!(plain.headings.len(), hl.headings.len());
        let text = |d: &RenderedDoc| {
            d.lines
                .iter()
                .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect::<String>())
                .collect::<Vec<_>>()
        };
        assert_eq!(text(&plain), text(&hl));
    }
}
