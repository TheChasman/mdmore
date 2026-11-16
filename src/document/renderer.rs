// src/document/renderer.rs

use super::highlight;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
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
        render_block(block, &mut doc, highlight, max_width, base_dir);
    }

    doc
}

fn render_block(
    block: &crate::document::parser::Block,
    doc: &mut RenderedDoc,
    highlight: bool,
    max_width: Option<u16>,
    base_dir: Option<&Path>,
) {
    use crate::document::parser::Block;

    match block {
        Block::Heading { level, inlines } => {
            let line_index = doc.lines.len();
            let style = match level {
                1 => Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                2 => Style::default().fg(Color::Cyan),
                3 => Style::default().fg(Color::Blue),
                _ => Style::default().fg(Color::Blue).add_modifier(Modifier::DIM),
            };
            let links_before = doc.links.len();
            let spans = render_inlines(inlines, style, doc, line_index);
            push_wrapped(doc, spans, links_before, max_width);
            let text = inlines_to_text(inlines);
            doc.headings.push(HeadingInfo {
                line: line_index,
                level: *level,
                text,
            });
        }
        Block::Paragraph { inlines } => {
            let line_index = doc.lines.len();
            let links_before = doc.links.len();
            let spans = render_inlines(inlines, Style::default(), doc, line_index);
            push_wrapped(doc, spans, links_before, max_width);
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
                doc.lines
                    .extend(highlight::highlight_code(code, lang.as_deref()));
            } else {
                doc.lines.extend(highlight::plain(code));
            }
            doc.lines
                .push(Line::from(Span::styled("```".to_string(), fence_style)));
            doc.lines.push(Line::from("")); // Empty line after code block
        }
        Block::BlockQuote { blocks: inner } => {
            let start_idx = doc.lines.len();
            let links_before = doc.links.len();
            let inner_width = max_width.map(|w| w.saturating_sub(2).max(1));
            for block in inner {
                render_block(block, doc, highlight, inner_width, base_dir);
            }
            // Prefix every line added (including wrapped continuations).
            for i in start_idx..doc.lines.len() {
                let prefix = Span::styled("│ ", Style::default().fg(Color::Gray));
                prefix_line(doc, i, links_before, prefix);
            }
        }
        Block::List { ordered, items } => {
            for (item_idx, item_blocks) in items.iter().enumerate() {
                let marker = if *ordered {
                    format!("{}. ", item_idx + 1)
                } else {
                    "• ".to_string()
                };
                // Nesting comes from the parent's prefix, so items need no level indent.
                let marker_w = marker.chars().count();
                let start_idx = doc.lines.len();
                let links_before = doc.links.len();
                let inner_width = max_width.map(|w| w.saturating_sub(marker_w as u16).max(1));
                for block in item_blocks {
                    render_block(block, doc, highlight, inner_width, base_dir);
                }
                // Marker on the first line; hanging indent under it on the rest.
                for i in start_idx..doc.lines.len() {
                    let prefix = if i == start_idx {
                        marker.clone()
                    } else {
                        " ".repeat(marker_w)
                    };
                    prefix_line(doc, i, links_before, Span::raw(prefix));
                }
            }
        }
        Block::Rule => {
            let style = Style::default().fg(Color::DarkGray);
            let len = max_width.map_or(40, |w| (w as usize).min(40));
            doc.lines
                .push(Line::from(Span::styled("─".repeat(len), style)));
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

/// Inserts `prefix` at the start of line `i`, shifting that line's links
/// (those registered at or after `links_from`) right by its width.
fn prefix_line(doc: &mut RenderedDoc, i: usize, links_from: usize, prefix: Span<'static>) {
    let w = prefix.content.chars().count();
    doc.lines[i].spans.insert(0, prefix);
    for l in doc.links[links_from..].iter_mut().filter(|l| l.line == i) {
        l.col_start += w;
        l.col_end += w;
    }
}

/// Pushes one logical line of `spans` (starting at `doc.lines.len()`), wrapped
/// to `width` at word boundaries; words longer than the width are split. A
/// `"\n"` span (hard break) forces a new line. Links registered at or after
/// `links_from` refer to the logical line and are re-registered per wrapped
/// segment. Widths are counted in chars. Upgrade path: unicode-width for CJK.
fn push_wrapped(
    doc: &mut RenderedDoc,
    spans: Vec<Span<'static>>,
    links_from: usize,
    width: Option<u16>,
) {
    if spans.is_empty() {
        return;
    }
    let has_break = spans.iter().any(|s| s.content.contains('\n'));
    let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    if !has_break && width.is_none_or(|w| total <= w as usize) {
        doc.lines.push(Line::from(spans));
        return;
    }
    let width = width.map_or(usize::MAX, |w| (w as usize).max(1));
    let chars: Vec<(char, Style)> = spans
        .iter()
        .flat_map(|s| s.content.chars().map(move |c| (c, s.style)))
        .collect();

    // Assign each char a (row, col), or None where dropped (spaces at wraps, breaks).
    let mut pos: Vec<Option<(usize, usize)>> = vec![None; chars.len()];
    let (mut row, mut col) = (0usize, 0usize);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i].0;
        if c == '\n' {
            row += 1;
            col = 0;
            i += 1;
            continue;
        }
        let space = c == ' ';
        let mut j = i;
        while j < chars.len() && chars[j].0 != '\n' && (chars[j].0 == ' ') == space {
            j += 1;
        }
        if space {
            // Kept only if the next word fits on this line after them.
            let mut k = j;
            while k < chars.len() && chars[k].0 != ' ' && chars[k].0 != '\n' {
                k += 1;
            }
            let next_len = k - j;
            if col > 0 && col + (j - i) + next_len <= width {
                for p in pos.iter_mut().take(j).skip(i) {
                    *p = Some((row, col));
                    col += 1;
                }
            } else if col > 0 && next_len > 0 {
                row += 1;
                col = 0;
            } else if col == 0 && row == 0 {
                // Leading spaces on the first line are content.
                for p in pos.iter_mut().take(j).skip(i) {
                    *p = Some((row, col));
                    col += 1;
                }
            }
        } else {
            let len = j - i;
            if col > 0 && col + len > width {
                row += 1;
                col = 0;
            }
            for p in pos.iter_mut().take(j).skip(i) {
                if col == width {
                    row += 1;
                    col = 0;
                }
                *p = Some((row, col));
                col += 1;
            }
        }
        i = j;
    }

    // Rebuild styled spans per row.
    let base = doc.lines.len();
    let mut rows: Vec<Vec<Span<'static>>> = vec![vec![]; row + 1];
    let mut run = String::new();
    let mut run_style = Style::default();
    let mut run_row = 0;
    for (k, &(c, style)) in chars.iter().enumerate() {
        let Some((r, _)) = pos[k] else { continue };
        if !run.is_empty() && (r != run_row || style != run_style) {
            rows[run_row].push(Span::styled(std::mem::take(&mut run), run_style));
        }
        run_row = r;
        run_style = style;
        run.push(c);
    }
    if !run.is_empty() {
        rows[run_row].push(Span::styled(run, run_style));
    }
    for r in rows {
        doc.lines.push(Line::from(r));
    }

    // Re-register links per wrapped segment.
    let old: Vec<LinkInfo> = doc.links.drain(links_from..).collect();
    for l in old {
        let mut seg: Option<(usize, usize, usize)> = None; // (row, start, end)
        for p in pos
            .iter()
            .take(l.col_end.min(chars.len()))
            .skip(l.col_start)
            .flatten()
        {
            match &mut seg {
                Some((r, _, e)) if *r == p.0 => *e = p.1 + 1,
                _ => {
                    if let Some((r, s, e)) = seg.take() {
                        doc.links.push(LinkInfo {
                            line: base + r,
                            col_start: s,
                            col_end: e,
                            url: l.url.clone(),
                        });
                    }
                    seg = Some((p.0, p.1, p.1 + 1));
                }
            }
        }
        if let Some((r, s, e)) = seg {
            doc.links.push(LinkInfo {
                line: base + r,
                col_start: s,
                col_end: e,
                url: l.url,
            });
        }
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
    let ncols = rows
        .iter()
        .map(|r| r.len())
        .chain([headers.len()])
        .max()
        .unwrap_or(0);
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
            let (i, w) = widths
                .iter()
                .copied()
                .enumerate()
                .max_by_key(|&(_, w)| w)
                .unwrap();
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
    let emit = |cells: Vec<Option<&Vec<crate::document::parser::Inline>>>,
                bold: bool,
                doc: &mut RenderedDoc| {
        let line_index = doc.lines.len();
        let style = if bold {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
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
                let col_start: usize = spans.iter().map(|s| s.content.chars().count()).sum();
                let col_end = col_start + text_str.chars().count();

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
            Inline::Link {
                text: link_text, ..
            } => {
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
        let doc = render_with_base(
            &parser::parse("![gone](nope.png)"),
            false,
            None,
            Some(Path::new("/tmp")),
        );
        assert!(doc.images.is_empty());
        let t: String = doc.lines[0]
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();
        assert_eq!(t, "[image: gone]");
    }

    #[test]
    fn local_image_reserves_rows() {
        let dir = std::env::temp_dir();
        let p = dir.join("mdmore_test_img.png");
        image::RgbaImage::new(4, 4).save(&p).unwrap();
        let doc = render_with_base(
            &parser::parse("![x](mdmore_test_img.png)"),
            false,
            None,
            Some(&dir),
        );
        assert_eq!(doc.images.len(), 1);
        assert_eq!(doc.images[0].height, 12);
        assert_eq!(doc.lines.len(), 12);
    }
    use crate::document::parser;

    fn texts(doc: &RenderedDoc) -> Vec<String> {
        doc.lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect())
            .collect()
    }

    #[test]
    fn one_blank_line_between_top_level_blocks() {
        let doc = render_with(
            &parser::parse("# H\n\npara\n\n> quote\n\n| a |\n|---|\n| 1 |"),
            false,
            None,
        );
        assert_eq!(
            texts(&doc),
            vec!["H", "", "para", "", "│ quote", "", "a", "─", "1"]
        );
        assert_eq!(doc.headings[0].line, 0);
    }

    #[test]
    fn code_blocks_are_not_double_spaced() {
        let doc = render_with(&parser::parse("para\n\n```\nx\n```\n\nafter"), false, None);
        assert_eq!(
            texts(&doc),
            vec!["para", "", "```", "x", "```", "", "after"]
        );
    }

    #[test]
    fn consecutive_code_blocks_share_one_blank() {
        let doc = render_with(&parser::parse("```\na\n```\n\n```\nb\n```"), false, None);
        assert_eq!(
            texts(&doc),
            vec!["", "```", "a", "```", "", "```", "b", "```", ""]
        );
    }

    #[test]
    fn tight_list_stays_tight() {
        let doc = render_with(&parser::parse("- a\n- b\n\npara"), false, None);
        assert_eq!(texts(&doc), vec!["• a", "• b", "", "para"]);
    }

    fn w(md: &str, width: u16) -> RenderedDoc {
        render_with(&parser::parse(md), false, Some(width))
    }

    #[test]
    fn wraps_paragraph_at_word_boundaries() {
        assert_eq!(texts(&w("aaa bbb ccc ddd", 8)), vec!["aaa bbb", "ccc ddd"]);
    }

    #[test]
    fn splits_overlong_words() {
        assert_eq!(
            texts(&w("abcdefghij xy", 4)),
            vec!["abcd", "efgh", "ij", "xy"]
        );
    }

    #[test]
    fn unwrapped_without_width() {
        let doc = render_with(&parser::parse("aaa bbb ccc ddd"), false, None);
        assert_eq!(texts(&doc), vec!["aaa bbb ccc ddd"]);
    }

    #[test]
    fn wraps_heading() {
        let doc = w("# one two three", 8);
        assert_eq!(texts(&doc), vec!["one two", "three"]);
        assert_eq!(doc.headings[0].line, 0);
    }

    #[test]
    fn list_items_wrap_with_hanging_indent() {
        assert_eq!(texts(&w("- aaa bbb ccc", 9)), vec!["• aaa bbb", "  ccc"]);
        assert_eq!(
            texts(&w("1. aaa bbb ccc", 10)),
            vec!["1. aaa bbb", "   ccc"]
        );
    }

    #[test]
    fn nested_list_wraps_inside_parent() {
        assert_eq!(
            texts(&w("- a\n  - bbb ccc", 9)),
            vec!["• a", "  • bbb", "    ccc"]
        );
    }

    #[test]
    fn block_quote_wraps_with_prefix_on_every_line() {
        assert_eq!(texts(&w("> aaa bbb ccc", 9)), vec!["│ aaa bbb", "│ ccc"]);
    }

    #[test]
    fn code_blocks_do_not_wrap() {
        let doc = w("```\naaaa bbbb cccc\n```", 5);
        assert!(texts(&doc).contains(&"aaaa bbbb cccc".to_string()));
    }

    #[test]
    fn wrapped_lines_fit_width() {
        let md = "A long paragraph with **bold words** and `code` and [a link](./x.md) that goes on and on.\n\n> quoted text that also goes on for quite a while\n\n- item text long enough to wrap around\n  - nested item also long enough";
        for width in [10u16, 17, 23, 40] {
            for l in texts(&w(md, width)) {
                assert!(l.chars().count() <= width as usize, "{width}: {l:?}");
            }
        }
    }

    #[test]
    fn wrapped_link_registers_each_segment() {
        let doc = w("see [one two](./x.md) end", 7);
        assert_eq!(texts(&doc), vec!["see one", "two end"]);
        let segs: Vec<_> = doc
            .links
            .iter()
            .map(|l| (l.line, l.col_start, l.col_end, l.url.as_str()))
            .collect();
        assert_eq!(segs, vec![(0, 4, 7, "./x.md"), (1, 0, 3, "./x.md")]);
    }

    #[test]
    fn link_cols_include_list_and_quote_prefixes() {
        let doc = w("- [go](./g.md)", 40);
        assert_eq!(
            (
                doc.links[0].line,
                doc.links[0].col_start,
                doc.links[0].col_end
            ),
            (0, 2, 4)
        );
        let doc = w("> [go](./g.md)", 40);
        assert_eq!((doc.links[0].col_start, doc.links[0].col_end), (2, 4));
    }

    #[test]
    fn plain_and_highlighted_same_line_count_when_wrapped() {
        let md = "# T\n\n```rust\nfn a() { let long_name = 1; }\n```\n\nsome words that wrap here [l](./x.md)";
        let blocks = parser::parse(md);
        let plain = render_with(&blocks, false, Some(12));
        let hl = render_with(&blocks, true, Some(12));
        assert_eq!(texts(&plain), texts(&hl));
        assert_eq!(plain.links.len(), hl.links.len());
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
        let fence: String = doc.lines[1]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(fence, "```rust");
        assert!(doc.lines[2].spans.iter().any(|s| s.style.fg.is_some()));
        assert!(doc
            .lines
            .iter()
            .flat_map(|l| &l.spans)
            .all(|s| s.style.bg.is_none()));
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
                .map(|l| {
                    l.spans
                        .iter()
                        .map(|s| s.content.to_string())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(text(&plain), text(&hl));
    }

    #[test]
    fn renders_table_grid() {
        let doc = render(&parser::parse(
            "| A | Bee |\n|---|---|\n| 1 | 2 |\n| long cell | x |",
        ));
        let text: Vec<String> = doc
            .lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect())
            .collect();
        assert_eq!(
            text,
            vec![
                "A         │ Bee",
                "──────────┼────",
                "1         │ 2",
                "long cell │ x",
            ]
        );
    }

    #[test]
    fn wide_table_is_truncated_to_width() {
        let md = "| a | b |\n|---|---|\n| aaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbb |";
        let doc = render_with(&parser::parse(md), false, Some(20));
        for l in &doc.lines {
            let w: usize = l.spans.iter().map(|s| s.content.chars().count()).sum();
            assert!(w <= 20, "line too wide: {w}");
        }
        assert!(doc
            .lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains('…'))));
    }

    #[test]
    fn table_links_have_column_offsets() {
        let doc = render(&parser::parse(
            "| A | B |\n|---|---|\n| xx | [go](./g.md) |",
        ));
        assert_eq!(doc.links.len(), 1);
        assert_eq!(
            (
                doc.links[0].line,
                doc.links[0].col_start,
                doc.links[0].col_end
            ),
            (2, 5, 7)
        );
    }
}
