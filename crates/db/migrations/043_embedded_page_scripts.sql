ALTER TABLE embedded_pages
ADD COLUMN allow_scripts INTEGER NOT NULL DEFAULT 0
CHECK (allow_scripts IN (0, 1));
