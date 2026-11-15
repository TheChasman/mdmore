use crate::document::{Document, STDIN_PATH};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AppMode {
    Reading,
    Search,
    Error,
}

pub struct AppState {
    pub mode: AppMode,
    pub current_location: PathBuf,
    pub current_document: Document,
    pub nav_stack: Vec<PathBuf>,
    pub nav_index: usize,
    pub toc_visible: bool,
    pub search_input: String,
    pub search_pattern: Option<String>,
    pub search_matches: Vec<(usize, usize, usize)>, // (line, char_start, char_end)
    pub search_index: Option<usize>,
    pub status_message: Option<String>,
    pub error_message: Option<String>,
    pub scroll_offset: u16,
    pub viewport_height: u16,
    pub cursor_line: Option<usize>,  // None = cursor dormant, Some(n) = active on line n
    pub toc_cursor: usize,           // selected heading index when TOC visible
    pub generation: u64,             // increments on every document change
    pub stdin_document: Option<Document>, // set when the initial document came from stdin
    /// A lone `g` was pressed; a second `g` jumps to the start, any other key clears it.
    pub pending_g: bool,
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
            search_input: String::new(),
            search_pattern: None,
            search_matches: Vec::new(),
            search_index: None,
            status_message: None,
            error_message: None,
            scroll_offset: 0,
            viewport_height: 20,
            cursor_line: None,
            toc_cursor: 0,
            generation: 0,
            stdin_document: None,
            pending_g: false,
        }
    }

    fn load_location(&self, path: &std::path::Path) -> anyhow::Result<Document> {
        if path.as_os_str() == STDIN_PATH {
            if let Some(doc) = &self.stdin_document {
                return Ok(doc.clone());
            }
        }
        Document::from_file(path)
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

    /// Handles a plain `g`: the second of `gg` jumps to the start. Never blocks.
    pub fn press_g(&mut self) {
        if self.pending_g {
            self.pending_g = false;
            self.jump_to_start();
        } else {
            self.pending_g = true;
        }
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

    pub fn go_back(&mut self) -> anyhow::Result<()> {
        if self.nav_index > 0 {
            self.nav_index -= 1;
            let path = self.nav_stack[self.nav_index].clone();
            let doc = self.load_location(&path)?;
            self.current_location = path;
            self.current_document = doc;
            self.generation += 1;
            self.clear_search();
            self.cursor_line = None;
            self.scroll_offset = 0;
        }
        Ok(())
    }

    pub fn go_forward(&mut self) -> anyhow::Result<()> {
        if self.nav_index < self.nav_stack.len() - 1 {
            self.nav_index += 1;
            let path = self.nav_stack[self.nav_index].clone();
            let doc = self.load_location(&path)?;
            self.current_location = path;
            self.current_document = doc;
            self.generation += 1;
            self.clear_search();
            self.cursor_line = None;
            self.scroll_offset = 0;
        }
        Ok(())
    }

    pub fn open_document(&mut self, location: PathBuf, document: Document) {
        self.nav_stack.truncate(self.nav_index + 1);
        self.nav_stack.push(location.clone());
        self.nav_index = self.nav_stack.len() - 1;
        self.current_location = location;
        self.current_document = document;
        self.generation += 1;
        self.clear_search();
        self.cursor_line = None;
        self.scroll_offset = 0;
    }

    pub fn enter_search(&mut self) {
        self.mode = AppMode::Search;
        self.search_input.clear();
    }

    fn clear_search(&mut self) {
        self.search_pattern = None;
        self.search_matches.clear();
        self.search_index = None;
    }

    pub fn exit_search(&mut self) {
        self.mode = AppMode::Reading;
        self.search_input.clear();
    }

    /// Case-insensitive substring search over each line's plain text.
    /// Plain text is identical before and after syntax highlighting.
    /// Upgrade path: regex patterns, if ever wanted.
    pub fn run_search(&mut self, pattern: &str) {
        self.search_matches.clear();
        self.search_index = None;
        self.search_pattern = Some(pattern.to_string());
        let needle: Vec<char> = pattern.chars().map(fold).collect();
        if !needle.is_empty() {
            for (i, line) in self.current_document.rendered.lines.iter().enumerate() {
                let hay: Vec<char> = line
                    .spans
                    .iter()
                    .flat_map(|s| s.content.chars())
                    .map(fold)
                    .collect();
                let mut at = 0;
                while at + needle.len() <= hay.len() {
                    if hay[at..at + needle.len()] == needle[..] {
                        self.search_matches.push((i, at, at + needle.len()));
                        at += needle.len();
                    } else {
                        at += 1;
                    }
                }
            }
        }
        if self.search_matches.is_empty() {
            self.status_message = Some("Pattern not found".to_string());
            return;
        }
        let top = self.scroll_offset as usize;
        let first = self
            .search_matches
            .iter()
            .position(|m| m.0 >= top)
            .unwrap_or(0);
        self.search_index = Some(first);
        self.scroll_to_match(first);
    }

    pub fn search_next(&mut self) {
        let n = self.search_matches.len();
        if n == 0 {
            return;
        }
        let i = self.search_index.map_or(0, |i| (i + 1) % n);
        self.search_index = Some(i);
        self.scroll_to_match(i);
    }

    pub fn search_prev(&mut self) {
        let n = self.search_matches.len();
        if n == 0 {
            return;
        }
        let i = self.search_index.map_or(n - 1, |i| (i + n - 1) % n);
        self.search_index = Some(i);
        self.scroll_to_match(i);
    }

    fn scroll_to_match(&mut self, idx: usize) {
        let line = self.search_matches[idx].0;
        let top = self.scroll_offset as usize;
        let h = self.viewport_height as usize;
        if line < top || line >= top + h.max(1) {
            let max_offset = self.current_document.rendered.lines.len().saturating_sub(h);
            self.scroll_offset = line.min(max_offset).min(u16::MAX as usize) as u16;
        }
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

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Applies a background highlight result if it belongs to the current document.
/// Replaces only the lines; links and headings are unchanged by highlighting.
pub fn apply_highlight(state: &mut AppState, generation: u64, rendered: crate::document::RenderedDoc) -> bool {
    if generation != state.generation {
        return false;
    }
    state.current_document.rendered.lines = rendered.lines;
    true
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

    fn state_from(md: &str) -> AppState {
        let doc = Document::from_source(md.to_string(), PathBuf::from("test.md"));
        AppState::new(PathBuf::from("test.md"), doc)
    }

    #[test]
    fn search_finds_and_cycles() {
        let mut s = state_from("alpha\n\nbeta ALPHA\n\ngamma alpha");
        s.run_search("alpha");
        assert_eq!(s.search_matches.len(), 3);
        assert_eq!(s.search_index, Some(0));
        s.search_next(); assert_eq!(s.search_index, Some(1));
        s.search_next(); s.search_next(); assert_eq!(s.search_index, Some(0)); // wraps
        s.search_prev(); assert_eq!(s.search_index, Some(2));
    }

    #[test]
    fn search_no_match() {
        let mut s = state_from("alpha");
        let before = s.scroll_offset;
        s.run_search("zzz");
        assert!(s.search_matches.is_empty());
        assert_eq!(s.status_message.as_deref(), Some("Pattern not found"));
        assert_eq!(s.scroll_offset, before);
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
    fn gg_jumps_to_start_without_blocking() {
        let mut state = AppState::new(PathBuf::from("test.md"), create_test_document());
        state.scroll_offset = 3;
        state.press_g();
        assert!(state.pending_g);
        assert_eq!(state.scroll_offset, 3);
        state.press_g();
        assert!(!state.pending_g);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn search_match_near_end_is_clamped() {
        let src: String = (0..50).map(|i| format!("line{i}\n\n")).collect();
        let doc = Document::from_source(src, PathBuf::from("t.md"));
        let mut state = AppState::new(PathBuf::from("t.md"), doc);
        state.viewport_height = 10;
        state.run_search("line49");
        let max = state.current_document.rendered.lines.len() - 10;
        assert_eq!(state.scroll_offset as usize, max);
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

    #[test]
    fn stale_highlight_is_ignored() {
        let mut state = AppState::new(PathBuf::from("test.md"), create_test_document());
        let before = state.current_document.rendered.lines.len();
        let mut rd = state.current_document.rendered.clone();
        rd.lines.clear();
        state.generation = 2;
        assert!(!apply_highlight(&mut state, 1, rd.clone()));
        assert_eq!(state.current_document.rendered.lines.len(), before);
        assert!(apply_highlight(&mut state, 2, rd));
        assert_eq!(state.current_document.rendered.lines.len(), 0);
    }
}

#[cfg(test)]
mod stdin_tests {
    use super::*;
    use crate::document::STDIN_PATH;

    #[test]
    fn go_back_returns_to_stdin_document() {
        let stdin_doc = Document::from_source("# From stdin".into(), PathBuf::from(STDIN_PATH));
        let mut state = AppState::new(PathBuf::from(STDIN_PATH), stdin_doc.clone());
        state.stdin_document = Some(stdin_doc);
        let other = std::env::temp_dir().join("frogrust_go_back_test.md");
        std::fs::write(&other, "# Other").unwrap();
        let doc = Document::from_file(&other).unwrap();
        state.open_document(other.clone(), doc);
        state.go_back().unwrap();
        assert_eq!(state.current_location, PathBuf::from("-"));
        assert_eq!(state.current_document.rendered.headings[0].text, "From stdin");
    }
}
