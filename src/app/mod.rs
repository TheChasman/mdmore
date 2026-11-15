pub mod state;

use crate::document::Document;
use crate::fetch::Fetcher;
use crate::ui::Ui;
use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::path::PathBuf;
use std::io;

pub struct App {
    state: state::AppState,
    ui: Ui,
    fetcher: Fetcher,
}

impl App {
    pub async fn new(initial_path: PathBuf) -> Result<Self> {
        let document = Document::from_file(&initial_path).await?;
        Self::with_document(initial_path, document)
    }

    pub fn with_document(location: PathBuf, document: Document) -> Result<Self> {
        let mut state = state::AppState::new(location.clone(), document.clone());
        if location.as_os_str() == crate::document::STDIN_PATH {
            state.stdin_document = Some(document);
        }
        let ui = Ui::new()?;
        let fetcher = Fetcher::new();

        Ok(Self { state, ui, fetcher })
    }

    pub async fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        let result = self.event_loop(&mut terminal).await;

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    async fn event_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        use crossterm::event::{self, Event, KeyCode, KeyModifiers};
        use std::time::Duration;

        let mut terminal_size = crossterm::terminal::size()?;
        self.state.viewport_height = terminal_size.1.saturating_sub(2);

        loop {
            terminal_size = crossterm::terminal::size()?;
            self.state.viewport_height = terminal_size.1.saturating_sub(2);
            terminal.draw(|f| self.ui.render(f, &self.state))?;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Up => {
                            if !self.state.is_address_bar_active() {
                                if self.state.toc_visible {
                                    self.state.toc_cursor_up();
                                } else {
                                    self.state.cursor_up();
                                }
                            }
                        }
                        KeyCode::Down => {
                            if !self.state.is_address_bar_active() {
                                if self.state.toc_visible {
                                    self.state.toc_cursor_down();
                                } else {
                                    self.state.cursor_down();
                                }
                            }
                        }
                        KeyCode::Left => {
                            if !self.state.is_address_bar_active() && self.state.cursor_line.is_some() {
                                self.state.cursor_to_prev_link();
                            }
                        }
                        KeyCode::Right => {
                            if !self.state.is_address_bar_active() && self.state.cursor_line.is_some() {
                                self.state.cursor_to_next_link();
                            }
                        }
                        KeyCode::Char('j') => {
                            if !self.state.is_address_bar_active() && self.state.cursor_line.is_none() && !self.state.toc_visible {
                                self.state.scroll_down();
                            }
                        }
                        KeyCode::Char('k') => {
                            if !self.state.is_address_bar_active() && self.state.cursor_line.is_none() && !self.state.toc_visible {
                                self.state.scroll_up();
                            }
                        }
                        KeyCode::Char(' ') => self.state.page_down(),
                        KeyCode::PageDown => self.state.page_down(),
                        KeyCode::Char('b') => self.state.page_up(),
                        KeyCode::PageUp => self.state.page_up(),
                        KeyCode::Char('g') if matches!(key.modifiers, KeyModifiers::NONE) => {
                            if let Event::Key(next_key) = event::read()? {
                                if matches!(next_key.code, KeyCode::Char('g')) {
                                    self.state.jump_to_start();
                                }
                            }
                        }
                        KeyCode::Char('G') => self.state.jump_to_end(),
                        KeyCode::Char('t') => self.state.toggle_toc(),
                        KeyCode::Char('f') => self.state.go_forward().await?,
                        KeyCode::Char(':') => self.state.enter_address_bar(),
                        KeyCode::Char(c) if self.state.is_address_bar_active() => {
                            self.state.address_input.push(c);
                        }
                        KeyCode::Backspace if self.state.is_address_bar_active() => {
                            self.state.address_input.pop();
                        }
                        KeyCode::Backspace => self.state.go_back().await?,
                        KeyCode::Enter => {
                            if self.state.is_address_bar_active() {
                                let input = self.state.address_input.clone();
                                self.state.address_input.clear();
                                self.state.exit_address_bar();
                                let path = PathBuf::from(&input);
                                if path.exists() {
                                    if let Ok(doc) = Document::from_file(&path).await {
                                        self.state.open_document(path, doc).await;
                                    }
                                }
                            } else if self.state.toc_visible {
                                self.state.jump_to_heading();
                            } else if let Some(line) = self.state.cursor_line {
                                if let Some(link) = self.state.current_document.rendered.links.iter().find(|l| l.line == line) {
                                    let path = PathBuf::from(&link.url);
                                    if path.exists() {
                                        if let Ok(doc) = Document::from_file(&path).await {
                                            self.state.open_document(path, doc).await;
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Esc => {
                            if self.state.toc_visible {
                                self.state.toc_visible = false;
                            } else if self.state.cursor_line.is_some() {
                                self.state.cursor_line = None;
                            } else if self.state.is_address_bar_active() {
                                self.state.address_input.clear();
                                self.state.exit_address_bar();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }
}
