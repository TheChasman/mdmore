# Pager Tier 1 Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish PRD Tier 1. `b` becomes page up as in `less`, Markdown can be read from stdin, and highlighting is deferred so every document opens within 100 ms.

**Architecture:** The key changes live in `App::event_loop`. Stdin adds a pure `Document::from_source` constructor (`from_file` becomes a thin wrapper) and keeps the stdin document in `AppState` so back/forward can return to it. Deferred highlighting renders code plain first, then re-renders with highlighting on a `std::thread` and swaps the lines in. Highlighting never changes the line count, so scroll, cursor, link and heading indices stay valid.

**Tech Stack:** Rust 2021, ratatui 0.28, crossterm 0.28, syntect 5.3 + two-face 0.5, `std::sync::mpsc`, `std::io::IsTerminal`.

**Spec:** `docs/FrogRust_PRD.md`: Tier 1 #3, #4 and #5, Known issues (TypeScript startup), and Success Criteria #1, #3 and #4. The owner ruled on the `b` key on 2026-11-15: **`b` = page up; back moves to `Backspace`.**

## Global Constraints

- `frogrust README.md` (and `TEST.md`, which has 8 code blocks including TypeScript) shows its first frame within **100 ms** on an Apple Silicon Mac.
- `cat notes.md | frogrust` and `frogrust -` both work, and the keys still work when stdin is a pipe.
- `less` keys: `j k space PageUp PageDown g G q`, and now `b` = page up.
- `q` leaves the terminal exactly as it was. Errors give a clear message and a non-zero exit, with no panic.
- No new dependencies. Keep the parser semantic and the UI dumb. UK English in comments and docs.
- Out of scope: search, themes, plain output when not a TTY (Tier 2).

## Review Focus

1. **Stdin piped but no input** (`frogrust < /dev/null`). Expected: it opens an empty document and quits cleanly with `q`. No hang, no panic. Pinned in Task 2 (`from_source_empty`).
2. **Going back to the stdin document after following a link.** Expected: the stdin content comes back, with no attempt to re-read a file called `-`. Pinned in Task 2 (`go_back_returns_to_stdin_document`).
3. **A highlight result arriving after the user has moved to another document.** Expected: it's discarded, and the new document isn't overwritten. Pinned in Task 3 (`stale_highlight_is_ignored`).
4. **Backspace in the address bar** still deletes a character and doesn't go back. Pinned in Task 1 by keeping the guarded arm first (checked in the manual step).
5. **A document with no languages on its code blocks.** Expected: no highlight thread is spawned. Pinned in Task 3 (`needs_highlight_false_without_languages`).

---

### Task 1: `b` = page up, `Backspace` = back

**Files:**
- Modify: `src/app/mod.rs`: in the `event_loop` key match, `Char('b')` calls `self.state.page_up()`. Add `KeyCode::Backspace => self.state.go_back().await?` *after* the existing `Backspace if self.state.is_address_bar_active()` arm.
- Modify: `README.md` keys table (`b` / `PageUp` = page up; `Backspace` / `f` = back / forward), and delete the README note about `b` meaning back. `TEST.md` Navigation list: same change. `docs/FrogRust_PRD.md`: replace the "Open decisions" `b` bullet with "Resolved 2026-11-15: `b` = page up (less); back = `Backspace`"; add `b` to Tier 1 #3's key list.

- [ ] **Step 1: Make the two key changes**
- [ ] **Step 2: Run** `cargo build && cargo test`. Expected: builds, all tests pass.
- [ ] **Step 3: Manual check in a pty** (`script -q /dev/null bash -c 'stty rows 30 cols 80; ./target/debug/frogrust TEST.md'`, then feed keys: `G`, `b`, `q`). Expected: it quits cleanly with exit 0. Note the result in your report.
- [ ] **Step 4: Update the docs as listed**
- [ ] **Step 5: Commit** `feat: b pages up like less; Backspace goes back`

