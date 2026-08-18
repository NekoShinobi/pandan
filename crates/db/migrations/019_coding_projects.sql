CREATE TABLE coding_projects (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK(provider IN ('github', 'gitlab', 'codeberg', 'gitea', 'forgejo')),
    host TEXT NOT NULL CHECK(length(trim(host)) BETWEEN 1 AND 253),
    repository TEXT NOT NULL CHECK(length(trim(repository)) BETWEEN 3 AND 240),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(user_id, provider, host, repository)
);

CREATE INDEX coding_projects_user_provider_idx
    ON coding_projects(user_id, provider, repository COLLATE NOCASE);

CREATE TABLE coding_credentials (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK(provider IN ('github', 'gitlab', 'codeberg', 'gitea', 'forgejo')),
    host TEXT NOT NULL CHECK(length(trim(host)) BETWEEN 1 AND 253),
    ciphertext TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(user_id, provider, host)
);

