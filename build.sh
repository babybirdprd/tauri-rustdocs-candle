#!/bin/bash

# Exit immediately if a command exits with a non-zero status.
set -e

echo "Starting build process..."

# Step 1: Install frontend dependencies
echo "Installing frontend dependencies with pnpm..."
if ! command -v pnpm &> /dev/null
then
    echo "pnpm could not be found. Please install pnpm first."
    echo "You can install it via npm: npm install -g pnpm"
    exit 1
fi
pnpm install

# Step 2: Build the Tauri application
echo "Building the Tauri application..."
if ! command -v cargo &> /dev/null
then
    echo "cargo (Rust toolchain) could not be found. Please install Rust."
    exit 1
fi
# Assuming tauri-cli is installed via cargo install tauri-cli or is a direct project dependency.
# If tauri-cli is not globally available, this command might need to be `cargo tauri` if invoked from within a crate that depends on tauri-cli.
# However, `cargo tauri build` is the standard command when tauri-cli is installed.
cargo tauri build

echo "Build process completed successfully."
echo "The application bundle can be found in src-tauri/target/release/bundle/"
