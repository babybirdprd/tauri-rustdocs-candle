# build.ps1 - Windows PowerShell build script for the Tauri application

# Stop script on first error
$ErrorActionPreference = "Stop"

Write-Host "Starting build process..."

# Step 1: Install frontend dependencies
Write-Host "Installing frontend dependencies with pnpm..."
$pnpmExists = Get-Command pnpm -ErrorAction SilentlyContinue
if (-not $pnpmExists) {
    Write-Error "pnpm could not be found. Please install pnpm first."
    Write-Host "You can install it via npm: npm install -g pnpm"
    exit 1
}
pnpm install
if ($LASTEXITCODE -ne 0) {
    Write-Error "pnpm install failed with exit code $LASTEXITCODE"
    exit 1
}

# Step 2: Build the Tauri application
Write-Host "Building the Tauri application..."
$cargoExists = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargoExists) {
    Write-Error "cargo (Rust toolchain) could not be found. Please install Rust."
    exit 1
}
# Assuming tauri-cli is installed via cargo install tauri-cli or is a direct project dependency.
cargo tauri build
if ($LASTEXITCODE -ne 0) {
    Write-Error "cargo tauri build failed with exit code $LASTEXITCODE"
    exit 1
}

Write-Host "Build process completed successfully."
Write-Host "The application bundle can be found in src-tauri/target/release/bundle/"
