mod app;
mod ui;
mod game;
mod ga;
mod event;
mod server;

use crate::app::{App};
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::CrosstermBackend, Terminal};
use std::{error::Error, io, panic, process, thread};


#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the syzygy tablebase files
    #[arg(long)]
    tablebase_path: Option<String>,

    /// Path to the PGN opening book file
    #[arg(long)]
    opening_book: Option<String>,
}

#[cfg(not(test))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let _args = Args::parse();

    // Start the server in a new thread
    thread::spawn(|| {
        if let Err(e) = actix_rt::System::new().block_on(server::start_server()) {
            eprintln!("Server error: {e}");
        }
    });

    panic::set_hook(Box::new(|info| {
        let payload = info.payload().downcast_ref::<&str>().unwrap_or(&"");
        let location = info.location().unwrap();
        let msg = format!("panic occurred: {payload}, location: {location}");
        eprintln!("{msg}");
    }));


    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let mut app = App::new();
    let res = app.run(&mut terminal).await;

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
        process::exit(1);
    } else if let Some(err) = app.error_message {
        println!("Application exited with an error: {err}");
        process::exit(1);
    } else if app.should_restart {
        process::exit(10);
    }

    Ok(())
}