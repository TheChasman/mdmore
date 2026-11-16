use crate::app::state::AppState;
use anyhow::Result;
use crossterm::terminal::size;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, StatefulImage};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct Ui {
    pub width: u16,
    pub height: u16,
    /// Terminal graphics picker; None until set after raw mode (or if detection failed).
    pub picker: Option<Picker>,
    /// Decoded images per path; None records a decode failure (alt text shown).
    images: HashMap<PathBuf, Option<Box<dyn StatefulProtocol>>>,
}

impl Ui {
    pub fn new() -> Result<Self> {
        let (width, height) = size()?;
        Ok(Self {
            width,
            height,
            picker: None,
            images: HashMap::new(),
        })
    }

    pub fn render(&mut self, f: &mut Frame, state: &AppState) {
        let chunks = split_columns(f.area(), state.toc_visible);

        let main_area = chunks[chunks.len() - 1];
        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(main_area);

        let doc_area = content_chunks[0];
        let status_area = content_chunks[1];

        self.render_document(f, state, doc_area);
        self.render_status_bar(f, state, status_area);

        if state.toc_visible {
            self.render_toc(f, state, chunks[0]);
        }
    }

    fn render_document(&mut self, f: &mut Frame, state: &AppState, area: Rect) {
        let start = state.scroll_offset as usize;
        let line_count = state.current_document.rendered.lines.len();
        let end = (start + area.height as usize).min(line_count);

        let visible_lines: Vec<Line> = state.current_document.rendered.lines[start..end]
            .iter()
            .enumerate()
            .map(|(visual_idx, line)| {
                let absolute_line = start + visual_idx;
                if let Some(cursor_line) = state.cursor_line {
                    if cursor_line == absolute_line {
                        // Highlight cursor line with reversed colors
                        let mut highlighted_spans = Vec::new();
                        for span in &line.spans {
                            let highlighted_style =
                                Style::default().bg(Color::White).fg(Color::Black);
                            highlighted_spans
                                .push(Span::styled(span.content.clone(), highlighted_style));
                        }
                        return Line::from(highlighted_spans);
                    }
                }
                let ranges: Vec<(usize, usize)> = state
                    .search_matches
                    .iter()
                    .filter(|m| m.0 == absolute_line)
                    .map(|m| (m.1, m.2))
                    .collect();
                if ranges.is_empty() {
                    line.clone()
                } else {
                    highlight_ranges(line, &ranges)
                }
            })
            .collect();

        let paragraph = Paragraph::new(visible_lines)
            .block(Block::default().borders(Borders::NONE))
            .alignment(Alignment::Left);

        f.render_widget(paragraph, area);
        self.render_images(f, state, area);
    }

    /// Draws each on-screen image into its placeholder rows. A partly visible
    /// image shows its alt text in its first visible row instead.
    fn render_images(&mut self, f: &mut Frame, state: &AppState, area: Rect) {
        let start = state.scroll_offset as usize;
        let end = start + area.height as usize;
        for info in &state.current_document.rendered.images {
            let (top, bottom) = (info.line, info.line + info.height as usize);
            if bottom <= start || top >= end {
                continue;
            }
            let fully = top >= start && bottom <= end;
            if fully {
                let rect = Rect::new(
                    area.x,
                    area.y + (top - start) as u16,
                    area.width,
                    info.height,
                );
                if let Some(proto) = self.image_for(&info.path) {
                    f.render_stateful_widget(StatefulImage::new(None), rect, proto);
                    continue;
                }
            }
            let row = area.y + (top.max(start) - start) as u16;
            let alt = Paragraph::new(Line::from(Span::styled(
                format!("[image: {}]", info.alt),
                Style::default().add_modifier(Modifier::DIM),
            )));
            f.render_widget(alt, Rect::new(area.x, row, area.width, 1));
        }
    }

    /// Lazily decodes `path` on first display; decode failures are cached as None.
    fn image_for(&mut self, path: &Path) -> Option<&mut Box<dyn StatefulProtocol>> {
        let picker = self.picker.as_mut()?;
        self.images
            .entry(path.to_path_buf())
            .or_insert_with(|| {
                let img = image::ImageReader::open(path)
                    .ok()?
                    .with_guessed_format()
                    .ok()?
                    .decode()
                    .ok()?;
                // ratatui-image's iTerm2 path encodes JPEG (no alpha) and only blends the
                // padding, so flatten transparency onto the terminal background ourselves.
                let bg = picker.background_color.map_or([0, 0, 0], |c| c.0);
                let img = if img.color().has_alpha() {
                    image::DynamicImage::ImageRgb8(composite_over(&img.to_rgba8(), bg))
                } else {
                    img
                };
                Some(picker.new_resize_protocol(img))
            })
            .as_mut()
    }

