ALTER TABLE user_appearance
ADD COLUMN highlight_color TEXT NOT NULL DEFAULT '#72D577'
CHECK (
    length(highlight_color) = 7
    AND highlight_color GLOB '#[0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f]'
);

CREATE TABLE instance_branding (
    id                    INTEGER PRIMARY KEY CHECK (id = 1),
    logo_mime_type        TEXT CHECK (
        logo_mime_type IS NULL OR logo_mime_type IN (
            'image/avif', 'image/jpeg', 'image/png', 'image/webp'
        )
    ),
    logo_data             BLOB CHECK (
        logo_data IS NULL OR length(logo_data) BETWEEN 1 AND 10485760
    ),
    favicon_mime_type     TEXT CHECK (
        favicon_mime_type IS NULL OR favicon_mime_type IN (
            'image/avif', 'image/jpeg', 'image/png', 'image/webp'
        )
    ),
    favicon_data          BLOB CHECK (
        favicon_data IS NULL OR length(favicon_data) BETWEEN 1 AND 1048576
    ),
    updated_at            TEXT NOT NULL,
    CHECK (
        (logo_mime_type IS NULL AND logo_data IS NULL)
        OR (logo_mime_type IS NOT NULL AND logo_data IS NOT NULL)
    ),
    CHECK (
        (favicon_mime_type IS NULL AND favicon_data IS NULL)
        OR (favicon_mime_type IS NOT NULL AND favicon_data IS NOT NULL)
    )
);

INSERT INTO instance_branding (
    id,
    logo_mime_type,
    logo_data,
    favicon_mime_type,
    favicon_data,
    updated_at
) VALUES (1, NULL, NULL, NULL, NULL, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
