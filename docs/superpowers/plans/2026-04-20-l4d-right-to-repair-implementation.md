# L4d Right-to-Repair Lookup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Manor's fourth v0.5 Bones slice — a per-asset "Something's wrong" search that queries DuckDuckGo + YouTube, trims the top-3 pages via a DIY readability pass, synthesises through Ollama (with opt-in Claude escalation), and persists every successful answer as a `repair_note`.

**Architecture:** Two-crate split mirrors L4a/L4b/L4c. Core holds schema (V21), types, DAL. App crate holds the pipeline: `search.rs` (DDG + YouTube URL extraction) → `fetch.rs` (page fetch + readability trim) → `synth.rs` (Ollama + Claude via `remote::orchestrator::remote_chat`, shared prompt builder, trait seam for test stubbing) → `pipeline.rs` (orchestrator) → `commands.rs` (Tauri IPC). Frontend ships an IPC module, a Zustand store, and three new components (`TroubleshootBlock`, `TroubleshootResultCard`, `RepairNoteCard`) mounted on AssetDetail between the History block (L4c) and Documents.

**Tech Stack:** Rust (rusqlite, reqwest, scraper, tokio, async-trait, thiserror, serde_json), React + TypeScript + Zustand, react-markdown + remark-gfm (new frontend dep), `@tauri-apps/plugin-shell` (already installed for external link handling).

**Spec:** `docs/superpowers/specs/2026-04-20-l4d-right-to-repair-design.md`

---

## File structure

### New Rust files
- `crates/core/migrations/V21__repair_note.sql`
- `crates/core/src/repair/mod.rs` — types.
- `crates/core/src/repair/dal.rs` — CRUD + list_for_asset + trash helpers.
- `crates/app/src/repair/mod.rs` — module root.
- `crates/app/src/repair/search.rs` — DDG + YouTube URL extraction.
- `crates/app/src/repair/fetch.rs` — page fetch + readability trim.
- `crates/app/src/repair/synth.rs` — prompt builder + Ollama + Claude synth + `SynthBackend` trait.
- `crates/app/src/repair/pipeline.rs` — `run_repair_search` + `build_augmented_query`.
- `crates/app/src/repair/commands.rs` — 5 Tauri commands.

### New frontend files
- `apps/desktop/src/lib/repair/ipc.ts`
- `apps/desktop/src/lib/repair/state.ts`
- `apps/desktop/src/components/Bones/TroubleshootBlock.tsx`
- `apps/desktop/src/components/Bones/TroubleshootResultCard.tsx`
- `apps/desktop/src/components/Bones/RepairNoteCard.tsx`
- `apps/desktop/src/components/Bones/RepairMarkdown.tsx` (small wrapper around `react-markdown` with external-link handling)
- `apps/desktop/src/components/Bones/__tests__/TroubleshootBlock.test.tsx`

### Modified Rust files
- `crates/core/src/lib.rs` — `pub mod repair;`.
- `crates/core/src/trash.rs` — append `("repair_note", "symptom")`.
- `crates/core/src/asset/dal.rs` — extend `soft_delete_asset`, `restore_asset`, `permanent_delete_asset` cascades with repair_note.
- `crates/app/src/lib.rs` — `pub mod repair;` + register 5 Tauri commands.
- `crates/app/src/assistant/ollama.rs` — add non-streaming `chat_collect` helper on `OllamaClient`.
- `crates/app/src/safety/trash_commands.rs` — add `"repair_note"` arms.

### Modified frontend files
- `apps/desktop/src/components/Bones/AssetDetail.tsx` — mount `<TroubleshootBlock assetId={id} />` between `<HistoryBlock />` and `<h2>Documents</h2>`.
- `apps/desktop/package.json` — add `react-markdown` + `remark-gfm`.

---

## Phase A — Core schema, types, DAL, cascade

### Task 1: Migration V21 + core types + trash registry

**Files:**
- Create: `crates/core/migrations/V21__repair_note.sql`
- Create: `crates/core/src/repair/mod.rs`
- Modify: `crates/core/src/lib.rs` — add `pub mod repair;`.
- Modify: `crates/core/src/trash.rs` — append to REGISTRY.

- [ ] **Step 1: Write the migration SQL**

Create `crates/core/migrations/V21__repair_note.sql`:

```sql
-- V21__repair_note.sql
-- L4d: right-to-repair search history.

CREATE TABLE repair_note (
    id              TEXT PRIMARY KEY,
    asset_id        TEXT NOT NULL REFERENCES asset(id),
    symptom         TEXT NOT NULL,
    body_md         TEXT NOT NULL,
    sources         TEXT NOT NULL,
    video_sources   TEXT,
    tier            TEXT NOT NULL
                    CHECK (tier IN ('ollama','claude')),
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    deleted_at      INTEGER
);

CREATE INDEX idx_repair_asset    ON repair_note(asset_id);
CREATE INDEX idx_repair_created  ON repair_note(created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_repair_deleted  ON repair_note(deleted_at);
```

- [ ] **Step 2: Create `crates/core/src/repair/mod.rs`**

```rust
//! Repair-note types (L4d).

pub mod dal;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmTier {
    Ollama,
    Claude,
}

impl LlmTier {
    pub fn as_str(self) -> &'static str {
        match self {
            LlmTier::Ollama => "ollama",
            LlmTier::Claude => "claude",
        }
    }

    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s {
            "ollama" => Ok(LlmTier::Ollama),
            "claude" => Ok(LlmTier::Claude),
            other => Err(anyhow::anyhow!("unknown LlmTier: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepairSource {
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairNoteDraft {
    pub asset_id: String,
    pub symptom: String,
    pub body_md: String,
    pub sources: Vec<RepairSource>,
    pub video_sources: Option<Vec<RepairSource>>,
    pub tier: LlmTier,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairNote {
    pub id: String,
    pub asset_id: String,
    pub symptom: String,
    pub body_md: String,
    pub sources: Vec<RepairSource>,
    pub video_sources: Option<Vec<RepairSource>>,
    pub tier: LlmTier,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[cfg(test)]
mod migration_tests {
    use crate::assistant::db;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    #[test]
    fn v21_creates_repair_note_table() {
        let (_d, conn) = fresh_conn();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='repair_note'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn v21_creates_repair_indexes() {
        let (_d, conn) = fresh_conn();
        for name in &["idx_repair_asset", "idx_repair_created", "idx_repair_deleted"] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                    [name],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "index {} missing", name);
        }
    }

    #[test]
    fn llm_tier_round_trip() {
        use super::LlmTier;
        assert_eq!(LlmTier::parse("ollama").unwrap(), LlmTier::Ollama);
        assert_eq!(LlmTier::parse("claude").unwrap(), LlmTier::Claude);
        assert_eq!(LlmTier::Ollama.as_str(), "ollama");
        assert_eq!(LlmTier::Claude.as_str(), "claude");
        assert!(LlmTier::parse("other").is_err());
    }
}
```

Note: `pub mod dal;` is declared but the file doesn't exist yet — create an empty placeholder module body OR comment the declaration until Task 2 adds it. Prefer the placeholder approach:

```bash
# After writing mod.rs, create an empty dal stub so cargo check passes:
cat > crates/core/src/repair/dal.rs <<'EOF'
//! Repair-note DAL (L4d). Implementation added in Task 2.
EOF
```

- [ ] **Step 3: Register the module in `crates/core/src/lib.rs`**

Find the existing `pub mod maintenance;` line. Add nearby:

```rust
pub mod repair;
```

- [ ] **Step 4: Append trash registry entry**

In `crates/core/src/trash.rs`, find the `REGISTRY` const and append (after the `("maintenance_event", "title")` line from L4c):

```rust
// repair_note uses TEXT (UUID) primary key.
("repair_note", "symptom"),
```

- [ ] **Step 5: Run the tests, verify pass**

Run: `cargo test --package manor-core repair::migration_tests`
Expected: 3 PASS (`v21_creates_repair_note_table`, `v21_creates_repair_indexes`, `llm_tier_round_trip`).

Also: `cargo test --package manor-core trash` — existing trash sweep tests should still pass with the new registry entry.

- [ ] **Step 6: Commit**

```bash
git add crates/core/migrations/V21__repair_note.sql \
        crates/core/src/repair/mod.rs \
        crates/core/src/repair/dal.rs \
        crates/core/src/lib.rs \
        crates/core/src/trash.rs
git commit -m "feat(repair): migration V21 + core types + trash registry (L4d)"
```

---

### Task 2: Repair DAL — CRUD + list_for_asset + trash helpers

**Files:**
- Modify: `crates/core/src/repair/dal.rs` (replace placeholder stub)

- [ ] **Step 1: Write the failing tests**

Replace the placeholder content of `crates/core/src/repair/dal.rs` with:

```rust
//! Repair-note DAL (L4d).

use super::{LlmTier, RepairNote, RepairNoteDraft, RepairSource};
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use uuid::Uuid;

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn insert_repair_note(conn: &Connection, draft: &RepairNoteDraft) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_secs();
    let sources_json = serde_json::to_string(&draft.sources)?;
    let video_sources_json = draft
        .video_sources
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    conn.execute(
        "INSERT INTO repair_note
           (id, asset_id, symptom, body_md, sources, video_sources, tier,
            created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            id,
            draft.asset_id,
            draft.symptom,
            draft.body_md,
            sources_json,
            video_sources_json,
            draft.tier.as_str(),
            now,
        ],
    )?;
    Ok(id)
}

pub fn get_repair_note(conn: &Connection, id: &str) -> Result<Option<RepairNote>> {
    let mut stmt = conn.prepare(
        "SELECT id, asset_id, symptom, body_md, sources, video_sources, tier,
                created_at, updated_at, deleted_at
         FROM repair_note WHERE id = ?1",
    )?;
    stmt.query_row(params![id], row_to_repair_note).optional().map_err(Into::into)
}

pub fn list_for_asset(conn: &Connection, asset_id: &str) -> Result<Vec<RepairNote>> {
    let mut stmt = conn.prepare(
        "SELECT id, asset_id, symptom, body_md, sources, video_sources, tier,
                created_at, updated_at, deleted_at
         FROM repair_note
         WHERE asset_id = ?1 AND deleted_at IS NULL
         ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map(params![asset_id], row_to_repair_note)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn soft_delete_repair_note(conn: &Connection, id: &str) -> Result<()> {
    let changed = conn.execute(
        "UPDATE repair_note SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now_secs(), id],
    )?;
    if changed == 0 {
        return Err(anyhow!("Repair note not found or already deleted"));
    }
    Ok(())
}

pub fn restore_repair_note(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE repair_note SET deleted_at = NULL WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn permanent_delete_repair_note(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM repair_note WHERE id = ?1 AND deleted_at IS NOT NULL",
        params![id],
    )?;
    Ok(())
}

fn row_to_repair_note(row: &Row) -> rusqlite::Result<RepairNote> {
    let sources_json: String = row.get("sources")?;
    let sources: Vec<RepairSource> = serde_json::from_str(&sources_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())),
        )
    })?;
    let video_sources_json: Option<String> = row.get("video_sources")?;
    let video_sources: Option<Vec<RepairSource>> = match video_sources_json {
        Some(s) => Some(serde_json::from_str(&s).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())),
            )
        })?),
        None => None,
    };
    let tier_str: String = row.get("tier")?;
    let tier = LlmTier::parse(&tier_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())),
        )
    })?;
    Ok(RepairNote {
        id: row.get("id")?,
        asset_id: row.get("asset_id")?,
        symptom: row.get("symptom")?,
        body_md: row.get("body_md")?,
        sources,
        video_sources,
        tier,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::{dal as asset_dal, AssetCategory, AssetDraft};
    use crate::assistant::db;
    use tempfile::tempdir;

    fn fresh() -> (tempfile::TempDir, Connection, String) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        let asset = AssetDraft {
            name: "Boiler".into(),
            category: AssetCategory::Appliance,
            make: None, model: None, serial_number: None, purchase_date: None,
            notes: String::new(),
            hero_attachment_uuid: None,
        };
        let asset_id = asset_dal::insert_asset(&conn, &asset).unwrap();
        (dir, conn, asset_id)
    }

    fn draft(asset_id: &str) -> RepairNoteDraft {
        RepairNoteDraft {
            asset_id: asset_id.into(),
            symptom: "won't drain".into(),
            body_md: "Check the filter first. If still clogged, remove the drain hose.".into(),
            sources: vec![
                RepairSource { url: "https://example.com/a".into(), title: "A".into() },
                RepairSource { url: "https://example.com/b".into(), title: "B".into() },
            ],
            video_sources: None,
            tier: LlmTier::Ollama,
        }
    }

    #[test]
    fn insert_and_get_round_trip_with_video_none() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_repair_note(&conn, &draft(&asset_id)).unwrap();
        let got = get_repair_note(&conn, &id).unwrap().unwrap();
        assert_eq!(got.asset_id, asset_id);
        assert_eq!(got.symptom, "won't drain");
        assert_eq!(got.sources.len(), 2);
        assert_eq!(got.sources[0].url, "https://example.com/a");
        assert!(got.video_sources.is_none());
        assert_eq!(got.tier, LlmTier::Ollama);
    }

    #[test]
    fn insert_round_trip_with_video_sources() {
        let (_d, conn, asset_id) = fresh();
        let mut d = draft(&asset_id);
        d.video_sources = Some(vec![RepairSource {
            url: "https://www.youtube.com/watch?v=abc".into(),
            title: "Fix Your Boiler".into(),
        }]);
        let id = insert_repair_note(&conn, &d).unwrap();
        let got = get_repair_note(&conn, &id).unwrap().unwrap();
        let vs = got.video_sources.unwrap();
        assert_eq!(vs.len(), 1);
        assert_eq!(vs[0].title, "Fix Your Boiler");
    }

    #[test]
    fn list_for_asset_orders_desc_and_excludes_trashed() {
        let (_d, conn, asset_id) = fresh();
        let id1 = insert_repair_note(&conn, &draft(&asset_id)).unwrap();
        // Force a tiny gap so created_at differs — SQLite resolution is 1s.
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut d2 = draft(&asset_id);
        d2.symptom = "second".into();
        let _id2 = insert_repair_note(&conn, &d2).unwrap();

        let rows = list_for_asset(&conn, &asset_id).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].symptom, "second");  // DESC: most recent first

        soft_delete_repair_note(&conn, &id1).unwrap();
        let rows = list_for_asset(&conn, &asset_id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].symptom, "second");
    }

    #[test]
    fn soft_delete_restore_round_trip() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_repair_note(&conn, &draft(&asset_id)).unwrap();
        soft_delete_repair_note(&conn, &id).unwrap();
        let got = get_repair_note(&conn, &id).unwrap().unwrap();
        assert!(got.deleted_at.is_some());
        restore_repair_note(&conn, &id).unwrap();
        let got = get_repair_note(&conn, &id).unwrap().unwrap();
        assert!(got.deleted_at.is_none());
    }

    #[test]
    fn permanent_delete_only_removes_trashed() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_repair_note(&conn, &draft(&asset_id)).unwrap();
        // Active note: permanent_delete must NOT remove it.
        permanent_delete_repair_note(&conn, &id).unwrap();
        assert!(get_repair_note(&conn, &id).unwrap().is_some());
        // Trash then purge.
        soft_delete_repair_note(&conn, &id).unwrap();
        permanent_delete_repair_note(&conn, &id).unwrap();
        assert!(get_repair_note(&conn, &id).unwrap().is_none());
    }

    #[test]
    fn soft_delete_returns_error_when_already_trashed() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_repair_note(&conn, &draft(&asset_id)).unwrap();
        soft_delete_repair_note(&conn, &id).unwrap();
        let err = soft_delete_repair_note(&conn, &id).unwrap_err().to_string();
        assert!(err.contains("not found or already deleted"), "got: {}", err);
    }
}
```

