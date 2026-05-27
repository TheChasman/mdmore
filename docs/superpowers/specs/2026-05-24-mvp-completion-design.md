# FrogRust MVP Completion — Design Spec

**Date:** 2026-05-24
**Scope:** Finish the Tier 1 MVP — proper Markdown rendering, link following, TOC navigation.

---

## Problem

The current renderer uses raw-line prefix detection. It does not produce inline styles (bold, italic, code spans, links), does not populate the `links` vec, and does not support link following or TOC jumping. Three Tier 1 success criteria are unmet:

- Criterion 3: Follow links (click or Enter on a link to open a local file)
- Criterion 2 (partial): TOC sidebar shows headings but cannot navigate to them
- Criterion 1 (partial): Markdown renders but without inline styles

---

## Approach

**Approach C: Pre-rendered document with parser/renderer split.**

`Document::from_file` orchestrates two pure functions:
1. `parser::parse(&content) -> Vec<Block>` — pulldown-cmark events → block/inline AST
2. `renderer::render(&blocks) -> RenderedDoc` — AST → styled ratatui Lines + metadata

`AppState` holds the `RenderedDoc`. The UI consumes it directly. Rendering happens once per document load, not per frame.

---

## Module Structure

```
src/document/
  mod.rs       — Document struct, from_file orchestration
  parser.rs    — pulldown-cmark → Vec<Block>
  renderer.rs  — Vec<Block> → RenderedDoc
```

`Document` becomes:

```rust
pub struct Document {
    pub path: PathBuf,
    pub content: String,
    pub rendered: RenderedDoc,
}
```

`from_file` calls `parser::parse` then `renderer::render`. The old `lines`, `headings`, and `links` fields on `Document` are removed; all consumers use `document.rendered`.

---

## Block AST (parser.rs)

```rust
pub enum Block {
    Heading { level: u8, inlines: Vec<Inline> },
    Paragraph { inlines: Vec<Inline> },
    CodeBlock { lang: Option<String>, code: String },
    BlockQuote { blocks: Vec<Block> },
    List { ordered: bool, items: Vec<Vec<Block>> },
    Rule,
    Table { headers: Vec<Vec<Inline>>, rows: Vec<Vec<Vec<Inline>>> },
}

pub enum Inline {
    Text(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Code(String),
    Link { text: Vec<Inline>, url: String },
    SoftBreak,
    HardBreak,
}
```

`parse` is a pure function: `pub fn parse(content: &str) -> Vec<Block>`. It walks pulldown-cmark events and builds the tree. No I/O, no side effects.

---

## RenderedDoc (renderer.rs)

```rust
pub struct LinkInfo {
    pub line: usize,       // index into rendered_lines
    pub col_start: usize,
    pub col_end: usize,
    pub url: String,
}

pub struct HeadingInfo {
    pub line: usize,       // index into rendered_lines
    pub level: u8,
    pub text: String,
}

pub struct RenderedDoc {
    pub lines: Vec<Line<'static>>,
    pub links: Vec<LinkInfo>,
    pub headings: Vec<HeadingInfo>,
}
```

`render` is a pure function: `pub fn render(blocks: &[Block]) -> RenderedDoc`. It walks blocks, pushes `Line<'static>` per visual row, and accumulates `LinkInfo` and `HeadingInfo`.

**Styling rules:**

| Element | Style |
|---|---|
| H1 | Cyan + Bold |
| H2 | Cyan |
| H3 | Blue |
| H4–H6 | Blue + DIM |
| Bold inline | BOLD modifier |
| Italic inline | ITALIC modifier |
| Inline code | Yellow fg |
| Link text | Green fg + Underlined |
| Code block | DarkGray bg, White fg |
| Blockquote | `│ ` prefix span in Gray, body in Gray + ITALIC |
| Bullet list | `• ` prefix, indented per nesting level |
| Ordered list | `N. ` prefix, indented per nesting level |
| Horizontal rule | `────────────` spanning full line in DarkGray |

