#!/bin/bash

set -e

echo "=== Installing Rust and Cargo ==="

# Install Rust using rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Add Rust to PATH
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> $HOME/.profile
source $HOME/.cargo/env

# Verify installation
echo "=== Verifying Rust installation ==="
rustc --version
cargo --version

echo "=== Installing additional Rust components ==="
rustup component add clippy rustfmt

echo "=== Installing system dependencies ==="
# Install system dependencies that might be needed
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    ffmpeg \
    curl \
    wget

echo "=== Rust environment setup completed ==="
