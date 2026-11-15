# FrogRust Test Document

A sample file for checking FrogRust by eye: `frogrust TEST.md`.

## Introduction

FrogRust is `more` for Markdown, with syntax highlighting: a fast pager for reading Markdown in the terminal.

### What to check

- Headings, **bold**, *italic* and `inline code` render
- Lists, tables, quotes and rules render
- Fenced code blocks below are coloured by language (not built yet; they show as plain code until syntax highlighting lands)
- An unknown language falls back to plain code styling

## Code blocks

```rust
fn main() {
    let name = "FrogRust";
    println!("Hello from {name}!");
}
```

```python
def greet(name: str) -> str:
    return f"Hello from {name}!"
```

```bash
cargo install --path .
frogrust TEST.md
```

```typescript
const greet = (name: string): string => `Hello from ${name}!`;
```

```json
{ "version": "0.1.0", "environment": "unknown", "commit": "abc1234" }
```

```toml
[package]
name = "frogrust"
version = "0.1.0"
```

```nosuchlanguage
This block names a language FrogRust doesn't know, so it should fall back to plain code.
```

```
This block names no language at all.
```

## Other blocks

> A block quote, to check indentation and styling.

| Key | Action |
|---|---|
| `j` / `k` | Scroll down / up |
| `space` | Page down |
| `q` | Quit |

---

A wide table with inline formatting:

| Feature | Description | Link |
|---|---|---|
| **Bold** | A deliberately long description cell so the table is wider than a narrow terminal window | [Example](./README.md) |
| `code` | Inline *italic* and `code` inside cells | [PRD](./docs/FrogRust_PRD.md) |

## Navigation

- `j` / `k` scroll a line; `↓` / `↑` move the line cursor
- `space` / `PageDown` page down; `b` / `PageUp` page up
- `gg` jumps to the start; `G` to the end
- `t` toggles the table of contents
- `←` / `→` move between links; `Enter` follows one
- `Backspace` / `f` go back / forward
- `q` quits

## More content

Lines to test scrolling.

Line 1
Line 2
Line 3
Line 4
Line 5
Line 6
Line 7
Line 8
Line 9
Line 10
Line 11
Line 12
Line 13
Line 14
Line 15
Line 16
Line 17
Line 18
Line 19
Line 20
Line 21
Line 22
Line 23
Line 24
Line 25

## End

If you can read this, scrolling to the end works.
