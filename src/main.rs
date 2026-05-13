mod app;
mod document;
mod fetch;
mod storage;
mod ui;

use anyhow::{anyhow, Result};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "frogrust")]
#[command(about = "A terminal Markdown browser")]
struct Args {
    /// Path to Markdown file to open
    path: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    let initial_path = args.path.unwrap_or_else(|| PathBuf::from("README.md"));
    
    if !initial_path.exists() {
        return Err(anyhow!("File not found: {}", initial_path.display()));
    }

    let mut app = app::App::new(initial_path).await?;
    app.run().await?;

    Ok(())
}
