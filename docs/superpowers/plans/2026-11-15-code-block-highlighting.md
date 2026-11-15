# Code Block Syntax Highlighting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Colour fenced code blocks by language in FrogRust's pager. This is PRD Tier 1 #1, the product's USP.

**Architecture:** A new `document/highlight.rs` turns `(code, info string)` into styled ratatui `Line`s, using syntect with bat's grammar and theme set (via `two-face`). It uses bat's **`ansi` theme**, which encodes colours as the terminal's own palette indices, so code stays readable on light and dark terminals. `renderer.rs` calls it for each `Block::CodeBlock`. The grammars load lazily, once, on the first code block.

**Tech Stack:** Rust 2021, ratatui 0.28, syntect 5.3 (pure-Rust `fancy-regex`), two-face 0.5 (bat 0.26.1 assets).

**Spec:** `docs/FrogRust_PRD.md`: Tier 1 #1, Success Criteria #1, #2 and #7, and Architecture ("Syntax highlighting belongs in the renderer").

## Global Constraints

- Language comes from the fence info string. "Unknown or missing languages fall back to plain code styling." (PRD Tier 1 #1)
- "The colours must stay readable on both light and dark terminals." Never hard-code RGB foregrounds or a background colour on code.
- Required languages, highlighted correctly: **Rust, Python, shell, TypeScript, JSON, YAML, TOML** (Success Criteria #2).
- Startup: `frogrust README.md` opens within **100 ms** on an Apple Silicon Mac (Success Criteria #1). Documents with no code blocks must not pay for loading grammars.
- Single native binary per machine and no C dependencies: use the pure-Rust regex backend (`fancy`), never `onig`.
- Dependencies, exactly:
  - `syntect = { version = "5.3", default-features = false, features = ["default-fancy"] }`
  - `two-face = { version = "0.5", default-features = false, features = ["syntect-default-fancy"] }`
- Keep the parser semantic and the UI dumb: no changes to `parser.rs` or `ui/`.
- UK English in comments and docs.

## Review Focus

1. **Info strings with extras or aliases:** ` ```rust,ignore `, ` ```python title="x" `, ` ```py `, ` ```sh `, ` ```JSON `, ` ```ts `. Expected: each highlights as its language. Use the first token, split on whitespace or comma, and match case-insensitively. Pinned in Task 1 (`resolves_language_tokens`).
2. **Tabs in code** (Makefiles, Go). Expected: tabs expanded to 4 spaces, so the terminal shows aligned code and no raw `\t`. Pinned in Task 1 (`expands_tabs`).
3. **Blank lines and trailing spaces inside a block.** Expected: the output has exactly one `Line` per source line, blank lines kept, and no `\n` inside any span. Pinned in Task 1 (`preserves_line_structure`).
4. **Light-background terminal.** Expected: no RGB foreground from the theme and no background colour, only palette indices or default. Pinned in Task 1 (`ansi_theme_uses_palette_colours`).
5. **A document with no code blocks.** Expected: grammars are never loaded, so there's no startup cost. Pinned in Task 2 (`no_code_blocks_does_not_load_grammars`).

---

### Task 1: Highlighter module

**Files:**
- Modify: `Cargo.toml` (add the two dependencies, exactly as in Global Constraints)
- Create: `src/document/highlight.rs` (code + unit tests)
- Modify: `src/document/mod.rs` (add `pub mod highlight;`)

**Interfaces:**
- Produces:
  - `pub fn highlight_code(code: &str, info: Option<&str>) -> Vec<ratatui::text::Line<'static>>`: one `Line` per line of `code` (`code.lines()` count). Recognised language → coloured spans. Unknown, missing, or a highlighter error → each line as a single unstyled span (`Style::default()`).
  - `pub fn language_token(info: &str) -> Option<String>`: the first token of the info string, split on whitespace or `,`, lower-cased. Empty → `None`.
  - `pub fn grammars_loaded() -> bool`: whether the lazy grammar set has been initialised (for Task 2's test).
- Internals (your call how, but must hold): `SyntaxSet` from `two_face::syntax::extra_newlines()` and the `ansi` theme from `two_face::theme::extra()`, each held in a `std::sync::OnceLock`. Look a syntax up with `SyntaxSet::find_syntax_by_token(&token)`.

**Colour conversion.** This decides the light/dark behaviour; it's bat's encoding for the `ansi` theme. A `syntect::highlighting::Color { r, g, b, a }` maps to the ratatui foreground as follows:
- `a == 0` → `Color::Indexed(r)` (r is an ANSI palette index)
- `a == 1` → no foreground (`Style::default()` foreground, i.e. the terminal default)
- otherwise → `Color::Rgb(r, g, b)`

Never set a background. Map the font style to modifiers: `BOLD` → `Modifier::BOLD`, `ITALIC` → `Modifier::ITALIC`, `UNDERLINE` → `Modifier::UNDERLINED`.

- [ ] **Step 1: Add the dependencies and `pub mod highlight;`, then confirm the build**

Run: `cargo build`
Expected: compiles. If two-face's API names differ from those above (check docs.rs for `two_face` 0.5), use the real ones; the behaviour stays the same.

- [ ] **Step 2: Write the failing tests in `src/document/highlight.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Style};

    fn coloured_spans(lines: &[ratatui::text::Line]) -> usize {
        lines.iter().flat_map(|l| &l.spans).filter(|s| s.style.fg.is_some()).count()
    }

    #[test]
    fn highlights_required_languages() {
        for (lang, code) in [
            ("rust", "fn main() { let x = 1; }"),
            ("python", "def f(x):\n    return x"),
            ("bash", "echo \"hi\" && ls -la"),
            ("typescript", "const f = (n: number): string => `${n}`;"),
            ("json", "{ \"a\": 1, \"b\": true }"),
            ("yaml", "key: value\nlist:\n  - 1"),
            ("toml", "[package]\nname = \"x\""),
        ] {
            let lines = highlight_code(code, Some(lang));
            assert!(coloured_spans(&lines) > 0, "{lang} produced no colour");
        }
    }

    #[test]
    fn resolves_language_tokens() {
        assert_eq!(language_token("rust,ignore").as_deref(), Some("rust"));
        assert_eq!(language_token("python title=\"x\"").as_deref(), Some("python"));
        assert_eq!(language_token("JSON").as_deref(), Some("json"));
        assert_eq!(language_token("   ").as_deref(), None);
        for (alias, code) in [
            ("py", "def f(): return 1"),
            ("sh", "echo \"hi\""),
            ("ts", "const x: number = 1;"),
            ("rs", "fn f() {}"),
            ("yml", "key: value"),
            ("rust,ignore", "fn f() {}"),
        ] {
            assert!(coloured_spans(&highlight_code(code, Some(alias))) > 0, "{alias} not resolved");
        }
    }

    #[test]
    fn unknown_or_missing_language_is_plain() {
        for info in [Some("nosuchlanguage"), None] {
            let lines = highlight_code("plain text here\nsecond", info);
            assert_eq!(lines.len(), 2);
            assert!(lines.iter().flat_map(|l| &l.spans).all(|s| s.style == Style::default()));
        }
    }

    #[test]
    fn preserves_line_structure() {
        let code = "fn a() {}\n\n  let y = 2;   \n";
        let lines = highlight_code(code, Some("rust"));
        assert_eq!(lines.len(), code.lines().count());
        assert!(lines.iter().flat_map(|l| &l.spans).all(|s| !s.content.contains('\n')));
        let text: Vec<String> = lines.iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect();
        assert_eq!(text, vec!["fn a() {}", "", "  let y = 2;   "]);
    }

    #[test]
    fn expands_tabs() {
        let lines = highlight_code("all:\n\techo hi", Some("makefile"));
        let second: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(!second.contains('\t'));
        assert!(second.starts_with("    echo"));
    }

    #[test]
    fn ansi_theme_uses_palette_colours() {
        let lines = highlight_code("fn main() { let s = \"x\"; }", Some("rust"));
        for span in lines.iter().flat_map(|l| &l.spans) {
            assert!(!matches!(span.style.fg, Some(Color::Rgb(..))), "RGB fg leaked: {span:?}");
            assert!(span.style.bg.is_none(), "background set: {span:?}");
        }
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --lib highlight`
Expected: FAIL, because `highlight_code` / `language_token` aren't defined.

- [ ] **Step 4: Implement `language_token`, `highlight_code`, `grammars_loaded` and the colour conversion**

Use `syntect::easy::HighlightLines` over `syntect::util::LinesWithEndings`. Strip the trailing `\n`/`\r\n` from each span, and expand `\t` to 4 spaces before highlighting. On any `Err` from syntect, return the plain fallback for the whole block.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --lib highlight`
Expected: all 6 pass. Then `cargo test`: the full suite passes (21 existing + 6 new).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/document/highlight.rs src/document/mod.rs
git commit -m "feat: syntect highlighter for code blocks (bat ansi theme)"
```

---

### Task 2: Use the highlighter in the renderer, verify and document

**Files:**
- Modify: `src/document/renderer.rs` (the `Block::CodeBlock` arm, plus its tests)
- Modify: `README.md` (status table: highlighting ✅), `docs/FrogRust_PRD.md` (Status section and Tier 1 #1 marked built)

**Interfaces:**
- Consumes: `highlight::highlight_code(code: &str, info: Option<&str>) -> Vec<Line<'static>>`, `highlight::grammars_loaded() -> bool`, and `highlight::language_token(info: &str) -> Option<String>` (Task 1).
- Produces: no new API. `render()` keeps its signature, and `RenderedDoc.lines` now holds coloured code lines.

**Rendering rules for a code block** (replacing today's White-on-DarkGray):
- a blank line before and after, as now;
- the opening fence line shows ` ```<lang token> ` (just ` ``` ` when there's no language), and both fence lines use `Style::default().add_modifier(Modifier::DIM)`;
- the body is exactly the `highlight_code(code, lang.as_deref())` lines, with no background.

- [ ] **Step 1: Write the failing renderer tests (add them to `renderer.rs` tests; keep the existing `renders_code_block`)**

```rust
#[test]
fn code_block_body_is_highlighted() {
    let doc = render(&parser::parse("```rust,ignore\nfn main() {}\n```"));
    // [blank, fence, body, fence, blank]
    assert_eq!(doc.lines.len(), 5);
    let fence: String = doc.lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(fence, "```rust");
    assert!(doc.lines[2].spans.iter().any(|s| s.style.fg.is_some()));
    assert!(doc.lines.iter().flat_map(|l| &l.spans).all(|s| s.style.bg.is_none()));
}

#[test]
fn no_code_blocks_does_not_load_grammars() {
    // Must run in its own process to observe the lazy state reliably:
    // checks the OnceLock is untouched after rendering prose only.
    let _ = render(&parser::parse("# Title\n\nJust prose, a [link](./x.md) and `inline code`."));
    assert!(!crate::document::highlight::grammars_loaded());
}
```

Put `no_code_blocks_does_not_load_grammars` in **`tests/no_grammar_load.rs`** (an integration test, so its own binary and process) using `frogrust::document::{parser, renderer, highlight}`, not in the unit-test module where other tests load grammars first.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test code_block_body_is_highlighted; cargo test --test no_grammar_load`
Expected: `code_block_body_is_highlighted` FAILs (the body isn't coloured yet, and the fence shows `rust,ignore`). `no_grammar_load` may already pass; that's fine, it guards against regression.

- [ ] **Step 3: Rewrite the `Block::CodeBlock` arm in `renderer.rs` to the rendering rules above**

- [ ] **Step 4: Run the full suite**

Run: `cargo test`
Expected: everything passes.

- [ ] **Step 5: Check startup time**

Run: `cargo build --release && for f in README.md TEST.md; do /usr/bin/time -p target/release/frogrust --version >/dev/null; done` to get a baseline. Then measure the time to the first frame for `TEST.md` (it has 8 code blocks) using `hyperfine` if installed, or a pty one-off with `script`. Expected: under 100 ms on an Apple Silicon Mac. If over, report the figure; don't optimise silently.

- [ ] **Step 6: Update the docs**

README status row: "**Syntax highlighting in code blocks**, the point of FrogRust | ✅". PRD: in Status, move highlighting to Built; in Tier 1, mark #1 *(built)*; in Key Dependencies, add `two-face = "0.5"` with the comment `# bat's grammars + ansi theme for syntect`, and change the syntect line to `syntect = "5.3"`.

- [ ] **Step 7: Commit**

```bash
git add src/document/renderer.rs tests/no_grammar_load.rs README.md docs/FrogRust_PRD.md
git commit -m "feat: highlight fenced code blocks in the pager"
```

---

## Orchestrator checks at the end (not worker steps)

- A visual check of `TEST.md` in a real terminal, in both a dark and a light Terminal profile: all seven languages coloured, the `nosuchlanguage` block and the bare block plain.
- Deploy to the dev targets: `cargo install --path .` and `scripts/deploy-mac.sh` on each machine (native builds). Confirm `frogrust --version` reports the merge commit on both.
