ALTER TABLE youtube_channels
ADD COLUMN thumbnail_url TEXT NOT NULL DEFAULT '';

ALTER TABLE youtube_channels
ADD COLUMN thumbnail_fetched_at TEXT;

ALTER TABLE youtube_channels
ADD COLUMN thumbnail_content_type TEXT NOT NULL DEFAULT '';

ALTER TABLE youtube_channels
ADD COLUMN thumbnail_data BLOB;
