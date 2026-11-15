mod app;
mod document;
mod fetch;
mod storage;
mod ui;
mod version;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "frogrust")]
#[command(about = "A terminal Markdown browser")]
#[command(disable_version_flag = true, args_conflicts_with_subcommands = true)]
struct Args {
    /// Path to Markdown file to open
    path: Option<PathBuf>,

    /// Print version, environment and commit as JSON
    #[arg(short = 'V', long)]
    version: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Print version, environment and commit as JSON
    Version,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.version || matches!(args.command, Some(Command::Version)) {
        println!("{}", version::version_json());
        return Ok(());
    }

    let initial_path = args.path.unwrap_or_else(|| PathBuf::from("README.md"));
    
    if !initial_path.exists() {
        return Err(anyhow!("File not found: {}", initial_path.display()));
    }

    let mut app = app::App::new(initial_path).await?;
    app.run().await?;

    Ok(())
}
