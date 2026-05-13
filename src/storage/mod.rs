use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub path: PathBuf,
    pub name: String,
}

pub struct Storage {}

impl Storage {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn load_bookmarks() -> anyhow::Result<Vec<Bookmark>> {
        Ok(Vec::new())
    }

    pub async fn save_bookmarks(_bookmarks: &[Bookmark]) -> anyhow::Result<()> {
        Ok(())
    }
}
