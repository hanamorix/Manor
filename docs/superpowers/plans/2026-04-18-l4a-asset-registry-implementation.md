# L4a Asset Registry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Manor's first v0.5 Bones slice — an asset registry tab listing household things (appliances, vehicles, fixtures) with structured-lite fields, a hero image, and attached PDFs/images. Includes a small core cleanup (`attachment::list_for_text_entity`) closing a L3a follow-up.

**Architecture:** Two-crate split mirroring L3a — core holds schema + types + DAL + a shared attachment helper; app holds Tauri commands + attachment-staging + orphan sweep. New `Bones` top-level nav tab. React UI with Zustand store, matching L3a's Recipe pattern.

**Tech Stack:** Rust (rusqlite, refinery, chrono, uuid), React + TypeScript + Zustand, Lucide icons, Tauri v2 dialog plugin (file picker).

**Spec:** `docs/superpowers/specs/2026-04-18-l4a-asset-registry-design.md`

---

## File structure

### New Rust files
- `crates/core/migrations/V18__asset.sql`
- `crates/core/src/asset/mod.rs` — types.
- `crates/core/src/asset/dal.rs` — CRUD + list with filter.
- `crates/app/src/asset/mod.rs` — module root.
- `crates/app/src/asset/commands.rs` — CRUD + attach + list_documents.
- `crates/app/src/asset/importer.rs` — hero / document staging + `stage_sweep_run_on_startup`.

### New frontend files
- `apps/desktop/src/lib/asset/ipc.ts`
- `apps/desktop/src/lib/asset/state.ts`
- `apps/desktop/src/components/Bones/BonesTab.tsx`
- `apps/desktop/src/components/Bones/AssetCard.tsx`
- `apps/desktop/src/components/Bones/AssetDetail.tsx`
- `apps/desktop/src/components/Bones/AssetEditDrawer.tsx`
- `apps/desktop/src/components/Bones/DocumentList.tsx`

### Modified files
- `crates/core/src/lib.rs` — `pub mod asset;`.
- `crates/core/src/attachment.rs` — add `list_for_text_entity`.
- `crates/core/src/trash.rs` — append `("asset", "name")` to `REGISTRY`.
- `crates/app/src/lib.rs` — register Tauri commands, `pub mod asset;`, call startup sweep.
- `apps/desktop/src/lib/nav.ts` — add `"bones"` to `View` union.
- `apps/desktop/src/components/Nav/Sidebar.tsx` — add Bones entry.
- `apps/desktop/src/App.tsx` — route `view === "bones"` to `<BonesTab />`.

---

## Task 1: Migration V18 + trash registry

**Files:**
- Create: `crates/core/migrations/V18__asset.sql`
- Modify: `crates/core/src/trash.rs`

- [ ] **Step 1: Write migration SQL**

```sql
-- V18__asset.sql
-- L4a Asset Registry.

CREATE TABLE asset (
    id                    TEXT PRIMARY KEY,
    name                  TEXT NOT NULL,
    category              TEXT NOT NULL CHECK (category IN ('appliance','vehicle','fixture','other')),
    make                  TEXT,
    model                 TEXT,
    serial_number         TEXT,
    purchase_date         TEXT,
    notes                 TEXT NOT NULL DEFAULT '',
    hero_attachment_uuid  TEXT,
    created_at            INTEGER NOT NULL,
    updated_at            INTEGER NOT NULL,
    deleted_at            INTEGER
);

CREATE INDEX idx_asset_deleted  ON asset(deleted_at);
CREATE INDEX idx_asset_category ON asset(category) WHERE deleted_at IS NULL;
CREATE INDEX idx_asset_name     ON asset(name COLLATE NOCASE);
CREATE INDEX idx_asset_hero     ON asset(hero_attachment_uuid) WHERE hero_attachment_uuid IS NOT NULL;
```

- [ ] **Step 2: Register `asset` in the trash sweeper**

Open `crates/core/src/trash.rs` at line 21. The existing `REGISTRY: &[(&str, &str)]` list contains entries like `("recipe", "title")`, `("staple_item", "name")`. Append `("asset", "name")` in alphabetical (or similar) position — match the convention used by recipe/staple additions.

- [ ] **Step 3: Run migrations + workspace tests**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4a-asset-registry
cargo test -p manor-core --lib -- migrations
cargo test --workspace --lib
```
Expected: green. Baseline before L4a should be 236 core + 85 app = 321 lib tests.

- [ ] **Step 4: Commit**

```bash
git add crates/core/migrations/V18__asset.sql crates/core/src/trash.rs
git commit -m "feat(asset): migration V18 + trash registry entry"
```

---

## Task 2: Core types + DAL

**Files:**
- Create: `crates/core/src/asset/mod.rs`
- Create: `crates/core/src/asset/dal.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Create types**

`crates/core/src/asset/mod.rs`:

```rust
//! Asset registry — types + CRUD. Pure data layer.

pub mod dal;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetCategory {
    Appliance,
    Vehicle,
    Fixture,
    Other,
}

impl AssetCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            AssetCategory::Appliance => "appliance",
            AssetCategory::Vehicle => "vehicle",
            AssetCategory::Fixture => "fixture",
            AssetCategory::Other => "other",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "appliance" => Self::Appliance,
            "vehicle" => Self::Vehicle,
            "fixture" => Self::Fixture,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetDraft {
    pub name: String,
    pub category: AssetCategory,
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub purchase_date: Option<String>,
    pub notes: String,
    pub hero_attachment_uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    pub name: String,
    pub category: AssetCategory,
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub purchase_date: Option<String>,
    pub notes: String,
    pub hero_attachment_uuid: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}
```

- [ ] **Step 2: Implement DAL + tests**

`crates/core/src/asset/dal.rs`:

