ALTER TABLE calendar_subscriptions
ADD COLUMN display_mode TEXT NOT NULL DEFAULT 'full'
CHECK(display_mode IN ('full', 'dot'));
