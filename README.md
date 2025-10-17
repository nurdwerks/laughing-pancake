# Rust Chess TUI

A terminal-based chess application written in Rust. Play against a configurable AI or watch two AIs battle it out.

## Features

- **Player vs. AI**: Play a game of chess against a configurable AI opponent.
- **AI vs. AI Simulation**: Watch a game between two AI players.
- **Configurable AI**: Press 'c' to open the AI configuration screen. Here you can:
    - Load and save AI "profiles" (configurations).
    - Toggle various search algorithms and pruning techniques.
- **Syzygy Tablebase Support**: For positions with 7 or fewer pieces, the AI can use Syzygy endgame tablebases to play perfectly.
- **PGN Opening Book Support**: The AI can play moves from a PGN opening book.
- **PGN Tracking**: The game's moves are tracked in PGN (Portable Game Notation) format.
- **TUI Interface**: A simple and intuitive terminal user interface built with `ratatui`.

## AI Deep Dive

This section provides a detailed explanation of the AI's decision-making process, covering both the evaluation of a position and the search for the best move.

### The Evaluation Function

The AI evaluates a chess position by assigning it a numerical score in "centipawns" (1/100th of a pawn). A positive score favors White, while a negative score favors Black. The evaluation is a weighted sum of many different positional and material factors. This modular, weighted approach allows for fine-tuning the AI's playing style.

The evaluation process works as follows:

1.  **Game Phase Calculation**: The AI first determines the game phase—a value from 0 (endgame) to 256 (opening)—based on the number and type of pieces on the board. This allows the AI to value certain features differently depending on the stage of the game.

2.  **Core Evaluation (Material & PSTs)**:
    *   **Material**: The AI starts with a simple count of the material on the board using standard piece values (e.g., Pawn=100, Knight=320, Rook=500).
    *   **Piece-Square Tables (PSTs)**: A piece's value is not static; it also depends on its position. For example, a knight in the center is more valuable than a knight on the rim. The AI uses PSTs to assign a positional bonus or penalty to each piece. Crucially, there are separate PSTs for the middlegame and the endgame, and the final PST score is interpolated based on the calculated game phase.

3.  **Weighted Components**: A series of additional evaluation components are calculated. Each component's raw score is multiplied by a weight from the AI's configuration, allowing for customization. The main components include:
    *   **Pawn Structure**:
        *   **Basic**: Penalizes doubled, isolated, and backward pawns. Rewards passed pawns.
        *   **Advanced**: Rewards pawn chains, blockading pawns ("rams"), and "candidate" pawns that have the potential to become passed.
    *   **Piece Mobility**: Rewards pieces for having more available moves. A player with a mobility advantage has more options and can adapt to the changing situation more easily.
    *   **King Safety**: This is a critical factor, especially in the middlegame.
        *   **Pawn Shield**: Rewards having a protective shield of pawns in front of the king.
        *   **Open Files**: Penalizes the king for being on or near open or semi-open files, which can be used for attacks.
        *   **Attackers**: Counts the number of enemy pieces attacking the "king zone" (the 3x3 square around the king) and applies a penalty based on the value of the attackers.
    *   **Piece Placement**:
        *   **Rooks**: Rewards rooks for being on open/semi-open files and for reaching the 7th rank.
        *   **Bishops**: Rewards the "bishop pair" and penalizes bishops that are blocked by their own pawns.
        *   **Knights**: Rewards knights for being on outposts and in the center of the board.
    *   **Development**: Encourages moving minor pieces (knights and bishops) off the back rank early in the game.
    *   **Threats & Initiative**: Rewards moves that create threats (e.g., attacking an undefended piece) that the opponent must respond to.
    *   **Space**: Measures territorial control by analyzing how many squares on the opponent's side of the board are controlled by pawns.
    *   **Tempo**: A small bonus is added for the player whose turn it is to move, encouraging proactive play.

### The Search Algorithm

The evaluation function can only score a single position. To find the best move, the AI must look ahead and explore the tree of possible future moves. This is the job of the search algorithm.