```rust
//! Asset DAL — CRUD with soft-delete + filter.

use super::{Asset, AssetCategory, AssetDraft};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

#[derive(Debug, Clone, Default)]
pub struct AssetListFilter {
    pub search: Option<String>,
    pub category: Option<AssetCategory>,
    pub include_trashed: bool,
}

pub fn insert_asset(conn: &Connection, draft: &AssetDraft) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_secs();
    conn.execute(
        "INSERT INTO asset (id, name, category, make, model, serial_number, purchase_date, notes,
                             hero_attachment_uuid, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![
            id, draft.name, draft.category.as_str(),
            draft.make, draft.model, draft.serial_number, draft.purchase_date,
            draft.notes, draft.hero_attachment_uuid, now,
        ],
    )?;
    Ok(id)
}

pub fn get_asset(conn: &Connection, id: &str) -> Result<Option<Asset>> {
    select_one(conn, id, /*include_trashed=*/ false)
}

pub fn get_asset_including_trashed(conn: &Connection, id: &str) -> Result<Option<Asset>> {
    select_one(conn, id, /*include_trashed=*/ true)
}

fn select_one(conn: &Connection, id: &str, include_trashed: bool) -> Result<Option<Asset>> {
    let mut sql = String::from(
        "SELECT id, name, category, make, model, serial_number, purchase_date, notes,
                hero_attachment_uuid, created_at, updated_at, deleted_at
         FROM asset WHERE id = ?1",
    );
    if !include_trashed {
        sql.push_str(" AND deleted_at IS NULL");
    }
    let mut stmt = conn.prepare(&sql)?;
    let row = stmt.query_row(params![id], row_to_asset).optional()?;
    Ok(row)
}

pub fn list_assets(conn: &Connection, filter: &AssetListFilter) -> Result<Vec<Asset>> {
    let mut sql = String::from(
        "SELECT id, name, category, make, model, serial_number, purchase_date, notes,
                hero_attachment_uuid, created_at, updated_at, deleted_at
         FROM asset WHERE 1=1",
    );
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if !filter.include_trashed {
        sql.push_str(" AND deleted_at IS NULL");
    }
    if let Some(q) = filter.search.as_ref().filter(|s| !s.is_empty()) {
        sql.push_str(" AND name LIKE ?");
        params.push(Box::new(format!("%{}%", q)));
    }
    if let Some(c) = filter.category {
        sql.push_str(" AND category = ?");
        params.push(Box::new(c.as_str().to_string()));
    }
    sql.push_str(" ORDER BY name COLLATE NOCASE ASC");

    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), row_to_asset)?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn update_asset(conn: &Connection, id: &str, draft: &AssetDraft) -> Result<()> {
    let now = now_secs();
    conn.execute(
        "UPDATE asset SET name = ?1, category = ?2, make = ?3, model = ?4, serial_number = ?5,
                          purchase_date = ?6, notes = ?7, hero_attachment_uuid = ?8, updated_at = ?9
         WHERE id = ?10",
        params![
            draft.name, draft.category.as_str(),
            draft.make, draft.model, draft.serial_number, draft.purchase_date,
            draft.notes, draft.hero_attachment_uuid, now, id,
        ],
    )?;
    Ok(())
}

pub fn soft_delete_asset(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE asset SET deleted_at = ?1 WHERE id = ?2", params![now_secs(), id])?;
    Ok(())
}

pub fn restore_asset(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE asset SET deleted_at = NULL WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn set_hero_attachment(conn: &Connection, id: &str, uuid: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE asset SET hero_attachment_uuid = ?1, updated_at = ?2 WHERE id = ?3",
        params![uuid, now_secs(), id],
    )?;
    Ok(())
}

fn row_to_asset(row: &rusqlite::Row) -> rusqlite::Result<Asset> {
    let category: String = row.get(2)?;
    Ok(Asset {
        id: row.get(0)?,
        name: row.get(1)?,
        category: AssetCategory::from_db(&category),
        make: row.get(3)?,
        model: row.get(4)?,
        serial_number: row.get(5)?,
        purchase_date: row.get(6)?,
        notes: row.get(7)?,
        hero_attachment_uuid: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        deleted_at: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use tempfile::tempdir;

    fn fresh() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn draft(name: &str, cat: AssetCategory) -> AssetDraft {
        AssetDraft {
            name: name.into(),
            category: cat,
            make: None, model: None, serial_number: None, purchase_date: None,
            notes: String::new(),
            hero_attachment_uuid: None,
        }
    }

    #[test]
    fn insert_and_get_roundtrips_with_all_optional_fields_null() {
        let (_d, conn) = fresh();
        let id = insert_asset(&conn, &draft("Boiler", AssetCategory::Appliance)).unwrap();
        let got = get_asset(&conn, &id).unwrap().unwrap();
        assert_eq!(got.name, "Boiler");
        assert_eq!(got.category, AssetCategory::Appliance);
        assert!(got.make.is_none());
        assert!(got.model.is_none());
        assert!(got.hero_attachment_uuid.is_none());
    }

    #[test]
    fn update_replaces_fields_cleanly_including_clearing_to_none() {
        let (_d, conn) = fresh();
        let mut d = draft("Boiler", AssetCategory::Appliance);
        d.make = Some("Worcester".into());
        d.serial_number = Some("123".into());
        let id = insert_asset(&conn, &d).unwrap();

        let mut d2 = draft("Boiler", AssetCategory::Appliance);
        d2.make = None;
        d2.serial_number = None;
        update_asset(&conn, &id, &d2).unwrap();

        let got = get_asset(&conn, &id).unwrap().unwrap();
        assert!(got.make.is_none());
        assert!(got.serial_number.is_none());
    }

    #[test]
    fn get_asset_hides_trashed_get_including_surfaces_them() {
        let (_d, conn) = fresh();
        let id = insert_asset(&conn, &draft("Gone", AssetCategory::Other)).unwrap();
        soft_delete_asset(&conn, &id).unwrap();
        assert!(get_asset(&conn, &id).unwrap().is_none());
        let ghost = get_asset_including_trashed(&conn, &id).unwrap().unwrap();
        assert!(ghost.deleted_at.is_some());
    }

    #[test]
    fn list_filters_by_search_and_category() {
        let (_d, conn) = fresh();
        insert_asset(&conn, &draft("Boiler", AssetCategory::Appliance)).unwrap();
        insert_asset(&conn, &draft("Honda Civic", AssetCategory::Vehicle)).unwrap();
        insert_asset(&conn, &draft("Dishwasher", AssetCategory::Appliance)).unwrap();

        let all = list_assets(&conn, &AssetListFilter::default()).unwrap();
        assert_eq!(all.len(), 3);

        let appliances = list_assets(&conn, &AssetListFilter {
            category: Some(AssetCategory::Appliance),
            ..Default::default()
        }).unwrap();
        assert_eq!(appliances.len(), 2);

        let search = list_assets(&conn, &AssetListFilter {
            search: Some("boil".into()),
            ..Default::default()
        }).unwrap();
        assert_eq!(search.len(), 1);
        assert_eq!(search[0].name, "Boiler");
    }

    #[test]
    fn list_orders_alphabetical_case_insensitive() {
        let (_d, conn) = fresh();
        insert_asset(&conn, &draft("zebra", AssetCategory::Other)).unwrap();
        insert_asset(&conn, &draft("Apple", AssetCategory::Other)).unwrap();
        insert_asset(&conn, &draft("banana", AssetCategory::Other)).unwrap();
        let list = list_assets(&conn, &AssetListFilter::default()).unwrap();
        let names: Vec<_> = list.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["Apple", "banana", "zebra"]);
    }

    #[test]
    fn restore_clears_deleted_at() {
        let (_d, conn) = fresh();
        let id = insert_asset(&conn, &draft("X", AssetCategory::Other)).unwrap();
        soft_delete_asset(&conn, &id).unwrap();
        restore_asset(&conn, &id).unwrap();
        assert!(get_asset(&conn, &id).unwrap().is_some());
    }

    #[test]
    fn set_hero_attachment_updates_field() {
        let (_d, conn) = fresh();
        let id = insert_asset(&conn, &draft("X", AssetCategory::Other)).unwrap();
        set_hero_attachment(&conn, &id, Some("uuid-123")).unwrap();
        assert_eq!(get_asset(&conn, &id).unwrap().unwrap().hero_attachment_uuid.as_deref(), Some("uuid-123"));
        set_hero_attachment(&conn, &id, None).unwrap();
        assert!(get_asset(&conn, &id).unwrap().unwrap().hero_attachment_uuid.is_none());
    }
}
```

- [ ] **Step 3: Register module in `crates/core/src/lib.rs`**

Insert `pub mod asset;` alphabetically (likely right after `pub mod assistant;` or wherever the alphabetical order fits).

- [ ] **Step 4: Run tests**

