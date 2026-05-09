-- V24: Per-item apply errors for bundle proposals.
-- When a bundled proposal partially applies, the failed indices' typed
-- ApplyError values are persisted here as a JSON array.
-- NULL = no errors recorded (single-item proposals or fully-applied bundles).
ALTER TABLE proposal ADD COLUMN apply_errors_json TEXT;
