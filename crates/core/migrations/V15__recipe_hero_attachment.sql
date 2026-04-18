-- V15__recipe_hero_attachment.sql
-- L3a follow-up: link recipe → hero image via attachment.uuid (TEXT),
-- avoiding the type mismatch of storing TEXT UUIDs in attachment.entity_id INTEGER.
--
-- hero_attachment_uuid references attachment.uuid directly (both TEXT).
-- The orphan sweep excludes attachments whose uuid appears here, so a linked
-- hero image is never purged even though entity_id stays NULL in the attachment table.

ALTER TABLE recipe ADD COLUMN hero_attachment_uuid TEXT;
CREATE INDEX idx_recipe_hero ON recipe(hero_attachment_uuid) WHERE hero_attachment_uuid IS NOT NULL;
