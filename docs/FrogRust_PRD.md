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

**Built:** rendering of headings, paragraphs, bold, italic, inline code, syntax-highlighted code blocks (syntect with bat's grammars and `ansi` theme, loaded lazily), block quotes, lists, rules, tables and links. Paragraphs, headings, list items (hanging indent) and quotes wrap to the terminal width and re-wrap on resize; code blocks don't wrap (pager convention), tables truncate, and piped output stays unwrapped like `cat`. Scrolling, a line/link cursor, a TOC sidebar, reading Markdown from stdin, following local links, back/forward, and `--version` JSON.

**Highlighting:** built. Fenced blocks use the first token of the info string (`rust,ignore` gives `rust`); unknown or missing languages render plain. The `ansi` theme defers colours to the terminal palette, so it suits light and dark profiles. Grammars load on first use, so documents without code blocks pay nothing (about 5 ms to first frame). Highlighting is applied just after the first frame: code shows plain first, then a background thread colours it (about 0.4 s for `TEST.md`, dominated by the TypeScript grammar's first use), so the first frame stays under 100 ms (about 5 ms measured for `README.md` and `TEST.md` on an Apple Silicon Mac).

---

## Requirements

### Tier 1 — The pager (next)

1. **Syntax-highlighted code blocks** *(the USP; built)*. Use syntect with the fence's info string (` ```rust `) to choose the grammar. Unknown or missing languages fall back to plain code styling. The colours must stay readable on both light and dark terminals.
2. **Open and render a local `.md` file** *(built)*. `frogrust FILE` renders headings, emphasis, inline code, lists, tables, quotes, rules and links.
3. **Pager movement with `less` muscle memory** *(built)*. `j`/`k` and arrow keys scroll by a line, `space`/`PageDown` go down a page, `b` and `PageUp` go up a page, and `g`/`gg` and `G` jump to the start and end.
4. **Read from stdin** *(built)*. `cat notes.md | frogrust` and `frogrust -` both work, as with `less`. The keys still work when stdin is a pipe (read them from the terminal device).
5. **Quit cleanly** *(built)*. `q` exits and restores the terminal: no leftover text, cursor visible.

### Tier 2 — Comfortable reading

6. **Search** *(built)*. `/pattern`, then `n`/`N` for the next/previous match, highlighting matches as `less` does.
7. **Table of contents** *(built)*. `t` toggles a heading sidebar, and `Enter` jumps to the selected heading.
8. **Follow local links and go back/forward** *(built)*. Kept because it's cheap and already works, but it stays local-only.
9. **Highlighting themes** *(built)*. A sensible default for light and dark terminals, with `--theme` / `FROGRUST_THEME` to choose a syntect theme and `--list-themes` to list them.
10. **Plain output when not a terminal** *(built)*. If stdout isn't a TTY (for example `frogrust x.md | head`), print the rendered text and exit instead of starting the interactive pager, as `bat` does.

### Tier 3 — Nice to have

11. **Exit if it fits on one screen** *(built)*. `-F` / `--quit-if-one-screen`, like `less -F`.
12. **Inline images** *(built)* in terminals that support iTerm2, Kitty or Sixel graphics, with a half-block fallback elsewhere; alt text for remote, missing or undecodable images. Local files only: relative paths resolve against the document's directory, or the current directory for documents read from stdin.

### Dropped (browser-era features)

These came from Frogmouth's browser shape and don't serve a pager. **Do not build them:**
- the address bar / omnibox (`:`) *(removed)*;
- remote URL fetching and GitHub shorthand;
- persistent bookmarks and history, the history sidebar, and a local file browser.

### Open decisions

- Resolved 2026-11-15: `b` = page up (less); back = `Backspace`.

### Known issues

None.

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
```

**Pipeline:** file (or stdin) → parser → AST → renderer → lines → ratatui. Syntax highlighting belongs in the **renderer**: turn each `CodeBlock { lang, code }` into styled spans with syntect. That keeps the parser semantic and the UI dumb.

**Concurrency:** everything is synchronous. Documents are local and load inline. The only background work is syntax highlighting: a `std::thread` per document sends finished highlights over a `std::sync::mpsc` channel (tagged with a generation counter so stale results are dropped), and the event loop applies them each tick (16 ms poll). There is no async runtime.

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
ratatui-image = "2.0.1"   # inline images (iTerm2/Kitty/Sixel, half-block fallback)
image = "0.25"            # decoders: png, jpeg, gif, webp only
rustix = "0.38"           # stdio/fs/termios: restore stdin after the graphics query
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
