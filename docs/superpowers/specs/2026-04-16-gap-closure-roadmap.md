# Gap Closure Roadmap

- **Date**: 2026-04-16 (updated 2026-04-18)
- **Status**: Landmarks 1 & 2 ✅ SHIPPED. Landmark 3 deferred (brainstorm per release).
- **Authors**: Hana (product), Nell (architect)

## Purpose

Life Assistant ("Manor") has shipped v0.1 → v0.3b end-to-end but skipped foundational work the overall design (`2026-04-14-life-assistant-design.md`) said belongs in v0.1. This document sequences those gaps into discrete, independently shippable chunks. It is **not** an implementation plan — it is the order we will write specs and plans in.

---

## What we will and won't ship from this roadmap

**In scope**: finishing v0.1 Heartbeat + unlocking v0.4/v0.5 by landing the foundational tables every skill depends on.
**Out of scope for this roadmap**: v0.4 Hearth, v0.5 Bones, v1.0 Companion. Each needs its own brainstorm → design spec → plan cycle. They are listed at the bottom for visibility but not sequenced.

---

## Gap inventory (derived from §4 gap scan 2026-04-16)

| # | Gap | Source | Status |
|---|---|---|---|
| G1 | `person` + `household` tables | design §4.2 | ✅ Phase A |
| G2 | `attachment` table + file storage layer | design §4.2 | ✅ Phase A |
| G3 | `tag` + `tag_link` tables | design §4.2 | ✅ Phase A |
| G4 | `note` table | design §4.2 | ✅ Phase A |
| G5 | `setting(key, value)` table | design §4.2 | ✅ Phase A |
| G6 | `remote_call_log` table | design §4.2, §4.6 | ✅ Landmark 2 (shipped 2026-04-17) |
| G7 | `embedding` table | design §4.2, §4.8 | ✅ Phase C (pure-Rust cosine; sqlite-vec deferred — DAL stable for later swap) |
| G8 | First-launch wizard | design §6.1 | ✅ Phase D |
| G9 | Weather strip in Today header | design §6.2, §11.3 | ✅ Phase D |
| G10 | Trash view + panic button + auto-empty | design §4.7 | ✅ Phase B |
| G11 | Snapshot backup / `.lifebackup` archives | design §8.1 | ✅ Phase B |
| G12 | Remote LLM support (keys, tier routing, redaction, budget caps) | design §5.1–5.7 | ✅ Landmark 2 (shipped 2026-04-17) |
| G13 | Settings sections 1, 2, 4, 5 | design §6.4 | ✅ Phase E |
| G14 | v0.3 Ledger design drift re: contracts table split | self-review | ✅ Phase E Task 1 |
| G15 | Phase 3c plan checkboxes unchecked despite code being live | self-review | ✅ Phase E Task 1 |
| G16 | v0.4 Hearth | roadmap gap | 🔮 Landmark 3 (not sequenced) |
| G17 | v0.5 Bones | roadmap gap | 🔮 Landmark 3 (not sequenced) |
| G18 | v1.0 Companion (iOS + sync) | roadmap gap | 🔮 Landmark 3 (not sequenced) |

---

## Sequencing (dependency-ordered)

Three landmarks. Each is a single spec + implementation plan that ships a testable, reviewable chunk.

### Landmark 1 — v0.1 Completion (bundles G1–G11, G13, G14, G15) — ✅ **SHIPPED 2026-04-17**

**40 commits, +30 tests, 5 phases.** See `specs/2026-04-16-v0.1-completion-design.md` + 5 phase plans.

A single design spec that says "finish what v0.1 already promised." Subsystems chunked into **five phases inside the plan**:

**Phase A — Foundation tables** (unblocks everything that follows)
- G5 `setting(key, value)` table + DAL — simplest, landing first lets later phases persist config
- G1 `person` + `household` tables — unblocks Rhythm fairness queries and Hearth meal assignees
- G3 `tag` + `tag_link` — universal labels, used by subsequent phases
- G4 `note` — markdown notes attachable to any entity
- G2 `attachment` table + `~/Library/Application Support/Manor/attachments/<uuid>` directory management

**Phase B — Deletion + safety** (needs foundation tables first)
- G10 Trash view + 30-day auto-empty job + panic button
- G11 Snapshot backup (weekly `.lifebackup` age-encrypted tarball)

**Phase C — Local intelligence surface** (separate because of `sqlite-vec` system extension)
- G7 `embedding` table + `sqlite-vec` loadable extension + embed-on-write pipeline for notes, attachments, transactions

**Phase D — Today polish**
- G9 Weather strip (wttr.in, cached, offline-graceful)

**Phase E — Settings + housekeeping**
- G13 Settings tabs 1, 2, 4, 5 (Data & Backup, AI, Household, About) — driven by the phase A+B+C data model additions
- G14 Update v0.3 Ledger design re: contracts separation
- G15 Tick Phase 3c plan checkboxes

**Defers**: G6 `remote_call_log` and G12 Remote LLM — they only matter once a remote key can be added, which is Landmark 2.

### Landmark 2 — Remote LLM support (bundles G6, G12) — ✅ **SHIPPED 2026-04-17**

Own design spec + plan, shipped cleanly. Cross-cut skills and UX, carried the most privacy risk (redaction pipeline, budget caps, keys-in-keychain UX). Landing this while people were still on local-only was the safe moment.

Subsystems (all shipped):
- G6 `remote_call_log` schema + audit writer
- Provider abstraction (`crates/core/src/models/` per original design §3.3)
- Key storage in macOS Keychain + UI for adding/removing
- Tier-based routing (§5.1)
- Redaction pipeline (§5.6) — hit this with property tests
- Budget guardrails + warning UI (§5.7)
- Settings → AI tab upgrade to show call log, budget, keys

### Landmark 3 — v0.4+ releases (G16, G17, G18) — **not sequenced here**

Each needs its own brainstorm to flesh out what's only one paragraph in the overall design. Not planned in this roadmap. Surface them after Landmark 2 lands and we have user-signal on what Manor needs next.

---

## What gets written next

1. ~~`specs/2026-04-16-v0.1-completion-design.md` — the big one (Landmark 1).~~ ✅ shipped.
2. ~~5 phase plans (A–E), executed via subagent-driven-development.~~ ✅ all shipped.
3. ~~**Landmark 2 design spec** — Remote LLM support.~~ ✅ shipped.
4. ~~**Landmark 2 plan** — redaction pipeline, tier routing, keychain, budget caps.~~ ✅ shipped.
5. ~~Execute Landmark 2.~~ ✅ Manor now supports BYO remote keys with privacy guardrails.
6. **Landmark 3 (v0.4 Hearth onward)** — one brainstorm per release, each its own spec + plan cycle. v0.4 Hearth is next.

## Non-goals for this roadmap

- Don't block on v0.4 Hearth scoping just because v0.1 is incomplete.
- Don't redesign existing Phase 3/4/5 work. They ship as-is.
- Don't pretend v1.0 Companion (iOS + sync) is close. It isn't.

---

*End of roadmap. Next step: v0.4 Hearth brainstorm (Landmark 3, first release).*
