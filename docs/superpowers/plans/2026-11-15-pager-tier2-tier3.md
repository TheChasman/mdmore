# Pager Tier 2 & 3 Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish everything left in the PRD. Render tables (currently a `[Table]` placeholder). Replace the address bar with `less`-style search. Add plain output when not a TTY, plus `-F` quit-if-one-screen. Add highlighting themes. Remove browser-era code and tokio. Show inline images.

**Architecture:** Each feature stays in its layer. The renderer turns the AST into lines (tables, image placeholders). `AppState` holds pure navigation and search state. `App` maps keys and drives the terminal. `main.rs` chooses between interactive and print mode. Everything stays line-based, so scroll, cursor, link and heading indices keep working.

**Tech Stack:** Rust 2021, ratatui 0.28, crossterm 0.28 (`use-dev-tty`), pulldown-cmark 0.11, syntect 5.3 + two-face 0.5. New in Task 6: `ratatui-image = "2.0.1"` (the last release built for ratatui 0.28) and `image = "0.25"`.

**Spec:** `docs/mdmore_PRD.md`: Tier 1 #2 (tables), Tier 2 #6, #9 and #10, Tier 3 #11 and #12, Dropped (address bar), Known issues, and Architecture (tokio removal candidate; fetch/storage placeholders).

## Global Constraints

- First frame within **100 ms** for `README.md` and `TEST.md` on an Apple Silicon Mac (currently about 5 ms). Don't regress.
- `less` keys stay: `j k space b PageUp PageDown g G q`, `Backspace` = back.
- `q` leaves the terminal exactly as it was. Errors give a clear message and a non-zero exit, with no panic.
- Colours come from the terminal palette by default (the `ansi` theme). No hard-coded RGB in the default look, and no background on code.
- Parser semantic, UI dumb. Highlighting never changes line count.
- No crates beyond those named above. UK English in comments and docs.
- Each task updates the README and PRD rows it completes, so the docs never overclaim.

## Review Focus

1. **Tables wider than the terminal.** Expected: cells are truncated with `…` so the table fits the width, with no wrapping chaos. Pinned in Task 1 (`wide_table_is_truncated_to_width`).
2. **A search pattern with no matches.** Expected: the status bar says "Pattern not found", the view doesn't move, and there's no panic. Pinned in Task 2 (`search_no_match`).
3. **Piped output with code blocks** (`frogrust x.md | cat`). Expected: plain text, with no escape codes and no grammars loaded for colour. Pinned in Task 3 (`print_plain_has_no_escapes`).
4. **An unknown theme name.** Expected: a clear error listing how to see the valid names, and exit code 2. Pinned in Task 4 (`unknown_theme_errors`).
5. **An image that's missing, remote, or undecodable.** Expected: the alt text is shown in its place, with no panic and no network access. Pinned in Task 6 (`missing_image_falls_back_to_alt`).

---

### Task 1: Render tables

**Files:** `src/document/renderer.rs` (the `Block::Table` arm and tests), `TEST.md` (add a wide table and an inline-formatted table).

**Interfaces:**
- Produces: `render_with(blocks, highlight)` emits table lines.
  - A header row in bold, then a separator row of `─` joined by `┼`, then body rows.
  - Columns are separated by ` │ `, and each column is padded to its widest cell, measured in chars of the plain text from `inlines_to_text`.
  - Links inside cells still register in `doc.links` with correct column offsets.
- The width cap needs a terminal width the renderer doesn't know. Add a `pub max_width: Option<u16>` parameter by changing the signature to `render_with(blocks, highlight, max_width: Option<u16>)`. `render(blocks)` passes `None`, and `Document` passes `None` (column truncation only applies when a width is given; Task 3 passes the terminal width for print mode). Interactive mode passes `None` too; horizontal overflow is clipped by ratatui.

- [ ] **Step 1: Failing tests**

```rust
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
```

(Trailing padding on the last column is trimmed. The header row is bold.)

- [ ] **Step 2: Run** `cargo test renders_table` to confirm the failures.
- [ ] **Step 3: Implement** it. Truncation shrinks the widest columns first, down to a minimum of 3 chars, adding `…`.
- [ ] **Step 4: Run** `cargo test`: all pass.
- [ ] **Step 5: Commit** `feat: render Markdown tables as aligned grids`

