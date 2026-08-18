CREATE TABLE IF NOT EXISTS tasks (
    id         TEXT PRIMARY KEY NOT NULL,
    title      TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 120),
    completed  INTEGER NOT NULL DEFAULT 0 CHECK (completed IN (0, 1)),
    priority   TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('high', 'normal', 'low')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS tasks_completion_created_idx
    ON tasks (completed, created_at);

CREATE TABLE IF NOT EXISTS feed_items (
    id              TEXT PRIMARY KEY NOT NULL,
    category        TEXT NOT NULL CHECK (category IN ('Design', 'Technology', 'Culture')),
    source          TEXT NOT NULL,
    title           TEXT NOT NULL,
    summary         TEXT NOT NULL,
    reading_minutes INTEGER NOT NULL CHECK (reading_minutes > 0),
    published_at    TEXT NOT NULL
);

INSERT OR IGNORE INTO tasks (id, title, completed, priority, created_at, updated_at) VALUES
    ('task-review-notes', 'Review sprint notes', 1, 'normal', '2026-08-13T08:00:00Z', '2026-08-13T09:15:00Z'),
    ('task-concept-review', 'Prepare concept review', 0, 'high', '2026-08-13T08:05:00Z', '2026-08-13T08:05:00Z'),
    ('task-research-reply', 'Reply to research team', 0, 'normal', '2026-08-13T08:10:00Z', '2026-08-13T08:10:00Z'),
    ('task-references', 'Organize saved references', 1, 'low', '2026-08-13T08:15:00Z', '2026-08-13T09:30:00Z');

INSERT OR IGNORE INTO feed_items (id, category, source, title, summary, reading_minutes, published_at) VALUES
    ('feed-design-systems', 'Design', 'Dense Discovery', 'Design systems that age well', 'A practical field note on tokens, governance, and restraint.', 6, '2026-08-13T07:45:00Z'),
    ('feed-small-models', 'Technology', 'MIT Technology Review', 'The small-model shift', 'Why focused systems are becoming more useful at the edge.', 8, '2026-08-13T06:30:00Z'),
    ('feed-studio-attention', 'Culture', 'Creative Independent', 'A studio built around attention', 'An interview about protecting deep work in public practice.', 11, '2026-08-12T18:10:00Z'),
    ('feed-spatial-interfaces', 'Design', 'Sidebar', 'Interfaces beyond the rectangle', 'Spatial transitions as orientation, not decoration.', 5, '2026-08-12T16:00:00Z');
