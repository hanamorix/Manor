# L4a Asset Registry — Design Spec

- **Date**: 2026-04-18
- **Landmark**: v0.5 Bones → L4a (first sub-landmark)
- **Status**: Approved design. Implementation plan next.
- **Authors**: Hana (product), Nell (architect)
- **Roadmap**: `specs/2026-04-18-v0.5-bones-roadmap.md`
- **Depends on**: foundation tables (shipped in v0.1 Completion Phase A), L3a's attachment + trash-sweep infrastructure.

## 1. Purpose

Ship Manor's house-operations foundation: a registry of things the user owns — boilers, washing machines, cars, fixtures — with make, model, serial, purchase date, notes, a hero image, and any number of attached PDFs (manuals, warranties, invoices). L4a ships alone as "a list of stuff I own with photos, manuals, and purchase info." Everything later in v0.5 Bones (schedules, events, repair lookup, PDF extraction) hangs off `asset`.

## 2. Scope decisions (locked during brainstorm)

| Area | Decision |
|---|---|
| **Data model depth** | Structured-lite — name, category, make, model, serial_number, purchase_date, notes, hero image. Optional fields nullable. No purchase price, warranty expiry, location, or depreciation. |
| **Category taxonomy** | Fixed 4-value enum: `appliance | vehicle | fixture | other`. No sub-categories. Free-form tags via shared `tag_link` for finer slicing. |
| **Attachments** | L3a-style: `asset.hero_attachment_uuid` column (single hero image) + any number of additional attachments via `attachment` table with `entity_type='asset'`, `entity_id=asset.id`. |
| **Nav** | New top-level **Bones** tab. L4a ships without sub-nav; sub-nav arrives with L4b (matches how Hearth evolved). Icon: Lucide `Wrench`. |
| **Attachment helper** | Add `attachment::list_for_text_entity(conn, entity_type, entity_id: &str)` to core — closes the L3a code-review follow-up. L4a is its first caller. |
| **Trash** | Soft-delete + 30-day purge via existing sweeper. Cascades hero + document attachments. |

## 3. Architecture

Two-crate split identical to L3a/L3b/L3c.

### 3.1 New Rust files

- `crates/core/src/asset/mod.rs` — types (`Asset`, `AssetDraft`, `AssetCategory`).
- `crates/core/src/asset/dal.rs` — CRUD + list with filter.
- `crates/app/src/asset/mod.rs` — module root.
- `crates/app/src/asset/commands.rs` — Tauri IPC (list, get, create, update, delete, restore, attach_document, list_documents).
- `crates/app/src/asset/importer.rs` — hero image staging (no URL fetch; local file path only) + document attach flow, mirroring L3a's `importer::fetch_and_link_hero_arc` shape.

### 3.2 New frontend files

- `apps/desktop/src/lib/asset/ipc.ts`
- `apps/desktop/src/lib/asset/state.ts`
- `apps/desktop/src/components/Bones/BonesTab.tsx`
- `apps/desktop/src/components/Bones/AssetCard.tsx`
- `apps/desktop/src/components/Bones/AssetDetail.tsx`
- `apps/desktop/src/components/Bones/AssetEditDrawer.tsx`
- `apps/desktop/src/components/Bones/DocumentList.tsx`

### 3.3 Modified files

- `crates/core/src/lib.rs` — `pub mod asset;`.
- `crates/core/src/attachment.rs` — add `list_for_text_entity`.
- `crates/core/src/trash.rs` — register `asset` in REGISTRY.
- `crates/app/src/lib.rs` — register Tauri commands + `pub mod asset;` + startup hook for asset-stage-orphan sweep (mirrors L3a's `stage_sweep::run_on_startup` pattern).
- `apps/desktop/src/components/Nav/Sidebar.tsx` — add Bones entry between Ledger and Hearth.
- `apps/desktop/src/App.tsx` — route `view === "bones"` to `<BonesTab />`.
- `apps/desktop/src/lib/nav.ts` — extend `View` union with `"bones"`.

## 4. Schema — migration V18

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
    purchase_date         TEXT,                     -- ISO YYYY-MM-DD; nullable
    notes                 TEXT NOT NULL DEFAULT '', -- markdown
    hero_attachment_uuid  TEXT,                     -- references attachment.uuid; no FK enforced
    created_at            INTEGER NOT NULL,         -- seconds since epoch
    updated_at            INTEGER NOT NULL,
    deleted_at            INTEGER                   -- nullable; trash soft-delete
);

