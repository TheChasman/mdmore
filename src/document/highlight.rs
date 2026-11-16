//! Syntax highlighting for fenced code blocks (syntect + bat's `ansi` theme).
//!
//! The `ansi` theme encodes ANSI palette indices in the colour alpha channel, so
//! the terminal's own light/dark palette decides the final colours. Grammars are
//! loaded lazily, so documents without code blocks pay nothing.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Style as SynStyle, Theme};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use two_face::theme::{EmbeddedLazyThemeSet, EmbeddedThemeName};

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
static THEME: OnceLock<Theme> = OnceLock::new();

fn syntaxes() -> &'static SyntaxSet {
    SYNTAXES.get_or_init(two_face::syntax::extra_newlines)
}

fn theme() -> &'static Theme {
    THEME.get_or_init(|| {
        two_face::theme::extra()
            .get(EmbeddedThemeName::Ansi)
            .clone()
    })
}

/// Embedded theme names, sorted.
pub fn theme_names() -> Vec<String> {
    let mut names: Vec<String> = EmbeddedLazyThemeSet::theme_names()
        .iter()
        .map(|n| n.as_name().to_string())
        .collect();
    names.sort();
    names
}

fn resolve_theme(name: &str) -> Result<EmbeddedThemeName, String> {
    EmbeddedLazyThemeSet::theme_names()
        .iter()
        .copied()
        .find(|n| n.as_name().eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("unknown theme '{name}'; run `mdmore --list-themes`"))
}

/// Pure name check (no global state).
pub fn validate_theme(name: &str) -> Result<(), String> {
    resolve_theme(name).map(|_| ())
}

/// Choose the theme; valid only before the first highlight.
pub fn set_theme(name: &str) -> Result<(), String> {
    let id = resolve_theme(name)?;
    THEME
        .set(two_face::theme::extra().get(id).clone())
        .map_err(|_| "theme already in use; set it before highlighting".to_string())
}

/// Whether the lazy grammar set has been initialised.
pub fn grammars_loaded() -> bool {
    SYNTAXES.get().is_some()
}

/// First token of a fence info string (split on whitespace or `,`), lower-cased.
pub fn language_token(info: &str) -> Option<String> {
    info.split(|c: char| c.is_whitespace() || c == ',')
        .find(|t| !t.is_empty())
        .map(str::to_lowercase)
}

pub(crate) fn plain(code: &str) -> Vec<Line<'static>> {
    code.lines()
        .map(|l| Line::from(Span::styled(l.replace('\t', "    "), Style::default())))
        .collect()
}

fn convert(style: SynStyle) -> Style {
    let c = style.foreground;
    let mut out = match c.a {
        0 => Style::default().fg(Color::Indexed(c.r)),
        1 => Style::default(),
        _ => Style::default().fg(Color::Rgb(c.r, c.g, c.b)),
    };
    if style.font_style.contains(FontStyle::BOLD) {
        out = out.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        out = out.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        out = out.add_modifier(Modifier::UNDERLINED);
    }
    out
}

/// One `Line` per source line. Unknown language or any highlighter error gives
/// unstyled lines.
pub fn highlight_code(code: &str, info: Option<&str>) -> Vec<Line<'static>> {
    let Some(token) = info.and_then(language_token) else {
        return plain(code);
    };
    let set = syntaxes();
    let Some(syntax) = set.find_syntax_by_token(&token) else {
        return plain(code);
    };
    let expanded = code.replace('\t', "    ");
    let mut hl = HighlightLines::new(syntax, theme());
    let mut lines = Vec::new();
    for raw in LinesWithEndings::from(&expanded) {
        let Ok(ranges) = hl.highlight_line(raw, set) else {
            return plain(code);
        };
        let spans: Vec<Span<'static>> = ranges
            .into_iter()
            .filter_map(|(st, text)| {
                let text = text.trim_end_matches(['\n', '\r']);
                (!text.is_empty()).then(|| Span::styled(text.to_string(), convert(st)))
            })
            .collect();
        lines.push(if spans.is_empty() {
            Line::from("")
        } else {
            Line::from(spans)
        });
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Style};

    fn coloured_spans(lines: &[ratatui::text::Line]) -> usize {
        lines
            .iter()
            .flat_map(|l| &l.spans)
            .filter(|s| s.style.fg.is_some())
            .count()
    }

    #[test]
    fn theme_names_include_ansi() {
        assert!(theme_names().iter().any(|n| n == "ansi"));
    }

    #[test]
    fn unknown_theme_errors() {
        let err = validate_theme("no-such-theme").unwrap_err();
        assert!(err.contains("unknown theme 'no-such-theme'"));
        assert!(validate_theme("ansi").is_ok());
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
        assert_eq!(
            language_token("python title=\"x\"").as_deref(),
            Some("python")
        );
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
            assert!(
                coloured_spans(&highlight_code(code, Some(alias))) > 0,
                "{alias} not resolved"
            );
        }
    }

    #[test]
    fn unknown_or_missing_language_is_plain() {
        for info in [Some("nosuchlanguage"), None] {
            let lines = highlight_code("plain text here\nsecond", info);
            assert_eq!(lines.len(), 2);
            assert!(lines
                .iter()
                .flat_map(|l| &l.spans)
                .all(|s| s.style == Style::default()));
        }
    }

    #[test]
    fn preserves_line_structure() {
        let code = "fn a() {}\n\n  let y = 2;   \n";
        let lines = highlight_code(code, Some("rust"));
        assert_eq!(lines.len(), code.lines().count());
        assert!(lines
            .iter()
            .flat_map(|l| &l.spans)
            .all(|s| !s.content.contains('\n')));
        let text: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
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
            assert!(
                !matches!(span.style.fg, Some(Color::Rgb(..))),
                "RGB fg leaked: {span:?}"
            );
            assert!(span.style.bg.is_none(), "background set: {span:?}");
        }
    }
}
