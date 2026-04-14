#!/usr/bin/env bash
set -euo pipefail

# First-time dev environment setup for Life Assistant.
# Installs Homebrew packages, pnpm dependencies, and fetches Cargo crates.

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "life-assistant bootstrap currently supports macOS only." >&2
  exit 1
fi

if ! command -v brew >/dev/null 2>&1; then
  echo "Homebrew not found. Install from https://brew.sh first." >&2
  exit 1
fi

if ! command -v rustc >/dev/null 2>&1; then
  echo "Installing Rust via rustup…"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  source "${HOME}/.cargo/env"
fi

if ! command -v node >/dev/null 2>&1; then
  echo "Installing Node 20…"
  brew install node@20
  brew link --force --overwrite node@20
fi

if ! command -v pnpm >/dev/null 2>&1; then
  echo "Installing pnpm…"
  brew install pnpm
fi

echo "Installing pnpm dependencies…"
pnpm install --frozen-lockfile

echo "Fetching cargo crates…"
cargo fetch

echo ""
echo "Bootstrap complete. Run ./scripts/dev.sh to start the app."
