mod app;
mod ui;
mod game;
mod ga;

use crate::app::App;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::CrosstermBackend;
use std::error::Error;
use tracing_subscriber::prelude::*;


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
fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // setup tracing
    let (log_tx, log_rx) = crossbeam_channel::unbounded();
    *app::TUI_WRITER_SENDER.lock().unwrap() = Some(log_tx);
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(TuiMakeWriter::new).with_filter(tracing_subscriber::filter::LevelFilter::INFO))
        .init();

    std::panic::set_hook(Box::new(|info| {
        let payload = info.payload().downcast_ref::<&str>().unwrap_or(&"");
        let location = info.location().unwrap();
        let msg = format!("panic occurred: {}, location: {}", payload, location);
        tracing::error!("{}", msg);
    }));


    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let mut app = App::new(args.tablebase_path, args.opening_book, log_rx);
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
    } else if let Some(err) = app.error_message {
        println!("Application exited with an error: {}", err);
    }

    Ok(())
}
