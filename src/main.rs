use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use frogrust::{app, print, version};
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

    /// Print and exit if the document fits on one screen
    #[arg(short = 'F', long = "quit-if-one-screen")]
    quit_if_one_screen: bool,

    /// Highlighting theme (default: ansi; env FROGRUST_THEME); see --list-themes
    #[arg(long)]
    theme: Option<String>,

    /// List available highlighting themes and exit
    #[arg(long)]
    list_themes: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Print version, environment and commit as JSON
    Version,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Restore the terminal before the panic message so it isn't lost in raw mode / alt screen.
    let default_hook = std::panic::take_hook();
    // Only the main thread owns the terminal; a panic on the background highlight
    // thread must not tear down the screen while the pager keeps running.
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().name() == Some("main") {
            app::restore_terminal();
        }
        default_hook(info);
    }));

    if args.version || matches!(args.command, Some(Command::Version)) {
        println!("{}", version::version_json());
        return Ok(());
    }

    if args.list_themes {
        for name in frogrust::document::highlight::theme_names() {
            println!("{name}");
        }
        return Ok(());
    }
    let theme = args.theme.clone().or_else(|| std::env::var("FROGRUST_THEME").ok().filter(|s| !s.is_empty()));
    if let Some(name) = &theme {
        if let Err(e) = frogrust::document::highlight::set_theme(name) {
            eprintln!("frogrust: {e}");
            std::process::exit(2);
        }
    }

    let from_stdin = match &args.path {
        Some(p) => p.as_os_str() == STDIN_PATH,
        None => !std::io::stdin().is_terminal(),
    };

    let stdout_tty = std::io::stdout().is_terminal();
    let need_text = !stdout_tty || args.quit_if_one_screen;

    // Read stdin before raw mode; crossterm reads keys from /dev/tty.
    let mut app = if from_stdin {
        let mut content = String::new();
        std::io::stdin().read_to_string(&mut content)?;
        if let Some(()) = print_if_needed(&content, stdout_tty, args.quit_if_one_screen)? {
            return Ok(());
        }
        let doc = Document::from_source(content, PathBuf::from(STDIN_PATH));
        app::App::with_document(PathBuf::from(STDIN_PATH), doc)?
    } else {
        let initial_path = args.path.unwrap_or_else(|| PathBuf::from("README.md"));
        if !initial_path.exists() {
            return Err(anyhow!("File not found: {}", initial_path.display()));
        }
        if need_text {
            let content = std::fs::read_to_string(&initial_path)?;
            if let Some(()) = print_if_needed(&content, stdout_tty, args.quit_if_one_screen)? {
                return Ok(());
            }
        }
        app::App::new(initial_path)?
    };
    app.run()?;

    Ok(())
}

/// Print instead of paging when piped, or with -F when the document fits.
/// Returns Some(()) if it printed (caller should exit).
fn print_if_needed(content: &str, stdout_tty: bool, quit_if_one_screen: bool) -> Result<Option<()>> {
    let mut out = std::io::stdout().lock();
    if !stdout_tty {
        match print::print_document(content, &mut out, false, None) {
            Ok(()) => {}
            // `| head` closes the pipe early; that is a normal exit.
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
            Err(e) => return Err(e.into()),
        }
        return Ok(Some(()));
    }
    if quit_if_one_screen {
        let (cols, rows) = crossterm::terminal::size()?;
        if print::fits_in(content, cols, usize::from(rows.saturating_sub(2))) {
            print::print_document(content, &mut out, true, Some(cols))?;
            return Ok(Some(()));
        }
    }
    Ok(None)
}
