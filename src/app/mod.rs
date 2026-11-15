pub mod state;

use crate::document::Document;
use crate::ui::Ui;
use anyhow::Result;
use crossterm::{
    execute,
    cursor::Show,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::path::PathBuf;
use std::io;

pub struct App {
    state: state::AppState,
    ui: Ui,
    /// Generation whose highlight job has been spawned (None = nothing spawned yet).
    highlight_spawned_for: Option<u64>,
    /// Whether the image picker query has run (it runs at most once).
    picker_tried: bool,
    highlight_tx: std::sync::mpsc::Sender<(u64, crate::document::RenderedDoc)>,
    highlight_rx: std::sync::mpsc::Receiver<(u64, crate::document::RenderedDoc)>,
}

/// Builds the image picker. Must run after entering the alternate screen and before
/// reading events. ratatui-image's terminal query can leave stdin non-blocking and
/// with altered termios when the terminal doesn't reply, which breaks crossterm's
/// reads, so both are restored here.
fn detect_picker() -> Option<ratatui_image::picker::Picker> {
    let stdin = rustix::stdio::stdin();
    let flags = rustix::fs::fcntl_getfl(stdin).ok();
    let termios = rustix::termios::tcgetattr(stdin).ok();
    let picker = ratatui_image::picker::Picker::from_termios().ok().map(|mut p| {
        p.guess_protocol();
        p
    });
    if let Some(flags) = flags {
        let _ = rustix::fs::fcntl_setfl(stdin, flags);
    }
    if let Some(t) = termios {
        let _ = rustix::termios::tcsetattr(stdin, rustix::termios::OptionalActions::Now, &t);
    }
    picker
}

/// Restores the terminal on drop: raw mode off, main screen, cursor shown. Errors are ignored.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// Best-effort terminal restore, shared by the guard and the panic hook.
pub fn restore_terminal() {
    // Only touch the terminal if the pager set it up (print mode never enters raw mode).
    if !crossterm::terminal::is_raw_mode_enabled().unwrap_or(false) {
        return;
    }
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
}

/// True when the graphics picker should be set up now: not yet tried and the
/// document has images. Documents without images never run the terminal query.
fn needs_picker(tried: bool, doc: &Document) -> bool {
    !tried && !doc.rendered.images.is_empty()
}

impl App {
    pub fn new(initial_path: PathBuf) -> Result<Self> {
        let width = crossterm::terminal::size().ok().map(|(cols, _)| cols);
        let document = Document::from_file(&initial_path, width)?;
        Self::with_document(initial_path, document)
    }

    /// `document` is re-wrapped to the terminal width if it isn't already.
    pub fn with_document(location: PathBuf, mut document: Document) -> Result<Self> {
        document.set_width(crossterm::terminal::size().ok().map(|(cols, _)| cols));
        let mut state = state::AppState::new(location.clone(), document.clone());
        if location.as_os_str() == crate::document::STDIN_PATH {
            state.stdin_document = Some(document);
        }
        let ui = Ui::new()?;

        let (highlight_tx, highlight_rx) = std::sync::mpsc::channel();

        Ok(Self {
            state,
            ui,
            highlight_spawned_for: None,
            picker_tried: false,
            highlight_tx,
            highlight_rx,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        // From here on every exit path (error, early return, unwind) restores the terminal.
        let _guard = TerminalGuard;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        self.event_loop(&mut terminal)
    }

    /// Runs the graphics query once, the first time a document with images is shown.
    /// Returns true if the query ran.
    fn ensure_picker(&mut self) -> Result<bool> {
        if !needs_picker(self.picker_tried, &self.state.current_document) {
            return Ok(false);
        }
        self.picker_tried = true;
        self.ui.picker = detect_picker();
        // A terminal that answered late can leave reply bytes queued as input; drop them.
        while crossterm::event::poll(std::time::Duration::ZERO)? {
            let _ = crossterm::event::read()?;
        }
        Ok(true)
    }

    fn event_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        use crossterm::event::{self, Event, KeyCode, KeyModifiers};
        use std::time::Duration;

        let mut terminal_size = crossterm::terminal::size()?;
        self.state.viewport_height = terminal_size.1.saturating_sub(1);

        // Draw only when something changed: first frame, any input or resize event,
        // or an applied highlight. Idle writes nothing to the terminal.
        let mut dirty = true;
        loop {
            terminal_size = crossterm::terminal::size()?;
            // Document rows: everything above the 1-row status bar.
            self.state.viewport_height = terminal_size.1.saturating_sub(1);
            // Wrap to the text area beside the TOC (when shown), using the UI's
            // own split. A width change (resize or TOC toggle) bumps the
            // generation, so the highlight re-runs and stale results drop.
            let text_width = crate::ui::text_area_width(terminal_size.0, self.state.toc_visible);
            if self.state.set_render_width(Some(text_width)) {
                dirty = true;
            }
            if dirty {
                terminal.draw(|f| self.ui.render(f, &self.state))?;
                dirty = false;
            }
            // After the frame, so a slow graphics query never delays the first paint
            // (alt text shows until it finishes); redraw if it ran.
            if self.ensure_picker()? {
                dirty = true;
            }

            // Highlight just after the frame is on screen; never block on the job.
            while let Ok((generation, rendered)) = self.highlight_rx.try_recv() {
                state::apply_highlight(&mut self.state, generation, rendered);
                dirty = true;
            }
            let generation = self.state.generation;
            if self.highlight_spawned_for != Some(generation) {
                self.highlight_spawned_for = Some(generation);
                if self.state.current_document.needs_highlight() {
                    let doc = self.state.current_document.clone();
                    let tx = self.highlight_tx.clone();
                    std::thread::spawn(move || {
                        let _ = tx.send((generation, doc.highlighted()));
                    });
                }
            }

            if event::poll(Duration::from_millis(16))? {
                let event = event::read()?;
                if matches!(event, Event::Key(_) | Event::Resize(..)) {
                    dirty = true;
                }
                if let Event::Key(key) = event {
                    self.state.status_message = None;
                    // Search-mode handling comes before every other key arm.
                    if self.state.mode == state::AppMode::Search {
                        match key.code {
                            KeyCode::Esc => self.state.exit_search(),
                            KeyCode::Enter => {
                                let pattern = self.state.search_input.clone();
                                self.state.exit_search();
                                if !pattern.is_empty() {
                                    self.state.run_search(&pattern);
                                }
                            }
                            KeyCode::Backspace => {
                                if self.state.search_input.pop().is_none() {
                                    self.state.exit_search();
                                }
                            }
                            KeyCode::Char(c) => self.state.search_input.push(c),
                            _ => {}
                        }
                        continue;
                    }
                    let plain_g = matches!(key.code, KeyCode::Char('g')) && key.modifiers == KeyModifiers::NONE;
                    if !plain_g {
                        self.state.pending_g = false;
                    }
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Up => {
                            if self.state.toc_visible {
                                self.state.toc_cursor_up();
                            } else {
                                self.state.cursor_up();
                            }
                        }
                        KeyCode::Down => {
                            if self.state.toc_visible {
                                self.state.toc_cursor_down();
                            } else {
                                self.state.cursor_down();
                            }
                        }
                        KeyCode::Left => {
                            if self.state.cursor_line.is_some() {
                                self.state.cursor_to_prev_link();
                            }
                        }
                        KeyCode::Right => {
                            if self.state.cursor_line.is_some() {
                                self.state.cursor_to_next_link();
                            }
                        }
                        KeyCode::Char('j') => {
                            if self.state.cursor_line.is_none() && !self.state.toc_visible {
                                self.state.scroll_down();
                            }
                        }
                        KeyCode::Char('k') => {
                            if self.state.cursor_line.is_none() && !self.state.toc_visible {
                                self.state.scroll_up();
                            }
                        }
                        KeyCode::Char(' ') => self.state.page_down(),
                        KeyCode::PageDown => self.state.page_down(),
                        KeyCode::Char('b') => self.state.page_up(),
                        KeyCode::PageUp => self.state.page_up(),
                        KeyCode::Char('g') if plain_g => self.state.press_g(),
                        KeyCode::Char('G') => self.state.jump_to_end(),
                        KeyCode::Char('t') => self.state.toggle_toc(),
                        KeyCode::Char('f') => self.state.go_forward()?,
                        KeyCode::Char('/') => self.state.enter_search(),
                        KeyCode::Char('n') => self.state.search_next(),
                        KeyCode::Char('N') => self.state.search_prev(),
                        KeyCode::Backspace => self.state.go_back()?,
                        KeyCode::Enter => {
                            if self.state.toc_visible {
                                self.state.jump_to_heading();
                            } else if let Some(line) = self.state.cursor_line {
                                if let Some(link) = self.state.current_document.rendered.links.iter().find(|l| l.line == line) {
                                    let path = PathBuf::from(&link.url);
                                    if path.exists() {
                                        if let Ok(doc) = Document::from_file(&path, self.state.render_width) {
                                            self.state.open_document(path, doc);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_only_for_documents_with_images() {
        let plain = Document::from_source("# Hi\n\ntext".into(), PathBuf::from("x.md"), None);
        let md = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("artwork/icon/x.md");
        let with_img = Document::from_source("![alt](RustFrog.png)".into(), md, None);
        assert!(!needs_picker(false, &plain));
        assert!(needs_picker(false, &with_img));
        assert!(!needs_picker(true, &with_img));
    }
}
