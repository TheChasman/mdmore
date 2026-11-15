# FrogRust — Product Requirements Document

## Overview

**FrogRust is `more` for Markdown, with syntax highlighting.** From a shell you type `frogrust README.md`, read the document rendered (headings, emphasis, lists, tables, links) with **language-aware syntax highlighting in its code blocks**, press `q`, and you're back at your prompt.

It started from one question: *"Is there a version of the pager `more` for Markdown?"* It's a single Rust binary that starts instantly and has no runtime to rot.

**Philosophy:** Compilation, small, fast. A pager first; anything that doesn't serve reading a file from the terminal is out of scope.

**USP:** the syntax highlighting. `less` and `more` show raw Markdown, and most terminal renderers show code blocks in a single colour. FrogRust aims for highlighting as good as [`bat`](https://github.com/sharkdp/bat), which also uses syntect, inside properly rendered Markdown.

### Where it's used

- **Primary:** in a terminal, locally or over SSH. It runs where you already are and leaves nothing behind when you quit.
- **Secondary:** `scripts/deploy-mac.sh` also builds a macOS app bundle that appears in Finder's Open With and opens the file in a Terminal window. It's a convenience; the terminal is home.

### Prior art

| Tool | What we take | What we leave |
|---|---|---|
| `less` / `more` | Muscle memory: keys, `q` to quit, `/` search, reading stdin | Raw text only |
| [`bat`](https://github.com/sharkdp/bat) | The bar for highlighting quality (syntect grammars and themes) | It highlights source; it doesn't render Markdown |
| [`glow`](https://github.com/charmbracelet/glow) | A fast single-binary terminal Markdown renderer with a pager | Its file-picker/stash features |
| [Frogmouth](https://github.com/Textualize/frogmouth) | The original inspiration: in-document TOC and following local links | Its browser shape: address bar/omnibox, remote URLs, GitHub shorthand, bookmarks, history panes, file browser. Frogmouth has been dormant since 2024 (last release v0.9.1, Nov 2023) |

> **History:** FrogRust was first specified (May 2026) as a Rust port of Frogmouth's *browser*. That's why the original Tier 2/3 lists below were dropped. In November 2026 we tried, and reversed, a pivot to a Finder GUI app. The owner confirmed the original intent: a pager with syntax highlighting. The May spec and plan in `docs/superpowers/` record the browser-era MVP that is already built.

---

## Status (2026-11-15)

**Built:** rendering of headings, paragraphs, bold, italic, inline code, syntax-highlighted code blocks (syntect with bat's grammars and `ansi` theme, loaded lazily), block quotes, lists, rules, tables and links. Scrolling, a line/link cursor, a TOC sidebar, following local links, back/forward, and `--version` JSON.

**Highlighting:** built. Fenced blocks use the first token of the info string (`rust,ignore` gives `rust`); unknown or missing languages render plain. The `ansi` theme defers colours to the terminal palette, so it suits light and dark profiles. Grammars load on first use, so documents without code blocks pay nothing.

---

## Requirements

### Tier 1 — The pager (next)

1. **Syntax-highlighted code blocks** *(the USP; built)*. Use syntect with the fence's info string (` ```rust `) to choose the grammar. Unknown or missing languages fall back to plain code styling. The colours must stay readable on both light and dark terminals.
2. **Open and render a local `.md` file** *(built)*. `frogrust FILE` renders headings, emphasis, inline code, lists, tables, quotes, rules and links.
3. **Pager movement with `less` muscle memory** *(mostly built)*. `j`/`k` and arrow keys scroll by a line, `space`/`PageDown` go down a page, `PageUp` goes up a page, and `g`/`gg` and `G` jump to the start and end.
4. **Read from stdin** *(not built)*. `cat notes.md | frogrust` and `frogrust -` both work, as with `less`. The keys still work when stdin is a pipe (read them from the terminal device).
5. **Quit cleanly** *(built)*. `q` exits and restores the terminal: no leftover text, cursor visible.

### Tier 2 — Comfortable reading

6. **Search** *(not built)*. `/pattern`, then `n`/`N` for the next/previous match, highlighting matches as `less` does.
7. **Table of contents** *(built)*. `t` toggles a heading sidebar, and `Enter` jumps to the selected heading.
8. **Follow local links and go back/forward** *(built)*. Kept because it's cheap and already works, but it stays local-only.
9. **Highlighting themes** *(not built)*. A sensible default for light and dark terminals, with an option to choose a syntect theme.
10. **Plain output when not a terminal** *(not built)*. If stdout isn't a TTY (for example `frogrust x.md | head`), print the rendered text and exit instead of starting the interactive pager, as `bat` does.

### Tier 3 — Nice to have

11. **Exit if it fits on one screen** (like `less -F`).
12. **Inline images** in terminals that support Kitty or Sixel graphics, falling back to the alt text.

### Dropped (browser-era features)

These came from Frogmouth's browser shape and don't serve a pager. **Do not build them:**
- the address bar / omnibox (`:`; it exists in the code today and should be removed);
- remote URL fetching and GitHub shorthand;
- persistent bookmarks and history, the history sidebar, and a local file browser.

### Open decisions

- **`b` key.** In `less`, `b` means page up; in FrogRust it currently means *back*. For a pager, `less` habits should win. *Proposal:* `b` = page up, and *back* moves to `Backspace`. This needs the owner's confirmation before it changes.

### Known issues

- In the address bar, the letters `q j k b f g G t` and space are taken as commands instead of typed, and `q` quits. It will be resolved by removing the address bar (see Dropped).

---

## Architecture (as built)

```
src/
  main.rs            — clap CLI: FILE, -V/--version, `version` subcommand
  lib.rs             — module root (the binary imports frogrust::{app, version})
  version.rs         — {"version","environment","commit"} JSON (commit stamped by build.rs)
  app/
    mod.rs           — App: terminal setup/teardown, key-handling event loop
    state.rs         — AppState: scroll, cursor, TOC, nav stack (pure, unit-tested)
  document/
    mod.rs           — Document::from_file → parse → render
    parser.rs        — pulldown-cmark events → Block/Inline AST (CodeBlock keeps `lang`)
    renderer.rs      — AST → display lines + link and heading indices
  ui/mod.rs          — ratatui view of AppState (document, TOC, status bar)
  fetch/, storage/   — placeholders from the browser era; storage (bookmarks) is dropped
```

**Pipeline:** file (or stdin, once Tier 1 lands) → parser → AST → renderer → lines → ratatui. Syntax highlighting belongs in the **renderer**: turn each `CodeBlock { lang, code }` into styled spans with syntect. That keeps the parser semantic and the UI dumb.

**Concurrency:** documents are local and load almost instantly, so loading runs inline. The browser-era design (background tokio tasks plus channels for HTTP fetches) is **not needed**, because remote fetching is dropped. tokio is only used for file reads now and could be removed.

---

## Success Criteria

FrogRust is a pager worth using when:

1. `frogrust README.md` opens within 100 ms on an Apple Silicon Mac and shows the rendered document.
2. Fenced code blocks in common languages (Rust, Python, shell, TypeScript, JSON, YAML, TOML) are highlighted correctly, and unknown languages degrade to plain code styling.
3. `cat file.md | frogrust` works.
4. The `less` keys work: `j k space PageUp PageDown g G q`, plus `/ n N` from Tier 2.
5. `q` leaves the terminal exactly as it was.
6. Errors (missing file, unreadable input) give a clear message and a non-zero exit, with no panic.
7. It's a single native binary per machine: aarch64 on Apple Silicon, x86_64 on Intel.

---

## Key Dependencies

```toml
ratatui = "0.28"          # terminal UI
crossterm = "0.28"        # terminal backend
pulldown-cmark = "0.11"   # Markdown parser
syntect = "5.3"           # syntax highlighting
two-face = "0.5"          # bat's grammars + ansi theme for syntect
clap = { version = "4", features = ["derive"] }
anyhow = "1"
serde = { version = "1", features = ["derive"] }   # version JSON
serde_json = "1"
tokio = { version = "1", features = ["full"] }     # file reads only; removal candidate
```

---

## Deliverables

- `cargo install --path .` puts `frogrust` on your PATH (`~/.cargo/bin`, on PATH on both Macs).
- `scripts/deploy-mac.sh` optionally installs the macOS app bundle for Finder's Open With.
- Readable, modular code with the navigation logic unit-tested.

---

## Notes for Claude Code

- Prioritise correctness and clarity over micro-optimisations.
- If a design choice is ambiguous, ask. Don't guess.
- Favour Rust idioms (Result, Option, iterators) over imperative loops.
- Use `anyhow` for error propagation; surface errors to the UI, don't panic.
- Build natively on each machine (aarch64 on Apple Silicon, x86_64 on Intel). Target: compile in under 30 s, start in under 100 ms.
- Before adding a feature, ask: does this help someone reading a file in a terminal? If it only makes sense in a browser, it's out of scope.
