ALTER TABLE podcast_subscriptions
ADD COLUMN ntfy_notifications_enabled INTEGER NOT NULL DEFAULT 0
CHECK(ntfy_notifications_enabled IN (0, 1));

ALTER TABLE podcast_subscriptions
ADD COLUMN ntfy_topic_id TEXT REFERENCES ntfy_topics(id) ON DELETE SET NULL;

CREATE TABLE podcast_notification_deliveries (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    episode_id TEXT NOT NULL REFERENCES podcast_episodes(id) ON DELETE CASCADE,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts >= 0),
    last_error TEXT NOT NULL DEFAULT '',
    next_attempt_at TEXT NOT NULL,
    lease_started_at TEXT,
    delivered_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (user_id, episode_id)
);

CREATE INDEX podcast_notification_deliveries_queue_idx
    ON podcast_notification_deliveries(delivered_at, next_attempt_at, created_at);

-- A topic is account-owned. Removing it also disables every podcast route that selected it
-- and discards only the deliveries that have not left Pandan yet.
CREATE TRIGGER podcast_notifications_disable_deleted_topic
BEFORE DELETE ON ntfy_topics
FOR EACH ROW
BEGIN
    DELETE FROM podcast_notification_deliveries
    WHERE delivered_at IS NULL
      AND user_id = OLD.user_id
      AND episode_id IN (
          SELECT podcast_episodes.id
          FROM podcast_episodes
          JOIN podcast_subscriptions
            ON podcast_subscriptions.podcast_id = podcast_episodes.podcast_id
          WHERE podcast_subscriptions.user_id = OLD.user_id
            AND podcast_subscriptions.ntfy_topic_id = OLD.id
      );

    UPDATE podcast_subscriptions
       SET ntfy_notifications_enabled = 0,
           ntfy_topic_id = NULL
     WHERE user_id = OLD.user_id
       AND ntfy_topic_id = OLD.id;
END;
