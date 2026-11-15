use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use frogrust::{app, version};
use frogrust::document::{Document, STDIN_PATH};
use std::io::{IsTerminal, Read};
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

    let from_stdin = match &args.path {
        Some(p) => p.as_os_str() == STDIN_PATH,
        None => !std::io::stdin().is_terminal(),
    };

    // Read stdin before raw mode; crossterm reads keys from /dev/tty.
    let mut app = if from_stdin {
        let mut content = String::new();
        std::io::stdin().read_to_string(&mut content)?;
        let doc = Document::from_source(content, PathBuf::from(STDIN_PATH));
        app::App::with_document(PathBuf::from(STDIN_PATH), doc)?
    } else {
        let initial_path = args.path.unwrap_or_else(|| PathBuf::from("README.md"));
        if !initial_path.exists() {
            return Err(anyhow!("File not found: {}", initial_path.display()));
        }
        app::App::new(initial_path).await?
    };
    app.run().await?;

    Ok(())
}