```bash
cargo test -p manor-core --lib asset::dal
cargo test --workspace --lib
```
Expected: 7 new tests pass; workspace total 321 + 7 = 328.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/asset/ crates/core/src/lib.rs
git commit -m "feat(asset): core types + DAL (CRUD with trash, search, category filter)"
```

---

## Task 3: `attachment::list_for_text_entity` helper

**Files:**
- Modify: `crates/core/src/attachment.rs`

- [ ] **Step 1: Add the helper + failing test**

Inspect the existing `attachment.rs` to find where `list_for` is defined (around line 119 per prior L3a work). Directly below it, add:

```rust
/// Like `list_for` but accepts a TEXT entity_id. Used for UUID-keyed entities
/// (recipes, assets) that store text in the `entity_id` column via SQLite's
/// dynamic typing.
pub fn list_for_text_entity(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
) -> Result<Vec<Attachment>> {
    let mut stmt = conn.prepare(
        "SELECT id, uuid, original_name, mime_type, size_bytes, sha256,
                entity_type, entity_id, created_at
         FROM attachment
         WHERE entity_type = ?1 AND entity_id = ?2 AND deleted_at IS NULL
         ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map(params![entity_type, entity_id], Attachment::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}
```

Append a test to the existing `tests` module:

```rust
    #[test]
    fn list_for_text_entity_finds_by_uuid_entity_id() {
        let (_dir, conn, root) = fresh_env();
        // Stage an attachment, then link it to a text-keyed entity ("asset", "asset-uuid").
        let att_id = store(
            &conn, &root,
            b"fake pdf bytes",
            "manual.pdf",
            "application/pdf",
            Some("asset"),
            None,
        ).unwrap();
        link_to_entity(&conn, att_id, "asset", "asset-uuid-123").unwrap();

        let list = list_for_text_entity(&conn, "asset", "asset-uuid-123").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].original_name, "manual.pdf");
    }
```

`store`, `link_to_entity`, and `fresh_env` are the canonical helpers shipped in prior L3a work — use whatever signatures match the file. If `store` takes slightly different args (e.g. a different trailing size param), adapt.

- [ ] **Step 2: Run tests**

```bash
cargo test -p manor-core --lib attachment
cargo test --workspace --lib
```
Expected: +1 test passes; no regressions.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/attachment.rs
git commit -m "feat(attachment): list_for_text_entity helper for UUID-keyed entities"
```

---

## Task 4: Tauri CRUD commands

**Files:**
- Create: `crates/app/src/asset/mod.rs`
- Create: `crates/app/src/asset/commands.rs`
- Modify: `crates/app/src/lib.rs`

- [ ] **Step 1: Module root**

`crates/app/src/asset/mod.rs`:

```rust
//! Asset registry — Tauri command layer + attachment staging.

pub mod commands;
pub mod importer;  // filled in Task 5
```

Create an empty placeholder `importer.rs`:

```rust
//! Hero + document staging — filled in Task 5.
```

- [ ] **Step 2: CRUD commands**

`crates/app/src/asset/commands.rs`:

```rust
use crate::assistant::commands::Db;
use manor_core::asset::{dal::{self, AssetListFilter}, Asset, AssetCategory, AssetDraft};
use serde::Deserialize;
use tauri::State;

#[derive(Deserialize)]
pub struct AssetListArgs {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub category: Option<AssetCategory>,
}

#[tauri::command]
pub fn asset_list(args: AssetListArgs, state: State<'_, Db>) -> Result<Vec<Asset>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let filter = AssetListFilter {
        search: args.search,
        category: args.category,
        include_trashed: false,
    };
    dal::list_assets(&conn, &filter).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asset_get(id: String, state: State<'_, Db>) -> Result<Option<Asset>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::get_asset(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asset_create(draft: AssetDraft, state: State<'_, Db>) -> Result<String, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::insert_asset(&conn, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asset_update(id: String, draft: AssetDraft, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::update_asset(&conn, &id, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asset_delete(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::soft_delete_asset(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asset_restore(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::restore_asset(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asset_list_documents(
    id: String,
    state: State<'_, Db>,
) -> Result<Vec<manor_core::attachment::Attachment>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    manor_core::attachment::list_for_text_entity(&conn, "asset", &id).map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Register in `crates/app/src/lib.rs`**

Add `pub mod asset;` near other module declarations. Append to `invoke_handler!`:

```rust
            asset::commands::asset_list,
            asset::commands::asset_get,
            asset::commands::asset_create,
            asset::commands::asset_update,
            asset::commands::asset_delete,
            asset::commands::asset_restore,
            asset::commands::asset_list_documents,
```

(attach commands land in Task 5.)

- [ ] **Step 4: Build + clippy**

```bash
cargo build -p manor-app
cargo clippy --workspace -- -D warnings
```
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/asset/ crates/app/src/lib.rs
git commit -m "feat(asset): Tauri CRUD + list_documents commands"
```

---

## Task 5: Hero + document staging + orphan sweep

**Files:**
- Modify: `crates/app/src/asset/importer.rs`
- Modify: `crates/app/src/asset/commands.rs` — add attach commands.
- Modify: `crates/app/src/lib.rs` — register attach commands + call startup sweep.

- [ ] **Step 1: importer.rs — staging helpers + orphan sweep**

Overwrite `crates/app/src/asset/importer.rs`:

```rust
//! Asset attachment staging: copy local file into attachments dir, create attachment row,
//! link to the asset (by UUID text entity_id). Orphan sweep at startup reaps crash-leftover
//! hero stagings (entity_id IS NULL, entity_type='asset', >24h old).

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

const ORPHAN_AGE_SECS: i64 = 24 * 60 * 60;

/// Copy a file at `source_path` into the attachments root, create an attachment row with
/// entity_type='asset' and entity_id linked to the asset (text UUID), and return the new
/// attachment uuid. For hero: caller follows up with `asset::dal::set_hero_attachment`.
pub fn attach_file(
    conn: &Connection,
    attachments_dir: &Path,
    source_path: &Path,
    asset_id: &str,
) -> Result<String> {
    let bytes = std::fs::read(source_path).with_context(|| format!("reading {}", source_path.display()))?;
    let original_name = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    let mime = guess_mime(&original_name);

    let att_id = manor_core::attachment::store(
        conn,
        attachments_dir,
        &bytes,
        &original_name,
        &mime,
        Some("asset"),
        None,  // staged; linked below
    )?;
    let uuid = manor_core::attachment::get_uuid(conn, att_id)?;
    manor_core::attachment::link_to_entity(conn, att_id, "asset", asset_id)?;
    Ok(uuid)
}

fn guess_mime(filename: &str) -> String {
    let lower = filename.to_lowercase();
    if lower.ends_with(".pdf") { "application/pdf".into() }
    else if lower.ends_with(".png") { "image/png".into() }
    else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") { "image/jpeg".into() }
    else if lower.ends_with(".webp") { "image/webp".into() }
    else if lower.ends_with(".heic") { "image/heic".into() }
    else { "application/octet-stream".into() }
}

/// Sweep crash-orphaned hero stagings (entity_type='asset', entity_id IS NULL, age >24h).
/// Recipes have their own sweep; this is the asset equivalent.
pub fn stage_sweep_run_on_startup(conn: &Connection, attachments_dir: &PathBuf) -> Result<usize> {
    let cutoff_ms = chrono::Utc::now().timestamp_millis() - ORPHAN_AGE_SECS * 1000;

    let mut stmt = conn.prepare(
        "SELECT id, uuid, filename FROM attachment
         WHERE entity_type = 'asset'
           AND entity_id IS NULL
           AND created_at < ?1
           AND uuid NOT IN (SELECT hero_attachment_uuid FROM asset WHERE hero_attachment_uuid IS NOT NULL)",
    )?;
    let rows: Vec<(i64, String, String)> = stmt
        .query_map(params![cutoff_ms], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .filter_map(Result::ok)
        .collect();

    let mut swept = 0usize;
    for (id, _uuid, filename) in rows {
        let path = attachments_dir.join(&filename);
        let _ = std::fs::remove_file(&path);
        conn.execute("DELETE FROM attachment WHERE id = ?1", params![id])?;
        swept += 1;
    }
    Ok(swept)
}
```

