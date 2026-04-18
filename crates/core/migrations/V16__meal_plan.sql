-- V16__meal_plan.sql
-- L3b Meal Plan: meal_plan_entry + staple_item.

CREATE TABLE meal_plan_entry (
    id          TEXT PRIMARY KEY,
    entry_date  TEXT NOT NULL UNIQUE,
    recipe_id   TEXT REFERENCES recipe(id),
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
CREATE INDEX idx_meal_plan_date ON meal_plan_entry(entry_date);

CREATE TABLE staple_item (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    aliases    TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER
);
CREATE INDEX idx_staple_deleted ON staple_item(deleted_at);
CREATE INDEX idx_staple_name    ON staple_item(name COLLATE NOCASE);
