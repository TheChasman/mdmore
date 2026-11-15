pub mod highlight;
pub mod parser;
pub mod renderer;

pub use renderer::RenderedDoc;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Path recorded for a document read from stdin.
pub const STDIN_PATH: &str = "-";

#[derive(Clone)]
pub struct Document {
    pub path: PathBuf,
    pub content: String,
    pub rendered: RenderedDoc,
}

impl Document {
    pub async fn from_file(path: &Path) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        Ok(Self::from_source(content, path.to_path_buf()))
    }

    pub fn from_source(content: String, path: PathBuf) -> Self {
        let blocks = parser::parse(&content);
        let rendered = renderer::render_with(&blocks, false);
        Self {
            path,
            content,
            rendered,
        }
    }

    /// True if any code block carries a language token (so highlighting would change output).
    pub fn needs_highlight(&self) -> bool {
        fn any_lang(blocks: &[parser::Block]) -> bool {
            blocks.iter().any(|b| match b {
                parser::Block::CodeBlock { lang, .. } => {
                    lang.as_deref().and_then(highlight::language_token).is_some()
                }
                parser::Block::BlockQuote { blocks } => any_lang(blocks),
                parser::Block::List { items, .. } => items.iter().any(|i| any_lang(i)),
                _ => false,
            })
        }
        any_lang(&parser::parse(&self.content))
    }

    /// Fully highlighted render of this document's content (slow on first grammar use).
    pub fn highlighted(&self) -> RenderedDoc {
        renderer::render_with(&parser::parse(&self.content), true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_source_renders_content() {
        let doc = Document::from_source("# Hi\n\ntext".into(), PathBuf::from(STDIN_PATH));
        assert_eq!(doc.rendered.headings.len(), 1);
        assert_eq!(doc.path, PathBuf::from("-"));
    }

    #[test]
    fn needs_highlight_false_without_languages() {
        let doc = Document::from_source("```\nplain\n```".into(), PathBuf::from("-"));
        assert!(!doc.needs_highlight());
        let doc = Document::from_source("```rust\nfn a(){}\n```".into(), PathBuf::from("-"));
        assert!(doc.needs_highlight());
    }

    #[test]
    fn from_source_empty() {
        let doc = Document::from_source(String::new(), PathBuf::from(STDIN_PATH));
        assert!(doc.rendered.lines.is_empty());
    }
}
