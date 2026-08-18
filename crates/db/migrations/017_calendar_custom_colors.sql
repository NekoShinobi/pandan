ALTER TABLE calendar_subscriptions
ADD COLUMN color_value TEXT NOT NULL DEFAULT '#2DD4BF'
CHECK(
    color_value GLOB '#[0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f]'
);

UPDATE calendar_subscriptions
SET color_value = CASE color
    WHEN 'amber' THEN '#FBBF24'
    WHEN 'rose' THEN '#FB7185'
    WHEN 'blue' THEN '#60A5FA'
    WHEN 'slate' THEN '#94A3B8'
    ELSE '#2DD4BF'
END;
