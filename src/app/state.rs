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
        }
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn page_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(self.viewport_height);
    }

    pub fn page_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(self.viewport_height);
    }

    pub fn jump_to_start(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn jump_to_end(&mut self) {
        let line_count = self.current_document.lines.len() as u16;
        self.scroll_offset = line_count.saturating_sub(self.viewport_height);
    }

    pub fn toggle_toc(&mut self) {
        self.toc_visible = !self.toc_visible;
    }

    pub async fn go_back(&mut self) -> anyhow::Result<()> {
        if self.nav_index > 0 {
            self.nav_index -= 1;
            let path = self.nav_stack[self.nav_index].clone();
            let doc = Document::from_file(&path).await?;
            self.current_location = path;
            self.current_document = doc;
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
        }
        Ok(())
    }

    pub async fn open_document(&mut self, location: PathBuf, document: Document) {
        self.nav_stack.truncate(self.nav_index + 1);
        self.nav_stack.push(location.clone());
        self.nav_index = self.nav_stack.len() - 1;
        self.current_location = location;
        self.current_document = document;
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
}
