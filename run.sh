#!/bin/bash

# Build the application in release mode
cargo build --release

# In the run script before running the application do a git pull. Before restarting wait for 5 seconds after the application closes
# Run the application in a loop. The application will always restart when it exits.
# To stop the script, use Ctrl+C.
git pull
while true; do
    ./target/release/rust-chess-tui
    sleep 5
done