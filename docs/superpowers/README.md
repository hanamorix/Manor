# Superpowers design archive

The files in `plans/` and `specs/` are historical implementation notes. They are useful for understanding why Manor evolved the way it did, but they are not the release-readiness source of truth.

For current behaviour, prefer:

- [`../../README.md`](../../README.md) for product capabilities and supported integrations.
- The current Rust/TypeScript implementation and migrations.
- GitHub CI for the active validation gates.

Current status notes, last checked 2026-05-09:

- Ledger is CSV-import first. GoCardless/Plaid bank sync was removed in v0.1.2 and any old bank-sync specs are archival only.
- Remote LLM support is Claude-only today. OpenAI, Gemini, Groq, OpenRouter, and similar providers are future adapters, not shipped features.
- Semantic search uses local Ollama embeddings stored in SQLite. `sqlite-vec` remains a future backend option, not the active implementation.
- Bones L4a through L4e are implemented despite older roadmap files describing them as future work.
