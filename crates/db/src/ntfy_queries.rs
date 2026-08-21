use crate::entities::{NtfyConnection, NtfyNotification, NtfyNotificationDraft, NtfyTopic};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Loads one account's ntfy server configuration and encrypted credential.
pub async fn get_ntfy_connection(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Option<NtfyConnection>, sqlx::Error> {
    sqlx::query_as::<_, NtfyConnection>(
        "SELECT user_id, base_url, token_ciphertext, last_synced_at, last_error, \
         created_at, updated_at FROM ntfy_connections WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Lists every configured ntfy connection for the server-owned realtime worker.
pub async fn list_ntfy_connections(pool: &SqlitePool) -> Result<Vec<NtfyConnection>, sqlx::Error> {
    sqlx::query_as::<_, NtfyConnection>(
        "SELECT user_id, base_url, token_ciphertext, last_synced_at, last_error, \
         created_at, updated_at FROM ntfy_connections ORDER BY user_id",
    )
    .fetch_all(pool)
    .await
}

/// Creates or updates an account's ntfy server and optionally replaces or clears its token.
pub async fn upsert_ntfy_connection(
    pool: &SqlitePool,
    user_id: &str,
    base_url: &str,
    token_update: Option<Option<&str>>,
) -> Result<NtfyConnection, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    let previous_base_url: Option<String> =
        sqlx::query_scalar("SELECT base_url FROM ntfy_connections WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(&mut *transaction)
            .await?;
    sqlx::query(
        "INSERT INTO ntfy_connections \
         (user_id, base_url, token_ciphertext, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT(user_id) DO UPDATE SET base_url = excluded.base_url, updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(base_url)
    .bind(token_update.flatten())
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    if let Some(token) = token_update {
        sqlx::query(
            "UPDATE ntfy_connections SET token_ciphertext = ?, updated_at = ? WHERE user_id = ?",
        )
        .bind(token)
        .bind(&now)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    }
    if previous_base_url.is_some_and(|previous| previous != base_url) {
        sqlx::query("DELETE FROM ntfy_notifications WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "UPDATE ntfy_topics SET last_message_id = NULL, updated_at = ? WHERE user_id = ?",
        )
        .bind(&now)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    get_ntfy_connection(pool, user_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Removes the connection and all account-owned ntfy topics and notifications.
pub async fn delete_ntfy_connection(pool: &SqlitePool, user_id: &str) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM ntfy_connections WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            == 1,
    )
}

/// Lists topic subscriptions owned by one account.
pub async fn list_ntfy_topics(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<NtfyTopic>, sqlx::Error> {
    sqlx::query_as::<_, NtfyTopic>(
        "SELECT id, topic, label, last_message_id, created_at, updated_at \
         FROM ntfy_topics WHERE user_id = ? ORDER BY lower(label), lower(topic)",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Adds one account-owned topic subscription.
pub async fn create_ntfy_topic(
    pool: &SqlitePool,
    user_id: &str,
    topic: &str,
    label: &str,
) -> Result<NtfyTopic, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO ntfy_topics (id, user_id, topic, label, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(topic)
    .bind(label)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    sqlx::query_as::<_, NtfyTopic>(
        "SELECT id, topic, label, last_message_id, created_at, updated_at \
         FROM ntfy_topics WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Renames one account-owned topic label.
pub async fn update_ntfy_topic_label(
    pool: &SqlitePool,
    user_id: &str,
    topic_id: &str,
    label: &str,
) -> Result<Option<NtfyTopic>, sqlx::Error> {
    let changed = sqlx::query(
        "UPDATE ntfy_topics SET label = ?, updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(label)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(topic_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    if changed.rows_affected() != 1 {
        return Ok(None);
    }
    sqlx::query_as::<_, NtfyTopic>(
        "SELECT id, topic, label, last_message_id, created_at, updated_at \
         FROM ntfy_topics WHERE id = ? AND user_id = ?",
    )
    .bind(topic_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Removes one account-owned topic and its locally cached notifications.
pub async fn delete_ntfy_topic(
    pool: &SqlitePool,
    user_id: &str,
    topic_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM ntfy_topics WHERE id = ? AND user_id = ?")
            .bind(topic_id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            == 1,
    )
}

/// Stores a topic refresh idempotently and advances its per-topic cursor.
pub async fn store_ntfy_messages(
    pool: &SqlitePool,
    user_id: &str,
    topic_id: &str,
    messages: &[NtfyNotificationDraft],
    last_message_id: Option<&str>,
) -> Result<u64, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let received_at = chrono::Utc::now().to_rfc3339();
    let mut inserted = 0;
    for message in messages {
        inserted += sqlx::query(
            "INSERT INTO ntfy_notifications \
             (id, user_id, topic_id, remote_id, occurred_at, title, message, priority, \
              tags_json, click_url, actions_json, received_at) \
             SELECT ?, ?, id, ?, ?, ?, ?, ?, ?, ?, ?, ? FROM ntfy_topics \
             WHERE id = ? AND user_id = ? ON CONFLICT(topic_id, remote_id) DO NOTHING",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(&message.remote_id)
        .bind(message.occurred_at)
        .bind(&message.title)
        .bind(&message.message)
        .bind(message.priority)
        .bind(&message.tags_json)
        .bind(&message.click_url)
        .bind(&message.actions_json)
        .bind(&received_at)
        .bind(topic_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    }
    if let Some(remote_id) = last_message_id {
        sqlx::query(
            "UPDATE ntfy_topics SET last_message_id = ?, updated_at = ? \
             WHERE id = ? AND user_id = ?",
        )
        .bind(remote_id)
        .bind(&received_at)
        .bind(topic_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(inserted)
}

/// Stores one message from a live subscription and returns the account-scoped stored record.
///
/// The inserted flag distinguishes a new delivery from another browser's concurrent copy while
/// the unique topic/message constraint keeps the local inbox idempotent.
pub async fn store_ntfy_realtime_message(
    pool: &SqlitePool,
    user_id: &str,
    topic_id: &str,
    message: &NtfyNotificationDraft,
) -> Result<(bool, NtfyNotification), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let id = Uuid::new_v4().to_string();
    let received_at = chrono::Utc::now().to_rfc3339();
    let inserted = sqlx::query(
        "INSERT INTO ntfy_notifications \
         (id, user_id, topic_id, remote_id, occurred_at, title, message, priority, \
          tags_json, click_url, actions_json, received_at) \
         SELECT ?, ?, id, ?, ?, ?, ?, ?, ?, ?, ?, ? FROM ntfy_topics \
         WHERE id = ? AND user_id = ? ON CONFLICT(topic_id, remote_id) DO NOTHING",
    )
    .bind(&id)
    .bind(user_id)
    .bind(&message.remote_id)
    .bind(message.occurred_at)
    .bind(&message.title)
    .bind(&message.message)
    .bind(message.priority)
    .bind(&message.tags_json)
    .bind(&message.click_url)
    .bind(&message.actions_json)
    .bind(&received_at)
    .bind(topic_id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await?
    .rows_affected()
        == 1;
    sqlx::query(
        "UPDATE ntfy_topics SET last_message_id = ?, updated_at = ? \
         WHERE id = ? AND user_id = ?",
    )
    .bind(&message.remote_id)
    .bind(&received_at)
    .bind(topic_id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await?;
    let notification = sqlx::query_as::<_, NtfyNotification>(
        "SELECT n.id, n.topic_id, t.topic, t.label AS topic_label, n.remote_id, \
         n.occurred_at, n.title, n.message, n.priority, n.tags_json, n.click_url, \
         n.actions_json, n.seen_at, n.archived_at, n.received_at \
         FROM ntfy_notifications n JOIN ntfy_topics t ON t.id = n.topic_id \
         WHERE n.user_id = ? AND n.topic_id = ? AND n.remote_id = ?",
    )
    .bind(user_id)
    .bind(topic_id)
    .bind(&message.remote_id)
    .fetch_one(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok((inserted, notification))
}

/// Records the most recent synchronization result without leaking provider details elsewhere.
pub async fn set_ntfy_sync_status(
    pool: &SqlitePool,
    user_id: &str,
    error: Option<&str>,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE ntfy_connections SET last_synced_at = ?, last_error = ?, updated_at = ? \
         WHERE user_id = ?",
    )
    .bind(&now)
    .bind(error)
    .bind(&now)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Lists one account's retained notifications with an optional topic filter.
pub async fn list_ntfy_notifications(
    pool: &SqlitePool,
    user_id: &str,
    topic_id: Option<&str>,
    limit: usize,
) -> Result<Vec<NtfyNotification>, sqlx::Error> {
    sqlx::query_as::<_, NtfyNotification>(
        "SELECT n.id, n.topic_id, t.topic, t.label AS topic_label, n.remote_id, \
         n.occurred_at, n.title, n.message, n.priority, n.tags_json, n.click_url, \
         n.actions_json, n.seen_at, n.archived_at, n.received_at \
         FROM ntfy_notifications n JOIN ntfy_topics t ON t.id = n.topic_id \
         WHERE n.user_id = ? AND n.archived_at IS NULL \
              AND (? IS NULL OR n.topic_id = ?) \
         ORDER BY n.occurred_at DESC, n.received_at DESC LIMIT ?",
    )
    .bind(user_id)
    .bind(topic_id)
    .bind(topic_id)
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(pool)
    .await
}

/// Counts active notifications that have not yet been exposed in the notification popover.
pub async fn count_unseen_ntfy_notifications(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM ntfy_notifications \
         WHERE user_id = ? AND seen_at IS NULL AND archived_at IS NULL",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Marks every active notification as seen for one account.
pub async fn mark_ntfy_notifications_seen(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE ntfy_notifications SET seen_at = ? \
         WHERE user_id = ? AND seen_at IS NULL AND archived_at IS NULL",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected())
}

/// Permanently removes one account-owned notification after upstream deletion succeeds.
pub async fn delete_ntfy_notification(
    pool: &SqlitePool,
    user_id: &str,
    notification_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM ntfy_notifications WHERE id = ? AND user_id = ?")
            .bind(notification_id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            == 1,
    )
}

/// Loads one notification for action execution after account ownership has been resolved.
pub async fn get_ntfy_notification(
    pool: &SqlitePool,
    user_id: &str,
    notification_id: &str,
) -> Result<Option<NtfyNotification>, sqlx::Error> {
    sqlx::query_as::<_, NtfyNotification>(
        "SELECT n.id, n.topic_id, t.topic, t.label AS topic_label, n.remote_id, \
         n.occurred_at, n.title, n.message, n.priority, n.tags_json, n.click_url, \
         n.actions_json, n.seen_at, n.archived_at, n.received_at \
         FROM ntfy_notifications n JOIN ntfy_topics t ON t.id = n.topic_id \
         WHERE n.id = ? AND n.user_id = ?",
    )
    .bind(notification_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn notifications_remain_account_scoped_and_delete_permanently() {
        let pool = crate::connect("sqlite::memory:")
            .await
            .expect("database connects");
        let (owner, _) = crate::queries::create_account(
            &pool,
            "ntfy-owner@example.com",
            "$argon2id$ntfy-owner",
            "Ntfy Owner",
        )
        .await
        .expect("owner creates");
        let (other, _) = crate::queries::create_account(
            &pool,
            "ntfy-other@example.com",
            "$argon2id$ntfy-other",
            "Ntfy Other",
        )
        .await
        .expect("other account creates");
        upsert_ntfy_connection(
            &pool,
            &owner.id,
            "https://ntfy.sh",
            Some(Some("ciphertext")),
        )
        .await
        .expect("connection stores");
        let topic = create_ntfy_topic(&pool, &owner.id, "home-alerts", "Home alerts")
            .await
            .expect("topic stores");
        let draft = NtfyNotificationDraft {
            remote_id: "remote-1".to_owned(),
            occurred_at: 1_777_777_777,
            title: "Door opened".to_owned(),
            message: "Front entry".to_owned(),
            priority: 4,
            tags_json: "[\"house\"]".to_owned(),
            click_url: Some("https://example.com/event".to_owned()),
            actions_json: "[]".to_owned(),
        };
        assert_eq!(
            store_ntfy_messages(
                &pool,
                &owner.id,
                &topic.id,
                std::slice::from_ref(&draft),
                Some("remote-1"),
            )
            .await
            .expect("message stores"),
            1
        );
        assert_eq!(
            store_ntfy_messages(&pool, &owner.id, &topic.id, &[draft], Some("remote-1"),)
                .await
                .expect("duplicate is ignored"),
            0
        );

        let realtime_draft = NtfyNotificationDraft {
            remote_id: "remote-2".to_owned(),
            occurred_at: 1_777_777_778,
            title: "Window opened".to_owned(),
            message: "Office".to_owned(),
            priority: 3,
            tags_json: "[]".to_owned(),
            click_url: None,
            actions_json: "[]".to_owned(),
        };
        let (inserted, realtime) =
            store_ntfy_realtime_message(&pool, &owner.id, &topic.id, &realtime_draft)
                .await
                .expect("realtime message stores");
        assert!(inserted);
        assert_eq!(realtime.remote_id, "remote-2");
        let (inserted, duplicate) =
            store_ntfy_realtime_message(&pool, &owner.id, &topic.id, &realtime_draft)
                .await
                .expect("realtime duplicate resolves");
        assert!(!inserted);
        assert_eq!(duplicate.id, realtime.id);

        let notification = list_ntfy_notifications(&pool, &owner.id, None, 20)
            .await
            .expect("owner inbox loads")
            .pop()
            .expect("notification exists");
        assert!(
            list_ntfy_notifications(&pool, &other.id, None, 20)
                .await
                .expect("other inbox loads")
                .is_empty()
        );
        assert!(
            !delete_ntfy_notification(&pool, &other.id, &notification.id)
                .await
                .expect("other delete resolves")
        );
        assert!(
            delete_ntfy_notification(&pool, &owner.id, &notification.id)
                .await
                .expect("owner deletes")
        );
        assert!(
            delete_ntfy_notification(&pool, &owner.id, &realtime.id)
                .await
                .expect("owner deletes realtime notification")
        );
        assert!(
            list_ntfy_notifications(&pool, &owner.id, None, 20)
                .await
                .expect("active inbox loads")
                .is_empty()
        );

        upsert_ntfy_connection(&pool, &owner.id, "https://push.example.com", None)
            .await
            .expect("server changes");
        assert!(
            list_ntfy_notifications(&pool, &owner.id, None, 20)
                .await
                .expect("inbox reloads")
                .is_empty(),
            "messages from the previous server are cleared"
        );
        assert!(
            list_ntfy_topics(&pool, &owner.id)
                .await
                .expect("topics reload")
                .iter()
                .all(|topic| topic.last_message_id.is_none()),
            "topic cursors reset for the new server"
        );

        assert!(
            delete_ntfy_connection(&pool, &owner.id)
                .await
                .expect("connection deletes")
        );
        assert!(
            list_ntfy_topics(&pool, &owner.id)
                .await
                .expect("topics load")
                .is_empty(),
            "connection removal cascades through topics and notifications"
        );
    }
}
