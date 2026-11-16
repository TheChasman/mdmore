pub mod highlight;
pub mod parser;
pub mod renderer;

use anyhow::Result;
pub use renderer::RenderedDoc;
use std::path::{Path, PathBuf};

/// Path recorded for a document read from stdin.
pub const STDIN_PATH: &str = "-";

#[derive(Clone)]
pub struct Document {
    pub path: PathBuf,
    pub content: String,
    pub rendered: RenderedDoc,
    /// Width text is wrapped to; None leaves lines unwrapped.
    pub width: Option<u16>,
}

impl Document {
    pub fn from_file(path: &Path, width: Option<u16>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_source(content, path.to_path_buf(), width))
    }

    /// Renders `content` (unhighlighted) wrapped to `width`.
    pub fn from_source(content: String, path: PathBuf, width: Option<u16>) -> Self {
        let blocks = parser::parse(&content);
        let rendered = renderer::render_with_base(&blocks, false, width, Some(&base_dir(&path)));
        Self {
            path,
            content,
            rendered,
            width,
        }
    }

    /// Re-renders (unhighlighted) at `width`. Returns false if already at that width.
    pub fn set_width(&mut self, width: Option<u16>) -> bool {
        if self.width == width {
            return false;
        }
        let blocks = parser::parse(&self.content);
        self.rendered =
            renderer::render_with_base(&blocks, false, width, Some(&base_dir(&self.path)));
        self.width = width;
        true
    }

    /// True if any code block carries a language token (so highlighting would change output).
    pub fn needs_highlight(&self) -> bool {
        fn any_lang(blocks: &[parser::Block]) -> bool {
            blocks.iter().any(|b| match b {
                parser::Block::CodeBlock { lang, .. } => lang
                    .as_deref()
                    .and_then(highlight::language_token)
                    .is_some(),
                parser::Block::BlockQuote { blocks } => any_lang(blocks),
                parser::Block::List { items, .. } => items.iter().any(|i| any_lang(i)),
                _ => false,
            })
        }
        any_lang(&parser::parse(&self.content))
    }

    /// Fully highlighted render of this document's content (slow on first grammar use).
    pub fn highlighted(&self) -> RenderedDoc {
        renderer::render_with_base(
            &parser::parse(&self.content),
            true,
            self.width,
            Some(&base_dir(&self.path)),
        )
    }
}

/// Directory local image urls resolve against: the file's directory, or cwd for stdin.
fn base_dir(path: &Path) -> PathBuf {
    if path.as_os_str() == STDIN_PATH {
        return std::env::current_dir().unwrap_or_default();
    }
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => std::env::current_dir().unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_source_renders_content() {
        let doc = Document::from_source("# Hi\n\ntext".into(), PathBuf::from(STDIN_PATH), None);
        assert_eq!(doc.rendered.headings.len(), 1);
        assert_eq!(doc.path, PathBuf::from("-"));
    }

    #[test]
    fn needs_highlight_false_without_languages() {
        let doc = Document::from_source("```\nplain\n```".into(), PathBuf::from("-"), None);
        assert!(!doc.needs_highlight());
        let doc = Document::from_source("```rust\nfn a(){}\n```".into(), PathBuf::from("-"), None);
        assert!(doc.needs_highlight());
    }

    #[test]
    fn set_width_rewraps_and_highlight_matches() {
        let src = "aaa bbb ccc\n\n```rust\nfn a() {}\n```".to_string();
        let mut doc = Document::from_source(src, PathBuf::from("-"), None);
        assert_eq!(doc.rendered.lines.len(), 6);
        assert!(doc.set_width(Some(4)));
        assert!(!doc.set_width(Some(4)));
        assert_eq!(doc.rendered.lines.len(), 8);
        assert_eq!(doc.highlighted().lines.len(), doc.rendered.lines.len());
    }

    #[test]
    fn from_source_empty() {
        let doc = Document::from_source(String::new(), PathBuf::from(STDIN_PATH), None);
        assert!(doc.rendered.lines.is_empty());
    }
}
