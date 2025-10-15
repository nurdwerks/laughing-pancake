mod app;
mod config;
mod ui;
mod game;
mod ga;

use app::App;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, Terminal};
use std::{error::Error, io};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the syzygy tablebase files
    #[arg(long)]
    tablebase_path: Option<String>,

    /// Path to the PGN opening book file
    #[arg(long)]
    opening_book: Option<String>,

    /// Run the genetic algorithm to evolve the AI
    #[arg(long)]
    evolve: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    if args.evolve {
        ga::run();
    } else {
        // setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // create app and run it
        let mut app = App::new(args.tablebase_path, args.opening_book);
        let res = app.run(&mut terminal);

        // restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        if let Err(err) = res {
            println!("{err:?}");
        }
    }

    Ok(())
}
