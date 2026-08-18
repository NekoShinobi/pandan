CREATE TABLE journal_documents (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    parent_id TEXT REFERENCES journal_documents(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 120),
    content TEXT NOT NULL DEFAULT '' CHECK(length(content) <= 1000000),
    position INTEGER NOT NULL DEFAULT 0 CHECK(position BETWEEN 0 AND 100000),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT INTO journal_documents
    (id, user_id, parent_id, name, content, position, created_at, updated_at)
SELECT id, user_id, parent_id, name, content, position, created_at, updated_at
FROM journal_nodes;

DROP TABLE journal_nodes;
ALTER TABLE journal_documents RENAME TO journal_nodes;

CREATE INDEX journal_nodes_user_parent_position_idx
    ON journal_nodes(user_id, parent_id, position, name COLLATE NOCASE);

CREATE UNIQUE INDEX journal_nodes_unique_root_name_idx
    ON journal_nodes(user_id, lower(name))
    WHERE parent_id IS NULL;

CREATE UNIQUE INDEX journal_nodes_unique_nested_name_idx
    ON journal_nodes(user_id, parent_id, lower(name))
    WHERE parent_id IS NOT NULL;
