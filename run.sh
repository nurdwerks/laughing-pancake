#!/bin/bash

# Build the application in release mode
cargo build --release

# Run the application in a loop. The application will always restart when it exits.
# To stop the script, use Ctrl+C.
while true; do
    ./target/release/rust-chess-tui
done