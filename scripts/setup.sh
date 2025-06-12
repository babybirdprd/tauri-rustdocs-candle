#!/usr/bin/env bash
#
# setup_rust_environment.sh
#
# This script provides a complete, idempotent solution for setting up a modern
# Rust development environment on Debian-based systems (like Ubuntu).
#
# It performs the following actions:
#   1. Thoroughly removes any existing system-wide or user-installed Rust versions.
#   2. Installs essential build tools and libraries required by many Rust crates.
#   3. Installs the latest stable version of Rust and Cargo using rustup.
#   4. Verifies the installed Rust version meets a minimum requirement (> 1.80).
#
# Usage:
#   chmod +x setup_rust_environment.sh
#   sudo ./setup_rust_environment.sh
#
# Note: This script is designed to be run as root (`sudo`) because it manages
# system packages and cleans up system-level directories.
#
set -euo pipefail

# --- Helper Functions for Colored Output ---
info() {
    echo -e "\e[33m[INFO]\e[0m $1"
}

success() {
    echo -e "\e[32m[SUCCESS]\e[0m $1"
}

error() {
    echo -e "\e[31m[ERROR]\e[0m $1"
}

# --- Root User Check ---
if [[ "$EUID" -ne 0 ]]; then
    error "This script must be run as root. Please use: sudo $0"
    exit 1
fi

# ==============================================================================
#  STEP 1: THOROUGH RUST REMOVAL
# ==============================================================================
info "--- Starting Step 1: Complete Removal of Existing Rust Installations ---"

# Purge any Rust version installed via apt
if dpkg -l | grep -E -q '^\w+ +rustc|^\w+ +cargo'; then
    info "Detected Rust/Cargo via apt. Purging packages..."
    apt-get update -qq
    DEBIAN_FRONTEND=noninteractive apt-get purge -y rustc cargo || true
    DEBIAN_FRONTEND=noninteractive apt-get autoremove -y || true
    success "Debian/Ubuntu-packaged Rust has been purged."
else
    info "No rustc/cargo packages found via apt. Skipping purge."
fi

# Remove executables from common system paths
BINS=(rustc cargo rustdoc rustfmt clippy-driver rls rust-gdb rust-lldb)
for bin in "${BINS[@]}"; do
    if [[ -f "/usr/bin/$bin" ]]; then
        info "Removing system binary: /usr/bin/$bin"
        rm -f "/usr/bin/$bin"
    fi
done

# Remove libraries, docs, and man pages
info "Removing system-wide Rust libraries and documentation..."
rm -rf /usr/lib/rustlib
rm -rf /usr/share/doc/rust
rm -rf /usr/share/doc/cargo
rm -rf /usr/share/man/man1/rustc.1
rm -rf /usr/share/man/man1/cargo.1

# Remove user-specific rustup and cargo directories
# This ensures a completely fresh start.
SUDO_USER_HOME=$(getent passwd "$SUDO_USER" | cut -d: -f6)
if [[ -n "$SUDO_USER_HOME" && -d "$SUDO_USER_HOME" ]]; then
    CARGO_HOME="${SUDO_USER_HOME}/.cargo"
    RUSTUP_HOME="${SUDO_USER_HOME}/.rustup"

    if [[ -d "$CARGO_HOME" ]]; then
        info "Removing user's Cargo directory: $CARGO_HOME"
        rm -rf "$CARGO_HOME"
    fi
    if [[ -d "$RUSTUP_HOME" ]]; then
        info "Removing user's Rustup directory: $RUSTUP_HOME"
        rm -rf "$RUSTUP_HOME"
    fi
else
    info "Could not determine user's home directory to remove .cargo and .rustup. Skipping."
fi

success "Step 1 complete. All known Rust installations have been removed."

# ==============================================================================
#  STEP 2: INSTALL SYSTEM DEPENDENCIES
# ==============================================================================
info "\n--- Starting Step 2: Installing System Dependencies ---"
apt-get update -qq
apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    ffmpeg \
    curl \
    wget
success "Step 2 complete. System dependencies are installed."

# ==============================================================================
#  STEP 3: INSTALL AND VERIFY RUST VIA RUSTUP
# ==============================================================================
info "\n--- Starting Step 3: Installing Latest Stable Rust via rustup ---"

# Run the rustup installer as the original user, not as root
runuser -l "$SUDO_USER" -c "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable"

# Source the environment for the current script to use the new installation
# This is crucial for the verification step.
CARGO_ENV_PATH="$SUDO_USER_HOME/.cargo/env"
if [[ -f "$CARGO_ENV_PATH" ]]; then
    source "$CARGO_ENV_PATH"
else
    error "Could not find rustup environment file at $CARGO_ENV_PATH"
    exit 1
fi

info "Installing additional Rust components (clippy, rustfmt)..."
runuser -l "$SUDO_USER" -c "rustup component add clippy rustfmt"

info "Verifying the new Rust installation meets minimum version requirement..."
# We must run the verification as the user to check their environment
MIN_VERSION="1.80"
INSTALLED_VERSION=$(runuser -l "$SUDO_USER" -c "rustc --version" | cut -d' ' -f2)

if ! printf '%s\n' "$MIN_VERSION" "$INSTALLED_VERSION" | sort -V -C; then
    error "Installed Rust version ($INSTALLED_VERSION) is older than the required version ($MIN_VERSION)."
    error "Please check the rustup installation."
    exit 1
fi

success "Verification successful: Installed version is $INSTALLED_VERSION (meets >$MIN_VERSION requirement)."
success "Step 3 complete. Rust is installed and configured."

echo
success "=== Rust environment setup has completed successfully! ==="
info "The new Rust toolchain is ready. Open a new terminal or run 'source ~/.profile' for the changes to take effect in your interactive shell."
