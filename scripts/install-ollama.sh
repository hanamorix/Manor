#!/usr/bin/env bash
set -euo pipefail

# Ensures Ollama is installed and the default Manor model is pulled.

MANOR_MODEL="qwen2.5:7b-instruct"

if ! command -v ollama >/dev/null 2>&1; then
  echo "Installing Ollama via Homebrew…"
  brew install ollama
fi

echo "Ollama is installed at: $(command -v ollama)"

echo "Ensuring Ollama service is running…"
if ! pgrep -f "ollama serve" >/dev/null 2>&1; then
  echo "  Ollama is not running. Start it in another terminal with: ollama serve"
fi

echo "Ensuring model ${MANOR_MODEL} is pulled…"
ollama pull "${MANOR_MODEL}"

echo ""
echo "Done. Manor will use ${MANOR_MODEL} as its default local model."