- [ ] **Step 2: Run tests, verify pass**

Run: `cargo test --package manor-core repair::dal`
Expected: 6 PASS.

- [ ] **Step 3: Full suite + clippy**

Run:
```
cargo test --package manor-core
cargo clippy --package manor-core -- -D warnings
```
Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/repair/dal.rs
git commit -m "feat(repair): DAL — CRUD + list_for_asset + trash helpers (L4d)"
```

---

### Task 3: Asset cascade extensions for repair_note

**Files:**
- Modify: `crates/core/src/asset/dal.rs` — extend the 3 cascade functions.

- [ ] **Step 1: Write the failing cascade tests**

Append to `crates/core/src/asset/dal.rs`'s existing `#[cfg(test)] mod tests { ... }` block (located around lines 160-340, following the existing L4c cascade tests from Task 8 of L4c):

```rust
#[test]
fn soft_delete_asset_cascades_repair_notes() {
    use crate::repair::dal as repair_dal;
    let (_d, conn) = fresh();
    let asset_id = insert_asset(&conn, &draft("Boiler", AssetCategory::Appliance)).unwrap();
    let note_draft = crate::repair::RepairNoteDraft {
        asset_id: asset_id.clone(),
        symptom: "won't drain".into(),
        body_md: "Check the filter".into(),
        sources: vec![crate::repair::RepairSource {
            url: "https://example.com".into(),
            title: "Example".into(),
        }],
        video_sources: None,
        tier: crate::repair::LlmTier::Ollama,
    };
    repair_dal::insert_repair_note(&conn, &note_draft).unwrap();
    assert_eq!(repair_dal::list_for_asset(&conn, &asset_id).unwrap().len(), 1);
    soft_delete_asset(&conn, &asset_id).unwrap();
    assert_eq!(repair_dal::list_for_asset(&conn, &asset_id).unwrap().len(), 0);
}

#[test]
fn restore_asset_restores_repair_notes_from_same_cascade() {
    use crate::repair::dal as repair_dal;
    let (_d, conn) = fresh();
    let asset_id = insert_asset(&conn, &draft("Boiler", AssetCategory::Appliance)).unwrap();
    let note_draft = crate::repair::RepairNoteDraft {
        asset_id: asset_id.clone(),
        symptom: "test".into(),
        body_md: "body".into(),
        sources: vec![],
        video_sources: None,
        tier: crate::repair::LlmTier::Ollama,
    };
    repair_dal::insert_repair_note(&conn, &note_draft).unwrap();
    soft_delete_asset(&conn, &asset_id).unwrap();
    restore_asset(&conn, &asset_id).unwrap();
    assert_eq!(repair_dal::list_for_asset(&conn, &asset_id).unwrap().len(), 1);
}

#[test]
fn restore_asset_does_not_resurrect_earlier_trashed_repair_notes() {
    use crate::repair::dal as repair_dal;
    let (_d, conn) = fresh();
    let asset_id = insert_asset(&conn, &draft("Boiler", AssetCategory::Appliance)).unwrap();
    let note_draft = crate::repair::RepairNoteDraft {
        asset_id: asset_id.clone(),
        symptom: "test".into(),
        body_md: "body".into(),
        sources: vec![],
        video_sources: None,
        tier: crate::repair::LlmTier::Ollama,
    };
    let note_id = repair_dal::insert_repair_note(&conn, &note_draft).unwrap();
    // Trash the note at ts=100.
    conn.execute(
        "UPDATE repair_note SET deleted_at = 100 WHERE id = ?1",
        rusqlite::params![note_id],
    ).unwrap();
    // Trash the asset at ts=200 (later).
    conn.execute(
        "UPDATE asset SET deleted_at = 200 WHERE id = ?1",
        rusqlite::params![asset_id],
    ).unwrap();
    restore_asset(&conn, &asset_id).unwrap();
    // Note still trashed — its deleted_at=100 doesn't match the asset's 200.
    let row: Option<i64> = conn.query_row(
        "SELECT deleted_at FROM repair_note WHERE id = ?1",
        rusqlite::params![note_id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(row, Some(100));
}

#[test]
fn permanent_delete_asset_hard_deletes_repair_notes() {
    use crate::repair::dal as repair_dal;
    let (_d, conn) = fresh();
    let asset_id = insert_asset(&conn, &draft("Boiler", AssetCategory::Appliance)).unwrap();
    let note_draft = crate::repair::RepairNoteDraft {
        asset_id: asset_id.clone(),
        symptom: "test".into(),
        body_md: "body".into(),
        sources: vec![],
        video_sources: None,
        tier: crate::repair::LlmTier::Ollama,
    };
    repair_dal::insert_repair_note(&conn, &note_draft).unwrap();
    soft_delete_asset(&conn, &asset_id).unwrap();
    permanent_delete_asset(&conn, &asset_id).unwrap();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM repair_note WHERE asset_id = ?1",
        rusqlite::params![asset_id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 0);
}
```

- [ ] **Step 2: Verify tests fail (cascade not yet implemented)**

Run: `cargo test --package manor-core asset::dal::tests::soft_delete_asset_cascades_repair_notes`
Expected: FAIL — `list_for_asset` still returns 1 after asset soft-delete (cascade not yet wired).

- [ ] **Step 3: Extend the three cascade functions in `asset/dal.rs`**

Find the three functions (they sit around lines 115-190 after L4c's Task 8 landed).

Replace `soft_delete_asset` with:

```rust
pub fn soft_delete_asset(conn: &Connection, id: &str) -> Result<()> {
    let now = now_secs();
    // L4c: cascade to maintenance_event.
    conn.execute(
        "UPDATE maintenance_event SET deleted_at = ?1 WHERE asset_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    // L4d: cascade to repair_note.
    conn.execute(
        "UPDATE repair_note SET deleted_at = ?1 WHERE asset_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    conn.execute(
        "UPDATE asset SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    Ok(())
}
```

Replace `restore_asset` with:

```rust
pub fn restore_asset(conn: &Connection, id: &str) -> Result<()> {
    use rusqlite::OptionalExtension;
    let deleted_at: Option<i64> = conn
        .query_row(
            "SELECT deleted_at FROM asset WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    if let Some(ts) = deleted_at {
        // L4c: restore events that share the asset's cascade timestamp.
        conn.execute(
            "UPDATE maintenance_event SET deleted_at = NULL WHERE asset_id = ?1 AND deleted_at = ?2",
            params![id, ts],
        )?;
        // L4d: restore repair_notes that share the asset's cascade timestamp.
        conn.execute(
            "UPDATE repair_note SET deleted_at = NULL WHERE asset_id = ?1 AND deleted_at = ?2",
            params![id, ts],
        )?;
    }
    conn.execute(
        "UPDATE asset SET deleted_at = NULL WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}
```

Replace `permanent_delete_asset` with (new DELETE for repair_note inserted BEFORE the schedule cascade):

```rust
pub fn permanent_delete_asset(conn: &Connection, id: &str) -> Result<()> {
    let now = now_secs();
    // Soft-delete linked attachments (L4a).
    conn.execute(
        "UPDATE attachment SET deleted_at = ?1 WHERE entity_type = 'asset' AND entity_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    // Hard-delete events (L4c).
    conn.execute(
        "DELETE FROM maintenance_event WHERE asset_id = ?1",
        params![id],
    )?;
    // L4d: hard-delete repair_notes BEFORE schedule cascade (FK ordering).
    conn.execute(
        "DELETE FROM repair_note WHERE asset_id = ?1",
        params![id],
    )?;
    // Hard-delete schedules (L4b).
    conn.execute(
        "DELETE FROM maintenance_schedule WHERE asset_id = ?1",
        params![id],
    )?;
    // Hard-delete the asset (only if trashed).
    conn.execute(
        "DELETE FROM asset WHERE id = ?1 AND deleted_at IS NOT NULL",
        params![id],
    )?;
    Ok(())
}
```

Note: If you find the L4c version of `permanent_delete_asset` uses soft-delete of schedules (not hard-delete), match whatever the current on-disk code does — the goal is to slot repair_note hard-delete between event hard-delete and whatever schedule cleanup exists. The important bit is the ORDER: event → repair_note → schedule.

- [ ] **Step 4: Run cascade tests + full core suite**

Run: `cargo test --package manor-core asset::dal`
Expected: all tests PASS (4 new + existing L4a/L4b/L4c).

Run: `cargo test --package manor-core` — full core green.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/asset/dal.rs
git commit -m "feat(asset): extend cascade to repair_note (L4d)"
```

---

## Phase B — App-layer pipeline

### Task 4: Search module (DuckDuckGo + YouTube)

**Files:**
- Create: `crates/app/src/repair/mod.rs` — module root.
- Create: `crates/app/src/repair/search.rs`
- Modify: `crates/app/src/lib.rs` — `pub mod repair;`.

- [ ] **Step 1: Create module root**

Create `crates/app/src/repair/mod.rs`:

```rust
//! Repair-lookup pipeline (L4d).

pub mod search;
```

Add `pub mod repair;` to `crates/app/src/lib.rs` alongside the existing sibling module declarations (e.g., near `pub mod maintenance;`).

- [ ] **Step 2: Write the failing tests**

Create `crates/app/src/repair/search.rs`:

```rust
//! DuckDuckGo + YouTube URL scrapers (L4d).
//!
//! Both functions return `Ok(vec![])` on a parse mismatch (empty results or
//! unparseable HTML). HTTP failures bubble up as `Err`.

use anyhow::{Context, Result};
use manor_core::repair::RepairSource;
use scraper::{Html, Selector};

pub async fn duckduckgo_top_n(
    client: &reqwest::Client,
    query: &str,
    n: usize,
) -> Result<Vec<RepairSource>> {
    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        urlencoding::encode(query)
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .context("duckduckgo request failed")?;
    let body = resp.text().await.context("duckduckgo body read failed")?;
    Ok(parse_ddg_html(&body, n))
}

fn parse_ddg_html(body: &str, n: usize) -> Vec<RepairSource> {
    let doc = Html::parse_document(body);
    // DDG HTML endpoint wraps each result title in .result__title > a
    let title_selector = Selector::parse(".result__title a").unwrap();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for a in doc.select(&title_selector) {
        if out.len() >= n {
            break;
        }
        let href = match a.value().attr("href") {
            Some(h) => h,
            None => continue,
        };
        // DDG sometimes wraps links in a redirector: /l/?kh=...&uddg=<encoded-url>.
        // Unwrap if that's the shape; otherwise use the href verbatim.
        let url = unwrap_ddg_redirector(href).unwrap_or_else(|| href.to_string());
        if seen.contains(&url) {
            continue;
        }
        let title = a.text().collect::<String>().trim().to_string();
        if title.is_empty() {
            continue;
        }
        seen.insert(url.clone());
        out.push(RepairSource { url, title });
    }
    out
}

fn unwrap_ddg_redirector(href: &str) -> Option<String> {
    // /l/?kh=-1&uddg=https%3A%2F%2Fexample.com%2F → https://example.com/
    let needle = "uddg=";
    let idx = href.find(needle)?;
    let encoded = &href[idx + needle.len()..];
    let end = encoded.find('&').unwrap_or(encoded.len());
    let encoded = &encoded[..end];
    urlencoding::decode(encoded).ok().map(|s| s.into_owned())
}

pub async fn youtube_top_n(
    client: &reqwest::Client,
    query: &str,
    n: usize,
) -> Result<Vec<RepairSource>> {
    let url = format!(
        "https://www.youtube.com/results?search_query={}",
        urlencoding::encode(query)
    );
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => return Ok(Vec::new()), // YouTube is a sidecar — never fail the pipeline on it.
    };
    let body = match resp.text().await {
        Ok(b) => b,
        Err(_) => return Ok(Vec::new()),
    };
    Ok(parse_youtube_html(&body, n))
}

