-- V14__recipe.sql
-- L3a Recipe Library: recipe + recipe_ingredient tables.

CREATE TABLE recipe (
    id             TEXT PRIMARY KEY,
    title          TEXT NOT NULL,
    servings       INTEGER,
    prep_time_mins INTEGER,
    cook_time_mins INTEGER,
    instructions   TEXT NOT NULL,
    source_url     TEXT,
    source_host    TEXT,
    import_method  TEXT,
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL,
    deleted_at     INTEGER
);

CREATE INDEX idx_recipe_deleted ON recipe(deleted_at);
CREATE INDEX idx_recipe_title   ON recipe(title COLLATE NOCASE);

CREATE TABLE recipe_ingredient (
    id              TEXT PRIMARY KEY,
    recipe_id       TEXT NOT NULL REFERENCES recipe(id) ON DELETE CASCADE,
    position        INTEGER NOT NULL,
    quantity_text   TEXT,
    ingredient_name TEXT NOT NULL,
    note            TEXT
);

CREATE INDEX idx_ri_recipe ON recipe_ingredient(recipe_id, position);