---

### Task 2: Replace the address bar with `less`-style search

**Files:** `src/app/state.rs`, `src/app/mod.rs`, `src/ui/mod.rs`, `README.md`, `docs/mdmore_PRD.md` and `TEST.md`.

**Interfaces:**
- `AppMode` becomes `Reading | Search | Error`. Remove `AddressBar`, `address_input`, `enter_address_bar`/`exit_address_bar`/`is_address_bar_active`, the `:` key, and `render_address_bar`. That also fixes the Known issue.
- `AppState` gains:
  - `pub search_input: String`
  - `pub search_pattern: Option<String>`
  - `pub search_matches: Vec<(usize, usize, usize)>`, as (line, char_start, char_end)
  - `pub search_index: Option<usize>`
  - `pub status_message: Option<String>`
- `pub fn run_search(&mut self, pattern: &str)`: a case-insensitive substring search over each line's plain text. It fills `search_matches` and jumps to the first match at or after the current top line, scrolling so the match line is visible. With no matches, it sets `status_message = Some("Pattern not found")` and doesn't move.
- `pub fn search_next(&mut self)` / `pub fn search_prev(&mut self)`: cycle through matches, wrapping, and scroll to them.
- Keys:
  - `/` enters Search mode.
  - In Search mode, printable chars (all of them, including `q j k b` and so on) append; `Backspace` deletes, and `Backspace` on empty input leaves Search mode; `Enter` runs `run_search` and returns to Reading; `Esc` cancels.
  - In Reading mode, `n`/`N` call next/prev.
  - Search-mode handling must come **before** every other key arm.
- UI: in Search mode the status line shows `/` + input. Matches are drawn with `Modifier::REVERSED` over the matched char range, applied at render time in `ui` without mutating `rendered.lines`. `status_message` shows in the status bar until the next key.
- Docs: README keys (`/`, `n`, `N`) and the status row "Search" ✅. PRD: Tier 2 #6 *(built)*; remove the address-bar Known issue and the "(remove it)" note; the Dropped list keeps "address bar" marked as removed. TEST.md: a Navigation entry.

- [ ] **Step 1: Failing tests in `state.rs`** (build state from `Document::from_source`)

```rust
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
```

(`state_from(md: &str) -> AppState` is a test helper you add.)

- [ ] **Step 2: Run** them to confirm the failures.
- [ ] **Step 3: Implement** the state, keys and UI.
- [ ] **Step 4: Run** `cargo test`: all pass. Do a pty check: open `TEST.md`, type `/rust` Enter, `n`, `q`; expect exit 0.
- [ ] **Step 5: Commit** `feat: less-style search (/ n N); remove the address bar`

---

### Task 3: Plain output when not a TTY, and `-F` quit-if-one-screen

**Files:** `src/main.rs`, a new `src/print.rs` (plus `pub mod print;` in `lib.rs`), `README.md` and `docs/mdmore_PRD.md`.

**Interfaces:**
- `pub fn print_document(doc_source: &str, out: &mut impl std::io::Write, colour: bool, width: Option<u16>) -> std::io::Result<()>`
  - It renders with `render_with(blocks, colour, width)` and writes each line's text.
  - With `colour`, each span's style is written as ANSI SGR (fg palette `Indexed(n)` → `ESC[38;5;nm`, named colours → `ESC[3Xm`, BOLD → `1`, DIM → `2`, ITALIC → `3`, UNDERLINED → `4`, then reset `ESC[0m` at line end).
  - Without `colour`, it writes plain text, with no escapes and no grammar loading.
- CLI:
  - If stdout isn't a terminal → `print_document(.., colour = false, width = None)` and exit 0, with no raw mode or alternate screen.
  - New flag `-F` / `--quit-if-one-screen`: when stdout *is* a terminal and the rendered line count ≤ terminal rows − 2 → `print_document(.., colour = true, width = Some(cols))` and exit, otherwise run the pager.
  - This works for file and stdin sources alike.
- Docs: README usage (`fmore x.md | head`, `fmore -F short.md`), with Status rows. PRD Tier 2 #10 and Tier 3 #11 *(built)*.