Nested inlines (e.g. bold-link) accumulate modifiers recursively.

---

## Cursor & Navigation

**State added to `AppState`:**

```rust
pub cursor_line: Option<usize>,  // None = dormant
pub toc_cursor: usize,           // selected heading index when TOC is visible
```

Both reset on every document load (`cursor_line = None`, `toc_cursor = 0`).

**Keybindings:**

| Key | Behaviour |
|---|---|
| `↑` / `↓` | Wake cursor if dormant (at `scroll_offset`). Move cursor up/down 1 line. Auto-scroll viewport if cursor moves above/below visible area. |
| `←` / `→` | Jump cursor to the prev/next line that has a link. No-op if no links exist. |
| `Enter` | If cursor is active and `cursor_line` has one or more links: follow the first link on that line. |
| `j` / `k` | Scroll only. Cursor stays dormant or stays put. |
| `Esc` | Set `cursor_line = None`. |

**Cursor rendering:** The rendered line at `cursor_line` index gets a reversed-bg highlight. Link spans on that line get an additional Underline modifier to stand out.

**Link following:** Resolve relative URLs against `current_location.parent()`. Absolute paths open directly. URLs starting with `http` display "unsupported (Tier 2)" in the status bar — no crash.

---

## TOC Navigation

TOC sidebar already renders. Add keyboard navigation:

- While TOC is visible, `↑`/`↓` navigate a `toc_cursor: usize` index (new field on `AppState`).
- `Enter` in TOC mode: set `scroll_offset = headings[toc_cursor].line`, dismiss TOC (`toc_visible = false`), set `cursor_line = None`.
- `Esc` dismisses TOC without navigating.

`HeadingInfo.line` is already in rendered-line space, so TOC jumps are a direct assignment — no translation needed.

---

## Error Handling & Edge Cases

- **Long lines:** `Paragraph::wrap(Wrap { trim: true })` handles visual wrapping. `LinkInfo.line` and `HeadingInfo.line` point to the first visual line of their block.
- **Nested inlines:** Bold-italic, bold-link, etc. accumulate modifiers recursively in the renderer.
- **Empty document:** `RenderedDoc` with empty vecs. All scroll/cursor ops clamp with `saturating_sub`.
- **Broken Markdown:** pulldown-cmark is lenient; all malformed input falls through as plain text.
- **Scroll clamping:** All scroll ops clamp `scroll_offset` to `(line_count - viewport_height).max(0)`. Currently missing — fix as part of this work.
- **Cursor clamping:** Cursor movement clamps to `[0, rendered_lines.len() - 1]`.

---

## Testing

Three unit tests, no UI or I/O:

**`parser.rs`:**
```rust
#[test]
fn parses_heading_and_paragraph() {
    let blocks = parse("# Hello\n\nWorld **bold**");
    // blocks[0] is Heading { level: 1 }, blocks[1] is Paragraph
}
```

**`renderer.rs`:**
```rust
#[test]
fn heading_and_link_recorded() {
    let blocks = parse("# Title\n\nSome text [link](./foo.md)");
    let doc = render(&blocks);
    assert_eq!(doc.headings.len(), 1);
    assert_eq!(doc.headings[0].text, "Title");
    assert_eq!(doc.links.len(), 1);
    assert_eq!(doc.links[0].url, "./foo.md");
}
```

**`app/state.rs`:**
```rust
#[test]
fn cursor_clamps_at_end() {
    // cursor at last line, move down — stays put
}

#[test]
fn scroll_clamps_at_end() {
    // scroll_offset cannot exceed line_count - viewport_height
}
```

---

## Out of Scope

- Remote URL fetching, GitHub shorthand (Tier 2)
- Persistent bookmarks / history (Tier 2)
- Syntax highlighting in code blocks (syntect, Tier 3)
- Image rendering (Tier 3)
- Mouse click support