CREATE INDEX idx_asset_deleted  ON asset(deleted_at);
CREATE INDEX idx_asset_category ON asset(category) WHERE deleted_at IS NULL;
CREATE INDEX idx_asset_name     ON asset(name COLLATE NOCASE);
CREATE INDEX idx_asset_hero     ON asset(hero_attachment_uuid) WHERE hero_attachment_uuid IS NOT NULL;
```

Trash sweeper registration: append `("asset", "name")` to the existing `REGISTRY` in `crates/core/src/trash.rs`.

Timestamp unit: seconds (matching L3a's fix at `7326ea8`).

## 5. Types (core)

```rust
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

## 6. DAL API

`crates/core/src/asset/dal.rs`:

```rust
pub fn insert_asset(conn: &Connection, draft: &AssetDraft) -> Result<String>;
pub fn get_asset(conn: &Connection, id: &str) -> Result<Option<Asset>>;      // filters deleted_at IS NULL
pub fn get_asset_including_trashed(conn: &Connection, id: &str) -> Result<Option<Asset>>;
pub fn list_assets(conn: &Connection, filter: &AssetListFilter) -> Result<Vec<Asset>>;
pub fn update_asset(conn: &Connection, id: &str, draft: &AssetDraft) -> Result<()>;
pub fn soft_delete_asset(conn: &Connection, id: &str) -> Result<()>;
pub fn restore_asset(conn: &Connection, id: &str) -> Result<()>;
pub fn set_hero_attachment(conn: &Connection, id: &str, uuid: Option<&str>) -> Result<()>;

#[derive(Debug, Clone, Default)]
pub struct AssetListFilter {
    pub search: Option<String>,              // name substring LIKE %q%
    pub category: Option<AssetCategory>,     // optional exact match
    pub include_trashed: bool,               // default false
}
```

Ordering in `list_assets`: `ORDER BY name COLLATE NOCASE ASC` (alphabetical).

## 7. Attachment helper — new

Add to `crates/core/src/attachment.rs`:

```rust
/// Like `list_for` but accepts a TEXT entity_id (e.g. UUID strings from recipes, assets).
/// Relies on SQLite's dynamic typing — the `entity_id INTEGER` column happily stores
/// text values that are compared equal to text-typed parameters.
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

Unit test verifying it finds an asset's documents.

## 8. Tauri commands

`crates/app/src/asset/commands.rs`:

```rust
#[tauri::command] pub fn asset_list(args: AssetListArgs, state: State<'_, Db>) -> Result<Vec<Asset>, String>;
#[tauri::command] pub fn asset_get(id: String, state: State<'_, Db>) -> Result<Option<Asset>, String>;
#[tauri::command] pub fn asset_create(draft: AssetDraft, state: State<'_, Db>) -> Result<String, String>;
#[tauri::command] pub fn asset_update(id: String, draft: AssetDraft, state: State<'_, Db>) -> Result<(), String>;
#[tauri::command] pub fn asset_delete(id: String, state: State<'_, Db>) -> Result<(), String>;
#[tauri::command] pub fn asset_restore(id: String, state: State<'_, Db>) -> Result<(), String>;

#[tauri::command] pub async fn asset_attach_hero_from_path(
    id: String, source_path: String, state: State<'_, Db>, app: AppHandle
) -> Result<String, String>;

#[tauri::command] pub async fn asset_attach_document_from_path(
    id: String, source_path: String, state: State<'_, Db>, app: AppHandle
) -> Result<String, String>;