**Implementer note:** `manor_core::attachment::get_uuid(conn, id)` may or may not exist. Inspect `crates/core/src/attachment.rs` — if not present, add a thin `pub fn get_uuid(conn: &Connection, id: i64) -> Result<String>` helper (single SQL query). If the `store` function already returns the uuid OR you can query the `attachment` row via `id`, adapt the code. Match existing patterns — don't invent new API surface if a close equivalent exists.

- [ ] **Step 2: Add attach-hero + attach-document Tauri commands**

Append to `crates/app/src/asset/commands.rs`:

```rust
use tauri::AppHandle;
use std::path::PathBuf;

fn resolve_attachments_dir(app: &AppHandle) -> Result<PathBuf, String> {
    // Mirror how L3a resolves attachments_dir; inspect crates/app/src/recipe/commands.rs
    // for the exact call. Typical pattern: app.path().app_data_dir().join("attachments").
    crate::paths::attachments_dir(app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn asset_attach_hero_from_path(
    id: String,
    source_path: String,
    state: State<'_, Db>,
    app: AppHandle,
) -> Result<String, String> {
    let dir = resolve_attachments_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let src = std::path::PathBuf::from(source_path);
    let conn = state.0.lock().map_err(|e| e.to_string())?;

    let uuid = crate::asset::importer::attach_file(&conn, &dir, &src, &id).map_err(|e| e.to_string())?;
    manor_core::asset::dal::set_hero_attachment(&conn, &id, Some(&uuid)).map_err(|e| e.to_string())?;
    Ok(uuid)
}

#[tauri::command]
pub async fn asset_attach_document_from_path(
    id: String,
    source_path: String,
    state: State<'_, Db>,
    app: AppHandle,
) -> Result<String, String> {
    let dir = resolve_attachments_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let src = std::path::PathBuf::from(source_path);
    let conn = state.0.lock().map_err(|e| e.to_string())?;

    crate::asset::importer::attach_file(&conn, &dir, &src, &id).map_err(|e| e.to_string())
}
```

**Implementer note:** check how `crate::paths::attachments_dir(...)` is called in L3a's `recipe::commands::recipe_import_commit` — mirror the exact signature. If the function takes no app handle argument, omit the `AppHandle` from the helpers here.

- [ ] **Step 3: Register attach commands + startup sweep in `crates/app/src/lib.rs`**

Append to invoke_handler:

```rust
            asset::commands::asset_attach_hero_from_path,
            asset::commands::asset_attach_document_from_path,
```

Locate the recipe stage-sweep call (around line 164 per earlier probe — `crate::recipe::stage_sweep::run_on_startup(&conn, &attachments_dir)`). Immediately after it, add:

```rust
                    match crate::asset::importer::stage_sweep_run_on_startup(&conn, &attachments_dir) {
                        Ok(n) if n > 0 => tracing::info!("asset stage_sweep: reaped {n} orphans"),
                        Ok(_) => {}
                        Err(e) => tracing::warn!("asset stage_sweep failed: {e}"),
                    }
```

- [ ] **Step 4: Build + clippy + tests**

```bash
cargo build -p manor-app
cargo clippy --workspace -- -D warnings
cargo test --workspace --lib
```
Expected: clean; no regressions.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/asset/importer.rs crates/app/src/asset/commands.rs crates/app/src/lib.rs
git commit -m "feat(asset): hero + document attachment staging + startup orphan sweep"
```

---

## Task 6: Frontend IPC + Zustand store

**Files:**
- Create: `apps/desktop/src/lib/asset/ipc.ts`
- Create: `apps/desktop/src/lib/asset/state.ts`

- [ ] **Step 1: IPC wrappers**

Create `apps/desktop/src/lib/asset/ipc.ts`:

```ts
import { invoke, convertFileSrc } from "@tauri-apps/api/core";

export type AssetCategory = "appliance" | "vehicle" | "fixture" | "other";

export interface Asset {
  id: string;
  name: string;
  category: AssetCategory;
  make: string | null;
  model: string | null;
  serial_number: string | null;
  purchase_date: string | null;
  notes: string;
  hero_attachment_uuid: string | null;
  created_at: number;
  updated_at: number;
  deleted_at: number | null;
}

export interface AssetDraft {
  name: string;
  category: AssetCategory;
  make: string | null;
  model: string | null;
  serial_number: string | null;
  purchase_date: string | null;
  notes: string;
  hero_attachment_uuid: string | null;
}

export interface AttachmentSummary {
  id: number;
  uuid: string;
  original_name: string;
  mime_type: string;
  size_bytes: number;
  sha256: string;
  entity_type: string | null;
  entity_id: string | null;
  created_at: number;
}

export async function list(search?: string, category?: AssetCategory | null): Promise<Asset[]> {
  return await invoke<Asset[]>("asset_list", { args: { search, category } });
}

export async function get(id: string): Promise<Asset | null> {
  return await invoke<Asset | null>("asset_get", { id });
}

export async function create(draft: AssetDraft): Promise<string> {
  return await invoke<string>("asset_create", { draft });
}

export async function update(id: string, draft: AssetDraft): Promise<void> {
  await invoke("asset_update", { id, draft });
}

export async function deleteAsset(id: string): Promise<void> {
  await invoke("asset_delete", { id });
}

export async function restore(id: string): Promise<void> {
  await invoke("asset_restore", { id });
}

export async function attachHeroFromPath(id: string, sourcePath: string): Promise<string> {
  return await invoke<string>("asset_attach_hero_from_path", { id, sourcePath });
}

export async function attachDocumentFromPath(id: string, sourcePath: string): Promise<string> {
  return await invoke<string>("asset_attach_document_from_path", { id, sourcePath });
}

export async function listDocuments(id: string): Promise<AttachmentSummary[]> {
  return await invoke<AttachmentSummary[]>("asset_list_documents", { id });
}

/**
 * Resolve an attachment uuid to a webview-safe URL for rendering. Reuses the
 * attachment_get_path_by_uuid command shipped in L3a.
 */
export async function attachmentSrc(uuid: string): Promise<string> {
  const absPath = await invoke<string>("attachment_get_path_by_uuid", { uuid });
  return convertFileSrc(absPath);
}
```

- [ ] **Step 2: Zustand store**

Create `apps/desktop/src/lib/asset/state.ts`:

```ts
import { create } from "zustand";
import * as ipc from "./ipc";

type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface AssetStore {
  assets: ipc.Asset[];
  search: string;
  category: ipc.AssetCategory | null;
  loadStatus: LoadStatus;

  load(): Promise<void>;
  setSearch(s: string): void;
  setCategory(c: ipc.AssetCategory | null): void;
}

