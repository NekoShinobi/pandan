ALTER TABLE youtube_videos ADD COLUMN view_count INTEGER CHECK (view_count >= 0);
ALTER TABLE youtube_videos ADD COLUMN duration_seconds INTEGER CHECK (duration_seconds > 0);