#[tauri::command] pub fn asset_list_documents(id: String, state: State<'_, Db>) -> Result<Vec<Attachment>, String>;
```

`AssetListArgs { search: Option<String>, category: Option<AssetCategory> }`.

Attach-hero / attach-document both: copy source file into `attachments_dir`, call `manor_core::attachment::store(...)` with `entity_type=Some("asset")` + `entity_id=None` (staged) for hero, or `entity_id=asset.id` directly for documents, then for hero update `asset.hero_attachment_uuid`.

## 9. Orphan sweep

Mirrors L3a exactly. Register a second `run_on_startup` call in `crates/app/src/lib.rs`'s setup hook:

```rust
// L4a — sweep orphan asset hero stagings (>24h old).
let _ = crate::asset::importer::stage_sweep_run_on_startup(&conn, &attachments_dir);
```

Helper lives at `crates/app/src/asset/importer.rs` — copy/adapt the pattern from L3a's `recipe::stage_sweep`.

## 10. UI

### 10.1 Nav

- New `Bones` entry in `Sidebar.tsx`, Lucide `Wrench` icon (size 22, stroke 1.8). Insert between `Ledger` and `Hearth` (or wherever the existing sidebar-item ordering suggests — follow convention of alphabetical or logical grouping).
- `View` union in `lib/nav.ts` extended with `"bones"`.
- `App.tsx` routes `view === "bones"` to `<BonesTab />`.

### 10.2 Bones landing (grid)

```
Assets                                 [+ New] [↓ Search]
[search name] [All categories ▾]

┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐
│ img  │ │ img  │ │ img  │ │ img  │
└──────┘ └──────┘ └──────┘ └──────┘
Boiler   Washing  Dishwash Honda
Worcest. Bosch    Bosch    Civic
🔧 2015  🔧 2022  🔧 2023  🚗 2019
```

- `AssetCard` render: hero image (4:3, `ImageOff` placeholder), name (1-line clamp), make subtitle, meta strip with category icon + purchase year.
- Category → icon mapping: `appliance` → `Wrench`, `vehicle` → `Car`, `fixture` → `Home`, `other` → `Box` (all Lucide).
- Search input: 200ms debounced (mirror L3a's HearthTab search).
- Category dropdown: single-select (`All | Appliance | Vehicle | Fixture | Other`).
- Grid: `repeat(auto-fit, minmax(240px, 1fr))` same as L3a Recipes list.
- Empty state: "Your asset registry is empty." + big `+ New asset` button.

### 10.3 Detail view

- Top bar: `← Back`, `Edit`, `Delete`.
- Hero image block (16:9 max-height, flat placeholder if absent).
- Title (h1), category chip inline.
- Meta strip: `{make} · {model}` / `Serial: {serial_number}  ·  Purchased: {purchase_date}` — null-safe (skip missing parts).
- Tags row (reuse existing tag rendering pattern from L3a).
- **Notes** section: rendered markdown (reuse whatever Manor already uses — inspect Today's or Recipes' markdown rendering).
- **Documents** section: flat list via `<DocumentList />`:
  - Each row: MIME icon (Lucide `FileText` for PDF, `Image` for image, `File` for other) + filename + size + date.
  - Click a row → Tauri `attachment_get_path_by_uuid` (shipped L3a) → `convertFileSrc` → `window.open` OR `shell.open()` to launch system default app.
  - `+ Add document` button at bottom → Tauri dialog plugin file picker → `asset_attach_document_from_path` → reloads list.
- Tri-state render: `{ loading, asset-exists, not-found }` pattern from L3b's RecipeDetail fix.

### 10.4 New/Edit drawer

Right-side slide-in drawer, same chrome as L3a's `RecipeEditDrawer`:

- Name (required)
- Category dropdown
- Make / Model / Serial (optional text inputs)
- Purchase date (`<input type="date">`)
- Hero image: thumbnail + Replace + Remove buttons; Replace opens file picker and calls `asset_attach_hero_from_path` (writes + updates `hero_attachment_uuid`).
- Tags (chip-input reusing whatever exists for recipes)
- Notes (markdown textarea)
- Cancel / Save buttons

### 10.5 Delete flow

Delete button → `window.confirm("Move to Trash?")` → `asset_delete(id)` → back to grid. Existing Trash view (shipped in v0.1 Phase B) surfaces trashed assets. Restore round-trips cleanly.

## 11. Frontend Zustand store

`apps/desktop/src/lib/asset/state.ts`:

```ts
type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface AssetStore {
  assets: Asset[];
  search: string;
  category: AssetCategory | null;
  loadStatus: LoadStatus;

  load(): Promise<void>;
  setSearch(s: string): void;           // 200ms-debounced via HearthTab-style useEffect in the view, not in the store
  setCategory(c: AssetCategory | null): void;
}
```

## 12. Error handling

| Error | User sees | Recovery |
|---|---|---|
| Save fails | Inline error in drawer; drawer stays open | Retry |
| List load fails | "Couldn't load assets" + Retry button | Retry |
| Hero upload fails | Asset saves without image; toast "Couldn't attach image" | Retry via Edit |
| Document upload fails | Toast "Couldn't attach file" | Retry |
| Delete fails | Toast; row stays | Retry |
| Opening a document fails (file missing) | Toast "Couldn't open — file may be gone" | User removes the attachment |
| Stale navigation to trashed asset | Tri-state detail: "Asset not found — may be in Trash" + Back | Back button |

## 13. Testing strategy

### 13.1 Unit (crates/core)

- `asset::dal` CRUD round-trip with all fields nullable.
- Update bumps `updated_at`; replaces nullable fields cleanly (setting make to None clears it).
- `get_asset` filters trashed; `get_asset_including_trashed` surfaces them.
- `list_assets` filters: search substring, category filter, include_trashed=false default.
- `list_assets` alphabetical ordering.
- Soft-delete + restore cycle.
- `attachment::list_for_text_entity` finds attachments with `entity_type='asset'` + UUID entity_id.
- Trash sweep purges asset after 30 days.

### 13.2 Integration (crates/app)

- `asset_create` + `asset_get` + `asset_list` round-trip.
- `asset_attach_hero_from_path` copies file + updates `hero_attachment_uuid`.
- `asset_attach_document_from_path` + `asset_list_documents` round-trip.
- Deleting an asset leaves its attachments for the sweeper (no premature delete).

### 13.3 Frontend

RTL:
- BonesTab renders empty state + `+ New` button.
- Create via drawer → grid updates (mocked IPC).
- Detail view tri-state (loading → asset → not-found).
- Search + category filter compose correctly.
- Document list + add-document flow (mocked file picker).

## 14. Out of scope for L4a (pinned)

- Maintenance schedules + due dates (L4b).
- Maintenance events / cost (L4c).
- Right-to-repair lookup (L4d).
- PDF manual extraction (L4e).
- Warranty expiry alerts.
- Vehicle-specific fields (MOT, insurance).
- Multi-property support.
- Room / location taxonomy beyond tags.
- Auto-creation from Ledger transactions.
- Barcode / QR scanning.
- Attachment metadata labels (manual/warranty/invoice).
- PDF preview thumbnails.
- Drag-to-reorder documents.

## 15. Definition of done

- Migration V18 runs cleanly on fresh + existing dev DBs.
- `Bones` nav entry renders with `Wrench` icon; clicking routes to `<BonesTab />`.
- Empty state → `+ New asset` → drawer → Save → grid updates.
- Grid card → click → detail view (tri-state) → Edit drawer → Save → grid refreshes.
- Hero image upload / replace / remove round-trip.
- Document upload (PDF, image) appears in Documents list; clicking opens in system default app.
- Search + category filter work together.
- Soft-delete + restore from Trash round-trip; 30-day sweep cascades attachments.
- `attachment::list_for_text_entity` shipped + tested.
- `cargo test --workspace` green. Clippy clean. TypeScript clean. Production build green.
- Manual QA: add a boiler with hero photo + user manual PDF + purchase invoice image, edit serial, delete, restore from Trash.

---

*End of L4a design spec. Next: implementation plan.*
