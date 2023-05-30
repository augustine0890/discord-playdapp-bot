#!/bin/bash

# Set sccache as the Rust compiler wrapper
export RUSTC_WRAPPER=sccache

# Clean your Rust application
# cargo clean

# Build your Rust application
cargo build --release

# Check if the application is already running
if [ -f discord-playdapp-bot.pid ]; then
    OLD_PID=$(cat discord-playdapp-bot.pid)
    if kill -0 $OLD_PID > /dev/null 2>&1; then
        echo "Stopping running application..."
        kill $OLD_PID
        # wait for the old process to stop
        while kill -0 $OLD_PID > /dev/null 2>&1; do
            sleep 1
        done
    fi
fi

# Remove old log and pid files
rm -f discord-playdapp-bot.log discord-playdapp-bot.pid

# Print out a message indicating the application is starting
echo "Starting application..."

# Start your application in the background, redirect output to a log file,
# and store its PID in a separate file
./target/release/discord-playdapp-bot > discord-playdapp-bot.log 2>&1 & echo $! > discord-playdapp-bot.pid

# Print out a message indicating the application has been started
echo "Application has been started."