ALTER TABLE dashboard_widgets
    ADD COLUMN grid_x INTEGER NOT NULL DEFAULT 0 CHECK (grid_x BETWEEN 0 AND 11);

ALTER TABLE dashboard_widgets
    ADD COLUMN grid_y INTEGER NOT NULL DEFAULT 0 CHECK (grid_y BETWEEN 0 AND 255);

ALTER TABLE dashboard_widgets
    ADD COLUMN grid_w INTEGER NOT NULL DEFAULT 6 CHECK (grid_w BETWEEN 1 AND 12);

ALTER TABLE dashboard_widgets
    ADD COLUMN grid_h INTEGER NOT NULL DEFAULT 4 CHECK (grid_h BETWEEN 1 AND 12);

UPDATE dashboard_widgets
SET grid_x = 0,
    grid_y = position * 5,
    grid_w = CASE size
        WHEN 'compact' THEN 4
        WHEN 'standard' THEN 6
        WHEN 'wide' THEN 8
        ELSE 12
    END,
    grid_h = CASE size
        WHEN 'compact' THEN 4
        WHEN 'standard' THEN 4
        WHEN 'wide' THEN 5
        ELSE 6
    END;

CREATE INDEX dashboard_widgets_grid_idx
    ON dashboard_widgets (user_id, workspace, grid_y, grid_x);
