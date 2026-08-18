CREATE TABLE user_wallpapers (
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    slot        TEXT NOT NULL CHECK (slot IN ('dashboard', 'welcome', 'login')),
    mime_type   TEXT NOT NULL CHECK (mime_type IN ('image/jpeg', 'image/png', 'image/webp', 'image/avif')),
    image_data  BLOB NOT NULL CHECK (length(image_data) BETWEEN 1 AND 31457280),
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (user_id, slot)
);

INSERT OR IGNORE INTO user_wallpapers (user_id, slot, mime_type, image_data, updated_at)
SELECT user_id, 'dashboard', mime_type, image_data, updated_at
FROM user_backgrounds
WHERE workspace = 0;

CREATE INDEX user_wallpapers_slot_updated_idx
ON user_wallpapers (slot, updated_at DESC);
