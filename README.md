# Rust Chess TUI

A terminal-based chess application written in Rust. Play against a simple AI or watch two AIs battle it out.

## Features

- **Player vs. AI**: Play a game of chess against a simple AI opponent. The player is White, and the AI is Black.
- **AI vs. AI Simulation**: Watch a game between two AI players.
- **Random Move AI**: The AI chooses a random valid move from the list of legal moves.
- **PGN Tracking**: The game's moves are tracked in PGN (Portable Game Notation) format using Standard Algebraic Notation (SAN) with move numbers.
- **TUI Interface**: A simple and intuitive terminal user interface built with `ratatui`.

## Dependencies

- `shakmaty`: For chess logic.
- `ratatui`: For the terminal user interface.
- `crossterm`: As a backend for `ratatui`.
- `rand`: For the AI's random move generation.

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

## Project Structure

- `src/main.rs`: The entry point of the application.
- `src/app/mod.rs`: Contains the main application loop and state management.
- `src/ui/mod.rs`: Handles the rendering of the TUI.
- `src/game/mod.rs`: Implements the core chess logic and AI.
