# FrogRust — Product Requirements Document

## Overview
FrogRust is a terminal Markdown browser and viewer, written in Rust. A single-binary replacement for the Python Frogmouth, solving the dependency-rot and slow-startup problems. Fast, reliable, no runtime dependencies.

**Philosophy:** Compilation, small, fast.

---

## MVP Features (Priority Order)

### Tier 1: Minimum Viable (First Build)
1. **Open and render local .md files** — Display Markdown with syntax highlighting, links, bold, italic, code blocks, headings, lists.
2. **Navigate within a document** — Arrow keys, Page Up/Down, `j`/`k`, `space` to scroll. `gg` / `G` to jump to start/end.
3. **Table of Contents (TOC)** — Auto-extract headings from document. Sidebar toggle (`t`). Jump to heading via keyboard or mouse click.
4. **Back/Forward navigation** — History stack (not persisted yet). `b` and `f` keys.
5. **Quit cleanly** — `q` to exit, restore terminal state.

### Tier 2: Essential (Second Sprint)
6. **Follow links** — Click or `Enter` on a link to open a new document (local path or remote URL).
7. **Address bar** — `:` to open input field. Type path or URL, press `Enter`. Autocomplete local paths.
8. **GitHub shorthand** — `gh owner/repo` syntax to fetch README from GitHub (requires `reqwest` + async).
9. **Persistent bookmarks** — `m` to bookmark current page. Store as JSON in `~/.local/share/frogrust/bookmarks.json`. Load on startup.
10. **Persistent history** — Every visited document is logged. Search/reopen from history pane (available in Tier 3).

### Tier 3: Polish (Third Sprint)
11. **History sidebar** — Navigable list of visited documents. Search within history.
12. **Local file browser** — Browse filesystem without leaving app. Preview .md files, open with `Enter`.
13. **Syntax highlighting in code blocks** — Use `syntect` for fenced code blocks (language-aware).
14. **Light/dark theme toggle** — Respect system terminal palette; toggle with keyboard.
15. **Sixel/Kitty image support** — Render inline images in terminals that support it; fall back to alt-text.

---

## Data Model & Architecture

### Module Structure
```
src/
  main.rs                 — Entry point, CLI arg parsing (clap)
  app/
    state.rs             — AppState (modes, nav stack, current doc)
    event.rs             — Input and async task events (crossbeam channels)
  document/
    model.rs             — Document struct (path/url, content, AST)
    parser.rs            — pulldown-cmark → internal AST + metadata (headings, links)
    renderer.rs          — AST → Ratatui widget tree (Paragraph, Table, etc.)
  fetch/
    http.rs              — reqwest + tokio, GitHub shorthand resolver
    local.rs             — Read local files, Path validation
  storage/
    bookmarks.rs         — JSON persistence of bookmarks
    history.rs           — JSON persistence of history (optional for MVP)
  ui/
    layout.rs            — Ratatui frame layout (main doc + sidebars)
    widgets.rs           — Custom widgets (TOC sidebar, address bar, status line)
    events.rs            — Keyboard/mouse input handling
```

### Data Flow (Text Diagram)
```
User Input (keyboard/mouse)
    ↓
Event Loop (crossterm + ratatui)
    ↓
  ├→ Local input (navigation, quit, bookmark)
    │   → AppState update
    │   → Re-render UI
    │
  └→ Async trigger (open link, fetch URL, GitHub shorthand)
      → Spawn tokio task (fetch/parse)
      → Send result via channel
      → Event loop receives, updates AppState
      → Re-render UI

Document Pipeline:
  Location (path/URL) → Fetcher → Raw content → Parser (pulldown-cmark) 
    → AST + metadata → Renderer → Ratatui widgets → Terminal
```

---

## State Machine

```rust
enum AppMode {
    Reading,         // Displaying document, waiting for nav input
    AddressBar,      // User typing in address bar (:)
    BookmarkSave,    // Save current doc as bookmark (m)
    History,         // Browse history sidebar (open/search)
    TOC,             // TOC sidebar visible (toggle with t)
    Error,           // Display error message (e.g., fetch failed)
}

enum Location {
    Local(PathBuf),
    Remote(Url),
    GitHub { owner: String, repo: String, path: Option<String> },
}

struct AppState {
    mode: AppMode,
    current_location: Location,
    current_document: Document,
    nav_stack: Vec<Location>,      // Back/forward history
    bookmarks: Vec<Bookmark>,      // Loaded from disk
    toc_visible: bool,
    address_input: String,         // User's typed input
    error_message: Option<String>, // Transient error to display
}
```

Valid transitions:
- `Reading → AddressBar` (`:` key)
- `Reading → TOC` (`t` key, toggle)
- `Reading → BookmarkSave` (`m` key)
- `AddressBar → Reading` (on `Enter` or `Esc`)
- `Error → Reading` (after timeout or key press)
- Any mode → `Reading` on successful nav or `Esc`

---

## Async & Threading Model

**Critical:** Single UI thread drives ratatui. HTTP fetches happen off-thread.

```
Main thread (UI loop):
  - Runs crossterm event loop
  - Polls for user input
  - Renders Ratatui to terminal
  - Polls channel for fetch results
  - Updates AppState
  - No blocking I/O

Background task (tokio spawn):
  - Fetch HTTP / read file
  - Parse Markdown
  - Send result back via crossbeam::channel
  - Exits

Channels:
  - `tx_fetch` / `rx_fetch`: UI → fetch task (new location to open)
  - `tx_result` / `rx_result`: fetch task → UI (document ready or error)
```

**No async/await in UI code.** Use `tokio::spawn_blocking` for file I/O if needed; prefer non-blocking.

---

## Success Criteria (MVP Definition)

A working FrogRust can:

1. ✅ Open `frogrust path/to/README.md` and render it.
2. ✅ Navigate within the document (scroll, jump, TOC).
3. ✅ Click or press `Enter` on a link to open a new local file.
4. ✅ Go back/forward through history.
5. ✅ Quit with `q` and restore terminal cleanly (no junk text, cursor visible).
6. ✅ Handle errors gracefully (file not found, malformed Markdown, etc.) without crashing.
7. ✅ Single binary, no runtime deps (Rust compile-time linking only).

**Not required for MVP:**
- Remote URLs or GitHub fetching (Tier 2).
- Persistent bookmarks or history (Tier 2–3).
- Image rendering.
- Mouse clicks (keyboard nav is sufficient).

---

## Key Dependencies (Cargo.toml)

```toml
ratatui = "0.28"           # TUI framework
crossterm = "0.28"         # Terminal backend
pulldown-cmark = "0.11"    # Markdown parser
syntect = "5.1"            # Syntax highlighting
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
url = "2"
clap = { version = "4", features = ["derive"] }
crossbeam = "0.8"          # Channels
anyhow = "1"               # Error handling
directories = "5"          # XDG paths
```

---

## Deliverables

- Runnable binary (`cargo build --release`).
- Renders local Markdown correctly.
- Responsive UI (no jank on resize).
- Exits cleanly.
- Code is modular and readable (for iteration).

---

## Notes for Claude Code

- Prioritise correctness and clarity over micro-optimisations.
- If a design choice is ambiguous, ask. Don't guess.
- Favour Rust idioms (Result, Option, iterators) over imperative loops.
- Use `anyhow` for error propagation; surface errors to UI, don't panic.
- Test locally on an Apple Silicon Mac. Should compile in <30s, startup <100ms.
