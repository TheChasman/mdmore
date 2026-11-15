// src/document/renderer.rs

use super::highlight;
use ratatui::text::{Line, Span};
use ratatui::style::{Color, Modifier, Style};
use std::path::{Path, PathBuf};

/// Rows reserved for an inline image.
pub const IMAGE_ROWS: u16 = 12;

#[derive(Clone)]
pub struct ImageInfo {
    pub line: usize,
    pub height: u16,
    pub path: PathBuf,
    pub alt: String,
}

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
    pub images: Vec<ImageInfo>,
}

pub fn render(blocks: &[crate::document::parser::Block]) -> RenderedDoc {
    render_with(blocks, true, None)
}

/// With `highlight == false`, code bodies are unstyled (same line count), so
/// no grammar is loaded; highlighting can be applied later via `render_with(.., true)`.
pub fn render_with(
    blocks: &[crate::document::parser::Block],
    highlight: bool,
    max_width: Option<u16>,
) -> RenderedDoc {
    render_with_base(blocks, highlight, max_width, None)
}

/// As `render_with`; local image urls resolve against `base_dir`. With `None`
/// (print mode) every image renders as its `[image: <alt>]` line.
pub fn render_with_base(
    blocks: &[crate::document::parser::Block],
    highlight: bool,
    max_width: Option<u16>,
    base_dir: Option<&Path>,
) -> RenderedDoc {
    let mut doc = RenderedDoc {
        lines: vec![],
        links: vec![],
        headings: vec![],
        images: vec![],
    };

    use crate::document::parser::Block;
    for (i, block) in blocks.iter().enumerate() {
        // One blank line between top-level blocks; code blocks pad themselves.
        let is_code = matches!(block, Block::CodeBlock { .. });
        let after_code = i > 0 && matches!(blocks[i - 1], Block::CodeBlock { .. });
        if is_code && after_code {
            doc.lines.pop(); // the previous block's trailing blank; this one adds its own
        } else if i > 0 && !is_code && !after_code {
            doc.lines.push(Line::from(""));
        }
        render_block(block, &mut doc, 0, highlight, max_width, base_dir);
    }

    doc
}

fn render_block(block: &crate::document::parser::Block, doc: &mut RenderedDoc, indent_level: usize, highlight: bool, max_width: Option<u16>, base_dir: Option<&Path>) {
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
                render_block(block, doc, indent_level + 1, highlight, max_width, base_dir);
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
                    render_block(block, doc, indent_level + 1, highlight, max_width, base_dir);
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
        Block::Table { headers, rows } => render_table(headers, rows, doc, max_width),
        Block::Image { url, alt } => match local_image(url, base_dir) {
            Some(path) => {
                doc.images.push(ImageInfo {
                    line: doc.lines.len(),
                    height: IMAGE_ROWS,
                    path,
                    alt: alt.clone(),
                });
                for _ in 0..IMAGE_ROWS {
                    doc.lines.push(Line::from(""));
                }
            }
            None => doc.lines.push(Line::from(Span::styled(
                format!("[image: {alt}]"),
                Style::default().add_modifier(Modifier::DIM),
            ))),
        },
    }
}

/// Resolves `url` to an existing local file, or None for remote/missing urls.
fn local_image(url: &str, base_dir: Option<&Path>) -> Option<PathBuf> {
    if url.contains("://") || url.starts_with("data:") {
        return None;
    }
    let path = base_dir?.join(url);
    path.is_file().then_some(path)
}

