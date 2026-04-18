-- V17__shopping_list.sql
-- L3c Shopping List: single always-on shopping list items.

CREATE TABLE shopping_list_item (
    id              TEXT PRIMARY KEY,
    ingredient_name TEXT NOT NULL,
    quantity_text   TEXT,
    note            TEXT,
    recipe_id       TEXT REFERENCES recipe(id),
    recipe_title    TEXT,
    source          TEXT NOT NULL CHECK (source IN ('generated', 'manual')),
    position        INTEGER NOT NULL,
    ticked          INTEGER NOT NULL DEFAULT 0,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE INDEX idx_shopping_item_order  ON shopping_list_item(ticked, position);
CREATE INDEX idx_shopping_item_source ON shopping_list_item(source);
