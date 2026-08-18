CREATE TABLE IF NOT EXISTS oidc_identities (
    issuer     TEXT NOT NULL,
    subject    TEXT NOT NULL,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY (issuer, subject)
);

CREATE INDEX IF NOT EXISTS oidc_identities_user_idx ON oidc_identities (user_id);

CREATE TABLE IF NOT EXISTS oidc_authorizations (
    state         TEXT PRIMARY KEY NOT NULL,
    pkce_verifier TEXT NOT NULL,
    nonce         TEXT NOT NULL,
    expires_at    TEXT NOT NULL,
    created_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS oidc_authorizations_expiry_idx
    ON oidc_authorizations (expires_at);
