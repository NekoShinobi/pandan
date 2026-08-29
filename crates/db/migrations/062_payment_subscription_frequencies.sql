ALTER TABLE payment_subscriptions
ADD COLUMN frequency_interval INTEGER
CHECK(frequency_interval IS NULL OR frequency_interval BETWEEN 1 AND 999);

ALTER TABLE payment_subscriptions
ADD COLUMN frequency_unit TEXT
CHECK(frequency_unit IS NULL OR frequency_unit IN ('day', 'week', 'month', 'year'));

UPDATE payment_subscriptions
SET frequency_interval = CASE lower(trim(frequency))
        WHEN 'daily' THEN 1
        WHEN 'weekly' THEN 1
        WHEN 'every 2 weeks' THEN 2
        WHEN 'monthly' THEN 1
        WHEN 'every 2 months' THEN 2
        WHEN 'quarterly' THEN 3
        WHEN 'every 6 months' THEN 6
        WHEN 'yearly' THEN 1
    END,
    frequency_unit = CASE lower(trim(frequency))
        WHEN 'daily' THEN 'day'
        WHEN 'weekly' THEN 'week'
        WHEN 'every 2 weeks' THEN 'week'
        WHEN 'monthly' THEN 'month'
        WHEN 'every 2 months' THEN 'month'
        WHEN 'quarterly' THEN 'month'
        WHEN 'every 6 months' THEN 'month'
        WHEN 'yearly' THEN 'year'
    END
WHERE lower(trim(frequency)) IN (
    'daily',
    'weekly',
    'every 2 weeks',
    'monthly',
    'every 2 months',
    'quarterly',
    'every 6 months',
    'yearly'
);
