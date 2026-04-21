<p align="center">
  <img src="material/manor_face_smile.png" width="128" alt="Manor" />
</p>

<h1 align="center">Manor</h1>

<p align="center">
  <em>A calm desktop app for running your household — calendar, chores, money, meals, and the house itself. Local-first. Private. Yours.</em>
</p>

<p align="center">
  <a href="https://github.com/hanamorix/Manor/releases/latest"><img alt="latest release" src="https://img.shields.io/github/v/release/hanamorix/Manor?style=flat-square&label=release&color=111" /></a>
  <img alt="platform" src="https://img.shields.io/badge/platform-macOS-1d1d1f?style=flat-square" />
  <img alt="license" src="https://img.shields.io/badge/license-AGPL--3.0-111?style=flat-square" />
  <img alt="built with tauri" src="https://img.shields.io/badge/built%20with-Tauri%202-111?style=flat-square" />
</p>

---

Manor is the admin-half of a life in a single quiet window — the calendar, the chores, the bank transactions, the meals for the week, the boiler that needs its annual service. She holds all of it for you, proposes changes as reviewable diffs, and never sends your data anywhere you didn't explicitly point her at.

She's built for one person (and optionally their household), runs on your Mac, keeps everything in a single SQLite file on your disk, and uses a local **Ollama** model by default so the little assistant inside her doesn't need the internet to be useful.

**No accounts. No cloud backend. No telemetry. No "sign in to continue".**

---

## ✦ What she does today

Manor **v0.1 — Heartbeat** ships five rooms:

### Today
- A live clock, tasks, and calendar events at a glance.
- Add tasks with `/task` or the pill input. Click to complete, hover to edit or delete. 4-second undo on everything.
- The **Assistant** — a small avatar in the corner — can add, edit, and complete tasks for you. Every action shows up as a pending proposal you can accept or reject.

### Rhythm
- Routines and recurring chores. Weekly hoover, bin day, pay the gardener, clean the coffee machine.
- Overdue and due-today chores bubble into your Today view so you see them without looking.

### Ledger
- Transactions, categories, budgets, recurring payments, contracts.
- Optional **bank sync via GoCardless** (bring your own credentials) — reads your transactions, never initiates a payment.
- **Auto-categorisation** — the Assistant batch-categorises pending transactions in the background using your local Ollama model.
- Monthly AI review — Manor drafts a short summary of the month's spending; you decide whether to keep it.

### Hearth
- **Recipe library** — add by URL, PDF, or typed in. Tags, servings, nutrition where available.
- **Meal plans** for the week, generated from your library or handwritten.
- **Shopping lists** pulled from the plan + your pantry staples.
- **Meal ideas** that suggest dinners based on what's already in the house.

### Bones — the house itself
- **Asset registry** — the boiler, the washing machine, the roof, the car, whatever's worth remembering.
- **Maintenance schedules** — service the boiler every 12 months, change the smoke alarm battery every October.
- **Events ledger** — every repair, service, and quote, attached to its asset and optionally linked to the transaction that paid for it.
- **Right-to-Repair lookup** — for any asset, pull IPC-compliant repair information so you can fix things yourself.
- **Manual library** — drop a manufacturer PDF, Manor extracts the searchable text and can pull maintenance info out of it as proposals.

### And quietly in the background
- **Semantic search** across everything, on-device, via `sqlite-vec` embeddings.
- **Deletion safety** — nothing's ever really gone without an undo window. Trash is real.
- **Remote LLM, opt-in** — bring your own key for Claude / OpenAI / Gemini; Manor logs every outbound call so you can see exactly what was sent, and a budget ceiling stops runaway spend.
- **Flat design** — SF Pro, SF Mono for numbers, Lucide icons, monochrome palette with automatic light/dark. Feels native. Feels quiet.

---

## ✦ Install

1. Download the latest DMG from [Releases](https://github.com/hanamorix/Manor/releases/latest).
2. Open it and drag **Manor.app** into `/Applications`.
3. First launch: **right-click → Open** (macOS will warn you because the app is signed ad-hoc, not notarised — Gatekeeper just needs your permission once).
4. That's it.

Full walkthrough, troubleshooting (`xattr` fix if you ever see *"Manor is damaged"*), and the reasoning behind the ad-hoc signing choice — [docs/INSTALL.md](docs/INSTALL.md).

---

## ✦ Your data, your machine

- Everything lives in a single SQLite database under `~/Library/Application Support/com.hanamorix.manor/`.
- The default assistant is a **local Ollama** model — nothing leaves your Mac unless you turn on a remote provider.
- Remote LLMs (Claude / OpenAI / Gemini) are **opt-in and BYO key**. Manor stores the key in macOS Keychain, never on disk, and every outbound call is written to a transparent call log you can read any time.
- Budget ceilings on remote providers — set a monthly cap, Manor stops calling when you hit it.

---

## ✦ Built with

- **Shell** — Tauri 2 · React 18 · TypeScript · Vite · Zustand
- **Core** — Rust · SQLite (`rusqlite` + `refinery` migrations + `sqlite-vec`)
- **AI** — Ollama (default, local) · optional Claude / OpenAI / Gemini via BYO key
- **Platform** — macOS universal binary (Apple Silicon + Intel)
- **Design** — SF Pro · SF Mono · Lucide · monochrome with auto light/dark

---

## ✦ Development

Poking around, contributing, or running a dev build?

First-time setup:

```bash
./scripts/bootstrap.sh
```

Dev shell with hot reload:

```bash
./scripts/dev.sh
```

Run the tests:

```bash
cargo test --workspace
pnpm tsc
```

Cut a signed DMG locally:

```bash
./scripts/release-mac.sh
```

Output lands at `target/universal-apple-darwin/release/bundle/dmg/`.

---

## ✦ Status

**v0.1 — Heartbeat** shipped 2026-04-21.

All five rooms (Today · Rhythm · Ledger · Hearth · Bones) are live. The release sits on top of **Bundle B** — a hardening pass that brought lazy-loaded views, WebP-converted assets, PDF decompression caps, sandboxed asset paths, and a streaming fix on the assistant.

Manor is built by a very small team — one developer, one product owner — and moves at a human pace. The roadmap past v0.1 is polish, an iPad companion, and a stable API surface for third-party skills. No VC deadline, no growth hacks.

---

## ✦ License

[AGPL-3.0](LICENSE). If you ship a modified version of Manor, you must open-source your changes under the same license. This isn't hostile to commercial use — it's hostile to closed-source forks of a free app.

---

## ✦ Sustaining the project

Manor is free, open-source, and built by people who don't have a VC breathing down their necks. If she makes your life calmer, have a look at [FUNDING.md](FUNDING.md) for ways to keep her alive.

---

<p align="center"><sub>Built slowly. Kept local. ✦</sub></p>
