-- SQLite does not support ADD COLUMN IF NOT EXISTS. The migration runner
-- conditionally adds any missing channel portrait columns before recording
-- this migration, then executes this statement as its embedded SQL marker.
SELECT 1;