export const useAssetStore = create<AssetStore>((set, get) => ({
  assets: [],
  search: "",
  category: null,
  loadStatus: { kind: "idle" },

  async load() {
    set({ loadStatus: { kind: "loading" } });
    try {
      const assets = await ipc.list(get().search || undefined, get().category);
      set({ assets, loadStatus: { kind: "idle" } });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
    }
  },

  setSearch(s) { set({ search: s }); void get().load(); },
  setCategory(c) { set({ category: c }); void get().load(); },
}));
```

Note: `setSearch` here calls `load()` immediately; the view component wraps this in a 200ms useEffect-debounce on the input to avoid per-keystroke IPC.

- [ ] **Step 3: Typecheck**

```bash
cd apps/desktop && pnpm tsc --noEmit
```

- [ ] **Step 4: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4a-asset-registry
git add apps/desktop/src/lib/asset/
git commit -m "feat(asset): frontend IPC + Zustand store"
```

---

## Task 7: Bones nav entry + BonesTab scaffold

**Files:**
- Modify: `apps/desktop/src/lib/nav.ts` — add `"bones"` to View union.
- Modify: `apps/desktop/src/components/Nav/Sidebar.tsx` — add Bones entry with Wrench icon.
- Modify: `apps/desktop/src/App.tsx` — route `view === "bones"` to `<BonesTab />`.
- Create: `apps/desktop/src/components/Bones/BonesTab.tsx` — empty-state scaffold.

- [ ] **Step 1: Extend View union**

In `apps/desktop/src/lib/nav.ts`, find the existing `View` type:

```ts
export type View = "today" | "chores" | "ledger" | "hearth" | "assistant" | "settings";
```

Add `"bones"`:

```ts
export type View = "today" | "chores" | "ledger" | "bones" | "hearth" | "assistant" | "settings";
```

Match the actual order in the file.

- [ ] **Step 2: Add Bones to Sidebar**

Modify `apps/desktop/src/components/Nav/Sidebar.tsx`. Find the NavIcon rendering for existing tabs. Add `UtensilsCrossed` isn't the Bones one — inspect the file to see which icon each tab uses, then add a `Wrench` import from `lucide-react` and insert a new `<NavIcon view="bones" icon={Wrench} title="Bones" />` between Ledger and Hearth (or wherever matches the `View` union order you extended in step 1).

- [ ] **Step 3: Route to BonesTab**

Modify `apps/desktop/src/App.tsx`. Find the routing block (e.g. `{view === "hearth" && <HearthTab />}`). Add import + branch:

```tsx
import { BonesTab } from "./components/Bones/BonesTab";
// ...
{view === "bones" && <BonesTab />}
```

- [ ] **Step 4: Create scaffold BonesTab**

Create `apps/desktop/src/components/Bones/BonesTab.tsx`:

```tsx
import { useEffect } from "react";
import { useAssetStore } from "../../lib/asset/state";

export function BonesTab() {
  const { assets, loadStatus, load } = useAssetStore();
  useEffect(() => { void load(); }, [load]);

  return (
    <div style={{ padding: 32, maxWidth: 1200, margin: "0 auto" }}>
      <h1 style={{ fontSize: 24, fontWeight: 600, margin: 0 }}>Assets</h1>
      {loadStatus.kind === "loading" && <p style={{ color: "var(--ink-soft, #999)" }}>Loading…</p>}
      {loadStatus.kind === "error" && <p style={{ color: "var(--ink-danger, #b00020)" }}>{loadStatus.message}</p>}
      {loadStatus.kind === "idle" && assets.length === 0 && (
        <p style={{ color: "var(--ink-soft, #999)" }}>Your asset registry is empty.</p>
      )}
      {loadStatus.kind === "idle" && assets.length > 0 && (
        <ul>{assets.map(a => <li key={a.id}>{a.name}</li>)}</ul>
      )}
    </div>
  );
}
```

- [ ] **Step 5: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 6: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4a-asset-registry
git add apps/desktop/src/lib/nav.ts apps/desktop/src/components/Nav/Sidebar.tsx apps/desktop/src/App.tsx apps/desktop/src/components/Bones/
git commit -m "feat(asset): Bones nav entry + empty-state scaffold"
```

---

## Task 8: Full list view (AssetCard + filter bar + search)

**Files:**
- Create: `apps/desktop/src/components/Bones/AssetCard.tsx`
- Overwrite: `apps/desktop/src/components/Bones/BonesTab.tsx` — replace scaffold.

- [ ] **Step 1: AssetCard component**

Create `apps/desktop/src/components/Bones/AssetCard.tsx`:

```tsx
import { useEffect, useState } from "react";
import { ImageOff, Wrench, Car, Home, Box } from "lucide-react";
import * as ipc from "../../lib/asset/ipc";
import type { Asset, AssetCategory } from "../../lib/asset/ipc";

const CATEGORY_ICONS: Record<AssetCategory, typeof Wrench> = {
  appliance: Wrench,
  vehicle: Car,
  fixture: Home,
  other: Box,
};

interface Props {
  asset: Asset;
  onClick: () => void;
}