---

### Task 2: Read Markdown from stdin

**Files:**
- Modify: `src/document/mod.rs`, `src/main.rs`, `src/app/mod.rs`, `src/app/state.rs`, `README.md` and `docs/FrogRust_PRD.md`

**Interfaces:**
- Produces:
  - `pub fn Document::from_source(content: String, path: PathBuf) -> Document`, which is sync and pure: parse + render. `from_file` reads the file, then calls it.
  - `pub const STDIN_PATH: &str = "-";` in `document/mod.rs`, the path recorded for stdin documents.
  - `pub fn App::with_document(location: PathBuf, document: Document) -> anyhow::Result<App>`. `App::new(path)` stays as a wrapper.
  - `AppState` gains `pub stdin_document: Option<Document>`, set when the initial document came from stdin. `go_back`/`go_forward` use it (cloned) when the target path is `STDIN_PATH`, instead of `Document::from_file`.
- CLI rules in `main.rs`:
  - `path == Some("-")` → read all of stdin.
  - `path == None` and stdin isn't a terminal (`std::io::stdin().is_terminal() == false`) → read all of stdin.
  - Otherwise behave as today (default `README.md`, "File not found" error).
  - Read stdin *before* entering raw mode. crossterm opens `/dev/tty` for keys when stdin isn't a TTY; confirm that in the pty check.

- [ ] **Step 1: Write the failing tests**

In `src/document/mod.rs`:

```rust
#[test]
fn from_source_renders_content() {
    let doc = Document::from_source("# Hi\n\ntext".into(), PathBuf::from(STDIN_PATH));
    assert_eq!(doc.rendered.headings.len(), 1);
    assert_eq!(doc.path, PathBuf::from("-"));
}

#[test]
fn from_source_empty() {
    let doc = Document::from_source(String::new(), PathBuf::from(STDIN_PATH));
    assert!(doc.rendered.lines.is_empty());
}
```

In `src/app/state.rs` tests (use `#[tokio::test]`; build a real temp file for the linked doc with `std::env::temp_dir()`):

```rust
#[tokio::test]
async fn go_back_returns_to_stdin_document() {
    let stdin_doc = Document::from_source("# From stdin".into(), PathBuf::from(STDIN_PATH));
    let mut state = AppState::new(PathBuf::from(STDIN_PATH), stdin_doc.clone());
    state.stdin_document = Some(stdin_doc);
    let other = std::env::temp_dir().join("frogrust_go_back_test.md");
    std::fs::write(&other, "# Other").unwrap();
    let doc = Document::from_file(&other).await.unwrap();
    state.open_document(other.clone(), doc).await;
    state.go_back().await.unwrap();
    assert_eq!(state.current_location, PathBuf::from("-"));
    assert_eq!(state.current_document.rendered.headings[0].text, "From stdin");
}
```

