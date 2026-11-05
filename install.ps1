# Terminus Framework Windows PowerShell Installer
# Run via: irm https://raw.githubusercontent.com/prateekbhujel/terminus/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

Write-Host "  _____                   _                  " -ForegroundColor Cyan
Write-Host " |_   _|__ _ __ _ __ ___ (_)_ __  _   _ ___ " -ForegroundColor Cyan
Write-Host "   | |/ _ \ '__| '_ ` _ \| | '_ \| | | / __|" -ForegroundColor Cyan
Write-Host "   | |  __/ |  | | | | | | | | | | |_| \__ \" -ForegroundColor Cyan
Write-Host "   |_|\___|_|  |_| |_| |_|_|_| |_|\__,_|___/" -ForegroundColor Cyan
Write-Host ""
Write-Host "Installing Terminus Framework CLI for Windows..." -ForegroundColor White
Write-Host ""

# Ensure Cargo/Rust is installed
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "[!] Cargo was not found in your PATH." -ForegroundColor Yellow
    Write-Host "Terminus is built with pure Rust. Please install Rust from https://rustup.rs or run:"
    Write-Host "  winget install --id Rustlang.Rustup" -ForegroundColor Cyan
    Write-Host ""
    exit 1
}

# If inside repository directory, install from local path, otherwise install from crates.io
if (Test-Path "Cargo.toml") {
    $content = Get-Content "Cargo.toml" -Raw
    if ($content -match 'name = "terminus-framework"') {
        Write-Host "Installing from local repository directory..." -ForegroundColor Gray
        cargo install --path . --force
    } else {
        Write-Host "Installing terminus-framework from crates.io..." -ForegroundColor Gray
        cargo install terminus-framework --force
    }
} else {
    Write-Host "Installing terminus-framework from crates.io..." -ForegroundColor Gray
    cargo install terminus-framework --force
}

$cargoBin = Join-Path $HOME ".cargo\bin"
$terminusExe = Join-Path $cargoBin "terminus.exe"
$cargoTerminusExe = Join-Path $cargoBin "cargo-terminus.exe"

if ((Test-Path $terminusExe) -and (-not (Test-Path $cargoTerminusExe))) {
    Copy-Item $terminusExe $cargoTerminusExe -Force
}

Write-Host ""
Write-Host "[OK] Terminus Framework CLI installed successfully!" -ForegroundColor Green
Write-Host ""
Write-Host "You can now scaffold projects anywhere on your Windows machine:" -ForegroundColor White
Write-Host "  terminus new my_project" -ForegroundColor Cyan
Write-Host "  cd my_project && cargo run" -ForegroundColor Cyan
Write-Host ""
