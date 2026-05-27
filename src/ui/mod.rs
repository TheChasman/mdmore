use crate::app::state::AppState;
use anyhow::Result;
use crossterm::terminal::size;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub struct Ui {
    pub width: u16,
    pub height: u16,
}

impl Ui {
    pub fn new() -> Result<Self> {
        let (width, height) = size()?;
        Ok(Self { width, height })
    }

    pub fn render(&self, f: &mut Frame, state: &AppState) {
        let chunks = if state.toc_visible {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(f.area())
        } else {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(100)])
                .split(f.area())
        };

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

        if state.mode == crate::app::state::AppMode::AddressBar {
            self.render_address_bar(f, state, main_area);
        }
    }

    fn render_document(&self, f: &mut Frame, state: &AppState, area: Rect) {
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
                            let highlighted_style = Style::default()
                                .bg(Color::White)
                                .fg(Color::Black);
                            highlighted_spans.push(Span::styled(
                                span.content.clone(),
                                highlighted_style,
                            ));
                        }
                        return Line::from(highlighted_spans);
                    }
                }
                line.clone()
            })
            .collect();

        let paragraph = Paragraph::new(visible_lines)
            .block(Block::default().borders(Borders::NONE))
            .alignment(Alignment::Left);

        f.render_widget(paragraph, area);
    }

    fn render_toc(&self, f: &mut Frame, state: &AppState, area: Rect) {
        let headings = &state.current_document.rendered.headings;
        let items: Vec<ListItem> = headings
            .iter()
            .enumerate()
            .map(|(idx, h)| {
                let indent = "  ".repeat((h.level - 1).max(0) as usize);
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
        let file_name = state.current_location
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".to_string());

        let line_count = state.current_document.rendered.lines.len();
        let visible_end = (state.scroll_offset as usize + area.height as usize).min(line_count);
        
        let cursor_info = if let Some(c) = state.cursor_line {
            format!(" | Cursor: {}", c + 1)
        } else {
            String::new()
        };

        let status = format!("{} | {}-{}/{} lines{} | Mode: {:?}", 
            file_name,
            state.scroll_offset as usize + 1,
            visible_end,
            line_count,
            cursor_info,
            state.mode
        );

        let paragraph = Paragraph::new(status)
            .style(Style::default().bg(Color::DarkGray).fg(Color::White))
            .alignment(Alignment::Left);

        f.render_widget(paragraph, area);
    }

    fn render_address_bar(&self, f: &mut Frame, state: &AppState, area: Rect) {
        let bar_height = 3;
        let bar_area = Rect {
            x: area.x,
            y: area.y + area.height.saturating_sub(bar_height),
            width: area.width,
            height: bar_height,
        };

        let prompt = "Enter path: ";
        let content = format!("{}{}", prompt, state.address_input);
        let paragraph = Paragraph::new(content)
            .block(Block::default().borders(Borders::ALL).title("Address Bar"))
            .style(Style::default().fg(Color::White).bg(Color::Black));

        f.render_widget(paragraph, bar_area);
    }
}
