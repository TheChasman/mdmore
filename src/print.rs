//! Non-interactive output: plain text when piped, ANSI-coloured for `-F`.

use crate::document::{parser, renderer};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use std::io::Write;

/// Render a document to lines (highlighting only when `colour`).
pub fn render_lines(doc_source: &str, colour: bool, width: Option<u16>) -> Vec<Line<'static>> {
    renderer::render_with(&parser::parse(doc_source), colour, width).lines
}

/// Whether the document renders in at most `max_lines` lines. Counts without
/// highlighting (it never changes line count), so no grammars are loaded.
pub fn fits_in(doc_source: &str, width: u16, max_lines: usize) -> bool {
    render_lines(doc_source, false, Some(width)).len() <= max_lines
}

fn sgr(style: &Style) -> String {
    let mut codes: Vec<String> = Vec::new();
    match style.fg {
        Some(Color::Indexed(n)) => codes.push(format!("38;5;{n}")),
        Some(Color::Black) => codes.push("30".into()),
        Some(Color::Red) => codes.push("31".into()),
        Some(Color::Green) => codes.push("32".into()),
        Some(Color::Yellow) => codes.push("33".into()),
        Some(Color::Blue) => codes.push("34".into()),
        Some(Color::Magenta) => codes.push("35".into()),
        Some(Color::Cyan) => codes.push("36".into()),
        Some(Color::Gray) => codes.push("37".into()),
        Some(Color::DarkGray) => codes.push("90".into()),
        Some(Color::LightRed) => codes.push("91".into()),
        Some(Color::LightGreen) => codes.push("92".into()),
        Some(Color::LightYellow) => codes.push("93".into()),
        Some(Color::LightBlue) => codes.push("94".into()),
        Some(Color::LightMagenta) => codes.push("95".into()),
        Some(Color::LightCyan) => codes.push("96".into()),
        Some(Color::White) => codes.push("97".into()),
        Some(Color::Rgb(r, g, b)) => codes.push(format!("38;2;{r};{g};{b}")),
        _ => {}
    }
    let m = style.add_modifier;
    if m.contains(Modifier::BOLD) {
        codes.push("1".into());
    }
    if m.contains(Modifier::DIM) {
        codes.push("2".into());
    }
    if m.contains(Modifier::ITALIC) {
        codes.push("3".into());
    }
    if m.contains(Modifier::UNDERLINED) {
        codes.push("4".into());
    }
    if codes.is_empty() {
        String::new()
    } else {
        format!("\x1b[{}m", codes.join(";"))
    }
}

pub fn write_lines(
    lines: &[Line<'_>],
    out: &mut impl Write,
    colour: bool,
) -> std::io::Result<()> {
    for line in lines {
        let mut styled = false;
        for span in &line.spans {
            if colour {
                let style = line.style.patch(span.style);
                let s = sgr(&style);
                if !s.is_empty() {
                    out.write_all(s.as_bytes())?;
                    styled = true;
                }
            }
            out.write_all(span.content.as_bytes())?;
            if colour && styled {
                out.write_all(b"\x1b[0m")?;
                styled = false;
            }
        }
        out.write_all(b"\n")?;
    }
    Ok(())
}

pub fn print_document(
    doc_source: &str,
    out: &mut impl Write,
    colour: bool,
    width: Option<u16>,
) -> std::io::Result<()> {
    write_lines(&render_lines(doc_source, colour, width), out, colour)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_image_shows_alt() {
        let mut out = Vec::new();
        print_document("![a frog](artwork/icon/RustFrog.png)\n", &mut out, false, None).unwrap();
        assert_eq!(String::from_utf8(out).unwrap().trim(), "[image: a frog]");
    }

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
}
