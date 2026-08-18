ALTER TABLE payment_subscriptions
ADD COLUMN amount_micros INTEGER NOT NULL DEFAULT 0
CHECK(amount_micros BETWEEN 0 AND 1000000000000);

ALTER TABLE payment_subscriptions
ADD COLUMN currency TEXT NOT NULL DEFAULT 'USD'
CHECK(currency GLOB '[A-Z][A-Z][A-Z]');
