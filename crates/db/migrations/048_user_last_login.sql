ALTER TABLE users ADD COLUMN last_login_at TEXT;

UPDATE users
SET last_login_at = (
    SELECT MAX(sessions.created_at)
    FROM sessions
    WHERE sessions.user_id = users.id
);
