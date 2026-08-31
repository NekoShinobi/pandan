CREATE TABLE coding_categories (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL COLLATE NOCASE CHECK(length(trim(name)) BETWEEN 1 AND 48),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(user_id, name)
);

CREATE INDEX coding_categories_user_name_idx
    ON coding_categories(user_id, name COLLATE NOCASE);

CREATE TABLE coding_project_categories (
    project_id TEXT NOT NULL REFERENCES coding_projects(id) ON DELETE CASCADE,
    category_id TEXT NOT NULL REFERENCES coding_categories(id) ON DELETE CASCADE,
    PRIMARY KEY(project_id, category_id)
);

CREATE INDEX coding_project_categories_category_idx
    ON coding_project_categories(category_id, project_id);
