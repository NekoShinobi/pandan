CREATE TABLE contact_photos (
    contact_id TEXT PRIMARY KEY NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    mime_type TEXT NOT NULL CHECK(mime_type IN ('image/jpeg', 'image/png', 'image/webp', 'image/avif')),
    image_data BLOB NOT NULL CHECK(length(image_data) BETWEEN 1 AND 10485760),
    updated_at TEXT NOT NULL
);

CREATE INDEX contact_photos_user_idx ON contact_photos(user_id, contact_id);
