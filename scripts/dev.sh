#!/usr/bin/env bash
set -euo pipefail

# Runs the Tauri dev shell with hot reload.

cd "$(dirname "$0")/.."

if [[ ! -d node_modules ]]; then
  pnpm install --frozen-lockfile
fi

pnpm dev