- [ ] **Step 2: Run** `cargo test` and confirm these fail (the items aren't defined yet).
- [ ] **Step 3: Implement the interfaces and CLI rules above**
- [ ] **Step 4: Run** `cargo test`. Expected: all pass.
- [ ] **Step 5: Pty checks.** `printf '# Piped\n\nhello\n' | script -q /dev/null bash -c 'stty rows 20 cols 60; ./target/debug/frogrust'`, sending `q` through the tty (run it as `(sleep 1; printf q) | script -q /dev/null bash -c '... printf "# Piped" | ./target/debug/frogrust'` so the pipe is inside the pty session). Expected: exit 0, and the captured output contains `Piped`. Repeat with `frogrust -` and with `< /dev/null`. Record all three.
- [ ] **Step 6: Docs.** README status row "Reading from stdin" → ✅, with usage `cat x.md | frogrust` / `frogrust -`. PRD Tier 1 #4 → *(built)*, and update the Status "Built" list.
- [ ] **Step 7: Commit** `feat: read Markdown from stdin (frogrust - / piped input)`

---

### Task 3: Highlight after the first frame

**Files:**
- Modify: `src/document/renderer.rs`, `src/document/mod.rs`, `src/app/mod.rs`, `src/app/state.rs` (only if it holds the hook) and `docs/FrogRust_PRD.md`

**Interfaces:**
- Consumes: `Document::from_source` (Task 2) and `highlight::highlight_code` (existing).
- Produces:
  - `pub fn render_with(blocks: &[Block], highlight: bool) -> RenderedDoc` in `renderer.rs`. `render(blocks)` stays and equals `render_with(blocks, true)`. With `highlight == false`, code bodies are one unstyled span per line (the same text as `highlight_code`'s plain fallback, tabs expanded the same way), so the line count is identical.
  - `Document::from_source` uses `render_with(.., false)`.
  - `pub fn Document::needs_highlight(&self) -> bool`: true if any code block has a language token.
  - `pub fn Document::highlighted(&self) -> RenderedDoc`: re-parses `content` and returns `render_with(.., true)`.
- App behaviour:
  - After drawing a frame, if `current_document.needs_highlight()` and no job is pending for it, spawn `std::thread` running `doc.highlighted()` and send `(generation: u64, RenderedDoc)` over `std::sync::mpsc`.
  - Each loop tick, `try_recv`. Apply only if `generation` equals the current document's generation, which increments on every document change (open, back, forward). Applying means replacing `current_document.rendered.lines`; keep links and headings.
  - Nothing ever blocks on the thread.

- [ ] **Step 1: Write the failing tests**

In `renderer.rs`:

```rust
#[test]
fn plain_and_highlighted_have_same_shape() {
    let md = "# T\n\n```rust\nfn a() {}\n\n\tlet x = 1;\n```\n\n[l](./x.md)";
    let blocks = parser::parse(md);
    let plain = render_with(&blocks, false);
    let hl = render_with(&blocks, true);
    assert_eq!(plain.lines.len(), hl.lines.len());
    assert_eq!(plain.links.len(), hl.links.len());
    assert_eq!(plain.headings.len(), hl.headings.len());
    let text = |d: &RenderedDoc| d.lines.iter()
        .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect::<String>())
        .collect::<Vec<_>>();
    assert_eq!(text(&plain), text(&hl));
}
```

In `document/mod.rs`:

```rust
#[test]
fn needs_highlight_false_without_languages() {
    let doc = Document::from_source("```\nplain\n```".into(), PathBuf::from("-"));
    assert!(!doc.needs_highlight());
    let doc = Document::from_source("```rust\nfn a(){}\n```".into(), PathBuf::from("-"));
    assert!(doc.needs_highlight());
}
```

For `stale_highlight_is_ignored`, put the apply rule in a small pure function, `pub fn apply_highlight(state: &mut AppState, generation: u64, rendered: RenderedDoc) -> bool`, which returns whether it applied, and test it with generation mismatch → `false` and lines unchanged, and match → `true`.

- [ ] **Step 2: Run** `cargo test` and confirm the new tests fail.
- [ ] **Step 3: Implement the interfaces and app behaviour above**
- [ ] **Step 4: Run** `cargo test`. Expected: all pass, including the existing `no_grammar_load` integration test.
- [ ] **Step 5: Measure** the release build's time to first frame for `TEST.md` and `README.md`, using the same pty method as the earlier highlighting task (three runs each). Expected: both under 100 ms. Also confirm the captured output for `TEST.md` contains palette colour codes (`ESC[38;5;Nm`) within 1 s, meaning highlighting arrived.
- [ ] **Step 6: PRD.** Remove the TypeScript Known issues entry, note "Highlighting is applied just after the first frame", and remove the TypeScript caveat on Tier 1 #1.
- [ ] **Step 7: Commit** `feat: highlight code after the first frame so documents open fast`