- [ ] **Step 1: Failing tests in `print.rs`**

```rust
#[test]
fn print_plain_has_no_escapes() {
    let mut out = Vec::new();
    print_document("# T\n\n```rust\nfn a() {}\n```\n", &mut out, false, None).unwrap();
    let s = String::from_utf8(out).unwrap();
    assert!(!s.contains('\x1b'));
    assert!(s.contains("fn a() {}"));
}

#[test]
fn print_colour_emits_sgr() {
    let mut out = Vec::new();
    print_document("# T\n\n```rust\nfn a() {}\n```\n", &mut out, true, None).unwrap();
    let s = String::from_utf8(out).unwrap();
    assert!(s.contains("\x1b[38;5;") || s.contains("\x1b[3"));
    assert!(s.contains("\x1b[0m"));
}
```

- [ ] **Step 2: Run** them to confirm the failures.
- [ ] **Step 3: Implement** `print.rs` and the `main.rs` branching. Use `std::io::IsTerminal` on stdout, and `crossterm::terminal::size()` for `-F`.
- [ ] **Step 4: Run** `cargo test`. Then manually check that `./target/debug/frogrust TEST.md | head -5` prints plain text and exits 0, and that `./target/debug/frogrust TEST.md | grep -c $'\x1b'` gives `0`.
- [ ] **Step 5: Commit** `feat: plain output when piped; -F quits if it fits on one screen`

---

### Task 4: Highlighting themes

**Files:** `src/document/highlight.rs`, `src/main.rs`, `README.md` and `docs/mdmore_PRD.md`.