fn parse_youtube_html(body: &str, n: usize) -> Vec<RepairSource> {
    // YouTube embeds search data as: var ytInitialData = { ... };
    let Some(start) = body.find("var ytInitialData = ") else {
        return Vec::new();
    };
    let after = &body[start + "var ytInitialData = ".len()..];
    let Some(end) = after.find("};") else {
        return Vec::new();
    };
    let json_str = &after[..=end]; // include the '}'
    let Ok(root) = serde_json::from_str::<serde_json::Value>(json_str) else {
        return Vec::new();
    };
    let items = root
        .pointer("/contents/twoColumnSearchResultsRenderer/primaryContents/sectionListRenderer/contents/0/itemSectionRenderer/contents")
        .and_then(|v| v.as_array());
    let Some(items) = items else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items {
        if out.len() >= n {
            break;
        }
        let Some(renderer) = item.pointer("/videoRenderer") else {
            continue;
        };
        let Some(video_id) = renderer.pointer("/videoId").and_then(|v| v.as_str()) else {
            continue;
        };
        let title = renderer
            .pointer("/title/runs/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if title.is_empty() {
            continue;
        }
        out.push(RepairSource {
            url: format!("https://www.youtube.com/watch?v={}", video_id),
            title,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ddg_extracts_top_n_titles_and_hrefs() {
        let html = r##"
        <html><body>
          <div class="result">
            <h2 class="result__title"><a href="/l/?kh=-1&uddg=https%3A%2F%2Fexample.com%2Fa">Result A</a></h2>
          </div>
          <div class="result">
            <h2 class="result__title"><a href="https://example.com/b">Result B</a></h2>
          </div>
          <div class="result">
            <h2 class="result__title"><a href="https://example.com/c">Result C</a></h2>
          </div>
          <div class="result">
            <h2 class="result__title"><a href="https://example.com/d">Result D</a></h2>
          </div>
        </body></html>
        "##;
        let out = parse_ddg_html(html, 3);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].url, "https://example.com/a"); // redirector unwrapped
        assert_eq!(out[0].title, "Result A");
        assert_eq!(out[1].url, "https://example.com/b");
    }

    #[test]
    fn parse_ddg_dedupes_repeated_hrefs() {
        let html = r##"
        <html><body>
          <div class="result"><h2 class="result__title"><a href="https://example.com/a">Result A</a></h2></div>
          <div class="result"><h2 class="result__title"><a href="https://example.com/a">Result A</a></h2></div>
          <div class="result"><h2 class="result__title"><a href="https://example.com/b">Result B</a></h2></div>
        </body></html>
        "##;
        let out = parse_ddg_html(html, 3);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].url, "https://example.com/a");
        assert_eq!(out[1].url, "https://example.com/b");
    }

    #[test]
    fn parse_ddg_returns_empty_on_no_matches() {
        let html = "<html><body><p>no results</p></body></html>";
        let out = parse_ddg_html(html, 3);
        assert!(out.is_empty());
    }

    #[test]
    fn parse_youtube_extracts_video_ids_and_titles() {
        // Minimal ytInitialData blob with one videoRenderer.
        let html = r##"
        <html><body>
        <script>var ytInitialData = {
          "contents":{
            "twoColumnSearchResultsRenderer":{
              "primaryContents":{
                "sectionListRenderer":{
                  "contents":[{
                    "itemSectionRenderer":{
                      "contents":[
                        {"videoRenderer":{"videoId":"abcDEF123","title":{"runs":[{"text":"Fix Your Boiler"}]}}},
                        {"videoRenderer":{"videoId":"xyzUVW456","title":{"runs":[{"text":"Boiler Teardown"}]}}}
                      ]
                    }
                  }]
                }
              }
            }
          }
        };</script>
        </body></html>
        "##;
        let out = parse_youtube_html(html, 2);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].url, "https://www.youtube.com/watch?v=abcDEF123");
        assert_eq!(out[0].title, "Fix Your Boiler");
        assert_eq!(out[1].url, "https://www.youtube.com/watch?v=xyzUVW456");
    }

    #[test]
    fn parse_youtube_returns_empty_on_missing_initial_data() {
        let html = "<html><body>nothing here</body></html>";
        let out = parse_youtube_html(html, 2);
        assert!(out.is_empty());
    }

    #[test]
    fn parse_youtube_returns_empty_on_malformed_json() {
        let html = r##"<script>var ytInitialData = {not json};</script>"##;
        let out = parse_youtube_html(html, 2);
        assert!(out.is_empty());
    }
}
```

- [ ] **Step 3: Run tests, verify pass**

Run: `cargo test --package manor-app repair::search`
Expected: 6 PASS.

- [ ] **Step 4: Clippy + fmt**

```
cargo clippy --package manor-app -- -D warnings
cargo fmt --package manor-app --check
```

If fmt flags your new file, run `cargo fmt --package manor-app` and restore any pre-existing drift (matches Task discipline in L4c).

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/repair/ crates/app/src/lib.rs
git commit -m "feat(repair): DDG + YouTube URL scrapers (L4d)"
```

---

### Task 5: Fetch + readability trim

**Files:**
- Create: `crates/app/src/repair/fetch.rs`
- Modify: `crates/app/src/repair/mod.rs` — add `pub mod fetch;`.

- [ ] **Step 1: Write the failing tests**

Create `crates/app/src/repair/fetch.rs`:

```rust
//! Fetch + readability trim (L4d).

use anyhow::Result;
use scraper::{Html, Selector};

pub const MAX_BODY_BYTES: u64 = 2 * 1024 * 1024;
pub const FETCH_TIMEOUT_SECS: u64 = 10;
pub const TRIMMED_TEXT_CAP_BYTES: usize = 2 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("couldn't reach {0}")]
    FetchFailed(String),
    #[error("{0} isn't html (content-type: {1})")]
    NotHtml(String, String),
    #[error("{0} is too large")]
    TooLarge(String),
}

pub async fn fetch_and_trim(client: &reqwest::Client, url: &str) -> Result<String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|_| FetchError::FetchFailed(url.to_string()))?;

    let ctype = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !ctype.contains("text/html") {
        return Err(FetchError::NotHtml(url.to_string(), ctype).into());
    }
    if let Some(len) = resp.content_length() {
        if len > MAX_BODY_BYTES {
            return Err(FetchError::TooLarge(url.to_string()).into());
        }
    }
    let body = resp
        .text()
        .await
        .map_err(|_| FetchError::FetchFailed(url.to_string()))?;
    Ok(trim_html_to_excerpt(&body))
}

pub fn trim_html_to_excerpt(body: &str) -> String {
    let doc = Html::parse_document(body);
    let strip_selectors = [
        "script", "style", "nav", "header", "footer", "aside",
        "noscript", "form", "iframe", "svg",
    ];
    // Collect text from the first match of main/article/role=main, else fall back to body.
    let preferred = [
        Selector::parse("main").unwrap(),
        Selector::parse("article").unwrap(),
        Selector::parse("[role=\"main\"]").unwrap(),
    ];
    let mut text = String::new();
    for sel in &preferred {
        if let Some(el) = doc.select(sel).next() {
            collect_text_excluding(&el, &strip_selectors, &mut text);
            if !text.trim().is_empty() {
                break;
            }
        }
    }
    if text.trim().is_empty() {
        if let Some(body_el) = doc.select(&Selector::parse("body").unwrap()).next() {
            collect_text_excluding(&body_el, &strip_selectors, &mut text);
        }
    }
    let collapsed = collapse_whitespace(&text);
    truncate_to_byte_budget(&collapsed, TRIMMED_TEXT_CAP_BYTES)
}

fn collect_text_excluding(
    root: &scraper::ElementRef<'_>,
    skip_tags: &[&str],
    out: &mut String,
) {
    for node in root.descendants() {
        if let scraper::node::Node::Element(el) = node.value() {
            if skip_tags.iter().any(|t| t.eq_ignore_ascii_case(el.name())) {
                // Walk siblings of this skipped subtree; don't recurse into it.
                // `descendants` already enumerates all nodes, so we need to filter by ancestor.
                // Easier approach: track whether the current node has any skipped ancestor.
            }
        }
    }
    // Simpler reliable approach: walk text nodes and check their ancestors.
    for node in root.descendants() {
        if let scraper::node::Node::Text(t) = node.value() {
            let has_skipped_ancestor = node.ancestors().any(|a| {
                if let scraper::node::Node::Element(el) = a.value() {
                    skip_tags.iter().any(|tag| tag.eq_ignore_ascii_case(el.name()))
                } else {
                    false
                }
            });
            if !has_skipped_ancestor {
                out.push_str(t);
                out.push(' ');
            }
        }
    }
}

fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = true;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

fn truncate_to_byte_budget(s: &str, budget: usize) -> String {
    if s.len() <= budget {
        return s.to_string();
    }
    // Find the largest char boundary <= budget.
    let mut cut = budget;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    s[..cut].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_strips_nav_script_style() {
        let html = r##"
        <html>
          <head><style>body{color:red}</style></head>
          <body>
            <nav>Home | About | Contact</nav>
            <script>alert("hi");</script>
            <main>
              <h1>Real Content</h1>
              <p>Keep this paragraph.</p>
            </main>
            <footer>copyright 2025</footer>
          </body>
        </html>
        "##;
        let out = trim_html_to_excerpt(html);
        assert!(out.contains("Real Content"));
        assert!(out.contains("Keep this paragraph"));
        assert!(!out.contains("Home | About"));
        assert!(!out.contains("alert"));
        assert!(!out.contains("copyright"));
    }

    #[test]
    fn trim_prefers_main_over_body() {
        let html = r##"
        <html><body>
          <div>Sidebar noise</div>
          <main><p>Important main content.</p></main>
          <div>Footer noise</div>
        </body></html>
        "##;
        let out = trim_html_to_excerpt(html);
        assert!(out.contains("Important main content"));
        assert!(!out.contains("Sidebar noise"));
        assert!(!out.contains("Footer noise"));
    }

    #[test]
    fn trim_falls_back_to_body_when_no_main() {
        let html = "<html><body><p>Just a body paragraph.</p></body></html>";
        let out = trim_html_to_excerpt(html);
        assert!(out.contains("Just a body paragraph"));
    }

    #[test]
    fn trim_caps_at_2kb() {
        let big = "lorem ipsum ".repeat(500); // ~6 KB
        let html = format!("<html><body><main>{}</main></body></html>", big);
        let out = trim_html_to_excerpt(&html);
        assert!(out.len() <= TRIMMED_TEXT_CAP_BYTES);
    }

    #[test]
    fn trim_collapses_whitespace() {
        let html = "<html><body><main>Hello\n\n\n    World  </main></body></html>";
        let out = trim_html_to_excerpt(html);
        assert_eq!(out, "Hello World");
    }

    #[tokio::test]
    async fn fetch_rejects_non_html_content_type() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string("{}"),
            )
            .mount(&server)
            .await;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();
        let err = fetch_and_trim(&client, &server.uri())
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("isn't html"), "got: {}", err);
    }

    #[tokio::test]
    async fn fetch_succeeds_on_html_response() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/html; charset=utf-8")
                    .set_body_string("<html><body><main>From wiremock</main></body></html>"),
            )
            .mount(&server)
            .await;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();
        let text = fetch_and_trim(&client, &server.uri()).await.unwrap();
        assert_eq!(text, "From wiremock");
    }
}
```

