# Current GUI Description

## 1. Introduction

This document describes the existing graphical user interface (GUI) for the chess AI evolution application as it is currently implemented. It details the various screens and UI components used for monitoring the genetic algorithm and viewing historical data.

All pages are styled using a shared stylesheet (`static/main_style.css`).

## 2. Screens

### 2.1. Main Dashboard (`index.html`)

This is the primary screen for real-time monitoring of the application. The layout is divided into a top status bar, a central area for active matches, and a bottom area containing logs, worker threads, and system statistics.

- **2.1.1. Top Status Bar (`#status-bar`)**
    - A header bar containing three dynamically updating status gauges:
    - **Generation Status:** Displays the current generation, round, and match progress with a progress bar.
    - **CPU Status:** Displays the total CPU usage percentage with a progress bar.
    - **Memory Status:** Displays system memory usage (e.g., "X.XX / Y.YY GB") with a progress bar.

- **2.1.2. Active Matches Container (`#matches-container`)**
    - A dynamic grid that displays all currently running chess matches in "match panes".
    - **Each match pane contains:**
        - **Header:** Displays the match ID and player numbers in the format `M{id}: {white_num} v {black_num}`.
        - **Chessboard:** A visual representation of the current board state. The same piece symbols (♟, ♜, etc.) are used for both black and white pieces, distinguished by the CSS classes `.white-piece` and `.black-piece`.
        - **SAN Container:** A text area displaying the current evaluation (`Eval`), material difference (`Material`), and a list of moves in Standard Algebraic Notation (SAN).

- **2.1.3. Bottom Container (`#bottom-container`)**
    - A flex container divided into three sections:
    - **Log Pane:** A scrollable pane that displays a running log of events from the backend. New entries are added to the bottom, and the view auto-scrolls.
    - **Worker Threads Pane:** Displays a list of active AI search threads, sorted with the longest-running at the top. Each entry shows the thread's elapsed time and its name (e.g., `_s: Worker-M123`).
    - **System Statistics Pane:** Displays a list of component temperatures (e.g., `CPU Package: 50.00°C`).

- **2.1.4. Controls (`#buttons`)**
    - A set of controls located at the bottom of the page.
    - **Buttons:**
        - `View History`: A link that navigates to `history.html`.
        - `Request Quit`: Sends a graceful shutdown request to the backend.
        - `Force Quit`: Sends an immediate termination request.
        - `Reset Simulation`: Prompts the user for confirmation and then sends a request to reset the evolution process.

### 2.2. Evolution History (`history.html`)

This screen provides a high-level, tabular overview of all completed generations.

- **2.2.1. Generations Table (`.data-table`)**
    - A table listing every completed generation.
    - **Columns:**
        - **Generation:** The generation number, which links to the detailed view on `generation.html`.
        - **Individuals:** The number of individuals in the population.
        - **Matches:** The total number of matches played.
        - **White Wins:** Total wins for the white player.
        - **Black Wins:** Total wins for the black player.
        - **Draws:** Total number of drawn matches.
        - **Top ELO:** The highest ELO achieved in the generation.
        - **Avg ELO:** The average ELO of the population.
        - **Lowest ELO:** The lowest ELO in the generation.

### 2.3. Generation Details (`generation.html`)

This screen provides a detailed, expandable view of individuals within each generation.

- **2.3.1. Layout**
    - The page displays a list of collapsible buttons, one for each generation (e.g., "Generation 0", "Generation 1").
    - Clicking a button fetches and displays a detailed table for that generation's population.

- **2.3.2. Individuals Table (`.data-table`)**
    - A table listing every individual from the selected generation.
    - **Columns:**
        - **ID:** The individual's ID, which links to its specific `individual.html` page.
        - **ELO:** The individual's final ELO rating.
        - **Configuration:** The full `SearchConfig` of the AI, displayed in a formatted `<pre>` block.
    - A "View History" link is present at the bottom of the page.

### 2.4. Individual Details (`individual.html`)

This screen displays all information related to a single AI individual from a specific generation.

- **2.4.1. Header and Configuration**
    - A title displaying the individual's ID and generation number.
    - A formatted `<pre>` block showing the individual's complete `SearchConfig` JSON.

- **2.4.2. STS ELO Estimation**
    - A section for running Strategic Test Suite (STS) benchmarks.
    - **Controls:**
        - `Run STS Test` button to initiate a new test run for this individual.
    - **Display:**
        - Shows the `Progress`, `Score`, and estimated `ELO` from the test run. This section updates in real-time via WebSocket if a test is running.

- **2.4.3. Matches Table**
    - A table listing all matches played by this individual during its generation's tournament.
    - **Columns:**
        - **Round:** The tournament round number.
        - **White:** The name of the white player file.
        - **Black:** The name of the black player file.
        - **Result:** The match result (e.g., "1-0").
        - **PGN:** The full list of moves in Standard Algebraic Notation.

### 2.5. STS Dashboard (`sts.html`)

This is a placeholder page. It contains a title and text directing the user to navigate to an individual's detail page to run an STS test. No functionality is implemented on this page itself.
