# Manor

A calm, local-first desktop app for managing a household's life admin —
calendar, chores, money, meals, and home maintenance.

**Status: early development, v0.1 Heartbeat in progress.**

## Design

See the design spec:
[`docs/superpowers/specs/2026-04-14-life-assistant-design.md`](docs/superpowers/specs/2026-04-14-life-assistant-design.md).

## Stack

- **Shell:** Tauri 2 + React + TypeScript
- **Core:** Rust + SQLite
- **AI:** Ollama (local, default) with optional BYO keys for Claude/OpenAI/Gemini

## Development

First-time setup:

```bash
./scripts/bootstrap.sh
```

Run the dev shell with hot reload:

```bash
./scripts/dev.sh
```

Run tests:

```bash
cargo test --workspace
pnpm tsc
```

## License

[AGPL-3.0](LICENSE). Anyone who ships a modified version must open-source their changes.

## How this is sustained

Manor is free, open-source, and built by a very small team.
If the app makes your life calmer, see [FUNDING.md](FUNDING.md) for the
ways to keep it alive.
