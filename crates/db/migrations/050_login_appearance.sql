CREATE TABLE login_appearance (
    id                     INTEGER PRIMARY KEY CHECK (id = 1),
    background_blur        INTEGER NOT NULL DEFAULT 0 CHECK (background_blur BETWEEN 0 AND 24),
    background_brightness  INTEGER NOT NULL DEFAULT 78 CHECK (background_brightness BETWEEN 40 AND 140),
    background_contrast    INTEGER NOT NULL DEFAULT 108 CHECK (background_contrast BETWEEN 50 AND 160),
    background_saturation  INTEGER NOT NULL DEFAULT 72 CHECK (background_saturation BETWEEN 0 AND 180),
    updated_at             TEXT NOT NULL
);

INSERT INTO login_appearance (
    id,
    background_blur,
    background_brightness,
    background_contrast,
    background_saturation,
    updated_at
) VALUES (1, 0, 78, 108, 72, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