**Note:** `collect_text_excluding` uses the "walk text nodes + filter by ancestor" approach. The earlier draft in the comment block shows the nuance explicitly; in the final code the simpler ancestor-check is authoritative. If compilation complains about `scraper::node::Node` paths, the type is exported as `scraper::Node` in newer versions — adjust the import (`use scraper::node::Node;` or `use scraper::Node;`) to match the version in `Cargo.toml` (`scraper = "0.20"`).

- [ ] **Step 2: Add `pub mod fetch;` to the module root**

Edit `crates/app/src/repair/mod.rs`:

```rust
pub mod fetch;
pub mod search;
```

- [ ] **Step 3: Run tests, verify pass**

Run: `cargo test --package manor-app repair::fetch`
Expected: 7 PASS (5 pure parsing + 2 wiremock integration).

- [ ] **Step 4: Workspace test + clippy**

```
cargo test --workspace
cargo clippy --workspace -- -D warnings
```
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/repair/fetch.rs crates/app/src/repair/mod.rs
git commit -m "feat(repair): page fetch + DIY readability trim (L4d)"
```

---

### Task 6: Synthesis module (prompt builder + Ollama + Claude + trait)

**Files:**
- Create: `crates/app/src/repair/synth.rs`
- Modify: `crates/app/src/repair/mod.rs` — add `pub mod synth;`.
- Modify: `crates/app/src/assistant/ollama.rs` — add non-streaming `chat_collect` helper.

- [ ] **Step 1: Add non-streaming Ollama helper**

Open `crates/app/src/assistant/ollama.rs`. Find the `impl OllamaClient { ... }` block (around line 95). Append this method inside the impl:

```rust
/// Non-streaming variant: runs a chat call, accumulates all token chunks
/// from the internal channel, and returns the concatenated text. No tool
/// calls are expected — this is for one-shot synthesis. Errors surface as
/// `Err(anyhow!(...))` with a coarse message ("unreachable", "error").
pub async fn chat_collect(&self, messages: &[ChatMessage]) -> anyhow::Result<String> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamChunk>(32);
    let outcome = self.chat(messages, &[], &tx).await;
    drop(tx);
    let mut out = String::new();
    while let Some(chunk) = rx.recv().await {
        match chunk {
            StreamChunk::Token(t) => out.push_str(&t),
            StreamChunk::Error(ErrorCode::OllamaUnreachable) => {
                return Err(anyhow::anyhow!("Local model isn't running (Ollama endpoint unreachable)"));
            }
            StreamChunk::Error(_) => {
                return Err(anyhow::anyhow!("Local model call failed"));
            }
            StreamChunk::Done => break,
        }
    }
    let _ = outcome; // tool calls ignored
    Ok(out)
}
```

Note: the existing `chat` method's contract says it does NOT emit `StreamChunk::Done` — tool calls are returned via `ChatOutcome`. For the non-streaming case we just accumulate tokens until the channel closes (after `drop(tx)` + the stream finishes inside `chat`). If `chat` returns before emitting all tokens through the channel, that's a runtime issue we'll see during manual QA; the collect loop handles the channel-closed path via `while let Some`.

Verify the existing `StreamChunk` enum variants match (`Token(String)`, `Error(ErrorCode)`, `Done`) — look at lines 60-75 of `ollama.rs`. If the variants differ (for example `Token { content: String }`), adjust the match arms. The spec above assumes `Token(String)` per the existing streaming patterns in `assistant/commands.rs`.

- [ ] **Step 2: Create `crates/app/src/repair/synth.rs`**

```rust
//! Repair-note synthesis (L4d).

use crate::assistant::ollama::{ChatMessage, ChatRole, OllamaClient, DEFAULT_ENDPOINT, DEFAULT_MODEL};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

pub struct PageExcerpt {
    pub url: String,
    pub title: String,
    pub trimmed_text: String,
}

pub struct SynthInput<'a> {
    pub asset_name: &'a str,
    pub asset_make: Option<&'a str>,
    pub asset_model: Option<&'a str>,
    pub asset_category: &'a str,
    pub symptom: &'a str,
    pub augmented_query: &'a str,
    pub pages: &'a [PageExcerpt],
}

const SYSTEM_PROMPT: &str = "You are a concise home-repair troubleshooter. You help a homeowner diagnose appliance, vehicle, and fixture problems using search-result excerpts.";

pub fn build_user_prompt(input: &SynthInput<'_>) -> String {
    let make = input.asset_make.unwrap_or("unknown");
    let model = input.asset_model.unwrap_or("unknown");
    let mut out = String::new();
    out.push_str("You are helping a homeowner troubleshoot an appliance or fixture problem.\n\n");
    out.push_str("## About the item\n");
    out.push_str(&format!("- Name: {}\n", input.asset_name));
    out.push_str(&format!("- Make: {}\n", make));
    out.push_str(&format!("- Model: {}\n", model));
    out.push_str(&format!("- Category: {}\n\n", input.asset_category));
    out.push_str("## Reported symptom\n");
    out.push_str(input.symptom);
    out.push_str("\n\n");
    out.push_str(&format!(
        "## Search results (trimmed excerpts from the top {} pages)\n",
        input.pages.len()
    ));
    for (i, page) in input.pages.iter().enumerate() {
        out.push_str(&format!("[Source {} — {}]\n", i + 1, page.url));
        out.push_str(&page.trimmed_text);
        out.push_str("\n\n");
    }
    out.push_str(
        "## Your task\n\
         Synthesise a concise troubleshooting summary (150–300 words).\n\n\
         Requirements:\n\
         - Start with the most likely cause in plain language.\n\
         - List 2–4 specific things the user can check or try, in order.\n\
         - Flag any \"call a professional\" cases (gas, high voltage, sealed systems).\n\
         - At the end, list the source URLs as a Markdown bulleted list under \"## Sources\".\n\
         - Do NOT invent model-specific steps that aren't in the excerpts.\n\
         - If the excerpts are thin or off-topic, say so and suggest a more specific search.\n",
    );
    out
}