**Interfaces:**
- The theme is chosen once at startup:
  - `pub fn set_theme(name: &str) -> Result<(), String>`, which is valid only before the first highlight. The error text is `unknown theme '<name>'; run `frogrust --list-themes``.
  - `pub fn theme_names() -> Vec<String>` (two-face's embedded themes, sorted).
  - The default stays `ansi`.
- Theme colour conversion: palette (`a == 0`) and default (`a == 1`) work as now. For RGB themes, use `Color::Rgb`; that's an explicit opt-in, so the constraint against hard-coded RGB applies only to the default.
- CLI:
  - `--theme <NAME>`, falling back to the env var `FROGRUST_THEME`.
  - An unknown name prints the error to stderr and exits with code 2.
  - `--list-themes` prints the names one per line and exits 0.
- Docs: README "Themes" section; PRD Tier 2 #9 *(built)*.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn theme_names_include_ansi() { assert!(theme_names().iter().any(|n| n == "ansi")); }

#[test]
fn unknown_theme_errors() {
    let err = validate_theme("no-such-theme").unwrap_err();
    assert!(err.contains("unknown theme 'no-such-theme'"));
}
```

Split the name check into a pure `pub fn validate_theme(name: &str) -> Result<(), String>` used by `set_theme`, so the test doesn't depend on global state. Add an integration test, `tests/cli_theme.rs`, that runs the built binary via `env!("CARGO_BIN_EXE_frogrust")` with `--theme no-such-theme TEST.md` and asserts exit code 2.

- [ ] **Step 2: Run** them to confirm the failures.
- [ ] **Step 3: Implement** it. Map two-face's embedded theme names to strings (for example `EmbeddedThemeName` variants via its name or `as_name()`; check docs.rs).
- [ ] **Step 4: Run** `cargo test`: all pass.
- [ ] **Step 5: Commit** `feat: --theme / FROGRUST_THEME and --list-themes`

---

### Task 5: Remove browser-era code and tokio; apply highlights promptly

**Files:** `Cargo.toml` (remove `tokio`), `src/main.rs`, `src/app/mod.rs`, `src/app/state.rs`, `src/document/mod.rs`, `src/lib.rs`; delete `src/fetch/` and `src/storage/`; `docs/mdmore_PRD.md` (Architecture, Key Dependencies).

**Interfaces:**
- Everything becomes synchronous:
  - `Document::from_file(path: &Path) -> anyhow::Result<Document>` uses `std::fs::read_to_string`.
  - `App::new` / `App::with_document` / `run` / `event_loop`, `AppState::go_back` / `go_forward` / `open_document` lose `async`.
  - `main` becomes plain `fn main() -> anyhow::Result<()>`.
  - Tests that used `#[tokio::test]` become `#[test]`.
- Remove the `fetcher` field and the `fetch`/`storage` modules; the dead-code warning goes with them.
- Faster highlight apply: reduce the event poll timeout from 100 ms to **16 ms**, or recv-select the highlight channel. Either way, a finished highlight is drawn within 20 ms of arriving.
- PRD: drop tokio from Key Dependencies, update the Architecture tree (no fetch/storage) and the Concurrency paragraph (std::thread only).

- [ ] **Step 1: Make the changes**
- [ ] **Step 2: Run** `cargo build` with no warnings, then `cargo test` with all passing (no tokio).
- [ ] **Step 3: Measure** the release first frame for `README.md` and `TEST.md` (three runs each, the same pty method as before): under 100 ms. Also measure the time until the highlighted redraw for `TEST.md`; it should be about 100 ms sooner than the previous ~420 ms.
- [ ] **Step 4: Commit** `refactor: drop tokio and browser-era placeholders; prompt highlight redraw`

---

### Task 6: Inline images

**Files:** `Cargo.toml` (`ratatui-image = "2.0.1"`, `image = "0.25"`), `src/document/parser.rs` (Image), `src/document/renderer.rs`, `src/ui/mod.rs`, `src/app/mod.rs` (picker set-up), `src/print.rs` (alt text), `TEST.md` (add an image), `artwork/` (reuse `artwork/icon/RustFrog.png` as the test image), `README.md` and `docs/mdmore_PRD.md`.

**Interfaces:**
- Parser: a paragraph whose only content is one image → `Block::Image { url: String, alt: String }`. Images inside other text → `Inline::Text(alt)`.
- Renderer:
  - `RenderedDoc` gains `pub images: Vec<ImageInfo>`, where `pub struct ImageInfo { pub line: usize, pub height: u16, pub path: PathBuf, pub alt: String }`.
  - A `Block::Image` whose url is a local file (resolved relative to the document's directory, which needs a `base_dir: Option<&Path>` parameter threaded from `Document`) emits `IMAGE_ROWS = 12` placeholder blank lines and records an `ImageInfo`.
  - A remote, missing or non-file url emits one line, `[image: <alt>]` in DIM, with no `ImageInfo`.
- UI/App:
  - Create a `ratatui_image::picker::Picker` once after entering raw mode (`Picker::from_termios()` then `guess_protocol()`, or that version's equivalent; check docs.rs for 2.0.1). It covers iTerm2, Kitty and Sixel, with a half-block fallback.
  - Decode each image lazily on first display and cache it per path.
  - When an `ImageInfo`'s rows are on screen, draw the image widget into that area. When it's partly scrolled off, draw it only if fully visible; otherwise show the alt line in its first visible row.
  - A decode failure falls back to the alt text, and is never a panic.
- Print mode: `[image: <alt>]`.
- Docs: README status row "Inline images (iTerm2, Kitty, Sixel; half-blocks elsewhere)" ✅; PRD Tier 3 #12 *(built)*.

- [ ] **Step 1: Failing tests**

```rust
// parser.rs
#[test]
fn parses_standalone_image() {
    let blocks = parse("![a frog](frog.png)");
    assert!(matches!(&blocks[0], Block::Image { url, alt } if url == "frog.png" && alt == "a frog"));
}

// renderer.rs
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
```

(`render_with_base(blocks, highlight, max_width, base_dir)` is the full signature. `render_with(b, h, w)` forwards with `base_dir = None`.)

- [ ] **Step 2: Run** them to confirm the failures.
- [ ] **Step 3: Implement** the parser, renderer and UI.
- [ ] **Step 4: Run** `cargo test`: all pass. Do a pty check: open `TEST.md`, scroll to the image, `q`; expect exit 0 and no panic. Note the protocol the picker chose. If ratatui-image 2.0.1 can't be made to work with our ratatui 0.28 build, stop and report BLOCKED with the compiler errors. Don't upgrade ratatui inside this task.
- [ ] **Step 5: Commit** `feat: inline images via ratatui-image (iTerm2/Kitty/Sixel, half-block fallback)`
