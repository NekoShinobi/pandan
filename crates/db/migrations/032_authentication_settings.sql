CREATE TABLE authentication_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    password_login_enabled INTEGER NOT NULL DEFAULT 1 CHECK (password_login_enabled IN (0, 1)),
    password_registration_enabled INTEGER NOT NULL DEFAULT 1 CHECK (password_registration_enabled IN (0, 1)),
    oidc_registration_enabled INTEGER NOT NULL DEFAULT 1 CHECK (oidc_registration_enabled IN (0, 1)),
    updated_at TEXT NOT NULL
);

INSERT INTO authentication_settings (
    id,
    password_login_enabled,
    password_registration_enabled,
    oidc_registration_enabled,
    updated_at
) VALUES (1, 1, 1, 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
