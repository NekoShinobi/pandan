ALTER TABLE user_settings
ADD COLUMN sidebar_timezones_json TEXT NOT NULL DEFAULT '["UTC"]'
    CHECK (
        json_valid(sidebar_timezones_json)
        AND json_type(sidebar_timezones_json) = 'array'
        AND json_array_length(sidebar_timezones_json) BETWEEN 1 AND 5
    );

UPDATE user_settings
SET sidebar_timezones_json = json_array(timezone);
