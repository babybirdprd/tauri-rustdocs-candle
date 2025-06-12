#!/usr/bin/env bash
#
# remove_rust.sh
#
# This script removes a Rust installation that was built and installed
# from a source tarball under /usr (e.g., /usr/bin, /usr/lib/rustlib, etc.).
# It also checks for and purges any Debian/Ubuntu-packaged rustc/cargo if present.
#
# Usage: 
#   sudo bash remove_rust.sh
#
set -euo pipefail

# Helper: print a message in yellow
info() {
    echo -e "\e[33m[INFO]\e[0m $1"
}

# Helper: print a message in green
success() {
    echo -e "\e[32m[SUCCESS]\e[0m $1"
}

# Helper: print a message in red
error() {
    echo -e "\e[31m[ERROR]\e[0m $1"
}

# Confirm running as root
if [[ "$EUID" -ne 0 ]]; then
    error "This script must be run as root. Try: sudo bash $0"
    exit 1
fi

info "Starting Rust removal..."

# 1. Remove Rust executables from /usr/bin
BINARIES=(
    rustc
    cargo
    rustdoc
    rustfmt
    clippy-driver
    racer
    rls
    cargo-fmt
    cargo-clippy
    cargo-metadata
    rust-gdb
    rust-lldb
)

for bin in "${BINARIES[@]}"; do
    if [[ -x "/usr/bin/$bin" ]]; then
        info "Removing /usr/bin/$bin"
        rm -f "/usr/bin/$bin"
    else
        info "Skipping /usr/bin/$bin (not found)"
    fi
done

# Additionally remove any leftover rust-* or cargo-* executables in /usr/bin
# (useful if custom names were installed)
for pattern in rust-* cargo-*; do
    matches=(/usr/bin/$pattern)
    for file in "${matches[@]}"; do
        if [[ -e "$file" && ! -d "$file" ]]; then
            info "Removing stray executable: $file"
            rm -f "$file"
        fi
    done
done

# 2. Remove Rust libraries and sysroot directory
if [[ -d "/usr/lib/rustlib" ]]; then
    info "Removing /usr/lib/rustlib"
    rm -rf "/usr/lib/rustlib"
else
    info "Skipping /usr/lib/rustlib (not found)"
fi

# Also remove any stray libstd-*.rlib, libstd-*.so*, librustc*.* in /usr/lib
LDIRS=(/usr/lib /usr/lib/x86_64-linux-gnu)
for libdir in "${LDIRS[@]}"; do
    if [[ -d "$libdir" ]]; then
        info "Removing Rust library files in $libdir"
        rm -f "$libdir/libstd-"*.rlib     2>/dev/null || true
        rm -f "$libdir/libstd-"*.so*      2>/dev/null || true
        rm -f "$libdir/librustc"*        2>/dev/null || true
    fi
done

# 3. Remove man pages
MAN1=(
    rustc.1
    cargo.1
    rustdoc.1
    rustfmt.1
    clippy-driver.1
    rls.1
)
for m in "${MAN1[@]}"; do
    if [[ -f "/usr/share/man/man1/$m" ]]; then
        info "Removing /usr/share/man/man1/$m"
        rm -f "/usr/share/man/man1/$m"
    else
        info "Skipping /usr/share/man/man1/$m (not found)"
    fi
done

# Man pages in man7
if ls /usr/share/man/man7/rustc.* &> /dev/null; then
    info "Removing any /usr/share/man/man7/rustc.*"
    rm -f /usr/share/man/man7/rustc.* 
else
    info "No /usr/share/man/man7/rustc.* files found"
fi

# 4. Remove documentation directories
DOC_DIRS=(
    /usr/share/doc/rust
    /usr/share/doc/cargo
)
for dir in "${DOC_DIRS[@]}"; do
    if [[ -d "$dir" ]]; then
        info "Removing $dir"
        rm -rf "$dir"
    else
        info "Skipping $dir (not found)"
    fi
done

# 5. Remove shared data directories
SHARE_DIRS=(
    /usr/share/rust
    /usr/share/cargo
)
for dir in "${SHARE_DIRS[@]}"; do
    if [[ -d "$dir" ]]; then
        info "Removing $dir"
        rm -rf "$dir"
    else
        info "Skipping $dir (not found)"
    fi
done

# Remove license files like /usr/share/licenses/rust*
if ls /usr/share/licenses/rust* &> /dev/null; then
    info "Removing /usr/share/licenses/rust*"
    rm -rf /usr/share/licenses/rust*
else
    info "No /usr/share/licenses/rust* found"
fi

# 6. Purge Debian/Ubuntu-packaged Rust (optional; only if installed)
if dpkg -l | grep -E '^\w+ +rustc' &> /dev/null || dpkg -l | grep -E '^\w+ +cargo' &> /dev/null; then
    info "Detected Rust/Cargo via apt. Purging rustc and cargo packages..."
    apt-get update -qq
    DEBIAN_FRONTEND=noninteractive apt-get purge -y rustc cargo || true
    DEBIAN_FRONTEND=noninteractive apt-get autoremove -y || true
    success "Debian/Ubuntu-packaged Rust and Cargo have been purged."
else
    info "No rustc/cargo packages found via dpkg."
fi

# 7. (Optional) Remove ~/.cargo or ~/.rustup directories if present
# WARNING: this deletes your user-level cargo cache and configurations.
CARGO_HOME="${HOME}/.cargo"
RUSTUP_HOME="${HOME}/.rustup"
if [[ -d "$CARGO_HOME" ]]; then
    info "Removing user’s Cargo directory: $CARGO_HOME"
    rm -rf "$CARGO_HOME"
else
    info "Skipping $CARGO_HOME (not found)"
fi
if [[ -d "$RUSTUP_HOME" ]]; then
    info "Removing user’s Rustup directory: $RUSTUP_HOME"
    rm -rf "$RUSTUP_HOME"
else
    info "Skipping $RUSTUP_HOME (not found)"
fi

# 8. (Optional) Remove any stray profile entries – but do NOT modify ~/.bashrc automatically.
info "NOTE: If you added PATH or LD_LIBRARY_PATH entries for Rust in ~/.bashrc, ~/.profile, or /etc/profile, remove them manually."

# 9. Final verification
info "Verifying removal..."
if command -v rustc &> /dev/null || command -v cargo &> /dev/null; then
    error "rustc or cargo still found on PATH. Please check for any remaining files."
    command -v rustc || true
    command -v cargo || true
    exit 1
else
    success "No rustc/cargo binaries found. Rust has been removed."
fi

echo
echo "Rust removal complete."
