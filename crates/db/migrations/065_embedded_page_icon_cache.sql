ALTER TABLE embedded_pages
ADD COLUMN icon_content_type TEXT CHECK (
    icon_content_type IS NULL OR icon_content_type IN (
        'image/avif', 'image/jpeg', 'image/png', 'image/webp',
        'image/x-icon', 'image/vnd.microsoft.icon'
    )
);

ALTER TABLE embedded_pages
ADD COLUMN icon_data BLOB CHECK (
    icon_data IS NULL OR length(icon_data) BETWEEN 1 AND 262144
);

ALTER TABLE embedded_pages
ADD COLUMN icon_fetched_at TEXT;
