

mod app;
mod ui;
mod game;
mod ga;
mod event;
pub mod server;
mod constants;
mod sts;
mod worker;

use clap::Parser;


#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the syzygy tablebase files
    #[arg(long)]
    tablebase_path: Option<String>,

    /// Path to the PGN opening book file
    #[arg(long)]
    opening_book: Option<String>,

    /// Enable the Text-based User Interface (TUI)
    #[arg(long)]
    tui: bool,
}

#[cfg(not(test))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use crate::app::App;
    use crossterm::{
        event::DisableMouseCapture,
        execute,
        terminal::{disable_raw_mode, LeaveAlternateScreen},
    };
    use ratatui::{prelude::CrosstermBackend, Terminal};
    use std::panic;
    use std::process;
    use std::thread;

    let _worker_pool = worker::WorkerPool::new();
    let args = Args::parse();

    // Get the git hash
    let git_hash = match process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        Ok(output) => String::from_utf8(output.stdout).unwrap_or_default().trim().to_string(),
        Err(_) => "N/A".to_string(),
    };

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

    let mut app = App::new(git_hash);

    if args.tui {
        // TUI mode
        let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
        let res = app.run_tui(&mut terminal).await;

        // Restore terminal
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
        }
    } else {
        // Headless mode
        println!("Running in headless mode. Use --tui to show the interface.");
        let res = app.run_headless().await;
        if let Err(err) = res {
            eprintln!("Headless mode error: {err:?}");
            process::exit(1);
        }
    }

    if let Some(err) = app.error_message {
        println!("Application exited with an error: {err}");
        process::exit(1);
    } else {
        println!("Application exited gracefully. The run.sh script will restart it shortly.");
    }

    Ok(())
}