export function AssetCard({ asset, onClick }: Props) {
  const [heroSrc, setHeroSrc] = useState<string | null>(null);

  useEffect(() => {
    const uuid = asset.hero_attachment_uuid;
    setHeroSrc(null);
    if (uuid) { void ipc.attachmentSrc(uuid).then(setHeroSrc).catch(() => {}); }
  }, [asset.hero_attachment_uuid]);

  const Icon = CATEGORY_ICONS[asset.category];
  const year = asset.purchase_date ? new Date(asset.purchase_date + "T00:00:00").getFullYear() : null;

  return (
    <button
      onClick={onClick}
      style={{
        textAlign: "left",
        background: "var(--paper, #fff)",
        border: "1px solid var(--hairline, #e5e5e5)",
        borderRadius: 6,
        padding: 0,
        cursor: "pointer",
        overflow: "hidden",
        display: "flex",
        flexDirection: "column",
      }}
    >
      <div style={{
        aspectRatio: "4 / 3",
        background: "var(--paper-muted, #f5f5f5)",
        display: "flex", alignItems: "center", justifyContent: "center",
      }}>
        {heroSrc ? (
          <img src={heroSrc} alt={asset.name}
            style={{ width: "100%", height: "100%", objectFit: "cover" }} />
        ) : (
          <ImageOff size={32} strokeWidth={1.4} color="var(--ink-soft, #999)" />
        )}
      </div>
      <div style={{ padding: 12 }}>
        <div style={{
          fontSize: 16, fontWeight: 600,
          whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis",
        }}>
          {asset.name}
        </div>
        {asset.make && (
          <div style={{ fontSize: 12, color: "var(--ink-soft, #999)", marginTop: 2,
                        whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
            {asset.make}
          </div>
        )}
        <div style={{ display: "flex", alignItems: "center", gap: 4,
                      fontSize: 12, color: "var(--ink-soft, #999)", marginTop: 4 }}>
          <Icon size={12} strokeWidth={1.8} />
          {year != null && <span>{year}</span>}
        </div>
      </div>
    </button>
  );
}
```

- [ ] **Step 2: Full BonesTab**

Overwrite `apps/desktop/src/components/Bones/BonesTab.tsx`:

```tsx
import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { useAssetStore } from "../../lib/asset/state";
import type { AssetCategory } from "../../lib/asset/ipc";
import { AssetCard } from "./AssetCard";
import { AssetEditDrawer } from "./AssetEditDrawer";        // Task 9
import { AssetDetail } from "./AssetDetail";                // Task 10

type View = { mode: "list" } | { mode: "detail"; id: string };

const CATEGORIES: { key: AssetCategory; label: string }[] = [
  { key: "appliance", label: "Appliance" },
  { key: "vehicle",   label: "Vehicle" },
  { key: "fixture",   label: "Fixture" },
  { key: "other",     label: "Other" },
];

export function BonesTab() {
  const { assets, search, setSearch, category, setCategory, loadStatus, load } = useAssetStore();
  const [view, setView] = useState<View>({ mode: "list" });
  const [showNew, setShowNew] = useState(false);
  const [searchInput, setSearchInput] = useState(search);

  useEffect(() => { void load(); }, [load]);

  // 200ms debounce on search input
  useEffect(() => {
    const h = setTimeout(() => {
      if (searchInput !== search) setSearch(searchInput);
    }, 200);
    return () => clearTimeout(h);
  }, [searchInput, search, setSearch]);

  if (view.mode === "detail") {
    return (
      <AssetDetail
        id={view.id}
        onBack={() => { setView({ mode: "list" }); void load(); }}
      />
    );
  }

  return (
    <div style={{ padding: 32, maxWidth: 1200, margin: "0 auto" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
        <h1 style={{ fontSize: 24, fontWeight: 600, margin: 0 }}>Assets</h1>
        <button onClick={() => setShowNew(true)}
          style={{ display: "flex", alignItems: "center", gap: 4 }}>
          <Plus size={14} strokeWidth={1.8} /> New
        </button>
      </div>

      <div style={{ display: "flex", gap: 8, marginBottom: 16 }}>
        <input
          placeholder="Search assets"
          value={searchInput}
          onChange={(e) => setSearchInput(e.target.value)}
          style={{ flex: 1, padding: 8, fontSize: 14 }}
        />
        <select
          value={category ?? ""}
          onChange={(e) => setCategory(e.target.value ? (e.target.value as AssetCategory) : null)}
          style={{ padding: 8, fontSize: 14 }}
        >
          <option value="">All categories</option>
          {CATEGORIES.map((c) => <option key={c.key} value={c.key}>{c.label}</option>)}
        </select>
      </div>

      {loadStatus.kind === "loading" && <p style={{ color: "var(--ink-soft, #999)" }}>Loading…</p>}
      {loadStatus.kind === "error" && (
        <p style={{ color: "var(--ink-danger, #b00020)" }}>
          {loadStatus.message} — <button onClick={() => void load()}>Retry</button>
        </p>
      )}

      {loadStatus.kind === "idle" && assets.length === 0 && (
        <div style={{ padding: 48, textAlign: "center" }}>
          <p style={{ color: "var(--ink-soft, #999)", marginBottom: 16 }}>
            Your asset registry is empty.
          </p>
          <button onClick={() => setShowNew(true)}
            style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
            <Plus size={14} strokeWidth={1.8} /> New asset
          </button>
        </div>
      )}

      {loadStatus.kind === "idle" && assets.length > 0 && (
        <div style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))",
          gap: 16,
        }}>
          {assets.map((a) => (
            <AssetCard key={a.id} asset={a}
              onClick={() => setView({ mode: "detail", id: a.id })} />
          ))}
        </div>
      )}

      {showNew && (
        <AssetEditDrawer
          onClose={() => { setShowNew(false); void load(); }}
        />
      )}
    </div>
  );
}
```

(`AssetEditDrawer` and `AssetDetail` arrive in Tasks 9 and 10. Create stubs now so typecheck passes:)

Create `apps/desktop/src/components/Bones/AssetEditDrawer.tsx`:

```tsx
interface Props { recipeId?: string; onClose: () => void }
export function AssetEditDrawer({ onClose }: Props) {
  return <div onClick={onClose}>Stub — Task 9</div>;
}
```

Create `apps/desktop/src/components/Bones/AssetDetail.tsx`:

```tsx
interface Props { id: string; onBack: () => void }
export function AssetDetail({ onBack }: Props) {
  return <div><button onClick={onBack}>Back</button>Stub — Task 10</div>;
}
```

- [ ] **Step 3: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 4: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4a-asset-registry
git add apps/desktop/src/components/Bones/
git commit -m "feat(asset): full list view with search + category filter (drawer + detail stubs)"
```

---

## Task 9: AssetEditDrawer (create/edit + hero upload)

**Files:**
- Overwrite: `apps/desktop/src/components/Bones/AssetEditDrawer.tsx` — full drawer.

- [ ] **Step 1: Inspect Tauri dialog plugin**

Run:
```bash
grep -rn "from \"@tauri-apps/plugin-dialog\"" apps/desktop/src/ 2>&1 | head -5
grep -n "tauri-plugin-dialog" apps/desktop/package.json
```

The open-file-picker pattern in Tauri v2 is typically:

```ts
import { open } from "@tauri-apps/plugin-dialog";
const path = await open({ multiple: false, directory: false });
```

If the plugin is already imported somewhere in Manor (likely for prior CSV import or similar flows), mirror that pattern exactly. If not installed, add it to `apps/desktop/package.json` and run `pnpm install`.

- [ ] **Step 2: AssetEditDrawer.tsx**

Overwrite the stub with:

```tsx
import { useEffect, useState } from "react";
import { ImageOff } from "lucide-react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import * as ipc from "../../lib/asset/ipc";
import type { Asset, AssetCategory, AssetDraft } from "../../lib/asset/ipc";

interface Props {
  assetId?: string;                       // undefined = create mode
  onClose: () => void;
  onSaved?: (id: string) => void;
}

const EMPTY_DRAFT: AssetDraft = {
  name: "",
  category: "appliance",
  make: null,
  model: null,
  serial_number: null,
  purchase_date: null,
  notes: "",
  hero_attachment_uuid: null,
};

const CATEGORIES: { key: AssetCategory; label: string }[] = [
  { key: "appliance", label: "Appliance" },
  { key: "vehicle",   label: "Vehicle" },
  { key: "fixture",   label: "Fixture" },
  { key: "other",     label: "Other" },
];

export function AssetEditDrawer({ assetId, onClose, onSaved }: Props) {
  const [draft, setDraft] = useState<AssetDraft>(EMPTY_DRAFT);
  const [heroSrc, setHeroSrc] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (assetId) {
      void ipc.get(assetId).then((a: Asset | null) => {
        if (a) setDraft({
          name: a.name, category: a.category,
          make: a.make, model: a.model, serial_number: a.serial_number,
          purchase_date: a.purchase_date, notes: a.notes,
          hero_attachment_uuid: a.hero_attachment_uuid,
        });
      });
    }
  }, [assetId]);

  useEffect(() => {
    const uuid = draft.hero_attachment_uuid;
    setHeroSrc(null);
    if (uuid) { void ipc.attachmentSrc(uuid).then(setHeroSrc).catch(() => {}); }
  }, [draft.hero_attachment_uuid]);

  const pickHero = async () => {
    if (!assetId) {
      setError("Save the asset once before adding a hero image");
      return;
    }
    const picked = await openFileDialog({ multiple: false, directory: false });
    const path = typeof picked === "string" ? picked : null;
    if (!path) return;
    try {
      const uuid = await ipc.attachHeroFromPath(assetId, path);
      setDraft({ ...draft, hero_attachment_uuid: uuid });
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const removeHero = () => {
    setDraft({ ...draft, hero_attachment_uuid: null });
  };

  const save = async () => {
    if (!draft.name.trim()) { setError("Name required"); return; }
    setSaving(true); setError(null);
    try {
      const id = assetId
        ? (await ipc.update(assetId, draft), assetId)
        : await ipc.create(draft);
      onSaved?.(id);
      onClose();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div style={{
      position: "fixed", top: 0, right: 0, bottom: 0, width: 480,
      background: "var(--paper, #fff)", borderLeft: "1px solid var(--hairline, #e5e5e5)",
      padding: 24, overflow: "auto", zIndex: 50,
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 16 }}>
        <h2 style={{ margin: 0, fontSize: 20 }}>{assetId ? "Edit asset" : "New asset"}</h2>
        <button type="button" onClick={onClose} aria-label="Close">✕</button>
      </div>

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Name</label>
      <input value={draft.name}
        onChange={(e) => setDraft({ ...draft, name: e.target.value })}
        style={{ width: "100%", marginBottom: 12, padding: 6 }} />

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Category</label>
      <select value={draft.category}
        onChange={(e) => setDraft({ ...draft, category: e.target.value as AssetCategory })}
        style={{ width: "100%", marginBottom: 12, padding: 6 }}>
        {CATEGORIES.map((c) => <option key={c.key} value={c.key}>{c.label}</option>)}
      </select>

      <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
        <div style={{ flex: 1 }}>
          <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Make</label>
          <input value={draft.make ?? ""}
            onChange={(e) => setDraft({ ...draft, make: e.target.value || null })}
            style={{ width: "100%", padding: 6 }} />
        </div>
        <div style={{ flex: 1 }}>
          <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Model</label>
          <input value={draft.model ?? ""}
            onChange={(e) => setDraft({ ...draft, model: e.target.value || null })}
            style={{ width: "100%", padding: 6 }} />
        </div>
      </div>

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Serial number</label>
      <input value={draft.serial_number ?? ""}
        onChange={(e) => setDraft({ ...draft, serial_number: e.target.value || null })}
        style={{ width: "100%", marginBottom: 12, padding: 6 }} />

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Purchase date</label>
      <input type="date" value={draft.purchase_date ?? ""}
        onChange={(e) => setDraft({ ...draft, purchase_date: e.target.value || null })}
        style={{ width: "100%", marginBottom: 12, padding: 6 }} />

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Hero image</label>
      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 12 }}>
        <div style={{ width: 80, height: 60, background: "var(--paper-muted, #f5f5f5)",
                      display: "flex", alignItems: "center", justifyContent: "center",
                      borderRadius: 4, overflow: "hidden" }}>
          {heroSrc ? (
            <img src={heroSrc} alt="" style={{ width: "100%", height: "100%", objectFit: "cover" }} />
          ) : (
            <ImageOff size={20} strokeWidth={1.4} color="var(--ink-soft, #999)" />
          )}
        </div>
        <button type="button" onClick={pickHero} disabled={!assetId}>
          {draft.hero_attachment_uuid ? "Replace" : "Choose…"}
        </button>
        {draft.hero_attachment_uuid && (
          <button type="button" onClick={removeHero}>Remove</button>
        )}
      </div>
      {!assetId && (
        <div style={{ fontSize: 12, color: "var(--ink-soft, #999)", marginBottom: 12 }}>
          Save first, then add a hero image.
        </div>
      )}

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Notes (markdown)</label>
      <textarea value={draft.notes}
        onChange={(e) => setDraft({ ...draft, notes: e.target.value })}
        rows={6} style={{ width: "100%", fontFamily: "inherit", padding: 6 }} />

      {error && <div style={{ color: "var(--ink-danger, #b00020)", marginTop: 8 }}>{error}</div>}

      <div style={{ display: "flex", gap: 8, marginTop: 16 }}>
        <button type="button" onClick={onClose}>Cancel</button>
        <button type="button" onClick={save} disabled={saving}>
          {saving ? "Saving…" : "Save"}
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 4: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4a-asset-registry
git add apps/desktop/src/components/Bones/AssetEditDrawer.tsx
git commit -m "feat(asset): New/Edit drawer with hero upload via file picker"
```

---

## Task 10: AssetDetail + DocumentList + delete flow

**Files:**
- Create: `apps/desktop/src/components/Bones/DocumentList.tsx`
- Overwrite: `apps/desktop/src/components/Bones/AssetDetail.tsx` — full detail.

- [ ] **Step 1: DocumentList component**

Create `apps/desktop/src/components/Bones/DocumentList.tsx`:

```tsx
import { useEffect, useState, useCallback } from "react";
import { FileText, Image as ImageIcon, File, Plus } from "lucide-react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import * as ipc from "../../lib/asset/ipc";
import type { AttachmentSummary } from "../../lib/asset/ipc";

interface Props {
  assetId: string;
}

function iconFor(mime: string) {
  if (mime.includes("pdf")) return FileText;
  if (mime.startsWith("image/")) return ImageIcon;
  return File;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatDate(ts: number): string {
  return new Date(ts * 1000).toLocaleDateString(undefined, {
    month: "short", day: "numeric", year: "numeric",
  });
}

export function DocumentList({ assetId }: Props) {
  const [docs, setDocs] = useState<AttachmentSummary[]>([]);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    void ipc.listDocuments(assetId).then(setDocs).catch((e: unknown) => {
      setError(e instanceof Error ? e.message : String(e));
    });
  }, [assetId]);

  useEffect(() => { reload(); }, [reload]);

  const addDoc = async () => {
    const picked = await openFileDialog({ multiple: false, directory: false });
    const path = typeof picked === "string" ? picked : null;
    if (!path) return;
    try {
      await ipc.attachDocumentFromPath(assetId, path);
      reload();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const openDoc = async (uuid: string) => {
    try {
      const absPath = await invoke<string>("attachment_get_path_by_uuid", { uuid });
      // Open via system default app — Tauri v2 uses the opener plugin.
      // Fall back to a simple <a href> / window.open with a converted file URL if opener isn't wired.
      const url = (await import("@tauri-apps/api/core")).convertFileSrc(absPath);
      window.open(url, "_blank");
    } catch (e: unknown) {
      setError(`Couldn't open — ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  return (
    <div>
      {error && <div style={{ color: "var(--ink-danger, #b00020)", marginBottom: 8 }}>{error}</div>}
      {docs.map((d) => {
        const Icon = iconFor(d.mime_type);
        return (
          <div key={d.id}
               onClick={() => void openDoc(d.uuid)}
               style={{
                 display: "flex", alignItems: "center", gap: 12,
                 padding: "8px 12px",
                 borderBottom: "1px solid var(--hairline, #e5e5e5)",
                 cursor: "pointer",
               }}>
            <Icon size={16} strokeWidth={1.8} color="var(--ink-soft, #999)" />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontSize: 14, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                {d.original_name}
              </div>
              <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>
                {formatSize(d.size_bytes)} · {formatDate(d.created_at)}
              </div>
            </div>
          </div>
        );
      })}
      <button type="button" onClick={addDoc}
        style={{ marginTop: 8, display: "flex", alignItems: "center", gap: 4 }}>
        <Plus size={14} strokeWidth={1.8} /> Add document
      </button>
    </div>
  );
}
```

- [ ] **Step 2: AssetDetail component**

Overwrite `apps/desktop/src/components/Bones/AssetDetail.tsx`:

```tsx
import { useCallback, useEffect, useState } from "react";
import { ArrowLeft, Pencil, Trash2, ImageOff } from "lucide-react";
import * as ipc from "../../lib/asset/ipc";
import type { Asset } from "../../lib/asset/ipc";
import { AssetEditDrawer } from "./AssetEditDrawer";
import { DocumentList } from "./DocumentList";

interface Props { id: string; onBack: () => void }

const CATEGORY_LABEL: Record<string, string> = {
  appliance: "Appliance",
  vehicle: "Vehicle",
  fixture: "Fixture",
  other: "Other",
};

