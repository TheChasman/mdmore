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
        let rendered = renderer::render(&blocks);
        Self {
            path,
            content,
            rendered,
        }
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
    fn from_source_empty() {
        let doc = Document::from_source(String::new(), PathBuf::from(STDIN_PATH));
        assert!(doc.rendered.lines.is_empty());
    }
}
