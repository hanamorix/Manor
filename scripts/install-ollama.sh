#!/usr/bin/env bash
set -euo pipefail

# Ensures Ollama is installed and the default models are pulled.
# Expanded in Phase 3 (Models Layer) to pull qwen2.5:7b-instruct and
# nomic-embed-text.

if ! command -v ollama >/dev/null 2>&1; then
  echo "Installing Ollama via Homebrew…"
  brew install ollama
fi

echo "Ollama is installed at: $(command -v ollama)"
echo "Model pulls will be added in Phase 3."