fn render_table(
    headers: &[Vec<crate::document::parser::Inline>],
    rows: &[Vec<Vec<crate::document::parser::Inline>>],
    doc: &mut RenderedDoc,
    max_width: Option<u16>,
) {
    let ncols = rows.iter().map(|r| r.len()).chain([headers.len()]).max().unwrap_or(0);
    if ncols == 0 {
        return;
    }
    let text_of = |cell: Option<&Vec<crate::document::parser::Inline>>| {
        cell.map(|c| inlines_to_text(c)).unwrap_or_default()
    };
    let mut widths = vec![0usize; ncols];
    for (i, w) in widths.iter_mut().enumerate() {
        *w = std::iter::once(headers.get(i))
            .chain(rows.iter().map(|r| r.get(i)))
            .map(|c| text_of(c).chars().count())
            .max()
            .unwrap_or(0);
    }
    if let Some(max) = max_width {
        // Shrink the widest column first, down to a minimum of 3 chars.
        let overhead = 3 * (ncols - 1);
        while widths.iter().sum::<usize>() + overhead > max as usize {
            let (i, w) = widths.iter().copied().enumerate().max_by_key(|&(_, w)| w).unwrap();
            if w <= 3 {
                break;
            }
            widths[i] -= 1;
        }
    }

    let fit = |s: String, w: usize| -> String {
        if s.chars().count() > w {
            let mut t: String = s.chars().take(w.saturating_sub(1)).collect();
            t.push('…');
            t
        } else {
            s
        }
    };
    let emit = |cells: Vec<Option<&Vec<crate::document::parser::Inline>>>, bold: bool, doc: &mut RenderedDoc| {
        let line_index = doc.lines.len();
        let style = if bold { Style::default().add_modifier(Modifier::BOLD) } else { Style::default() };
        let mut spans: Vec<Span<'static>> = vec![];
        for (i, cell) in cells.into_iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw(" │ "));
            }
            let w = widths[i];
            let last = i + 1 == ncols;
            let full = cell.map(|c| inlines_to_text(c)).unwrap_or_default();
            if full.chars().count() > w {
                spans.push(Span::styled(fit(full, w), style));
                continue;
            }
            let mut cell_spans = match cell {
                Some(c) => {
                    let col_base: usize = spans.iter().map(|s| s.content.chars().count()).sum();
                    let before = doc.links.len();
                    let sp = render_inlines(c, style, doc, line_index);
                    for l in &mut doc.links[before..] {
                        l.col_start += col_base;
                        l.col_end += col_base;
                    }
                    sp
                }
                None => vec![],
            };
            spans.append(&mut cell_spans);
            if !last {
                spans.push(Span::raw(" ".repeat(w - full.chars().count())));
            }
        }
        doc.lines.push(Line::from(spans));
    };

    emit((0..ncols).map(|i| headers.get(i)).collect(), true, doc);
    let sep: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
    doc.lines.push(Line::from(Span::raw(sep.join("─┼─"))));
    for r in rows {
        emit((0..ncols).map(|i| r.get(i)).collect(), false, doc);
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
    use std::path::Path;

    #[test]
    fn missing_image_falls_back_to_alt() {
        let doc = render_with_base(&parser::parse("![gone](nope.png)"), false, None, Some(Path::new("/tmp")));
        assert!(doc.images.is_empty());
        let t: String = doc.lines[0].spans.iter().map(|s| s.content.to_string()).collect();
        assert_eq!(t, "[image: gone]");
    }

    #[test]
    fn local_image_reserves_rows() {
        let dir = std::env::temp_dir();
        let p = dir.join("frogrust_test_img.png");
        image::RgbaImage::new(4, 4).save(&p).unwrap();
        let doc = render_with_base(&parser::parse("![x](frogrust_test_img.png)"), false, None, Some(&dir));
        assert_eq!(doc.images.len(), 1);
        assert_eq!(doc.images[0].height, 12);
        assert_eq!(doc.lines.len(), 12);
    }
    use crate::document::parser;

    fn texts(doc: &RenderedDoc) -> Vec<String> {
        doc.lines.iter().map(|l| l.spans.iter().map(|s| s.content.to_string()).collect()).collect()
    }

    #[test]
    fn one_blank_line_between_top_level_blocks() {
        let doc = render_with(&parser::parse("# H\n\npara\n\n> quote\n\n| a |\n|---|\n| 1 |"), false, None);
        assert_eq!(texts(&doc), vec!["H", "", "para", "", "│ quote", "", "a", "─", "1"]);
        assert_eq!(doc.headings[0].line, 0);
    }

    #[test]
    fn code_blocks_are_not_double_spaced() {
        let doc = render_with(&parser::parse("para\n\n```\nx\n```\n\nafter"), false, None);
        assert_eq!(texts(&doc), vec!["para", "", "```", "x", "```", "", "after"]);
    }

    #[test]
    fn consecutive_code_blocks_share_one_blank() {
        let doc = render_with(&parser::parse("```\na\n```\n\n```\nb\n```"), false, None);
        assert_eq!(texts(&doc), vec!["", "```", "a", "```", "", "```", "b", "```", ""]);
    }

    #[test]
    fn tight_list_stays_tight() {
        let doc = render_with(&parser::parse("- a\n- b\n\npara"), false, None);
        assert_eq!(texts(&doc), vec!["• a", "• b", "", "para"]);
    }

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
        let plain = render_with(&blocks, false, None);
        let hl = render_with(&blocks, true, None);
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

    #[test]
    fn renders_table_grid() {
        let doc = render(&parser::parse("| A | Bee |\n|---|---|\n| 1 | 2 |\n| long cell | x |"));
        let text: Vec<String> = doc.lines.iter()
            .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect()).collect();
        assert_eq!(text, vec![
            "A         │ Bee",
            "──────────┼────",
            "1         │ 2",
            "long cell │ x",
        ]);
    }

    #[test]
    fn wide_table_is_truncated_to_width() {
        let md = "| a | b |\n|---|---|\n| aaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbb |";
        let doc = render_with(&parser::parse(md), false, Some(20));
        for l in &doc.lines {
            let w: usize = l.spans.iter().map(|s| s.content.chars().count()).sum();
            assert!(w <= 20, "line too wide: {w}");
        }
        assert!(doc.lines.iter().any(|l| l.spans.iter().any(|s| s.content.contains('…'))));
    }

    #[test]
    fn table_links_have_column_offsets() {
        let doc = render(&parser::parse("| A | B |\n|---|---|\n| xx | [go](./g.md) |"));
        assert_eq!(doc.links.len(), 1);
        assert_eq!((doc.links[0].line, doc.links[0].col_start, doc.links[0].col_end), (2, 5, 7));
    }
}
