ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'member'
    CHECK (role IN ('administrator', 'member'));

UPDATE users
SET role = 'administrator'
WHERE id = (
    SELECT id FROM users ORDER BY created_at ASC, id ASC LIMIT 1
)
AND NOT EXISTS (
    SELECT 1 FROM users WHERE role = 'administrator'
);

INSERT OR IGNORE INTO app_metadata (key, value, updated_at)
SELECT 'onboarding_complete', 'true', datetime('now')
WHERE EXISTS (SELECT 1 FROM users);
