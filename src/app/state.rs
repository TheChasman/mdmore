use crate::document::Document;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AppMode {
    Reading,
    AddressBar,
    Error,
}

pub struct AppState {
    pub mode: AppMode,
    pub current_location: PathBuf,
    pub current_document: Document,
    pub nav_stack: Vec<PathBuf>,
    pub nav_index: usize,
    pub toc_visible: bool,
    pub address_input: String,
    pub error_message: Option<String>,
    pub scroll_offset: u16,
    pub viewport_height: u16,
    pub cursor_line: Option<usize>,  // None = cursor dormant, Some(n) = active on line n
    pub toc_cursor: usize,           // selected heading index when TOC visible
}

impl AppState {
    pub fn new(location: PathBuf, document: Document) -> Self {
        Self {
            mode: AppMode::Reading,
            current_location: location.clone(),
            current_document: document,
            nav_stack: vec![location],
            nav_index: 0,
            toc_visible: false,
            address_input: String::new(),
            error_message: None,
            scroll_offset: 0,
            viewport_height: 20,
            cursor_line: None,
            toc_cursor: 0,
        }
    }

    pub fn scroll_down(&mut self) {
        let line_count = self.current_document.rendered.lines.len() as u16;
        let max_offset = line_count.saturating_sub(self.viewport_height);
        self.scroll_offset = self.scroll_offset.saturating_add(1).min(max_offset);
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn page_down(&mut self) {
        let line_count = self.current_document.rendered.lines.len() as u16;
        let max_offset = line_count.saturating_sub(self.viewport_height);
        self.scroll_offset = self.scroll_offset.saturating_add(self.viewport_height).min(max_offset);
    }

    pub fn page_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(self.viewport_height);
    }

