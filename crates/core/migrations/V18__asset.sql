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
