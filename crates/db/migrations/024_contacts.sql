CREATE TABLE contact_dav_sources (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 80),
    url TEXT NOT NULL CHECK(length(trim(url)) BETWEEN 8 AND 2048),
    username TEXT NOT NULL DEFAULT '' CHECK(length(username) <= 320),
    password_ciphertext TEXT,
    last_synced_at TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(user_id, url)
);

CREATE INDEX contact_dav_sources_user_idx
    ON contact_dav_sources(user_id, name COLLATE NOCASE);

CREATE TABLE contacts (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    dav_source_id TEXT REFERENCES contact_dav_sources(id) ON DELETE SET NULL,
    source_kind TEXT NOT NULL DEFAULT 'manual'
        CHECK(source_kind IN ('manual', 'monica', 'carddav')),
    source_reference TEXT,
    first_name TEXT NOT NULL DEFAULT '' CHECK(length(first_name) <= 120),
    middle_name TEXT NOT NULL DEFAULT '' CHECK(length(middle_name) <= 120),
    last_name TEXT NOT NULL DEFAULT '' CHECK(length(last_name) <= 120),
    nickname TEXT NOT NULL DEFAULT '' CHECK(length(nickname) <= 120),
    pronouns TEXT NOT NULL DEFAULT '' CHECK(length(pronouns) <= 80),
    company TEXT NOT NULL DEFAULT '' CHECK(length(company) <= 160),
    job_title TEXT NOT NULL DEFAULT '' CHECK(length(job_title) <= 160),
    birthday TEXT,
    emails_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(emails_json)),
    phones_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(phones_json)),
    addresses_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(addresses_json)),
    important_dates_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(important_dates_json)),
    tags_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tags_json)),
    relationship_context TEXT NOT NULL DEFAULT '' CHECK(length(relationship_context) <= 4000),
    notes TEXT NOT NULL DEFAULT '' CHECK(length(notes) <= 20000),
    favorite INTEGER NOT NULL DEFAULT 0 CHECK(favorite IN (0, 1)),
    archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK(length(trim(first_name)) + length(trim(last_name)) + length(trim(nickname)) > 0),
    UNIQUE(user_id, source_kind, source_reference)
);

CREATE INDEX contacts_user_name_idx
    ON contacts(user_id, archived, favorite DESC, last_name COLLATE NOCASE, first_name COLLATE NOCASE);

CREATE INDEX contacts_dav_source_idx
    ON contacts(user_id, dav_source_id, source_reference);
