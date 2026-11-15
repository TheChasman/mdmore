# FrogRust

`more` for Markdown, with syntax highlighting. A single fast Rust binary for reading Markdown in your terminal.

```bash
frogrust README.md      # read it rendered, press q to quit
```

Scope, priorities and decisions live in the PRD: [`docs/FrogRust_PRD.md`](docs/FrogRust_PRD.md).

## Status

| | |
|---|---|
| Rendered Markdown (headings, emphasis, lists, tables, quotes, links) | ✅ |
| Pager scrolling, TOC sidebar, follow local links, back/forward | ✅ |
| `--version` (JSON: version, environment, commit) | ✅ |
| **Syntax highlighting in code blocks**, the point of FrogRust | ✅ |
| Reading from stdin (`cat x.md \| frogrust` / `frogrust -`) | ✅ |
| Search (`/`, `n`, `N`) | ✅ |
| Plain output when piped (`frogrust x.md \| head`) | ✅ |
| `-F` / `--quit-if-one-screen` (`frogrust -F short.md`) | ✅ |
| Inline images (iTerm2, Kitty, Sixel; half-blocks elsewhere). Local files only; for documents piped on stdin, relative paths resolve against the current directory | ✅ |

## Install

```bash
cargo install --path .    # installs frogrust into ~/.cargo/bin
```

Build natively on each machine (Apple Silicon or Intel; no cross-compiling).

### Optional: macOS app bundle

```bash
scripts/deploy-mac.sh
```

Installs `~/Applications/FrogRust.app` with the icon in `artwork/icon/`. It appears in Finder's **Open With** menu for `.md` files and opens each file in a Terminal window that runs FrogRust directly, skipping your shell startup. It's a convenience; FrogRust is a terminal pager.

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

Code blocks use the `ansi` theme by default, which follows your terminal's own palette and so suits light and dark terminals. Pick another with `--theme <NAME>` or the `FROGRUST_THEME` environment variable (the flag wins). `frogrust --list-themes` prints the available names, one per line. Names are case-insensitive; an unknown name exits with code 2. Other themes use fixed RGB colours.

## License

MIT. See [`LICENSE`](./LICENSE).
