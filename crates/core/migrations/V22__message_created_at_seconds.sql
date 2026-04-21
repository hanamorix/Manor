-- Convert message.created_at from milliseconds to seconds.
-- Threshold 2_000_000_000: unix seconds max out below that until year 2033,
-- while ms values from year 2001+ are always well above 1e12. Anything at or
-- above 2e9 is treated as ms and divided by 1000.
UPDATE message
SET created_at = created_at / 1000
WHERE created_at >= 2000000000;