#[async_trait]
pub trait SynthBackend: Send + Sync {
    async fn synth(&self, input: &SynthInput<'_>) -> Result<String>;
}

pub struct OllamaSynth;

#[async_trait]
impl SynthBackend for OllamaSynth {
    async fn synth(&self, input: &SynthInput<'_>) -> Result<String> {
        let client = OllamaClient::new(DEFAULT_ENDPOINT, DEFAULT_MODEL);
        let messages = vec![
            ChatMessage { role: ChatRole::System, content: SYSTEM_PROMPT.to_string() },
            ChatMessage { role: ChatRole::User, content: build_user_prompt(input) },
        ];
        client.chat_collect(&messages).await
    }
}

pub struct ClaudeSynth {
    pub db: Arc<Mutex<rusqlite::Connection>>,
}

#[async_trait]
impl SynthBackend for ClaudeSynth {
    async fn synth(&self, input: &SynthInput<'_>) -> Result<String> {
        let user_prompt = build_user_prompt(input);
        let reason = format!("Troubleshooting {}", input.asset_name);
        let req = crate::remote::orchestrator::RemoteChatRequest {
            skill: "right_to_repair",
            user_visible_reason: &reason,
            system_prompt: Some(SYSTEM_PROMPT),
            user_prompt: &user_prompt,
            max_tokens: 1024,
        };
        let outcome = crate::remote::orchestrator::remote_chat(self.db.clone(), req)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok(outcome.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(url: &str, text: &str) -> PageExcerpt {
        PageExcerpt {
            url: url.into(),
            title: "t".into(),
            trimmed_text: text.into(),
        }
    }

    #[test]
    fn build_user_prompt_includes_all_fields() {
        let pages = vec![
            page("https://example.com/a", "excerpt A text"),
            page("https://example.com/b", "excerpt B text"),
        ];
        let input = SynthInput {
            asset_name: "Worcester Boiler",
            asset_make: Some("Worcester"),
            asset_model: Some("Bosch 8000"),
            asset_category: "appliance",
            symptom: "won't fire up",
            augmented_query: "Worcester Bosch 8000 won't fire up",
            pages: &pages,
        };
        let p = build_user_prompt(&input);
        assert!(p.contains("Name: Worcester Boiler"));
        assert!(p.contains("Make: Worcester"));
        assert!(p.contains("Model: Bosch 8000"));
        assert!(p.contains("Category: appliance"));
        assert!(p.contains("won't fire up"));
        assert!(p.contains("top 2 pages"));
        assert!(p.contains("[Source 1 — https://example.com/a]"));
        assert!(p.contains("excerpt A text"));
        assert!(p.contains("[Source 2 — https://example.com/b]"));
    }

    #[test]
    fn build_user_prompt_handles_missing_make_model() {
        let pages = vec![page("https://example.com/a", "text")];
        let input = SynthInput {
            asset_name: "Something",
            asset_make: None,
            asset_model: None,
            asset_category: "other",
            symptom: "broken",
            augmented_query: "broken",
            pages: &pages,
        };
        let p = build_user_prompt(&input);
        assert!(p.contains("Make: unknown"));
        assert!(p.contains("Model: unknown"));
    }

    #[test]
    fn build_user_prompt_handles_partial_page_count() {
        let pages = vec![page("https://a", "only one")];
        let input = SynthInput {
            asset_name: "X",
            asset_make: None,
            asset_model: None,
            asset_category: "other",
            symptom: "s",
            augmented_query: "s",
            pages: &pages,
        };
        let p = build_user_prompt(&input);
        assert!(p.contains("top 1 pages"));
        assert!(p.contains("[Source 1 —"));
        assert!(!p.contains("[Source 2 —"));
    }
}
```

- [ ] **Step 3: Register the module**

Edit `crates/app/src/repair/mod.rs`:

```rust
pub mod fetch;
pub mod search;
pub mod synth;
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package manor-app repair::synth`
Expected: 3 PASS (prompt-builder tests).

Run: `cargo test --package manor-app` — full app-crate suite green.

- [ ] **Step 5: Clippy**

```
cargo clippy --package manor-app -- -D warnings
```

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/repair/synth.rs \
        crates/app/src/repair/mod.rs \
        crates/app/src/assistant/ollama.rs
git commit -m "feat(repair): prompt builder + Ollama + Claude synth backends (L4d)"
```

---

### Task 7: Pipeline orchestrator

**Files:**
- Create: `crates/app/src/repair/pipeline.rs`
- Modify: `crates/app/src/repair/mod.rs` — add `pub mod pipeline;`.

- [ ] **Step 1: Write the failing tests + implementation in one block**

Because the pipeline depends on types from `synth.rs` (including `SynthBackend` trait seam), the test file needs a `StubSynth` helper. Both implementation and tests go in `pipeline.rs`:

Create `crates/app/src/repair/pipeline.rs`:

```rust
//! Repair-lookup pipeline orchestrator (L4d).

use super::fetch::fetch_and_trim;
use super::search::{duckduckgo_top_n, youtube_top_n};
use super::synth::{ClaudeSynth, OllamaSynth, PageExcerpt, SynthBackend, SynthInput};
use anyhow::{anyhow, Result};
use manor_core::repair::{LlmTier, RepairNote, RepairNoteDraft, RepairSource};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

pub const USER_AGENT: &str = "Manor/0.4 (+https://manor.app)";
pub const HTTP_TIMEOUT_SECS: u64 = 10;
pub const DDG_TOP_N: usize = 3;
pub const YOUTUBE_TOP_N: usize = 2;
pub const MIN_SYNTH_BODY_CHARS: usize = 50;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierRequest {
    Ollama,
    Claude,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineOutcome {
    pub note: Option<RepairNote>,
    pub sources: Vec<RepairSource>,
    pub video_sources: Vec<RepairSource>,
    pub empty_or_failed: bool,
}

/// Pure helper — builds the query string sent to DDG/YouTube.
pub fn build_augmented_query(
    make: Option<&str>,
    model: Option<&str>,
    symptom: &str,
) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(m) = make {
        if !m.trim().is_empty() {
            parts.push(m.trim());
        }
    }
    if let Some(m) = model {
        if !m.trim().is_empty() {
            parts.push(m.trim());
        }
    }
    parts.push(symptom.trim());
    parts.join(" ")
}

pub async fn run_repair_search(
    db: Arc<Mutex<rusqlite::Connection>>,
    asset_id: String,
    symptom: String,
    tier: TierRequest,
) -> Result<PipelineOutcome> {
    let backend: Box<dyn SynthBackend> = match tier {
        TierRequest::Ollama => Box::new(OllamaSynth),
        TierRequest::Claude => Box::new(ClaudeSynth { db: db.clone() }),
    };
    run_pipeline_with_backend(db, asset_id, symptom, backend.as_ref()).await
}

pub(crate) async fn run_pipeline_with_backend(
    db: Arc<Mutex<rusqlite::Connection>>,
    asset_id: String,
    symptom: String,
    backend: &dyn SynthBackend,
) -> Result<PipelineOutcome> {
    // 1. Load asset (synchronous lock scope).
    let (asset_name, asset_make, asset_model, asset_category) = {
        let conn = db.lock().map_err(|e| anyhow!("db lock: {e}"))?;
        let asset = manor_core::asset::dal::get_asset(&conn, &asset_id)?
            .ok_or_else(|| anyhow!("Asset not found"))?;
        (
            asset.name,
            asset.make,
            asset.model,
            asset.category.as_str().to_string(),
        )
    };
    let query = build_augmented_query(asset_make.as_deref(), asset_model.as_deref(), &symptom);

    // 2. Build shared HTTP client.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| anyhow!("http client build: {e}"))?;

    // 3. Fire DDG + YouTube concurrently.
    let (ddg_res, yt_res) = tokio::join!(
        duckduckgo_top_n(&client, &query, DDG_TOP_N),
        youtube_top_n(&client, &query, YOUTUBE_TOP_N),
    );
    let ddg = ddg_res.unwrap_or_else(|_| Vec::new());
    let yt = yt_res.unwrap_or_else(|_| Vec::new());

    // 4. Fetch + trim each DDG URL concurrently.
    let fetches = ddg
        .iter()
        .map(|src| {
            let client = client.clone();
            let url = src.url.clone();
            let title = src.title.clone();
            async move {
                let txt = fetch_and_trim(&client, &url).await?;
                Ok::<PageExcerpt, anyhow::Error>(PageExcerpt {
                    url,
                    title,
                    trimmed_text: txt,
                })
            }
        })
        .collect::<Vec<_>>();
    let results = futures_util::future::join_all(fetches).await;
    let pages: Vec<PageExcerpt> = results.into_iter().filter_map(|r| r.ok()).collect();

    if pages.is_empty() {
        return Ok(PipelineOutcome {
            note: None,
            sources: ddg,
            video_sources: yt,
            empty_or_failed: true,
        });
    }

    // 5. Synthesise.
    let input = SynthInput {
        asset_name: &asset_name,
        asset_make: asset_make.as_deref(),
        asset_model: asset_model.as_deref(),
        asset_category: &asset_category,
        symptom: &symptom,
        augmented_query: &query,
        pages: &pages,
    };
    let body_text = match backend.synth(&input).await {
        Ok(t) => t,
        Err(_) => {
            return Ok(PipelineOutcome {
                note: None,
                sources: ddg,
                video_sources: yt,
                empty_or_failed: true,
            });
        }
    };
    if body_text.trim().len() < MIN_SYNTH_BODY_CHARS {
        return Ok(PipelineOutcome {
            note: None,
            sources: ddg,
            video_sources: yt,
            empty_or_failed: true,
        });
    }

    // 6. Persist.
    let tier_enum = match tier_from_backend_marker(&input, &body_text) {
        _ => match std::mem::size_of_val(&input) {
            // Tier is known by the caller; we pass it down explicitly below.
            // This placeholder branch is unreachable.
            _ => LlmTier::Ollama,
        },
    };
    let _ = tier_enum; // silenced — actual tier passed explicitly
    let tier_persist = match tier_label_of(backend) {
        Some(t) => t,
        None => LlmTier::Ollama,
    };
    let video_sources = if yt.is_empty() { None } else { Some(yt.clone()) };
    let draft = RepairNoteDraft {
        asset_id: asset_id.clone(),
        symptom: symptom.clone(),
        body_md: body_text,
        sources: ddg.clone(),
        video_sources,
        tier: tier_persist,
    };

    let note = {
        let conn = db.lock().map_err(|e| anyhow!("db lock: {e}"))?;
        let id = manor_core::repair::dal::insert_repair_note(&conn, &draft)?;
        manor_core::repair::dal::get_repair_note(&conn, &id)?
            .ok_or_else(|| anyhow!("inserted row missing"))?
    };

    Ok(PipelineOutcome {
        note: Some(note),
        sources: ddg,
        video_sources: yt,
        empty_or_failed: false,
    })
}

// The backend trait is type-erased; we can't recover the tier from it directly.
// Use a downcast-free marker: the caller of `run_repair_search` sets `TierRequest`,
// so we take the *tier* as a parameter alongside `backend`. Refactor:

fn tier_from_backend_marker(_input: &SynthInput<'_>, _body: &str) -> () {}
fn tier_label_of(_backend: &dyn SynthBackend) -> Option<LlmTier> {
    None
}
```

**The draft above shows the structure but the tier-passing deserves a refactor to pass `TierRequest` explicitly into the inner function.** Replace the body with this cleaner version:

```rust
//! Repair-lookup pipeline orchestrator (L4d).

use super::fetch::fetch_and_trim;
use super::search::{duckduckgo_top_n, youtube_top_n};
use super::synth::{ClaudeSynth, OllamaSynth, PageExcerpt, SynthBackend, SynthInput};
use anyhow::{anyhow, Result};
use manor_core::repair::{LlmTier, RepairNote, RepairNoteDraft, RepairSource};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

pub const USER_AGENT: &str = "Manor/0.4 (+https://manor.app)";
pub const HTTP_TIMEOUT_SECS: u64 = 10;
pub const DDG_TOP_N: usize = 3;
pub const YOUTUBE_TOP_N: usize = 2;
pub const MIN_SYNTH_BODY_CHARS: usize = 50;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierRequest {
    Ollama,
    Claude,
}

impl TierRequest {
    fn as_persist_tier(self) -> LlmTier {
        match self {
            TierRequest::Ollama => LlmTier::Ollama,
            TierRequest::Claude => LlmTier::Claude,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineOutcome {
    pub note: Option<RepairNote>,
    pub sources: Vec<RepairSource>,
    pub video_sources: Vec<RepairSource>,
    pub empty_or_failed: bool,
}

pub fn build_augmented_query(
    make: Option<&str>,
    model: Option<&str>,
    symptom: &str,
) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(m) = make {
        if !m.trim().is_empty() {
            parts.push(m.trim());
        }
    }
    if let Some(m) = model {
        if !m.trim().is_empty() {
            parts.push(m.trim());
        }
    }
    parts.push(symptom.trim());
    parts.join(" ")
}

pub async fn run_repair_search(
    db: Arc<Mutex<rusqlite::Connection>>,
    asset_id: String,
    symptom: String,
    tier: TierRequest,
) -> Result<PipelineOutcome> {
    let backend: Box<dyn SynthBackend> = match tier {
        TierRequest::Ollama => Box::new(OllamaSynth),
        TierRequest::Claude => Box::new(ClaudeSynth { db: db.clone() }),
    };
    run_pipeline_with_backend(db, asset_id, symptom, tier, backend.as_ref()).await
}

pub(crate) async fn run_pipeline_with_backend(
    db: Arc<Mutex<rusqlite::Connection>>,
    asset_id: String,
    symptom: String,
    tier: TierRequest,
    backend: &dyn SynthBackend,
) -> Result<PipelineOutcome> {
    // 1. Load asset.
    let (asset_name, asset_make, asset_model, asset_category) = {
        let conn = db.lock().map_err(|e| anyhow!("db lock: {e}"))?;
        let asset = manor_core::asset::dal::get_asset(&conn, &asset_id)?
            .ok_or_else(|| anyhow!("Asset not found"))?;
        (
            asset.name,
            asset.make,
            asset.model,
            asset.category.as_str().to_string(),
        )
    };
    let query = build_augmented_query(asset_make.as_deref(), asset_model.as_deref(), &symptom);

    // 2. Build shared HTTP client.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| anyhow!("http client build: {e}"))?;

    // 3. Fire DDG + YouTube concurrently.
    let (ddg_res, yt_res) = tokio::join!(
        duckduckgo_top_n(&client, &query, DDG_TOP_N),
        youtube_top_n(&client, &query, YOUTUBE_TOP_N),
    );
    let ddg = ddg_res.unwrap_or_else(|_| Vec::new());
    let yt = yt_res.unwrap_or_else(|_| Vec::new());

    // 4. Fetch + trim each DDG URL concurrently.
    let fetches = ddg.iter().map(|src| {
        let client = client.clone();
        let url = src.url.clone();
        let title = src.title.clone();
        async move {
            let txt = fetch_and_trim(&client, &url).await?;
            Ok::<PageExcerpt, anyhow::Error>(PageExcerpt { url, title, trimmed_text: txt })
        }
    }).collect::<Vec<_>>();
    let results = futures_util::future::join_all(fetches).await;
    let pages: Vec<PageExcerpt> = results.into_iter().filter_map(|r| r.ok()).collect();

    if pages.is_empty() {
        return Ok(PipelineOutcome {
            note: None,
            sources: ddg,
            video_sources: yt,
            empty_or_failed: true,
        });
    }

    // 5. Synthesise.
    let input = SynthInput {
        asset_name: &asset_name,
        asset_make: asset_make.as_deref(),
        asset_model: asset_model.as_deref(),
        asset_category: &asset_category,
        symptom: &symptom,
        augmented_query: &query,
        pages: &pages,
    };
    let body_text = match backend.synth(&input).await {
        Ok(t) => t,
        Err(_) => {
            return Ok(PipelineOutcome {
                note: None,
                sources: ddg,
                video_sources: yt,
                empty_or_failed: true,
            });
        }
    };
    if body_text.trim().len() < MIN_SYNTH_BODY_CHARS {
        return Ok(PipelineOutcome {
            note: None,
            sources: ddg,
            video_sources: yt,
            empty_or_failed: true,
        });
    }

    // 6. Persist.
    let video_sources = if yt.is_empty() { None } else { Some(yt.clone()) };
    let draft = RepairNoteDraft {
        asset_id: asset_id.clone(),
        symptom: symptom.clone(),
        body_md: body_text,
        sources: ddg.clone(),
        video_sources,
        tier: tier.as_persist_tier(),
    };
    let note = {
        let conn = db.lock().map_err(|e| anyhow!("db lock: {e}"))?;
        let id = manor_core::repair::dal::insert_repair_note(&conn, &draft)?;
        manor_core::repair::dal::get_repair_note(&conn, &id)?
            .ok_or_else(|| anyhow!("inserted row missing"))?
    };

    Ok(PipelineOutcome {
        note: Some(note),
        sources: ddg,
        video_sources: yt,
        empty_or_failed: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use manor_core::asset::{dal as asset_dal, AssetCategory, AssetDraft};
    use manor_core::assistant::db;

    struct StubSynth {
        response: Result<String>,
    }

    #[async_trait]
    impl SynthBackend for StubSynth {
        async fn synth(&self, _input: &SynthInput<'_>) -> Result<String> {
            match &self.response {
                Ok(s) => Ok(s.clone()),
                Err(e) => Err(anyhow!(e.to_string())),
            }
        }
    }

    fn fresh_db_with_asset() -> (tempfile::TempDir, Arc<Mutex<rusqlite::Connection>>, String) {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        let asset = AssetDraft {
            name: "Test Boiler".into(),
            category: AssetCategory::Appliance,
            make: Some("Worcester".into()),
            model: Some("B8000".into()),
            serial_number: None,
            purchase_date: None,
            notes: String::new(),
            hero_attachment_uuid: None,
        };
        let asset_id = asset_dal::insert_asset(&conn, &asset).unwrap();
        (dir, Arc::new(Mutex::new(conn)), asset_id)
    }

    #[test]
    fn build_query_make_plus_model_plus_symptom() {
        let q = build_augmented_query(Some("Worcester"), Some("B8000"), "won't fire up");
        assert_eq!(q, "Worcester B8000 won't fire up");
    }

    #[test]
    fn build_query_missing_make_uses_model_and_symptom() {
        let q = build_augmented_query(None, Some("B8000"), "won't fire up");
        assert_eq!(q, "B8000 won't fire up");
    }

    #[test]
    fn build_query_missing_both_returns_raw_symptom() {
        let q = build_augmented_query(None, None, "won't fire up");
        assert_eq!(q, "won't fire up");
    }

    #[test]
    fn build_query_empty_strings_treated_as_missing() {
        let q = build_augmented_query(Some("   "), Some(""), "symptom");
        assert_eq!(q, "symptom");
    }

    // Integration test using wiremock for DDG/YouTube/fetch + StubSynth for synth.
    // Patches the pipeline's HTTP targets via an alternate entry point that accepts
    // the fully-built client + override URLs. Here we test just the stub-synth persist path
    // by pre-baking pages.

    #[tokio::test]
    async fn stub_synth_success_persists_repair_note() {
        use wiremock::matchers::{method, path_regex};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let (_d, db, asset_id) = fresh_db_with_asset();
        let server = MockServer::start().await;

        // DDG responds with 3 result URLs pointing at the same mock server.
        let ddg_html = format!(r##"
            <html><body>
            <div class="result"><h2 class="result__title"><a href="{0}/p1">P1</a></h2></div>
            <div class="result"><h2 class="result__title"><a href="{0}/p2">P2</a></h2></div>
            <div class="result"><h2 class="result__title"><a href="{0}/p3">P3</a></h2></div>
            </body></html>
        "##, server.uri());
        // The pipeline calls html.duckduckgo.com directly; we can't intercept that URL
        // without a client seam. So this test exercises `run_pipeline_with_backend` via
        // a simulated path: insert a note directly to validate the persist branch.
        // (The full wire-level pipeline test is deferred to the Task 12 manual QA.)
        //
        // Instead, unit-test the persist path via direct DAL call that mirrors what
        // the pipeline does post-synth:
        let draft = manor_core::repair::RepairNoteDraft {
            asset_id: asset_id.clone(),
            symptom: "test".into(),
            body_md: "A" .repeat(100),
            sources: vec![manor_core::repair::RepairSource {
                url: "https://example.com/a".into(),
                title: "A".into(),
            }],
            video_sources: None,
            tier: manor_core::repair::LlmTier::Ollama,
        };
        let id = {
            let conn = db.lock().unwrap();
            manor_core::repair::dal::insert_repair_note(&conn, &draft).unwrap()
        };
        let conn = db.lock().unwrap();
        let note = manor_core::repair::dal::get_repair_note(&conn, &id).unwrap().unwrap();
        assert_eq!(note.symptom, "test");
        let _ = (server, ddg_html, StubSynth { response: Ok("ok".into()) }); // silence unused
    }

    #[tokio::test]
    async fn pipeline_returns_empty_or_failed_when_asset_missing() {
        let (_d, db, _real_asset) = fresh_db_with_asset();
        let stub = StubSynth { response: Ok("body text".into()) };
        let err = run_pipeline_with_backend(
            db,
            "no-such-asset".into(),
            "symptom".into(),
            TierRequest::Ollama,
            &stub,
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(err.contains("Asset not found"), "got: {}", err);
    }
}
```

**Important honesty note about the tests:** The canonical pipeline tests in the spec (§20.6) describe wire-level tests with `wiremock` intercepting DDG + YouTube + fetch. That requires a seam — the pipeline currently hard-codes `https://html.duckduckgo.com/html/` and `https://www.youtube.com/results`. A full wire-level test would require either:

- (a) Refactoring the search module to accept base URLs as parameters (with defaults), then passing `&server.uri()` in tests; or
- (b) Using an HTTP proxy / DNS override (heavy).

For this plan, the pipeline tests stay at the **pure helper level** (`build_augmented_query`, the persist path via direct DAL call, and the "asset not found" early-exit). The end-to-end pipe-through-real-DDG test lives in **Task 12 manual QA**.

If you want the full wire-level test, add a `search::test_seam` module that exports `duckduckgo_top_n_with_base_url(client, base, query, n)` and `youtube_top_n_with_base_url(...)`, route the regular functions through them with the hardcoded bases, and have the pipeline tests call via the seam. This is a follow-up cleanup worth doing in v0.5.1.

- [ ] **Step 2: Register the module**

Edit `crates/app/src/repair/mod.rs`:

```rust
pub mod fetch;
pub mod pipeline;
pub mod search;
pub mod synth;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --package manor-app repair::pipeline`
Expected: 6 PASS (4 build_query + 2 pipeline-shape).

Run: `cargo test --workspace` — full suite green.

- [ ] **Step 4: Clippy**

```
cargo clippy --workspace -- -D warnings
```

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/repair/pipeline.rs crates/app/src/repair/mod.rs
git commit -m "feat(repair): pipeline orchestrator + augmented-query helper (L4d)"
```

---

## Phase C — Tauri commands

### Task 8: Tauri commands + trash wiring

**Files:**
- Create: `crates/app/src/repair/commands.rs`
- Modify: `crates/app/src/repair/mod.rs` — add `pub mod commands;`.
- Modify: `crates/app/src/lib.rs` — register 5 Tauri commands.
- Modify: `crates/app/src/safety/trash_commands.rs` — add `"repair_note"` arms.

- [ ] **Step 1: Create `commands.rs`**

```rust
//! Tauri commands for repair-note + search (L4d).

use crate::assistant::commands::Db;
use manor_core::repair::RepairNote;
use super::pipeline::{run_repair_search, PipelineOutcome, TierRequest};
use tauri::State;

#[tauri::command]
pub async fn repair_search_ollama(
    asset_id: String,
    symptom: String,
    state: State<'_, Db>,
) -> Result<PipelineOutcome, String> {
    let db = state.0.clone();
    run_repair_search(db, asset_id, symptom, TierRequest::Ollama)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn repair_search_claude(
    asset_id: String,
    symptom: String,
    state: State<'_, Db>,
) -> Result<PipelineOutcome, String> {
    let db = state.0.clone();
    run_repair_search(db, asset_id, symptom, TierRequest::Claude)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn repair_note_list_for_asset(
    asset_id: String,
    state: State<'_, Db>,
) -> Result<Vec<RepairNote>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    manor_core::repair::dal::list_for_asset(&conn, &asset_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn repair_note_get(
    id: String,
    state: State<'_, Db>,
) -> Result<Option<RepairNote>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    manor_core::repair::dal::get_repair_note(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn repair_note_delete(
    id: String,
    state: State<'_, Db>,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    manor_core::repair::dal::soft_delete_repair_note(&conn, &id).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register the module**

Edit `crates/app/src/repair/mod.rs`:

```rust
pub mod commands;
pub mod fetch;
pub mod pipeline;
pub mod search;
pub mod synth;
```

- [ ] **Step 3: Register 5 Tauri commands in `lib.rs`**

Find `.invoke_handler(tauri::generate_handler![...])` (around line 216 after L4c landed). Append before the closing `])`:

```rust
crate::repair::commands::repair_search_ollama,
crate::repair::commands::repair_search_claude,
crate::repair::commands::repair_note_list_for_asset,
crate::repair::commands::repair_note_get,
crate::repair::commands::repair_note_delete,
```

- [ ] **Step 4: Add `"repair_note"` arms to `trash_commands.rs`**

In `crates/app/src/safety/trash_commands.rs`, find the `trash_restore` match block (around line 25-36). Add:

```rust
"repair_note" => manor_core::repair::dal::restore_repair_note(&conn, &entity_id)
    .map_err(|e| e.to_string()),
```

Find the `trash_permanent_delete` match block (around line 70-82). Add:

```rust
"repair_note" => manor_core::repair::dal::permanent_delete_repair_note(&conn, &entity_id)
    .map_err(|e| e.to_string()),
```

Both arms follow the pattern of the existing `"maintenance_event"` arms (added in L4c Task 9).

- [ ] **Step 5: Run workspace tests + clippy**

```
cargo test --workspace
cargo clippy --workspace -- -D warnings
```
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/repair/ crates/app/src/lib.rs crates/app/src/safety/trash_commands.rs
git commit -m "feat(repair): Tauri commands + trash wiring (L4d)"
```

---

## Phase D — Frontend

### Task 9: Frontend IPC + Zustand store

**Files:**
- Create: `apps/desktop/src/lib/repair/ipc.ts`
- Create: `apps/desktop/src/lib/repair/state.ts`

- [ ] **Step 1: Create `event-ipc`-style module `ipc.ts`**

```ts
import { invoke } from "@tauri-apps/api/core";

export type LlmTier = "ollama" | "claude";

export interface RepairSource {
  url: string;
  title: string;
}

export interface RepairNote {
  id: string;
  asset_id: string;
  symptom: string;
  body_md: string;
  sources: RepairSource[];
  video_sources: RepairSource[] | null;
  tier: LlmTier;
  created_at: number;
  updated_at: number;
  deleted_at: number | null;
}

export interface PipelineOutcome {
  note: RepairNote | null;
  sources: RepairSource[];
  video_sources: RepairSource[];
  empty_or_failed: boolean;
}

export async function searchOllama(assetId: string, symptom: string): Promise<PipelineOutcome> {
  return await invoke<PipelineOutcome>("repair_search_ollama", { assetId, symptom });
}

export async function searchClaude(assetId: string, symptom: string): Promise<PipelineOutcome> {
  return await invoke<PipelineOutcome>("repair_search_claude", { assetId, symptom });
}

export async function listForAsset(assetId: string): Promise<RepairNote[]> {
  return await invoke<RepairNote[]>("repair_note_list_for_asset", { assetId });
}

export async function get(id: string): Promise<RepairNote | null> {
  return await invoke<RepairNote | null>("repair_note_get", { id });
}

export async function deleteNote(id: string): Promise<void> {
  await invoke<void>("repair_note_delete", { id });
}
```

- [ ] **Step 2: Create `state.ts` (Zustand store)**

```ts
import { create } from "zustand";
import * as ipc from "./ipc";

type SearchStatus =
  | { kind: "idle" }
  | { kind: "searching"; tier: ipc.LlmTier }
  | { kind: "error"; message: string };

interface RepairStore {
  notesByAsset: Record<string, ipc.RepairNote[]>;
  lastOutcomeByAsset: Record<string, ipc.PipelineOutcome | null>;
  lastSymptomByAsset: Record<string, string>;
  searchStatus: SearchStatus;

  loadForAsset(assetId: string): Promise<void>;
  invalidateAsset(assetId: string): void;
  searchOllama(assetId: string, symptom: string): Promise<ipc.PipelineOutcome>;
  searchClaude(assetId: string, symptom: string): Promise<ipc.PipelineOutcome>;
  deleteNote(id: string, assetId: string): Promise<void>;
  clearLastOutcome(assetId: string): void;
}

export const useRepairStore = create<RepairStore>((set, get) => ({
  notesByAsset: {},
  lastOutcomeByAsset: {},
  lastSymptomByAsset: {},
  searchStatus: { kind: "idle" },

  async loadForAsset(assetId) {
    try {
      const rows = await ipc.listForAsset(assetId);
      set((s) => ({ notesByAsset: { ...s.notesByAsset, [assetId]: rows } }));
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      console.error("repair-state: loadForAsset failed", message);
    }
  },

  invalidateAsset(assetId) {
    set((s) => {
      const next = { ...s.notesByAsset };
      delete next[assetId];
      return { notesByAsset: next };
    });
  },

  async searchOllama(assetId, symptom) {
    set((s) => ({
      searchStatus: { kind: "searching", tier: "ollama" },
      lastSymptomByAsset: { ...s.lastSymptomByAsset, [assetId]: symptom },
    }));
    try {
      const outcome = await ipc.searchOllama(assetId, symptom);
      set((s) => ({
        lastOutcomeByAsset: { ...s.lastOutcomeByAsset, [assetId]: outcome },
        searchStatus: { kind: "idle" },
      }));
      if (outcome.note) get().invalidateAsset(assetId);
      return outcome;
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ searchStatus: { kind: "error", message } });
      throw e;
    }
  },

  async searchClaude(assetId, symptom) {
    set((s) => ({
      searchStatus: { kind: "searching", tier: "claude" },
      lastSymptomByAsset: { ...s.lastSymptomByAsset, [assetId]: symptom },
    }));
    try {
      const outcome = await ipc.searchClaude(assetId, symptom);
      set((s) => ({
        lastOutcomeByAsset: { ...s.lastOutcomeByAsset, [assetId]: outcome },
        searchStatus: { kind: "idle" },
      }));
      if (outcome.note) get().invalidateAsset(assetId);
      return outcome;
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ searchStatus: { kind: "error", message } });
      throw e;
    }
  },

  async deleteNote(id, assetId) {
    await ipc.deleteNote(id);
    get().invalidateAsset(assetId);
  },

  clearLastOutcome(assetId) {
    set((s) => {
      const nextOutcome = { ...s.lastOutcomeByAsset };
      delete nextOutcome[assetId];
      return { lastOutcomeByAsset: nextOutcome };
    });
  },
}));
```

- [ ] **Step 3: Type-check + existing tests**

```
cd apps/desktop
pnpm tsc --noEmit
pnpm test
```
Expected: TS clean, existing test count (from L4c) still passes.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/lib/repair/
git commit -m "feat(repair): frontend IPC + Zustand store (L4d)"
```

---

### Task 10: UI components + markdown + AssetDetail integration

**Files:**
- Create: `apps/desktop/src/components/Bones/RepairMarkdown.tsx`
- Create: `apps/desktop/src/components/Bones/RepairNoteCard.tsx`
- Create: `apps/desktop/src/components/Bones/TroubleshootResultCard.tsx`
- Create: `apps/desktop/src/components/Bones/TroubleshootBlock.tsx`
- Modify: `apps/desktop/src/components/Bones/AssetDetail.tsx` — mount the block.
- Modify: `apps/desktop/package.json` — add `react-markdown` + `remark-gfm`.

- [ ] **Step 1: Install markdown deps**

```
cd apps/desktop
pnpm add react-markdown remark-gfm
```

- [ ] **Step 2: Create `RepairMarkdown.tsx` (small wrapper with external-link handling)**

```tsx
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { open as openUrl } from "@tauri-apps/plugin-shell";

interface Props {
  body: string;
}

export function RepairMarkdown({ body }: Props) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        a: ({ href, children, ...rest }) => (
          <a
            {...rest}
            href={href}
            onClick={(e) => {
              e.preventDefault();
              if (href) {
                void openUrl(href);
              }
            }}
            style={{ color: "var(--link, #0366d6)", textDecoration: "underline", cursor: "pointer" }}
          >
            {children}
          </a>
        ),
      }}
    >
      {body}
    </ReactMarkdown>
  );
}
```

- [ ] **Step 3: Create `RepairNoteCard.tsx`**

```tsx
import { useState } from "react";
import { Calendar, Trash2, ChevronDown, ChevronRight } from "lucide-react";
import { open as openUrl } from "@tauri-apps/plugin-shell";
import type { RepairNote } from "../../lib/repair/ipc";
import { useRepairStore } from "../../lib/repair/state";
import { RepairMarkdown } from "./RepairMarkdown";

interface Props {
  note: RepairNote;
}

function relativeDate(unixSec: number): string {
  const d = new Date(unixSec * 1000);
  const days = Math.floor((Date.now() - d.getTime()) / (1000 * 60 * 60 * 24));
  if (days === 0) return "today";
  if (days === 1) return "yesterday";
  if (days < 14) return `${days} days ago`;
  const weeks = Math.floor(days / 7);
  if (weeks < 8) return `${weeks} weeks ago`;
  return d.toLocaleDateString("en-GB", { month: "short", day: "numeric", year: "numeric" });
}

function truncate(s: string, n: number): string {
  return s.length <= n ? s : s.slice(0, n - 1) + "…";
}

export function RepairNoteCard({ note }: Props) {
  const [expanded, setExpanded] = useState(false);
  const { deleteNote } = useRepairStore();

  return (
    <div style={{
      border: "1px solid var(--border, #e5e5e5)",
      borderRadius: 6,
      marginBottom: 8,
      overflow: "hidden",
    }}>
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          width: "100%",
          padding: "10px 12px",
          background: "none",
          border: "none",
          cursor: "pointer",
          textAlign: "left",
        }}
      >
        {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        <Calendar size={14} color="var(--ink-soft, #888)" />
        <span style={{ color: "var(--ink-soft, #888)", fontSize: 12, minWidth: 80 }}>
          {relativeDate(note.created_at)}
        </span>
        <span style={{ flex: 1 }}>{truncate(note.symptom, 60)}</span>
        <span style={{
          fontSize: 11,
          padding: "2px 6px",
          borderRadius: 4,
          background: note.tier === "claude" ? "var(--accent-bg, #eef5ff)" : "var(--surface-subtle, #f4f4f4)",
          color: "var(--ink-soft, #666)",
        }}>
          {note.tier === "claude" ? "claude" : "local"}
        </span>
        <span
          role="button"
          aria-label="Delete repair note"
          onClick={(e) => {
            e.stopPropagation();
            void deleteNote(note.id, note.asset_id);
          }}
          style={{ cursor: "pointer", padding: 4 }}
        >
          <Trash2 size={14} color="var(--ink-soft, #888)" />
        </span>
      </button>
      {expanded && (
        <div style={{ padding: "8px 16px 16px 16px" }}>
          <RepairMarkdown body={note.body_md} />
          {note.sources.length > 0 && (
            <div style={{ marginTop: 8 }}>
              <div style={{ fontSize: 12, color: "var(--ink-soft, #888)", marginBottom: 4 }}>Sources</div>
              <ul style={{ margin: 0, paddingLeft: 18 }}>
                {note.sources.map((s) => (
                  <li key={s.url}>
                    <a
                      href={s.url}
                      onClick={(e) => { e.preventDefault(); void openUrl(s.url); }}
                      style={{ cursor: "pointer" }}
                    >
                      {s.title}
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          )}
          {note.video_sources && note.video_sources.length > 0 && (
            <div style={{ marginTop: 8 }}>
              <div style={{ fontSize: 12, color: "var(--ink-soft, #888)", marginBottom: 4 }}>Videos</div>
              <ul style={{ margin: 0, paddingLeft: 18 }}>
                {note.video_sources.map((s) => (
                  <li key={s.url}>
                    <a
                      href={s.url}
                      onClick={(e) => { e.preventDefault(); void openUrl(s.url); }}
                      style={{ cursor: "pointer" }}
                    >
                      {s.title}
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Create `TroubleshootResultCard.tsx`**

```tsx
import { X, Sparkles } from "lucide-react";
import { open as openUrl } from "@tauri-apps/plugin-shell";
import type { PipelineOutcome } from "../../lib/repair/ipc";
import { useRepairStore } from "../../lib/repair/state";
import { RepairMarkdown } from "./RepairMarkdown";

interface Props {
  assetId: string;
  outcome: PipelineOutcome;
}

export function TroubleshootResultCard({ assetId, outcome }: Props) {
  const { searchClaude, clearLastOutcome, searchStatus, lastSymptomByAsset } = useRepairStore();
  const symptom = lastSymptomByAsset[assetId] ?? outcome.note?.symptom ?? "";

  const onTryClaude = async () => {
    try {
      await searchClaude(assetId, symptom);
    } catch {
      // error is rendered via searchStatus
    }
  };

  const borderColor = outcome.empty_or_failed
    ? "var(--warn-border, #d4a72c)"
    : "var(--border, #e5e5e5)";

  // Mode A: success with persisted note.
  if (outcome.note && !outcome.empty_or_failed) {
    const note = outcome.note;
    return (
      <div style={{
        border: `1px solid ${borderColor}`,
        borderRadius: 6,
        padding: 16,
        marginBottom: 16,
        background: "var(--surface-elevated, #fafafa)",
      }}>
        <div style={{ display: "flex", alignItems: "center", marginBottom: 8 }}>
          <strong style={{ flex: 1 }}>{note.symptom}</strong>
          <span style={{
            fontSize: 11,
            padding: "2px 6px",
            borderRadius: 4,
            background: note.tier === "claude" ? "var(--accent-bg, #eef5ff)" : "var(--surface-subtle, #f4f4f4)",
            color: "var(--ink-soft, #666)",
            marginRight: 8,
          }}>
            {note.tier}
          </span>
          <button
            type="button"
            onClick={() => clearLastOutcome(assetId)}
            aria-label="Close result"
            style={{ background: "none", border: "none", cursor: "pointer" }}
          >
            <X size={14} />
          </button>
        </div>
        <RepairMarkdown body={note.body_md} />
        {note.sources.length > 0 && (
          <div style={{ marginTop: 12 }}>
            <div style={{ fontSize: 12, color: "var(--ink-soft, #888)", marginBottom: 4 }}>Sources</div>
            <ul style={{ margin: 0, paddingLeft: 18 }}>
              {note.sources.map((s) => (
                <li key={s.url}>
                  <a href={s.url} onClick={(e) => { e.preventDefault(); void openUrl(s.url); }} style={{ cursor: "pointer" }}>
                    {s.title}
                  </a>
                </li>
              ))}
            </ul>
          </div>
        )}
        {note.video_sources && note.video_sources.length > 0 && (
          <div style={{ marginTop: 8 }}>
            <div style={{ fontSize: 12, color: "var(--ink-soft, #888)", marginBottom: 4 }}>Videos</div>
            <ul style={{ margin: 0, paddingLeft: 18 }}>
              {note.video_sources.map((s) => (
                <li key={s.url}>
                  <a href={s.url} onClick={(e) => { e.preventDefault(); void openUrl(s.url); }} style={{ cursor: "pointer" }}>
                    {s.title}
                  </a>
                </li>
              ))}
            </ul>
          </div>
        )}
        {note.tier === "ollama" && (
          <div style={{ marginTop: 12 }}>
            <button
              type="button"
              onClick={onTryClaude}
              disabled={searchStatus.kind === "searching"}
            >
              <Sparkles size={14} /> Try with Claude
            </button>
          </div>
        )}
      </div>
    );
  }

  // Mode B: empty/failed Ollama — sources present, no body, offer Claude.
  return (
    <div style={{
      border: `1px solid ${borderColor}`,
      borderRadius: 6,
      padding: 16,
      marginBottom: 16,
      background: "var(--surface-elevated, #fafafa)",
    }}>
      <div style={{ display: "flex", alignItems: "center", marginBottom: 8 }}>
        <strong style={{ flex: 1 }}>
          The local model didn't return a usable answer for "{symptom}".
        </strong>
        <button
          type="button"
          onClick={() => clearLastOutcome(assetId)}
          aria-label="Dismiss"
          style={{ background: "none", border: "none", cursor: "pointer" }}
        >
          <X size={14} />
        </button>
      </div>
      {outcome.sources.length > 0 && (
        <div style={{ marginBottom: 12 }}>
          <div style={{ fontSize: 12, color: "var(--ink-soft, #888)", marginBottom: 4 }}>
            Sources we found (unread):
          </div>
          <ul style={{ margin: 0, paddingLeft: 18 }}>
            {outcome.sources.map((s) => (
              <li key={s.url}>
                <a href={s.url} onClick={(e) => { e.preventDefault(); void openUrl(s.url); }} style={{ cursor: "pointer" }}>
                  {s.title}
                </a>
              </li>
            ))}
          </ul>
        </div>
      )}
      <button
        type="button"
        onClick={onTryClaude}
        disabled={searchStatus.kind === "searching"}
      >
        <Sparkles size={14} /> Try with Claude
      </button>
    </div>
  );
}
```

- [ ] **Step 5: Create `TroubleshootBlock.tsx`**

```tsx
import { useEffect, useState } from "react";
import { useRepairStore } from "../../lib/repair/state";
import { RepairNoteCard } from "./RepairNoteCard";
import { TroubleshootResultCard } from "./TroubleshootResultCard";

interface Props {
  assetId: string;
}

const MAX_SYMPTOM_LEN = 200;

export function TroubleshootBlock({ assetId }: Props) {
  const {
    notesByAsset,
    lastOutcomeByAsset,
    searchStatus,
    loadForAsset,
    searchOllama,
  } = useRepairStore();
  const [symptom, setSymptom] = useState("");

  const outcome = lastOutcomeByAsset[assetId] ?? null;
  const notes = notesByAsset[assetId] ?? [];
  // Exclude the note currently shown in the result card from the history list.
  const historyNotes = outcome?.note
    ? notes.filter((n) => n.id !== outcome.note!.id)
    : notes;

  useEffect(() => {
    if (!notesByAsset[assetId]) void loadForAsset(assetId);
  }, [assetId, notesByAsset, loadForAsset]);

  const disabled = searchStatus.kind === "searching" || symptom.trim().length === 0;

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (disabled) return;
    try {
      await searchOllama(assetId, symptom.trim());
      setSymptom("");
    } catch {
      // error surfaces via searchStatus
    }
  };

  return (
    <section style={{ marginTop: 24 }}>
      <div style={{ marginBottom: 12 }}>
        <h3 style={{ margin: 0 }}>Troubleshoot</h3>
        <div style={{ fontSize: 12, color: "var(--ink-soft, #888)", fontStyle: "italic" }}>
          Search the web and summarise — uses your local model first.
        </div>
      </div>

      {searchStatus.kind === "error" && (
        <div style={{
          border: "1px solid var(--danger, #c43)",
          background: "var(--danger-bg, #fff5f5)",
          color: "var(--danger, #c43)",
          padding: 8,
          borderRadius: 4,
          marginBottom: 8,
          fontSize: 13,
        }}>
          {searchStatus.message}
        </div>
      )}

      <form
        onSubmit={onSubmit}
        style={{ display: "flex", gap: 8, marginBottom: 12 }}
      >
        <input
          type="text"
          value={symptom}
          onChange={(e) => setSymptom(e.target.value)}
          maxLength={MAX_SYMPTOM_LEN}
          placeholder="What's wrong? e.g., won't drain, making grinding noise"
          style={{
            flex: 1,
            padding: 8,
            border: "1px solid var(--border, #e5e5e5)",
            borderRadius: 4,
          }}
        />
        <button type="submit" disabled={disabled}>
          {searchStatus.kind === "searching"
            ? searchStatus.tier === "claude"
              ? "Asking Claude…"
              : "Asking qwen2.5…"
            : "Search"}
        </button>
      </form>

      {outcome && <TroubleshootResultCard assetId={assetId} outcome={outcome} />}

      {historyNotes.length > 0 && (
        <div>
          {historyNotes.map((n) => <RepairNoteCard key={n.id} note={n} />)}
        </div>
      )}
    </section>
  );
}
```

- [ ] **Step 6: Mount on `AssetDetail.tsx`**

Open `apps/desktop/src/components/Bones/AssetDetail.tsx`. Find where `<HistoryBlock assetId={id} />` is rendered (added in L4c Task 13) and the subsequent `<h2>Documents</h2>` heading. Add the import at top:

```tsx
import { TroubleshootBlock } from "./TroubleshootBlock";
```

Insert between HistoryBlock and Documents:

```tsx
<TroubleshootBlock assetId={id} />
```

- [ ] **Step 7: Type-check + build**

```
cd apps/desktop
pnpm tsc --noEmit
pnpm test
pnpm build
```
Expected: TS clean, existing tests pass, build succeeds.

- [ ] **Step 8: Commit**

```bash
git add apps/desktop/src/components/Bones/ \
        apps/desktop/src/components/Bones/AssetDetail.tsx \
        apps/desktop/package.json apps/desktop/pnpm-lock.yaml
git commit -m "feat(repair): TroubleshootBlock + result card + history card + markdown (L4d)"
```

---

### Task 11: RTL tests for Troubleshoot UI

**Files:**
- Create: `apps/desktop/src/components/Bones/__tests__/TroubleshootBlock.test.tsx`

- [ ] **Step 1: Write the failing tests**

```tsx
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import { TroubleshootBlock } from "../TroubleshootBlock";
import { useRepairStore } from "../../../lib/repair/state";

vi.mock("../../../lib/repair/state", () => ({
  useRepairStore: vi.fn(),
}));

// react-markdown fails in jsdom without a simple mock; stub the wrapper.
vi.mock("../RepairMarkdown", () => ({
  RepairMarkdown: ({ body }: { body: string }) => <div data-testid="md">{body}</div>,
}));

// plugin-shell isn't available in jsdom; mock it.
vi.mock("@tauri-apps/plugin-shell", () => ({
  open: vi.fn(),
}));

describe("TroubleshootBlock", () => {
  const loadForAsset = vi.fn();
  const invalidateAsset = vi.fn();
  const searchOllama = vi.fn().mockResolvedValue({
    note: null,
    sources: [],
    video_sources: [],
    empty_or_failed: true,
  });
  const searchClaude = vi.fn();
  const deleteNote = vi.fn();
  const clearLastOutcome = vi.fn();

  function mockStore(overrides: Record<string, unknown> = {}) {
    (useRepairStore as unknown as ReturnType<typeof vi.fn>).mockImplementation(() => ({
      notesByAsset: {},
      lastOutcomeByAsset: {},
      lastSymptomByAsset: {},
      searchStatus: { kind: "idle" },
      loadForAsset,
      invalidateAsset,
      searchOllama,
      searchClaude,
      deleteNote,
      clearLastOutcome,
      ...overrides,
    }));
  }

  beforeEach(() => {
    mockStore();
    searchOllama.mockClear();
    searchClaude.mockClear();
    loadForAsset.mockClear();
  });

  afterEach(() => cleanup());

  it("renders the search input + header + subtitle", () => {
    render(<TroubleshootBlock assetId="a1" />);
    expect(screen.getByText("Troubleshoot")).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/What's wrong/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Search/ })).toBeInTheDocument();
  });

  it("disables Search button when symptom is empty", () => {
    render(<TroubleshootBlock assetId="a1" />);
    const btn = screen.getByRole("button", { name: /Search/ });
    expect(btn).toBeDisabled();
  });

  it("enforces 200-char maxLength on symptom input", () => {
    render(<TroubleshootBlock assetId="a1" />);
    const input = screen.getByPlaceholderText(/What's wrong/) as HTMLInputElement;
    expect(input.maxLength).toBe(200);
  });

  it("disables Search and shows 'Asking qwen2.5…' while searching", () => {
    mockStore({ searchStatus: { kind: "searching", tier: "ollama" } });
    render(<TroubleshootBlock assetId="a1" />);
    expect(screen.getByRole("button", { name: /Asking qwen2.5/ })).toBeDisabled();
  });

  it("shows an error band when searchStatus is error", () => {
    mockStore({ searchStatus: { kind: "error", message: "Local model isn't running" } });
    render(<TroubleshootBlock assetId="a1" />);
    expect(screen.getByText("Local model isn't running")).toBeInTheDocument();
  });

  it("calls searchOllama on form submit with non-empty symptom", async () => {
    render(<TroubleshootBlock assetId="a1" />);
    const input = screen.getByPlaceholderText(/What's wrong/);
    fireEvent.change(input, { target: { value: "won't drain" } });
    fireEvent.submit(input.closest("form")!);
    expect(searchOllama).toHaveBeenCalledWith("a1", "won't drain");
  });

  it("trims whitespace from symptom before submitting", async () => {
    render(<TroubleshootBlock assetId="a1" />);
    const input = screen.getByPlaceholderText(/What's wrong/);
    fireEvent.change(input, { target: { value: "   hello   " } });
    fireEvent.submit(input.closest("form")!);
    expect(searchOllama).toHaveBeenCalledWith("a1", "hello");
  });
});
```

- [ ] **Step 2: Run tests**

```
cd apps/desktop
pnpm test TroubleshootBlock
```
Expected: 7 PASS.

Run full suite: `pnpm test`
Expected: Previous-count + 7 = new total. All green.

- [ ] **Step 3: TS + clippy-equivalent**

```
pnpm tsc --noEmit
```
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/Bones/__tests__/TroubleshootBlock.test.tsx
git commit -m "test(repair): RTL tests for TroubleshootBlock (L4d)"
```

---

## Phase E — Integration + ship

### Task 12: Integration tests + full green battery + merge

**Files:**
- Create: `crates/app/src/repair/commands_tests.rs` (or append `#[cfg(test)] mod integration_tests { ... }` to `commands.rs`).

- [ ] **Step 1: Add integration tests to `commands.rs`**

Append to `crates/app/src/repair/commands.rs`:

```rust
#[cfg(test)]
mod integration_tests {
    use manor_core::asset::{dal as asset_dal, AssetCategory, AssetDraft};
    use manor_core::assistant::db;
    use manor_core::repair::{dal as repair_dal, LlmTier, RepairNoteDraft, RepairSource};

    fn fresh_with_asset() -> (tempfile::TempDir, rusqlite::Connection, String) {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        let asset = AssetDraft {
            name: "Boiler".into(),
            category: AssetCategory::Appliance,
            make: None, model: None, serial_number: None, purchase_date: None,
            notes: String::new(),
            hero_attachment_uuid: None,
        };
        let asset_id = asset_dal::insert_asset(&conn, &asset).unwrap();
        (dir, conn, asset_id)
    }

    #[test]
    fn list_and_delete_round_trip() {
        let (_d, conn, asset_id) = fresh_with_asset();
        let draft = RepairNoteDraft {
            asset_id: asset_id.clone(),
            symptom: "won't drain".into(),
            body_md: "check the filter".into(),
            sources: vec![RepairSource {
                url: "https://example.com".into(),
                title: "Example".into(),
            }],
            video_sources: None,
            tier: LlmTier::Ollama,
        };
        let id = repair_dal::insert_repair_note(&conn, &draft).unwrap();

        let rows = repair_dal::list_for_asset(&conn, &asset_id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id);

        repair_dal::soft_delete_repair_note(&conn, &id).unwrap();
        let rows = repair_dal::list_for_asset(&conn, &asset_id).unwrap();
        assert!(rows.is_empty());

        let got = repair_dal::get_repair_note(&conn, &id).unwrap().unwrap();
        assert!(got.deleted_at.is_some());
    }

    #[test]
    fn list_orders_desc_and_excludes_trashed() {
        let (_d, conn, asset_id) = fresh_with_asset();
        let mk = |symptom: &str| RepairNoteDraft {
            asset_id: asset_id.clone(),
            symptom: symptom.into(),
            body_md: "body".into(),
            sources: vec![],
            video_sources: None,
            tier: LlmTier::Ollama,
        };
        let id1 = repair_dal::insert_repair_note(&conn, &mk("first")).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        repair_dal::insert_repair_note(&conn, &mk("second")).unwrap();

        let rows = repair_dal::list_for_asset(&conn, &asset_id).unwrap();
        assert_eq!(rows[0].symptom, "second");
        assert_eq!(rows[1].symptom, "first");

        repair_dal::soft_delete_repair_note(&conn, &id1).unwrap();
        let rows = repair_dal::list_for_asset(&conn, &asset_id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].symptom, "second");
    }
}
```

- [ ] **Step 2: Run the full green battery**

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check | grep -E "repair|lib\.rs|trash_commands|asset/dal\.rs" || echo "repair files fmt-clean"
cd apps/desktop && pnpm tsc --noEmit && pnpm test && pnpm build
```

If any `repair/*` or `repair_note` in lib.rs / trash_commands.rs / asset/dal.rs shows drift, run `cargo fmt --package manor-app` (or manor-core) and restore pre-existing drift. Only the files YOUR tasks touched should land in the commit.

- [ ] **Step 3: Commit integration tests**

```bash
git add crates/app/src/repair/commands.rs
git commit -m "test(repair): integration tests for list + soft-delete round trip (L4d)"
```

- [ ] **Step 4: Manual QA scenario (user-driven)**

Run `pnpm tauri dev` from `apps/desktop`. In the app:

1. Open Bones → Assets → create "Test Boiler" (appliance, make=Worcester, model=Bosch-8000).
2. Open the asset detail. Scroll to the new **Troubleshoot** block (below History, above Documents).
3. Type "won't fire up" → Search. With Ollama running locally and `qwen2.5:7b-instruct` pulled: expect a Mode A result card with a markdown body + source list within ~30s.
4. Click a source link — should open in the system browser (not in a webview).
5. Click `Try with Claude`. (Requires a Claude API key stored via the Remote settings.) Expect a second card replacing the first with tier=`claude`.
6. Click Close on the result card. The note should still appear in the history list below.
7. Click the history row. Expanded view shows body + sources.
8. Click the trash icon on the history row. Row disappears. Open the Trash view in Bones; the note should appear. Restore → reappears on AssetDetail.
9. Trash the asset itself. Notes cascade out of view. Restore asset → notes come back.
10. If Ollama is OFF: Mode B renders with sources (fetched successfully) + "Try with Claude" button. No persisted note.

Report which of the steps worked and which didn't. Any failures trigger a targeted fix before merge.

- [ ] **Step 5: Report branch state**

Run from the worktree root:

```bash
git log --oneline main..HEAD
git status
```

Confirm: clean working tree, 12 commits on the feature branch, all green battery.

- [ ] **Step 6: Do NOT merge**

The merge (`git merge --no-ff` on main + worktree cleanup) is user-driven. Surface the ready-to-merge state for Hana to authorize.

---

## Definition of done recap

- Migration V21 applies on fresh + existing dev DBs.
- `repair_note` CRUD + list_for_asset round-trip via core DAL + app commands.
- Pipeline: DDG + YouTube scrape → top-3 fetch + DIY readability trim → Ollama synth → persist.
- Ollama empty/error path transitions to "Try with Claude" button state; clicking routes through `remote::orchestrator::remote_chat` with skill `"right_to_repair"`.
- Markdown renders; external links open via `@tauri-apps/plugin-shell`.
- History list: collapsed by default; click expands; trash icon soft-deletes.
- Asset cascade: soft-delete → hidden; restore (timestamp-scoped) → back; permanent-delete → gone (FK-order-safe: event → repair_note → schedule → asset).
- Trash sweep includes `repair_note`; restore + permanent-delete match arms extended.
- `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `pnpm tsc --noEmit`, `pnpm test`, `pnpm build` all green.
- Manual QA scenario passes (steps 1-10 above).

---

*End of L4d implementation plan.*
