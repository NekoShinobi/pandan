use crate::entities::{Contact, ContactDavSource, ContactDraft, ContactPhoto};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, FromRow)]
struct ContactRow {
    id: String,
    dav_source_id: Option<String>,
    source_kind: String,
    source_reference: Option<String>,
    first_name: String,
    middle_name: String,
    last_name: String,
    nickname: String,
    pronouns: String,
    company: String,
    job_title: String,
    birthday: Option<String>,
    emails_json: String,
    phones_json: String,
    addresses_json: String,
    important_dates_json: String,
    tags_json: String,
    relationship_context: String,
    notes: String,
    favorite: bool,
    archived: bool,
    has_photo: bool,
    created_at: String,
    updated_at: String,
}

impl From<ContactRow> for Contact {
    fn from(row: ContactRow) -> Self {
        Self {
            id: row.id,
            dav_source_id: row.dav_source_id,
            source_kind: row.source_kind,
            source_reference: row.source_reference,
            first_name: row.first_name,
            middle_name: row.middle_name,
            last_name: row.last_name,
            nickname: row.nickname,
            pronouns: row.pronouns,
            company: row.company,
            job_title: row.job_title,
            birthday: row.birthday,
            emails: serde_json::from_str(&row.emails_json).unwrap_or_default(),
            phones: serde_json::from_str(&row.phones_json).unwrap_or_default(),
            addresses: serde_json::from_str(&row.addresses_json).unwrap_or_default(),
            important_dates: serde_json::from_str(&row.important_dates_json).unwrap_or_default(),
            tags: serde_json::from_str(&row.tags_json).unwrap_or_default(),
            relationship_context: row.relationship_context,
            notes: row.notes,
            favorite: row.favorite,
            archived: row.archived,
            has_photo: row.has_photo,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Lists contacts owned by one authenticated account.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when contacts cannot be loaded.
pub async fn list_contacts(pool: &SqlitePool, user_id: &str) -> Result<Vec<Contact>, sqlx::Error> {
    sqlx::query_as::<_, ContactRow>(
        "SELECT id, dav_source_id, source_kind, source_reference, first_name, middle_name, \
         last_name, nickname, pronouns, company, job_title, birthday, emails_json, phones_json, \
         addresses_json, important_dates_json, tags_json, relationship_context, notes, favorite, \
         archived, created_at, updated_at, EXISTS(SELECT 1 FROM contact_photos WHERE contact_photos.contact_id = contacts.id AND contact_photos.user_id = contacts.user_id) AS has_photo FROM contacts WHERE user_id = ? \
         ORDER BY archived ASC, favorite DESC, last_name COLLATE NOCASE ASC, \
         first_name COLLATE NOCASE ASC, nickname COLLATE NOCASE ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map(|rows| rows.into_iter().map(Contact::from).collect())
}

/// Loads one contact when it belongs to the requested account.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the contact cannot be loaded.
pub async fn get_contact(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
) -> Result<Option<Contact>, sqlx::Error> {
    sqlx::query_as::<_, ContactRow>(
        "SELECT id, dav_source_id, source_kind, source_reference, first_name, middle_name, \
         last_name, nickname, pronouns, company, job_title, birthday, emails_json, phones_json, \
         addresses_json, important_dates_json, tags_json, relationship_context, notes, favorite, \
         archived, created_at, updated_at, EXISTS(SELECT 1 FROM contact_photos WHERE contact_photos.contact_id = contacts.id AND contact_photos.user_id = contacts.user_id) AS has_photo FROM contacts WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map(|row| row.map(Contact::from))
}

/// Creates one account-owned contact.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the contact cannot be inserted.
pub async fn create_contact(
    pool: &SqlitePool,
    user_id: &str,
    draft: &ContactDraft,
) -> Result<Contact, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    insert_contact(pool, user_id, &id, draft).await?;
    get_contact(pool, user_id, &id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Updates one account-owned contact without changing its identity.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the contact cannot be updated.
pub async fn update_contact(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
    draft: &ContactDraft,
) -> Result<Option<Contact>, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let values = encoded_values(draft);
    let result = sqlx::query(
        "UPDATE contacts SET dav_source_id = ?, source_kind = ?, source_reference = ?, \
         first_name = ?, middle_name = ?, last_name = ?, nickname = ?, pronouns = ?, company = ?, \
         job_title = ?, birthday = ?, emails_json = ?, phones_json = ?, addresses_json = ?, \
         important_dates_json = ?, tags_json = ?, relationship_context = ?, notes = ?, favorite = ?, \
         archived = ?, updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(&draft.dav_source_id)
    .bind(&draft.source_kind)
    .bind(&draft.source_reference)
    .bind(&draft.first_name)
    .bind(&draft.middle_name)
    .bind(&draft.last_name)
    .bind(&draft.nickname)
    .bind(&draft.pronouns)
    .bind(&draft.company)
    .bind(&draft.job_title)
    .bind(&draft.birthday)
    .bind(&values.0)
    .bind(&values.1)
    .bind(&values.2)
    .bind(&values.3)
    .bind(&values.4)
    .bind(&draft.relationship_context)
    .bind(&draft.notes)
    .bind(draft.favorite)
    .bind(draft.archived)
    .bind(now)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    get_contact(pool, user_id, id).await
}

/// Inserts or updates one imported contact using its stable source reference.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the import cannot be persisted.
pub async fn upsert_imported_contact(
    pool: &SqlitePool,
    user_id: &str,
    draft: &ContactDraft,
) -> Result<Contact, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let values = encoded_values(draft);
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "INSERT INTO contacts (id, user_id, dav_source_id, source_kind, source_reference, \
         first_name, middle_name, last_name, nickname, pronouns, company, job_title, birthday, \
         emails_json, phones_json, addresses_json, important_dates_json, tags_json, \
         relationship_context, notes, favorite, archived, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(user_id, source_kind, source_reference) DO UPDATE SET \
         dav_source_id = excluded.dav_source_id, first_name = excluded.first_name, \
         middle_name = excluded.middle_name, last_name = excluded.last_name, \
         nickname = excluded.nickname, pronouns = excluded.pronouns, company = excluded.company, \
         job_title = excluded.job_title, birthday = excluded.birthday, emails_json = excluded.emails_json, \
         phones_json = excluded.phones_json, addresses_json = excluded.addresses_json, \
         important_dates_json = excluded.important_dates_json, tags_json = excluded.tags_json, \
         relationship_context = excluded.relationship_context, notes = excluded.notes, \
         updated_at = excluded.updated_at",
    )
    .bind(&id)
    .bind(user_id)
    .bind(&draft.dav_source_id)
    .bind(&draft.source_kind)
    .bind(&draft.source_reference)
    .bind(&draft.first_name)
    .bind(&draft.middle_name)
    .bind(&draft.last_name)
    .bind(&draft.nickname)
    .bind(&draft.pronouns)
    .bind(&draft.company)
    .bind(&draft.job_title)
    .bind(&draft.birthday)
    .bind(&values.0)
    .bind(&values.1)
    .bind(&values.2)
    .bind(&values.3)
    .bind(&values.4)
    .bind(&draft.relationship_context)
    .bind(&draft.notes)
    .bind(draft.favorite)
    .bind(draft.archived)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    let source_reference = draft
        .source_reference
        .as_deref()
        .ok_or(sqlx::Error::RowNotFound)?;
    let contact_id: String = sqlx::query_scalar(
        "SELECT id FROM contacts WHERE user_id = ? AND source_kind = ? AND source_reference = ?",
    )
    .bind(user_id)
    .bind(&draft.source_kind)
    .bind(source_reference)
    .fetch_one(&mut *transaction)
    .await?;
    if let Some(photo) = &draft.photo {
        sqlx::query(
            "INSERT INTO contact_photos (contact_id, user_id, mime_type, image_data, updated_at) \
             VALUES (?, ?, ?, ?, ?) ON CONFLICT(contact_id) DO UPDATE SET \
             user_id = excluded.user_id, mime_type = excluded.mime_type, \
             image_data = excluded.image_data, updated_at = excluded.updated_at",
        )
        .bind(&contact_id)
        .bind(user_id)
        .bind(&photo.mime_type)
        .bind(&photo.image_data)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;
    } else {
        sqlx::query("DELETE FROM contact_photos WHERE contact_id = ? AND user_id = ?")
            .bind(&contact_id)
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
    }
    let contact = sqlx::query_as::<_, ContactRow>(
        "SELECT id, dav_source_id, source_kind, source_reference, first_name, middle_name, \
         last_name, nickname, pronouns, company, job_title, birthday, emails_json, phones_json, \
         addresses_json, important_dates_json, tags_json, relationship_context, notes, favorite, \
         archived, created_at, updated_at, EXISTS(SELECT 1 FROM contact_photos WHERE contact_photos.contact_id = contacts.id AND contact_photos.user_id = contacts.user_id) AS has_photo FROM contacts \
         WHERE user_id = ? AND source_kind = ? AND source_reference = ?",
    )
    .bind(user_id)
    .bind(&draft.source_kind)
    .bind(source_reference)
    .fetch_one(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(Contact::from(contact))
}

/// Loads one private contact photo when both the contact and photo belong to the account.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn find_contact_photo(
    pool: &SqlitePool,
    user_id: &str,
    contact_id: &str,
) -> Result<Option<ContactPhoto>, sqlx::Error> {
    sqlx::query_as::<_, ContactPhoto>(
        "SELECT mime_type, image_data, updated_at FROM contact_photos \
         WHERE contact_id = ? AND user_id = ?",
    )
    .bind(contact_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Stores a photo for one account-owned contact.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the photo cannot be stored.
pub async fn upsert_contact_photo(
    pool: &SqlitePool,
    user_id: &str,
    contact_id: &str,
    mime_type: &str,
    image_data: &[u8],
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO contact_photos (contact_id, user_id, mime_type, image_data, updated_at) \
         VALUES (?, ?, ?, ?, ?) ON CONFLICT(contact_id) DO UPDATE SET \
         user_id = excluded.user_id, mime_type = excluded.mime_type, \
         image_data = excluded.image_data, updated_at = excluded.updated_at",
    )
    .bind(contact_id)
    .bind(user_id)
    .bind(mime_type)
    .bind(image_data)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Removes the photo for one account-owned contact.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the photo cannot be removed.
pub async fn delete_contact_photo(
    pool: &SqlitePool,
    user_id: &str,
    contact_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM contact_photos WHERE contact_id = ? AND user_id = ?")
            .bind(contact_id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

/// Deletes one account-owned contact.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_contact(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM contacts WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

/// Lists configured CardDAV resources without exposing credentials.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when sources cannot be loaded.
pub async fn list_dav_sources(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<ContactDavSource>, sqlx::Error> {
    sqlx::query_as::<_, ContactDavSource>(
        "SELECT id, name, url, username, password_ciphertext IS NOT NULL AS has_password, \
         last_synced_at, last_error, created_at, updated_at FROM contact_dav_sources \
         WHERE user_id = ? ORDER BY name COLLATE NOCASE ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Creates a CardDAV resource and stores only an optional encrypted password.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the source cannot be inserted.
pub async fn create_dav_source(
    pool: &SqlitePool,
    user_id: &str,
    name: &str,
    url: &str,
    username: &str,
    password_ciphertext: Option<&str>,
) -> Result<ContactDavSource, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO contact_dav_sources \
         (id, user_id, name, url, username, password_ciphertext, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(name)
    .bind(url)
    .bind(username)
    .bind(password_ciphertext)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    get_dav_source(pool, user_id, &id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Loads one account-owned DAV source without exposing its password.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the source cannot be loaded.
pub async fn get_dav_source(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
) -> Result<Option<ContactDavSource>, sqlx::Error> {
    sqlx::query_as::<_, ContactDavSource>(
        "SELECT id, name, url, username, password_ciphertext IS NOT NULL AS has_password, \
         last_synced_at, last_error, created_at, updated_at FROM contact_dav_sources \
         WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Loads the encrypted password for an account-owned DAV source.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the credential cannot be loaded.
pub async fn get_dav_password(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT password_ciphertext FROM contact_dav_sources WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map(Option::flatten)
}

/// Records a successful or failed DAV sync without exposing upstream response bodies.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the status cannot be stored.
pub async fn set_dav_sync_status(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
    error: Option<&str>,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE contact_dav_sources SET last_synced_at = CASE WHEN ? IS NULL THEN ? ELSE last_synced_at END, \
         last_error = ?, updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(error)
    .bind(&now)
    .bind(error)
    .bind(&now)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Deletes one account-owned DAV source and detaches its imported contacts.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_dav_source(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM contact_dav_sources WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

async fn insert_contact(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
    draft: &ContactDraft,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let values = encoded_values(draft);
    sqlx::query(
        "INSERT INTO contacts (id, user_id, dav_source_id, source_kind, source_reference, \
         first_name, middle_name, last_name, nickname, pronouns, company, job_title, birthday, \
         emails_json, phones_json, addresses_json, important_dates_json, tags_json, \
         relationship_context, notes, favorite, archived, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(user_id)
    .bind(&draft.dav_source_id)
    .bind(&draft.source_kind)
    .bind(&draft.source_reference)
    .bind(&draft.first_name)
    .bind(&draft.middle_name)
    .bind(&draft.last_name)
    .bind(&draft.nickname)
    .bind(&draft.pronouns)
    .bind(&draft.company)
    .bind(&draft.job_title)
    .bind(&draft.birthday)
    .bind(&values.0)
    .bind(&values.1)
    .bind(&values.2)
    .bind(&values.3)
    .bind(&values.4)
    .bind(&draft.relationship_context)
    .bind(&draft.notes)
    .bind(draft.favorite)
    .bind(draft.archived)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

fn encoded_values(draft: &ContactDraft) -> (String, String, String, String, String) {
    (
        serde_json::to_string(&draft.emails).expect("contact methods serialize"),
        serde_json::to_string(&draft.phones).expect("contact methods serialize"),
        serde_json::to_string(&draft.addresses).expect("contact addresses serialize"),
        serde_json::to_string(&draft.important_dates).expect("contact dates serialize"),
        serde_json::to_string(&draft.tags).expect("contact tags serialize"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn contacts_are_scoped_to_the_owning_account() {
        let pool = crate::connect("sqlite::memory:")
            .await
            .expect("database connects");
        for (id, email) in [("user-a", "a@example.com"), ("user-b", "b@example.com")] {
            sqlx::query(
                "INSERT INTO users (id, email, password_hash, role, created_at) VALUES (?, ?, 'hash', 'member', '2026-01-01T00:00:00Z')",
            )
            .bind(id)
            .bind(email)
            .execute(&pool)
            .await
            .expect("user inserts");
        }
        create_contact(&pool, "user-a", &draft("Ada"))
            .await
            .expect("first contact inserts");
        create_contact(&pool, "user-b", &draft("Mara"))
            .await
            .expect("second contact inserts");

        let contacts = list_contacts(&pool, "user-a").await.expect("contacts load");
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].first_name, "Ada");
    }

    #[tokio::test]
    async fn imported_contact_photos_are_private_to_the_owner() {
        let pool = crate::connect("sqlite::memory:")
            .await
            .expect("database connects");
        for (id, email) in [("user-a", "a@example.com"), ("user-b", "b@example.com")] {
            sqlx::query(
                "INSERT INTO users (id, email, password_hash, role, created_at) VALUES (?, ?, 'hash', 'member', '2026-01-01T00:00:00Z')",
            )
            .bind(id)
            .bind(email)
            .execute(&pool)
            .await
            .expect("user inserts");
        }

        let mut imported = draft("Ada");
        imported.source_kind = "monica".to_owned();
        imported.source_reference = Some("monica-ada".to_owned());
        imported.photo = Some(crate::entities::ContactPhotoDraft {
            mime_type: "image/png".to_owned(),
            image_data: vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
        });
        let contact = upsert_imported_contact(&pool, "user-a", &imported)
            .await
            .expect("contact imports");

        assert!(contact.has_photo);
        assert!(
            find_contact_photo(&pool, "user-a", &contact.id)
                .await
                .expect("owner lookup succeeds")
                .is_some()
        );
        assert!(
            find_contact_photo(&pool, "user-b", &contact.id)
                .await
                .expect("other account lookup succeeds")
                .is_none()
        );
    }

    fn draft(first_name: &str) -> ContactDraft {
        ContactDraft {
            dav_source_id: None,
            source_kind: "manual".to_owned(),
            source_reference: None,
            first_name: first_name.to_owned(),
            middle_name: String::new(),
            last_name: String::new(),
            nickname: String::new(),
            pronouns: String::new(),
            company: String::new(),
            job_title: String::new(),
            birthday: None,
            emails: Vec::new(),
            phones: Vec::new(),
            addresses: Vec::new(),
            important_dates: Vec::new(),
            tags: Vec::new(),
            relationship_context: String::new(),
            notes: String::new(),
            favorite: false,
            archived: false,
            photo: None,
        }
    }
}
