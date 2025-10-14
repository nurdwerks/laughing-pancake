# Rust Chess TUI

A terminal-based chess application written in Rust. Play against a simple AI or watch two AIs battle it out.

## Features

- **Player vs. AI**: Play a game of chess against a simple AI opponent. The player is White, and the AI is Black.
- **AI vs. AI Simulation**: Watch a game between two AI players.
- **Syzygy Tablebase Support**: For positions with 7 or fewer pieces, the AI can use Syzygy endgame tablebases to play perfectly.
- **PGN Tracking**: The game's moves are tracked in PGN (Portable Game Notation) format using Standard Algebraic Notation (SAN) with move numbers.
- **TUI Interface**: A simple and intuitive terminal user interface built with `ratatui`.

## Dependencies

- `shakmaty`: For chess logic.
- `shakmaty-syzygy`: For Syzygy tablebase probing.
- `ratatui`: For the terminal user interface.
- `crossterm`: As a backend for `ratatui`.
- `rand`: For the AI's random move generation (when not using tablebases).
- `clap`: For command-line argument parsing.

## Building and Running

1. **Clone the repository**:
   ```sh
   git clone <repository-url>
   cd rust-chess-tui
   ```

2. **Build the project**:
   ```sh
   cargo build --release
   ```

3. **Run the application**:
   ```sh
   ./target/release/rust-chess-tui
   ```

## How to Play

- The application will launch in "Player vs. AI" mode.
- You play as White. The AI plays as Black.
- To make a move, type the move in UCI notation (e.g., "e2e4") and press Enter.
- The selected squares will be highlighted as you type.
- If the move is invalid, an error message will be displayed.
- Press 's' to switch to "AI vs. AI" simulation mode. Press 's' again to switch back. The game will reset when you switch modes.
- Press 'q' to quit the application at any time.

## Endgame Tablebases

This application supports Syzygy endgame tablebases, which allow the AI to play perfectly in endgames with 7 or fewer pieces. To use this feature, you need to download the tablebase files and provide the path to them using a command-line argument.

### Downloading Tablebases

You can download the Syzygy tablebases from [https://tablebase.lichess.ovh/tables/standard/](https://tablebase.lichess.ovh/tables/standard/). It is recommended to download the 3, 4, and 5-piece sets for a good starting point.

### Using Tablebases

To use the tablebases, run the application with the `--tablebase-path` argument, followed by the path to the directory where you've stored the `.rtbw` and `.rtbz` files:

```sh
./target/release/rust-chess-tui --tablebase-path /path/to/your/syzygy-tables
```

If the path is invalid or no tablebase files are found, a warning will be displayed, and the application will continue without tablebase support.

## Project Structure

- `src/main.rs`: The entry point of the application.
- `src/app/mod.rs`: Contains the main application loop and state management.
- `src/ui/mod.rs`: Handles the rendering of the TUI.
- `src/game/mod.rs`: Implements the core chess logic and AI.
