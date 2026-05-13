use anyhow::Result;
use std::path::Path;

pub struct Fetcher {}

impl Fetcher {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn fetch_file(&self, path: &Path) -> Result<String> {
        tokio::fs::read_to_string(path).await.map_err(Into::into)
    }
}