export function AssetDetail({ id, onBack }: Props) {
  const [asset, setAsset] = useState<Asset | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [editing, setEditing] = useState(false);
  const [heroSrc, setHeroSrc] = useState<string | null>(null);

  const reload = useCallback(() => {
    setLoaded(false);
    void ipc.get(id).then((a) => { setAsset(a); setLoaded(true); });
  }, [id]);

  useEffect(() => { reload(); }, [reload, editing]);

  useEffect(() => {
    const uuid = asset?.hero_attachment_uuid;
    setHeroSrc(null);
    if (uuid) { void ipc.attachmentSrc(uuid).then(setHeroSrc).catch(() => {}); }
  }, [asset?.hero_attachment_uuid]);

  if (!loaded) return <div style={{ padding: 32 }}>Loading…</div>;
  if (!asset) {
    return (
      <div style={{ padding: 32, maxWidth: 720, margin: "0 auto" }}>
        <button type="button" onClick={onBack}
          style={{ display: "flex", alignItems: "center", gap: 4 }}>
          <ArrowLeft size={14} strokeWidth={1.8} /> Back
        </button>
        <h1 style={{ fontSize: 24, fontWeight: 600, marginTop: 16 }}>Asset not found</h1>
        <p style={{ color: "var(--ink-soft, #999)" }}>
          It may have been moved to Trash. You can restore it from the Trash view.
        </p>
      </div>
    );
  }

  const handleDelete = async () => {
    if (!window.confirm("Move this asset to Trash?")) return;
    try {
      await ipc.deleteAsset(id);
      onBack();
    } catch (e: unknown) {
      window.alert(`Failed to delete: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  const meta1 = [
    CATEGORY_LABEL[asset.category] ?? asset.category,
    asset.make,
    asset.model,
  ].filter(Boolean).join(" · ");

  const meta2 = [
    asset.serial_number ? `Serial: ${asset.serial_number}` : null,
    asset.purchase_date ? `Purchased: ${new Date(asset.purchase_date + "T00:00:00").toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" })}` : null,
  ].filter(Boolean).join("  ·  ");

  return (
    <div style={{ padding: 32, maxWidth: 720, margin: "0 auto" }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 16 }}>
        <button type="button" onClick={onBack}
          style={{ display: "flex", alignItems: "center", gap: 4 }}>
          <ArrowLeft size={14} strokeWidth={1.8} /> Back
        </button>
        <div style={{ display: "flex", gap: 8 }}>
          <button type="button" onClick={() => setEditing(true)}>
            <Pencil size={14} strokeWidth={1.8} /> Edit
          </button>
          <button type="button" onClick={handleDelete}>
            <Trash2 size={14} strokeWidth={1.8} /> Delete
          </button>
        </div>
      </div>

      <div style={{
        aspectRatio: "16 / 9",
        maxHeight: 360,
        background: "var(--paper-muted, #f5f5f5)",
        display: "flex", alignItems: "center", justifyContent: "center",
        marginBottom: 16, borderRadius: 6, overflow: "hidden",
      }}>
        {heroSrc ? (
          <img src={heroSrc} alt={asset.name}
            style={{ width: "100%", height: "100%", objectFit: "cover" }} />
        ) : (
          <ImageOff size={48} strokeWidth={1.4} color="var(--ink-soft, #999)" />
        )}
      </div>

      <h1 style={{ fontSize: 28, fontWeight: 600, margin: 0 }}>{asset.name}</h1>
      {meta1 && <div style={{ color: "var(--ink-soft, #999)", marginTop: 4 }}>{meta1}</div>}
      {meta2 && <div style={{ color: "var(--ink-soft, #999)", marginTop: 2 }}>{meta2}</div>}

      {asset.notes.trim() && (
        <>
          <h2 style={{ marginTop: 32, fontSize: 18 }}>Notes</h2>
          <pre style={{
            whiteSpace: "pre-wrap", fontFamily: "inherit",
            background: "var(--paper-muted, #f5f5f5)",
            padding: 16, borderRadius: 6,
          }}>
            {asset.notes}
          </pre>
        </>
      )}

      <h2 style={{ marginTop: 32, fontSize: 18 }}>Documents</h2>
      <DocumentList assetId={id} />

      {editing && (
        <AssetEditDrawer
          assetId={id}
          onClose={() => { setEditing(false); }}
        />
      )}
    </div>
  );
}
```

- [ ] **Step 3: Typecheck + build**

```bash
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```

- [ ] **Step 4: Commit**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4a-asset-registry
git add apps/desktop/src/components/Bones/
git commit -m "feat(asset): detail view with hero + metadata + documents + edit/delete"
```

---

## Task 11: Manual QA

**Files:** verification only.

- [ ] **Step 1: Full test suite**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4a-asset-registry
cargo test --workspace
```
Expected: baseline 321 + 7 DAL + 1 attachment helper = 329 lib + 3 integration tests all green.

- [ ] **Step 2: Clippy + typecheck + build**

```bash
cargo clippy --workspace -- -D warnings
cd apps/desktop && pnpm tsc --noEmit && pnpm build
```
Expected: clean.

- [ ] **Step 3: Dev-server golden path**

```bash
cd /Users/hanamori/life-assistant/.worktrees/l4a-asset-registry && pnpm tauri dev
```

Walk:
- Bones tab renders with empty-state "+ New asset" button.
- Create "Boiler" (appliance, make Worcester, model Greenstar 30i, purchase 2015-08-15) → appears in grid.
- Click card → detail view shows metadata + empty Documents list.
- Edit → change serial number → Save → detail refreshes.
- Edit → Hero image: Choose → pick a photo file → thumbnail appears in drawer + grid card + detail.
- Detail → Documents → + Add document → pick a PDF → appears in list with FileText icon + size + date. Click → opens in system default viewer.
- Add another doc (image) → appears with Image icon.
- Search "boil" → only Boiler shows.
- Category filter "Appliance" → same. Switch to "Vehicle" → empty.
- Delete → confirm → back to grid, row gone.
- Open Trash view → restore → Boiler back in Bones.

- [ ] **Step 4: If all green → invoke `superpowers:finishing-a-development-branch`.**

---

## Self-review

**Spec coverage:**
- §3 architecture → Tasks 2, 4, 5. ✓
- §4 migration V18 → Task 1. ✓
- §5 types → Task 2. ✓
- §6 DAL API → Task 2. ✓
- §7 attachment helper → Task 3. ✓
- §8 Tauri commands → Tasks 4–5. ✓
- §9 orphan sweep → Task 5. ✓
- §10 UI (nav, list, detail, drawer) → Tasks 7–10. ✓
- §11 Zustand store → Task 6. ✓
- §12 error handling → inline across tasks. ✓
- §13 testing — core unit tests in Tasks 2–3; manual QA in Task 11. ✓

**Placeholder scan:** none. Implementer notes in Tasks 3 + 5 direct inspection of existing code rather than leaving TBDs.

**Type consistency:** `Asset`, `AssetDraft`, `AssetCategory` used consistently Rust↔TS. `AttachmentSummary` on the TS side matches `Attachment` on the Rust side (field names align). IPC fn names (`list`, `get`, `create`, `update`, `deleteAsset`, `restore`, `attachHeroFromPath`, `attachDocumentFromPath`, `listDocuments`, `attachmentSrc`) match Tauri commands registered in Tasks 4–5.

---

*End of plan. Next: `superpowers:subagent-driven-development`.*
