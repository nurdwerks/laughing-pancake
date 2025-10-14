# Rust Chess TUI

A terminal-based chess application written in Rust. Play against a configurable AI or watch two AIs battle it out.

## Features

- **Player vs. AI**: Play a game of chess against a configurable AI opponent.
- **AI vs. AI Simulation**: Watch a game between two AI players.
- **Configurable AI**: Press 'c' to open the AI configuration screen. Here you can:
    - Load and save AI "profiles" (configurations).
    - Toggle various search algorithms and pruning techniques.
- **Advanced Search Algorithm**: The AI uses an alpha-beta search algorithm with the following features:
    - **Quiescence Search**: To mitigate the horizon effect and improve tactical calculations.
    - *Future (stubbed) implementations*: Principal Variation Search (PVS), Null Move Pruning, Late Move Reductions (LMR), Futility Pruning, and Delta Pruning.
- **Syzygy Tablebase Support**: For positions with 7 or fewer pieces, the AI can use Syzygy endgame tablebases to play perfectly.
- **PGN Opening Book Support**: The AI can play moves from a PGN opening book.
- **PGN Tracking**: The game's moves are tracked in PGN (Portable Game Notation) format.
- **TUI Interface**: A simple and intuitive terminal user interface built with `ratatui`.
- **Advanced Evaluation**: The AI's evaluation function includes several advanced features:
    - **Pawn Structure**: Penalizes doubled, isolated, and backward pawns, and rewards passed pawns.
    - **Piece Mobility**: Rewards pieces for having more available moves.
    - **King Safety**: Analyzes the pawn shield around the king and penalizes for open files.
    - **Piece Development**: Encourages the development of minor pieces and penalizes moving the queen too early.
    - **Static Exchange Evaluation (SEE)**: Used in quiescence search to prune losing captures.
    - **Rook Placement**: Rewards rooks for being on open and semi-open files, and for reaching the 7th rank.
    - **Bishop Placement**: Provides a bonus for the bishop pair and penalizes "bad" bishops that are blocked by their own pawns.
    - **Knight Placement**: Rewards knights for being on outposts and being centralized.

## Dependencies

- `shakmaty`: For chess logic.
- `shakmaty-syzygy`: For Syzygy tablebase probing.
- `ratatui`: For the terminal user interface.
- `crossterm`: As a backend for `ratatui`.
- `rand`: For random move generation.
- `clap`: For command-line argument parsing.
- `serde` & `serde_json`: For saving and loading AI configurations.

## Building and Running

1.  **Clone the repository**:
    ```sh
    git clone <repository-url>
    cd rust-chess-tui
    ```

2.  **Build the project**:
    ```sh
    cargo build --release
    ```

3.  **Run the application**:
    ```sh
    ./target/release/rust-chess-tui
    ```

## How to Play

- The application will launch in "Player vs. AI" mode. You play as White.
- To make a move, type the move in UCI notation (e.g., "e2e4") and press Enter.
- **'s'**: Switch between "Player vs. AI" and "AI vs. AI" modes.
- **'c'**: Open the AI configuration screen.
- **'q'**: Quit the application.

### AI Configuration Screen

- **Up/Down Arrows**: Navigate the list of profiles.
- **'k'/'j'**: Navigate the list of AI settings on the right.
- **'h'/'l'**: Adjust the value of the selected setting. All evaluation weights are percentages, so a value of 100 is the default.
- **Enter**: Load the selected profile. The screen will close, and the AI will use the new settings.
- **'s'**: Save the current settings to the selected profile.
- **'c' or Esc**: Close the configuration screen without loading a new profile.

## Endgame Tablebases and Opening Books

This application supports Syzygy endgame tablebases and PGN opening books to enhance the AI's play.

- **Tablebases**: Download from [Lichess Tablebase](https://tablebase.lichess.ovh/tables/standard/) and use the `--tablebase-path` argument.
- **Opening Books**: Use any PGN file and provide the path with the `--opening-book` argument.

Example:
```sh
./target/release/rust-chess-tui --tablebase-path /path/to/syzygy --opening-book /path/to/book.pgn
```

## Project Structure

- `src/main.rs`: The entry point of the application.
- `src/app/mod.rs`: Contains the main application loop and state management.
- `src/ui/mod.rs`: Handles the rendering of the TUI.
- `src/game/mod.rs`: Implements the core chess logic and AI.
- `src/config.rs`: Handles saving and loading of AI profiles.
- `src/game/search/`: Contains the search algorithm and pruning technique implementations.