    pub fn jump_to_start(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn jump_to_end(&mut self) {
        let line_count = self.current_document.rendered.lines.len() as u16;
        self.scroll_offset = line_count.saturating_sub(self.viewport_height);
    }

    pub fn toggle_toc(&mut self) {
        self.toc_visible = !self.toc_visible;
        self.toc_cursor = 0;
    }

    pub async fn go_back(&mut self) -> anyhow::Result<()> {
        if self.nav_index > 0 {
            self.nav_index -= 1;
            let path = self.nav_stack[self.nav_index].clone();
            let doc = Document::from_file(&path).await?;
            self.current_location = path;
            self.current_document = doc;
            self.cursor_line = None;
            self.scroll_offset = 0;
        }
        Ok(())
    }

    pub async fn go_forward(&mut self) -> anyhow::Result<()> {
        if self.nav_index < self.nav_stack.len() - 1 {
            self.nav_index += 1;
            let path = self.nav_stack[self.nav_index].clone();
            let doc = Document::from_file(&path).await?;
            self.current_location = path;
            self.current_document = doc;
            self.cursor_line = None;
            self.scroll_offset = 0;
        }
        Ok(())
    }

    pub async fn open_document(&mut self, location: PathBuf, document: Document) {
        self.nav_stack.truncate(self.nav_index + 1);
        self.nav_stack.push(location.clone());
        self.nav_index = self.nav_stack.len() - 1;
        self.current_location = location;
        self.current_document = document;
        self.cursor_line = None;
        self.scroll_offset = 0;
    }

    pub fn enter_address_bar(&mut self) {
        self.mode = AppMode::AddressBar;
        self.address_input.clear();
    }

    pub fn exit_address_bar(&mut self) {
        self.mode = AppMode::Reading;
    }

    pub fn is_address_bar_active(&self) -> bool {
        self.mode == AppMode::AddressBar
    }

    pub fn cursor_down(&mut self) {
        let line_count = self.current_document.rendered.lines.len();
        if let Some(line) = self.cursor_line {
            let new_line = (line + 1).min(line_count.saturating_sub(1));
            self.cursor_line = Some(new_line);
            self.auto_scroll_cursor();
        } else {
            self.cursor_line = Some(self.scroll_offset as usize);
        }
    }

    pub fn cursor_up(&mut self) {
        if let Some(line) = self.cursor_line {
            let new_line = line.saturating_sub(1);
            self.cursor_line = Some(new_line);
            self.auto_scroll_cursor();
        } else {
            self.cursor_line = Some(self.scroll_offset as usize);
        }
    }

    pub fn auto_scroll_cursor(&mut self) {
        if let Some(line) = self.cursor_line {
            let line_u16 = line as u16;
            if line_u16 < self.scroll_offset {
                self.scroll_offset = line_u16;
            } else if line_u16 >= self.scroll_offset + self.viewport_height {
                self.scroll_offset = line_u16.saturating_sub(self.viewport_height - 1);
            }
        }
    }

    pub fn cursor_to_prev_link(&mut self) {
        if let Some(line) = self.cursor_line {
            for link in self.current_document.rendered.links.iter().rev() {
                if link.line < line {
                    self.cursor_line = Some(link.line);
                    self.auto_scroll_cursor();
                    return;
                }
            }
        }
    }

    pub fn cursor_to_next_link(&mut self) {
        if let Some(line) = self.cursor_line {
            for link in &self.current_document.rendered.links {
                if link.line > line {
                    self.cursor_line = Some(link.line);
                    self.auto_scroll_cursor();
                    return;
                }
            }
        }
    }

    pub fn toc_cursor_down(&mut self) {
        let heading_count = self.current_document.rendered.headings.len();
        if heading_count > 0 {
            self.toc_cursor = (self.toc_cursor + 1) % heading_count;
        }
    }

    pub fn toc_cursor_up(&mut self) {
        let heading_count = self.current_document.rendered.headings.len();
        if heading_count > 0 {
            self.toc_cursor = if self.toc_cursor == 0 {
                heading_count - 1
            } else {
                self.toc_cursor - 1
            };
        }
    }

    pub fn jump_to_heading(&mut self) {
        if let Some(heading) = self.current_document.rendered.headings.get(self.toc_cursor) {
            self.scroll_offset = heading.line as u16;
            self.toc_visible = false;
            self.cursor_line = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, parser, renderer};
    use std::path::PathBuf;

    fn create_test_document() -> Document {
        let content = "Line 1\n\nLine 2\n\nLine 3".to_string();
        let blocks = parser::parse(&content);
        let rendered = renderer::render(&blocks);
        Document {
            path: PathBuf::from("test.md"),
            content,
            rendered,
        }
    }

    #[test]
    fn cursor_clamps_at_end() {
        let mut state = AppState::new(PathBuf::from("test.md"), create_test_document());
        state.cursor_line = Some(state.current_document.rendered.lines.len() - 1);
        let original = state.cursor_line;
        state.cursor_down();
        // Should not move past end
        assert_eq!(state.cursor_line, original);
    }

    #[test]
    fn cursor_wakes_on_up_down() {
        let mut state = AppState::new(PathBuf::from("test.md"), create_test_document());
        assert_eq!(state.cursor_line, None);
        state.cursor_down();
        // Cursor should wake
        assert!(state.cursor_line.is_some());
    }

    #[test]
    fn scroll_clamps_at_end() {
        let mut state = AppState::new(PathBuf::from("test.md"), create_test_document());
        state.viewport_height = 5;
        let line_count = state.current_document.rendered.lines.len() as u16;
        state.scroll_offset = 1000; // Way past end
        state.scroll_down();
        // Should be clamped
        let max_offset = line_count.saturating_sub(state.viewport_height);
        assert!(state.scroll_offset <= max_offset);
    }

    #[test]
    fn toc_cursor_wraps() {
        let mut state = AppState::new(PathBuf::from("test.md"), create_test_document());
        state.toc_cursor = 0;
        state.toc_cursor_up();
        // Should wrap to end if there are headings
        let heading_count = state.current_document.rendered.headings.len();
        if heading_count > 0 {
            assert_eq!(state.toc_cursor, heading_count - 1);
        }
    }
}
