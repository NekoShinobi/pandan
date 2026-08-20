ALTER TABLE embedded_pages
ADD COLUMN allow_same_origin INTEGER NOT NULL DEFAULT 0
CHECK (allow_same_origin IN (0, 1));
