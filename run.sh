#!/bin/bash

# Build the application in release mode
cargo build --release

# Run the application in a loop until it exits with a code other than 10
while true; do
    ./target/release/rust-chess-tui
    exit_code=$?
    if [ $exit_code -ne 10 ]; then
        break
    fi
done