    fn render_toc(&self, f: &mut Frame, state: &AppState, area: Rect) {
        let headings = &state.current_document.rendered.headings;
        let items: Vec<ListItem> = headings
            .iter()
            .enumerate()
            .map(|(idx, h)| {
                let indent = "  ".repeat(h.level.saturating_sub(1) as usize);
                let text = format!("{}{}", indent, h.text);
                let style = if idx == state.toc_cursor {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(text).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().title(" TOC ").borders(Borders::ALL))
            .style(Style::default().fg(Color::White));

        f.render_widget(list, area);
    }

    fn render_status_bar(&self, f: &mut Frame, state: &AppState, area: Rect) {
        let file_name = state
            .current_location
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".to_string());

        let line_count = state.current_document.rendered.lines.len();
        let (visible_start, visible_end) = visible_range(
            state.scroll_offset as usize,
            state.viewport_height as usize,
            line_count,
        );

        let cursor_info = if let Some(c) = state.cursor_line {
            format!(" | Cursor: {}", c + 1)
        } else {
            String::new()
        };

        let status = if state.mode == crate::app::state::AppMode::Search {
            format!("/{}", state.search_input)
        } else if let Some(msg) = &state.status_message {
            msg.clone()
        } else {
            format!(
                "{} | {}-{}/{} lines{} | Mode: {:?}",
                file_name, visible_start, visible_end, line_count, cursor_info, state.mode
            )
        };

        let paragraph = Paragraph::new(status)
            .style(Style::default().bg(Color::DarkGray).fg(Color::White))
            .alignment(Alignment::Left);

        f.render_widget(paragraph, area);
    }
}

/// Horizontal split: [TOC, text] when the TOC is visible, else [text].
fn split_columns(area: Rect, toc_visible: bool) -> std::rc::Rc<[Rect]> {
    let constraints: &[Constraint] = if toc_visible {
        &[Constraint::Percentage(30), Constraint::Percentage(70)]
    } else {
        &[Constraint::Percentage(100)]
    };
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area)
}

/// Width of the document text area for a `cols`-wide terminal (same split as `render`).
pub fn text_area_width(cols: u16, toc_visible: bool) -> u16 {
    let chunks = split_columns(Rect::new(0, 0, cols, 1), toc_visible);
    chunks[chunks.len() - 1].width
}

/// 1-based inclusive range of document lines shown in a `viewport`-row view.
fn visible_range(scroll: usize, viewport: usize, total: usize) -> (usize, usize) {
    let end = (scroll + viewport).min(total);
    ((scroll + 1).min(end), end)
}

/// Splits `line` so the char ranges in `ranges` carry `Modifier::REVERSED`.
fn highlight_ranges(line: &Line<'static>, ranges: &[(usize, usize)]) -> Line<'static> {
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut pos = 0usize;
    for span in &line.spans {
        let chars: Vec<char> = span.content.chars().collect();
        let (start, end) = (pos, pos + chars.len());
        let mut cur = 0usize;
        let push = |from: usize, to: usize, rev: bool, out: &mut Vec<Span<'static>>| {
            if from < to {
                let text: String = chars[from..to].iter().collect();
                let style = if rev {
                    span.style.add_modifier(Modifier::REVERSED)
                } else {
                    span.style
                };
                out.push(Span::styled(text, style));
            }
        };
        for &(ms, me) in ranges {
            let (ms, me) = (ms.max(start), me.min(end));
            if ms >= me {
                continue;
            }
            push(cur, ms - start, false, &mut out);
            push(ms - start, me - start, true, &mut out);
            cur = me - start;
        }
        push(cur, chars.len(), false, &mut out);
        pos = end;
    }
    Line::from(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_lines_fit_text_area_with_toc_open() {
        let w = text_area_width(80, true);
        assert!(w < 80);
        assert_eq!(text_area_width(80, false), 80);
        let src =
            "word ".repeat(100) + "\n\n> " + &"quote ".repeat(40) + "\n\n- " + &"item ".repeat(40);
        let doc =
            crate::document::Document::from_source(src, std::path::PathBuf::from("t.md"), Some(w));
        for l in &doc.rendered.lines {
            let n: usize = l.spans.iter().map(|s| s.content.chars().count()).sum();
            assert!(n <= w as usize, "line of {n} exceeds text area {w}");
        }
    }

    #[test]
    fn visible_range_uses_viewport() {
        assert_eq!(visible_range(0, 38, 85), (1, 38));
        assert_eq!(visible_range(35, 38, 85), (36, 73));
        assert_eq!(visible_range(60, 38, 85), (61, 85));
        assert_eq!(visible_range(0, 38, 10), (1, 10));
        assert_eq!(visible_range(0, 38, 0), (0, 0));
    }
}

/// Standard "over" blend of `rgba` onto a solid background colour.
pub fn composite_over(rgba: &image::RgbaImage, bg: [u8; 3]) -> image::RgbImage {
    image::RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let p = rgba.get_pixel(x, y).0;
        let a = p[3] as u32;
        let mix = |fg: u8, back: u8| ((fg as u32 * a + back as u32 * (255 - a) + 127) / 255) as u8;
        image::Rgb([mix(p[0], bg[0]), mix(p[1], bg[1]), mix(p[2], bg[2])])
    })
}

#[cfg(test)]
mod composite_tests {
    use super::*;

    #[test]
    fn transparent_becomes_background() {
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([200, 10, 10, 0]));
        assert_eq!(
            composite_over(&img, [30, 40, 50]).get_pixel(1, 1).0,
            [30, 40, 50]
        );
    }

    #[test]
    fn opaque_is_unchanged() {
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([200, 10, 90, 255]));
        assert_eq!(
            composite_over(&img, [30, 40, 50]).get_pixel(0, 0).0,
            [200, 10, 90]
        );
    }

    #[test]
    fn half_alpha_is_midpoint() {
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([200, 100, 0, 128]));
        let p = composite_over(&img, [0, 100, 200]).get_pixel(0, 0).0;
        for got in p {
            assert!((got as i32 - 100).abs() <= 1, "{p:?}");
        }
    }
}
