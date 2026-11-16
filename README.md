# mdmore

`more` for Markdown, with syntax highlighting. A single fast Rust binary for reading Markdown in your terminal.

```bash
mdmore README.md      # read it rendered, press q to quit
```

Scope, priorities and decisions live in the PRD: [`docs/mdmore_PRD.md`](docs/mdmore_PRD.md).

## Status

| | |
|---|---|
| Rendered Markdown (headings, emphasis, lists, tables, quotes, links) | ✅ |
| Text wraps to the terminal width (code blocks don't; piped output stays unwrapped) | ✅ |
| Pager scrolling, TOC sidebar, follow local links, back/forward | ✅ |
| `--version` (JSON: version, environment, commit) | ✅ |
| **Syntax highlighting in code blocks**, the point of mdmore | ✅ |
| Reading from stdin (`cat x.md \| mdmore` / `mdmore -`) | ✅ |
| Search (`/`, `n`, `N`) | ✅ |
| Plain output when piped (`mdmore x.md \| head`) | ✅ |
| `-F` / `--quit-if-one-screen` (`mdmore -F short.md`) | ✅ |
| Inline images (iTerm2, Kitty, Sixel; half-blocks elsewhere). Local files only; for documents piped on stdin, relative paths resolve against the current directory. Transparent PNG areas are blended onto the terminal background (black if the terminal does not report it) | ✅ |

## Requirements

- A Rust toolchain (stable) to build it.
- A Unix terminal. Developed and tested on macOS (iTerm2, Terminal.app); it should build on Linux, but that isn't tested yet. Windows isn't supported.

## Install

```bash
cargo install --path .    # installs mdmore into ~/.cargo/bin
```

Build natively on each machine (Apple Silicon or Intel; no cross-compiling).

### Optional: macOS app bundle

```bash
scripts/deploy-mac.sh
```

Installs `~/Applications/mdmore.app` with the icon in `artwork/icon/`. It appears in Finder's **Open With** menu for `.md` files and opens each file in a Terminal window that runs mdmore directly, skipping your shell startup. It's a convenience; mdmore is a terminal pager.

## Keys

| Key | Action |
|---|---|
| `j` / `k` | Scroll down / up a line |
| `↓` / `↑` | Move the line cursor (or the TOC selection when the TOC is open) |
| `space`, `PageDown` / `b`, `PageUp` | Page down / up |
| `gg`, `G` | Jump to start / end |
| `←` / `→` | Previous / next link (with the cursor active) |
| `Enter` | Follow the link under the cursor, or jump to the selected TOC heading |
| `t` | Toggle the table of contents |
| `/` then text, `Enter` | Search (case-insensitive); `n` / `N` next / previous match; `Esc` cancels |
| `Backspace` / `f` | Back / forward between followed links |
| `q` | Quit |

## Themes

Code blocks use the `ansi` theme by default, which follows your terminal's own palette and so suits light and dark terminals. Pick another with `--theme <NAME>` or the `MDMORE_THEME` environment variable (the flag wins). `mdmore --list-themes` prints the available names, one per line. Names are case-insensitive; an unknown name exits with code 2. Other themes use fixed RGB colours.

## Acknowledgements

- Syntax definitions and themes come from [bat](https://github.com/sharkdp/bat), packaged for syntect by [two-face](https://codeberg.org/CosmicHarper/two-face). The individual grammars carry their own licences; two-face lists them.
- **mdmore is based on [Frogmouth](https://github.com/Textualize/frogmouth)**, the Markdown browser for the terminal by Textualize (MIT licence, © 2023 Textualize, Inc). This crate began as FrogRust, a Rust reimplementation of Frogmouth: its rendered-Markdown view, table of contents and link navigation with back/forward were the starting point. It has since been reshaped into a pager. No Frogmouth source code is included; it is written from scratch in Rust.

## License

MIT. See [`LICENSE`](./LICENSE).
