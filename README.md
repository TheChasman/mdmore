# FrogRust - Markdown Viewer

A terminal-based Markdown viewer built in Rust.

## Features

- Syntax highlighting for Markdown
- Cursor-based navigation
- Link following
- Table of Contents

## Usage

```bash
cargo run --release -- <file.md>
```

## Install (macOS)

```bash
scripts/deploy-mac.sh
```

Builds a native release binary and installs `~/Applications/FrogRust.app` using the icon in `artwork/icon/`. FrogRust appears in Finder's **Open With** menu for `.md` files and accepts files dropped on its icon; launched on its own it asks for a file. Each file opens in a new Terminal window that runs FrogRust directly, skipping your shell startup. The plain binary is at `~/Applications/FrogRust.app/Contents/MacOS/frogrust`.
