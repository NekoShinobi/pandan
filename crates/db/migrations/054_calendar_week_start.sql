ALTER TABLE user_settings
ADD COLUMN calendar_week_start TEXT NOT NULL DEFAULT 'sunday'
CHECK (calendar_week_start IN ('sunday', 'monday'));
