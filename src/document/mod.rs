pub mod highlight;
pub mod parser;
pub mod renderer;

pub use renderer::RenderedDoc;
use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Document {
    pub path: PathBuf,
    pub content: String,
    pub rendered: RenderedDoc,
}

impl Document {
    pub async fn from_file(path: &Path) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        let blocks = parser::parse(&content);
        let rendered = renderer::render(&blocks);

        Ok(Self {
            path: path.to_path_buf(),
            content,
            rendered,
        })
    }
}
