#!/usr/bin/env bash
# Terminus Framework Universal Installer
set -e

# Detect if terminal supports color
if [ -t 1 ]; then
  BOLD="\033[1m"
  GREEN="\033[32m"
  CYAN="\033[36m"
  YELLOW="\033[33m"
  RESET="\033[0m"
else
  BOLD=""
  GREEN=""
  CYAN=""
  YELLOW=""
  RESET=""
fi

echo -e "${CYAN}"
echo "  _____                   _                  "
echo " |_   _|__ _ __ _ __ ___ (_)_ __  _   _ ___ "
echo "   | |/ _ \\ '__| '_ \` _ \\| | '_ \\| | | / __|"
echo "   | |  __/ |  | | | | | | | | | | |_| \\__ \\"
echo "   |_|\\___|_|  |_| |_| |_|_|_| |_|\\__,_|___/"
echo -e "${RESET}"
echo -e "${BOLD}Installing Terminus Framework CLI...${RESET}"
echo ""

# Ensure Cargo/Rust is installed
if ! command -v cargo >/dev/null 2>&1; then
  echo -e "${YELLOW}[!] Cargo is not found in your PATH.${RESET}"
  echo "Terminus is built with pure Rust. Installing Rust via rustup..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
fi

# If inside repository directory, install from local path, otherwise install from crates.io
if [ -f "Cargo.toml" ] && grep -q 'name = "terminus-framework"' Cargo.toml 2>/dev/null; then
  echo "Installing from local repository directory..."
  cargo install --path . --force
else
  echo "Installing terminus-framework from crates.io..."
  cargo install terminus-framework --force
fi

CARGO_BIN="$HOME/.cargo/bin"

# Symlink cargo-terminus if not present
if [ -f "$CARGO_BIN/terminus" ] && [ ! -f "$CARGO_BIN/cargo-terminus" ]; then
  ln -sf "$CARGO_BIN/terminus" "$CARGO_BIN/cargo-terminus"
fi

echo ""
echo -e "${GREEN}[OK] Terminus Framework CLI installed successfully!${RESET}"
echo ""
echo -e "You can now scaffold projects anywhere on your machine:"
echo -e "  ${BOLD}terminus new my_project${RESET}"
echo -e "  ${BOLD}cd my_project && cargo run${RESET}"
echo ""