The core of the search is a **Principal Variation Search (PVS)** algorithm, which is an optimized version of the standard alpha-beta pruning algorithm. The search is governed by the `search_depth` parameter, which determines how many moves (or "ply") the AI looks ahead.

#### How Alpha-Beta Pruning Works: An Example

The goal of alpha-beta pruning is to reduce the number of nodes the AI needs to evaluate in the search tree. It does this by ignoring branches that are guaranteed to be worse than a line of play that has already been found.

Let's use a simplified evaluation function: `Score = (Our Material) - (Opponent's Material)`. The search depth is 2-ply (White moves, then Black moves).

*   **Root Node (White to move):** White wants to maximize the score. It has two moves, **Move A** and **Move B**.
    *   `alpha` (White's best guaranteed score) = -infinity
    *   `beta` (Black's best guaranteed score) = +infinity

1.  **Explore Move A**:
    *   White plays Move A. It's a quiet move, and the material is still equal. Now it's Black's turn to move.
    *   Black (the minimizer) looks at all its responses. It finds that its best response leads to a position with an evaluation of `0`.
    *   So, the score for the "Move A" branch is `0`.
    *   Back at the Root Node, White now knows it can guarantee a score of at least `0`. It updates its `alpha` to `0`.

2.  **Explore Move B**:
    *   White plays Move B, capturing a pawn. The evaluation of this position is now `+1`. It's Black's turn.
    *   Black looks at its responses. It finds a powerful counter-attack that captures one of White's knights. The evaluation of this new position is `+1 (for the pawn) - 3 (for the knight) = -2`.
    *   **PRUNING OCCURS HERE!**
        *   Black has found a response that results in a score of `-2`.
        *   This score (`-2`) is less than `alpha` (`0`).
        *   This means that if White chooses Move B, Black can force a situation that is worse for White than the one White can already guarantee with Move A.
        *   Therefore, the AI doesn't need to look at any of Black's other possible responses to Move B. The entire "Move B" branch is "pruned" or ignored.

**Conclusion:** The search algorithm returns to the root. It saw that Move A results in a score of `0`, and Move B can result in a score of `-2`. As the maximizer, it chooses Move A.

#### Advanced Search Techniques

In addition to PVS, the AI uses several other techniques to improve search efficiency and tactical accuracy:

*   **Quiescence Search**: When the normal search depth is reached, the AI performs a "quiescence search". This is a shallow search that only considers "non-quiet" moves like captures. This helps to avoid the "horizon effect," where a tactical blunder just beyond the search depth is missed.
*   **Move Ordering**: Alpha-beta pruning is most effective when the best moves are searched first. The AI uses several heuristics to order moves:
    *   **Captures**: Generally searched first.
    *   **Killer Moves**: Non-capture moves that have caused beta cutoffs at the same depth in other branches of the tree.
    *   **History Heuristic**: Moves that have been found to be good in other parts of the search are prioritized.
*   **Null Move Pruning (NMP)**: A powerful pruning technique. The AI gives the opponent an extra turn (a "null move"). If the opponent's score is still poor, it's assumed that the current position is so strong that the search can be cut short.
*   **Late Move Reductions (LMR)**: Moves that are ordered later in the list are assumed to be less promising and are searched with a reduced depth.
*   **Futility Pruning**: At shallow depths, if the static evaluation is much worse than the current best score, the search is pruned, assuming the position is "futile" to explore further.

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
- **'r'**: Restart the application (requires running via `run.sh`).
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

## Restarting the Application

To enable the application to restart itself, you must run it using the provided `run.sh` script:

```sh
./run.sh
```

This script will first build the application and then launch it. If you press 'r' in the TUI, the application will exit with a special code that the `run.sh` script will detect, causing it to relaunch the application.

## Project Structure

- `src/main.rs`: The entry point of the application.
- `src/app/mod.rs`: Contains the main application loop and state management.
- `src/ui/mod.rs`: Handles the rendering of the TUI.
- `src/game/mod.rs`: Implements the core chess logic and AI.
- `src/config.rs`: Handles saving and loading of AI profiles.
- `src/game/search/`: Contains the search algorithm and pruning technique implementations.
