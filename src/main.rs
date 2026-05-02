use std::io;
use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::path::PathBuf;
use clap::Parser;

mod app;
mod config;
mod tabs;
mod ui;
mod file_browser;
mod editor;
mod utils;
mod theme;

use app::App;

#[derive(Parser)]
#[command(name = "nfm")]
#[command(about = "NeoFM - Modern TUI file manager with built-in editor", long_about = None)]
struct Cli {
    #[arg(name = "path")]
    path: Option<PathBuf>,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    
    // Config setup
    config::Config::create_default_if_not_exists();
    let conf = config::Config::load();

    // Start path
    let start_path = if let Some(p) = cli.path {
        p.canonicalize().unwrap_or(p)
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
    };

    // Terminal initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // App loop
    let mut app = App::new(conf, start_path);
    let res = run_app(&mut terminal, &mut app);

    // Terminal cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if app.should_quit {
            return Ok(());
        }

        app.run_tick();
        app.handle_events()?;
    }
}
