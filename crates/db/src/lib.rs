pub mod contact_queries;
pub mod entities;
mod podcast_queries;
pub mod queries;
pub mod wall_queries;
mod youtube_queries;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use std::time::Duration;

/// WAL permits concurrent readers, while `SQLite` still serializes writes.
const POOL_MAX_CONNECTIONS: u32 = 8;

const MIGRATIONS: &[(&str, &str)] = &[
    ("001_initial", include_str!("../migrations/001_initial.sql")),
    (
        "002_dashboard",
        include_str!("../migrations/002_dashboard.sql"),
    ),
    (
        "003_accounts",
        include_str!("../migrations/003_accounts.sql"),
    ),
    ("004_oidc", include_str!("../migrations/004_oidc.sql")),
    (
        "005_onboarding",
        include_str!("../migrations/005_onboarding.sql"),
    ),
    ("006_widgets", include_str!("../migrations/006_widgets.sql")),
    (
        "007_widget_integrations",
        include_str!("../migrations/007_widget_integrations.sql"),
    ),
    (
        "008_user_backgrounds",
        include_str!("../migrations/008_user_backgrounds.sql"),
    ),
    (
        "009_workspaces",
        include_str!("../migrations/009_workspaces.sql"),
    ),
    (
        "010_widget_grid",
        include_str!("../migrations/010_widget_grid.sql"),
    ),
    ("011_tasks", include_str!("../migrations/011_tasks.sql")),
    (
        "012_rss_reader",
        include_str!("../migrations/012_rss_reader.sql"),
    ),
    ("013_journal", include_str!("../migrations/013_journal.sql")),
    (
        "014_journal_documents",
        include_str!("../migrations/014_journal_documents.sql"),
    ),
    (
        "015_youtube_reader",
        include_str!("../migrations/015_youtube_reader.sql"),
    ),
    (
        "016_calendar_subscriptions",
        include_str!("../migrations/016_calendar_subscriptions.sql"),
    ),
    (
        "017_calendar_custom_colors",
        include_str!("../migrations/017_calendar_custom_colors.sql"),
    ),
    (
        "018_task_archiving",
        include_str!("../migrations/018_task_archiving.sql"),
    ),
    (
        "019_coding_projects",
        include_str!("../migrations/019_coding_projects.sql"),
    ),
    (
        "020_dashboard_appearance",
        include_str!("../migrations/020_dashboard_appearance.sql"),
    ),
    (
        "021_user_avatars",
        include_str!("../migrations/021_user_avatars.sql"),
    ),
    (
        "022_wallpaper_slots",
        include_str!("../migrations/022_wallpaper_slots.sql"),
    ),
    (
        "023_loading_wallpaper",
        include_str!("../migrations/023_loading_wallpaper.sql"),
    ),
    (
        "024_contacts",
        include_str!("../migrations/024_contacts.sql"),
    ),
    (
        "025_contact_photos",
        include_str!("../migrations/025_contact_photos.sql"),
    ),
    (
        "026_subscription_costs",
        include_str!("../migrations/026_subscription_costs.sql"),
    ),
    (
        "027_youtube_channel_thumbnails",
        include_str!("../migrations/027_youtube_channel_thumbnails.sql"),
    ),
    (
        "028_youtube_channel_thumbnail_repair",
        include_str!("../migrations/028_youtube_channel_thumbnail_repair.sql"),
    ),
    (
        "029_yearless_contact_birthdays",
        include_str!("../migrations/029_yearless_contact_birthdays.sql"),
    ),
    (
        "030_bible_verse_widget",
        include_str!("../migrations/030_bible_verse_widget.sql"),
    ),
    (
        "031_sidebar_timezones",
        include_str!("../migrations/031_sidebar_timezones.sql"),
    ),
    (
        "032_authentication_settings",
        include_str!("../migrations/032_authentication_settings.sql"),
    ),
    ("033_lines", include_str!("../migrations/033_lines.sql")),
    ("034_kanban", include_str!("../migrations/034_kanban.sql")),
    (
        "035_read_later",
        include_str!("../migrations/035_read_later.sql"),
    ),
    (
        "036_remove_search_widget",
        include_str!("../migrations/036_remove_search_widget.sql"),
    ),
    (
        "037_rss_refresh_schedule",
        include_str!("../migrations/037_rss_refresh_schedule.sql"),
    ),
    (
        "038_podcasts",
        include_str!("../migrations/038_podcasts.sql"),
    ),
    ("039_walls", include_str!("../migrations/039_walls.sql")),
    (
        "040_embedded_pages",
        include_str!("../migrations/040_embedded_pages.sql"),
    ),
    (
        "041_embedded_page_trust",
        include_str!("../migrations/041_embedded_page_trust.sql"),
    ),
    (
        "042_embedded_page_height",
        include_str!("../migrations/042_embedded_page_height.sql"),
    ),
];

/// Maps migration names used by earlier development builds to their canonical names.
///
/// The wallpaper migration originally shipped as `021_wallpaper_slots` before the avatar
/// migration was inserted ahead of it. The Read Later migration was briefly numbered `034`
/// before the Kanban migration claimed that number. Existing development databases may contain
/// either legacy ledger entry and must not reapply the corresponding schema.
const MIGRATION_ALIASES: &[(&str, &str)] = &[
    ("022_wallpaper_slots", "021_wallpaper_slots"),
    ("035_read_later", "034_read_later"),
];

/// Opens the configured `SQLite` pool and applies all pending migrations.
///
/// # Errors
///
/// Returns an `SQLx` error when the URL is invalid, the database cannot be opened, or a
/// migration fails.
pub async fn connect(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let max_connections = if database_url.contains(":memory:") {
        1
    } else {
        POOL_MAX_CONNECTIONS
    };
    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await?;

    run_migrations(&pool).await?;
    Ok(pool)
}

/// Confirms that the database can execute a query.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the database is unavailable.
pub async fn health_check(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT 1").execute(pool).await?;
    Ok(())
}

async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS _migrations (\
         name TEXT PRIMARY KEY, \
         applied_at TEXT NOT NULL\
         )",
    )
    .execute(pool)
    .await?;

    reconcile_migration_aliases(pool).await?;

    for (name, migration_sql) in MIGRATIONS {
        let applied: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM _migrations WHERE name = ?)")
                .bind(*name)
                .fetch_one(pool)
                .await?;

        if !applied {
            let mut transaction = pool.begin().await?;
            sqlx::raw_sql(*migration_sql)
                .execute(&mut *transaction)
                .await?;
            sqlx::query("INSERT INTO _migrations (name, applied_at) VALUES (?, ?)")
                .bind(*name)
                .bind(chrono::Utc::now().to_rfc3339())
                .execute(&mut *transaction)
                .await?;
            transaction.commit().await?;
        }
    }
    let mut transaction = pool.begin().await?;
    repair_youtube_channel_thumbnail_columns(&mut transaction).await?;
    transaction.commit().await?;

    Ok(())
}

async fn repair_youtube_channel_thumbnail_columns(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> Result<(), sqlx::Error> {
    let existing =
        sqlx::query_scalar::<_, String>("SELECT name FROM pragma_table_info('youtube_channels')")
            .fetch_all(&mut **transaction)
            .await?;
    let columns = [
        (
            "thumbnail_url",
            "ALTER TABLE youtube_channels ADD COLUMN thumbnail_url TEXT NOT NULL DEFAULT ''",
        ),
        (
            "thumbnail_fetched_at",
            "ALTER TABLE youtube_channels ADD COLUMN thumbnail_fetched_at TEXT",
        ),
        (
            "thumbnail_content_type",
            "ALTER TABLE youtube_channels ADD COLUMN thumbnail_content_type TEXT NOT NULL DEFAULT ''",
        ),
        (
            "thumbnail_data",
            "ALTER TABLE youtube_channels ADD COLUMN thumbnail_data BLOB",
        ),
    ];
    for (column, statement) in columns {
        if !existing.iter().any(|existing| existing == column) {
            sqlx::query(statement).execute(&mut **transaction).await?;
        }
    }
    Ok(())
}

async fn reconcile_migration_aliases(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    for (canonical_name, legacy_name) in MIGRATION_ALIASES {
        sqlx::query(
            "INSERT OR IGNORE INTO _migrations (name, applied_at) \
             SELECT ?, applied_at FROM _migrations WHERE name = ?",
        )
        .bind(*canonical_name)
        .bind(*legacy_name)
        .execute(pool)
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_are_applied_once() {
        let pool = connect("sqlite::memory:").await.expect("database connects");

        run_migrations(&pool).await.expect("migrations rerun");

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _migrations")
            .fetch_one(&pool)
            .await
            .expect("migration count loads");
        assert_eq!(
            usize::try_from(count).expect("migration count fits usize"),
            MIGRATIONS.len()
        );
    }

    #[tokio::test]
    async fn the_walls_migration_applies_over_an_existing_database() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str("sqlite::memory:")
                    .expect("memory url parses")
                    .foreign_keys(true),
            )
            .await
            .expect("database connects");
        sqlx::raw_sql(
            "CREATE TABLE IF NOT EXISTS _migrations (\
             name TEXT PRIMARY KEY, applied_at TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .expect("migration ledger creates");

        // Seed a database at the release before Walls, with a wallpaper already uploaded.
        let walls_index = MIGRATIONS
            .iter()
            .position(|(name, _)| *name == "039_walls")
            .expect("walls migration is registered");
        for (name, migration_sql) in MIGRATIONS.iter().take(walls_index) {
            let mut transaction = pool.begin().await.expect("migration starts");
            sqlx::raw_sql(*migration_sql)
                .execute(&mut *transaction)
                .await
                .expect("existing migration applies");
            sqlx::query("INSERT INTO _migrations (name, applied_at) VALUES (?, ?)")
                .bind(*name)
                .bind(chrono::Utc::now().to_rfc3339())
                .execute(&mut *transaction)
                .await
                .expect("existing migration records");
            transaction.commit().await.expect("migration commits");
        }

        let (user, _) = queries::create_account(
            &pool,
            "walls-upgrade@example.com",
            "$argon2id$upgrade",
            "Walls Upgrade",
        )
        .await
        .expect("account creates");
        sqlx::query(
            "INSERT INTO user_wallpapers (user_id, slot, mime_type, image_data, updated_at) \
             VALUES (?, 'welcome', 'image/png', ?, ?)",
        )
        .bind(&user.id)
        .bind(b"existing-wallpaper".as_slice())
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&pool)
        .await
        .expect("existing wallpaper stores");

        run_migrations(&pool)
            .await
            .expect("walls migration applies");

        // The upgrade preserves the uploaded image and leaves the slot resolving to it.
        let preserved = queries::find_user_wallpaper(&pool, &user.id, "welcome")
            .await
            .expect("wallpaper loads")
            .expect("wallpaper survives the upgrade");
        assert_eq!(preserved.image_data, b"existing-wallpaper");
        assert!(
            wall_queries::list_wall_selections(&pool, &user.id)
                .await
                .expect("selections load")
                .is_empty()
        );
        assert!(
            wall_queries::find_login_wall_selection(&pool)
                .await
                .expect("login selection loads")
                .is_none()
        );
    }

    #[tokio::test]
    async fn embedded_page_height_migration_defaults_existing_pages() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str("sqlite::memory:")
                    .expect("memory url parses")
                    .foreign_keys(true),
            )
            .await
            .expect("database connects");
        sqlx::raw_sql(
            "CREATE TABLE IF NOT EXISTS _migrations (\
             name TEXT PRIMARY KEY, applied_at TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .expect("migration ledger creates");

        let height_index = MIGRATIONS
            .iter()
            .position(|(name, _)| *name == "042_embedded_page_height")
            .expect("height migration is registered");
        for (name, migration_sql) in MIGRATIONS.iter().take(height_index) {
            let mut transaction = pool.begin().await.expect("migration starts");
            sqlx::raw_sql(*migration_sql)
                .execute(&mut *transaction)
                .await
                .expect("existing migration applies");
            sqlx::query("INSERT INTO _migrations (name, applied_at) VALUES (?, ?)")
                .bind(*name)
                .bind(chrono::Utc::now().to_rfc3339())
                .execute(&mut *transaction)
                .await
                .expect("existing migration records");
            transaction.commit().await.expect("migration commits");
        }

        let (owner, _) = queries::create_account(
            &pool,
            "embedded-height-upgrade@example.com",
            "$argon2id$upgrade",
            "Embedded Height Upgrade",
        )
        .await
        .expect("owner creates");
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO embedded_pages \
             (id, scope, owner_user_id, created_by_user_id, title, description, url, \
              allow_same_origin, position, created_at, updated_at) \
             VALUES ('legacy-embedded-page', 'user', ?, ?, 'Legacy', '', \
                     'https://example.com/', 0, 0, ?, ?)",
        )
        .bind(&owner.id)
        .bind(&owner.id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("legacy embedded page stores");

        run_migrations(&pool)
            .await
            .expect("height migration applies");
        let iframe_height: i64 = sqlx::query_scalar(
            "SELECT iframe_height FROM embedded_pages WHERE id = 'legacy-embedded-page'",
        )
        .fetch_one(&pool)
        .await
        .expect("migrated height loads");

        assert_eq!(iframe_height, 720);
    }

    #[tokio::test]
    async fn embedded_pages_are_scoped_ordered_and_deleted_with_their_owner() {
        let pool = connect("sqlite::memory:").await.expect("database connects");
        let (creator, _) = queries::create_account(
            &pool,
            "embedded-creator@example.com",
            "$argon2id$creator",
            "Creator",
        )
        .await
        .expect("creator account creates");
        let (owner, _) = queries::create_account(
            &pool,
            "embedded-owner@example.com",
            "$argon2id$owner",
            "Owner",
        )
        .await
        .expect("owner account creates");
        let (other, _) = queries::create_account(
            &pool,
            "embedded-other@example.com",
            "$argon2id$other",
            "Other",
        )
        .await
        .expect("other account creates");

        let global = queries::create_global_embedded_page(
            &pool,
            &creator.id,
            "Status",
            "Instance status",
            "https://status.example.com/",
            false,
            720,
        )
        .await
        .expect("global page creates");
        let first = queries::create_personal_embedded_page(
            &pool,
            &owner.id,
            "Notes",
            "Private notes",
            "https://notes.example.com/",
            true,
            960,
        )
        .await
        .expect("first personal page creates");
        let second = queries::create_personal_embedded_page(
            &pool,
            &owner.id,
            "Reports",
            "Private reports",
            "https://reports.example.com/",
            false,
            640,
        )
        .await
        .expect("second personal page creates");
        let foreign = queries::create_personal_embedded_page(
            &pool,
            &other.id,
            "Other",
            "Another account's page",
            "https://other.example.com/",
            false,
            720,
        )
        .await
        .expect("foreign personal page creates");

        let owner_pages = queries::list_personal_embedded_pages(&pool, &owner.id)
            .await
            .expect("owner pages load");
        assert_eq!(
            owner_pages
                .iter()
                .map(|page| page.id.as_str())
                .collect::<Vec<_>>(),
            vec![first.id.as_str(), second.id.as_str()]
        );
        assert!(owner_pages[0].allow_same_origin);
        assert!(!owner_pages[1].allow_same_origin);
        assert_eq!(owner_pages[0].iframe_height, 960);
        assert_eq!(owner_pages[1].iframe_height, 640);
        assert!(
            owner_pages.iter().all(|page| page.id != foreign.id),
            "another account's page never appears"
        );

        let reordered = queries::reorder_personal_embedded_pages(
            &pool,
            &owner.id,
            &[second.id.clone(), first.id.clone()],
        )
        .await
        .expect("personal pages reorder")
        .expect("complete owner order is accepted");
        assert_eq!(reordered[0].id, second.id);
        assert!(
            queries::reorder_personal_embedded_pages(
                &pool,
                &owner.id,
                &[first.id.clone(), foreign.id.clone()],
            )
            .await
            .expect("foreign reorder is evaluated")
            .is_none(),
            "a foreign identifier rejects the complete reorder"
        );

        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(&owner.id)
            .execute(&pool)
            .await
            .expect("owner deletes");
        assert!(
            queries::list_personal_embedded_pages(&pool, &owner.id)
                .await
                .expect("deleted owner pages query")
                .is_empty()
        );

        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(&creator.id)
            .execute(&pool)
            .await
            .expect("global creator deletes");
        let global_pages = queries::list_global_embedded_pages(&pool)
            .await
            .expect("global pages load");
        assert_eq!(global_pages.len(), 1);
        assert_eq!(global_pages[0].id, global.id);
        assert!(global_pages[0].created_by_user_id.is_none());
    }

    #[tokio::test]
    async fn authentication_settings_default_to_enabled_and_can_be_updated() {
        let pool = connect("sqlite::memory:").await.expect("database connects");

        let defaults = queries::get_authentication_settings(&pool)
            .await
            .expect("authentication settings load");
        assert!(defaults.password_login_enabled);
        assert!(defaults.password_registration_enabled);
        assert!(defaults.oidc_registration_enabled);

        let updated = queries::update_authentication_settings(&pool, true, false, false)
            .await
            .expect("authentication settings update");
        assert!(updated.password_login_enabled);
        assert!(!updated.password_registration_enabled);
        assert!(!updated.oidc_registration_enabled);
    }

    #[tokio::test]
    async fn bible_verse_widget_migration_preserves_existing_widgets_and_secrets() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("memory database URL parses")
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("database connects");

        for (_, migration) in MIGRATIONS.iter().take(29) {
            sqlx::raw_sql(*migration)
                .execute(&pool)
                .await
                .expect("legacy migration applies");
        }
        let (user, _) = queries::create_account(
            &pool,
            "verse-upgrade@example.com",
            "$argon2id$upgrade",
            "Verse Upgrade",
        )
        .await
        .expect("legacy account creates");
        let existing = queries::create_dashboard_widget(&pool, &user.id, "youtube", 0, "standard")
            .await
            .expect("legacy widget creates");
        queries::upsert_widget_secret(&pool, &user.id, &existing.id, "encrypted-token")
            .await
            .expect("legacy widget secret stores");

        sqlx::raw_sql(MIGRATIONS[29].1)
            .execute(&pool)
            .await
            .expect("Bible verse widget migration applies");

        let preserved = queries::get_dashboard_widget(&pool, &user.id, &existing.id)
            .await
            .expect("existing widget loads")
            .expect("existing widget remains");
        assert_eq!(preserved.kind, "youtube");
        assert_eq!(
            queries::get_widget_secret(&pool, &user.id, &existing.id)
                .await
                .expect("existing widget secret loads")
                .as_deref(),
            Some("encrypted-token")
        );

        let verse = queries::create_dashboard_widget(&pool, &user.id, "bible-verse", 0, "standard")
            .await
            .expect("Bible verse widget creates");
        assert_eq!(verse.kind, "bible-verse");
    }

    #[tokio::test]
    async fn sidebar_timezone_migration_seeds_the_existing_primary_timezone() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("database connects");

        for (_, migration) in MIGRATIONS.iter().take(30) {
            sqlx::raw_sql(*migration)
                .execute(&pool)
                .await
                .expect("legacy migration applies");
        }
        let (user, _) = queries::create_account(
            &pool,
            "sidebar-timezones@example.com",
            "$argon2id$upgrade",
            "Sidebar Timezones",
        )
        .await
        .expect("legacy account creates");
        sqlx::query("UPDATE user_settings SET timezone = 'Europe/London' WHERE user_id = ?")
            .bind(&user.id)
            .execute(&pool)
            .await
            .expect("legacy timezone updates");

        sqlx::raw_sql(MIGRATIONS[30].1)
            .execute(&pool)
            .await
            .expect("sidebar timezone migration applies");

        let stored: String = sqlx::query_scalar(
            "SELECT sidebar_timezones_json FROM user_settings WHERE user_id = ?",
        )
        .bind(&user.id)
        .fetch_one(&pool)
        .await
        .expect("sidebar timezones load");
        assert_eq!(stored, r#"["Europe/London"]"#);
    }

    #[tokio::test]
    async fn search_widget_migration_removes_instances_and_preserves_the_rest() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("memory database URL parses")
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("database connects");

        for (_, migration) in MIGRATIONS.iter().take(35) {
            sqlx::raw_sql(*migration)
                .execute(&pool)
                .await
                .expect("legacy migration applies");
        }
        let (user, _) = queries::create_account(
            &pool,
            "search-removal@example.com",
            "$argon2id$upgrade",
            "Search Removal",
        )
        .await
        .expect("legacy account creates");
        let kept = queries::create_dashboard_widget(&pool, &user.id, "youtube", 0, "standard")
            .await
            .expect("legacy widget creates");
        queries::upsert_widget_secret(&pool, &user.id, &kept.id, "encrypted-token")
            .await
            .expect("legacy widget secret stores");
        // The seed no longer places this kind, so recreate the legacy row directly.
        sqlx::query(
            "INSERT INTO dashboard_widgets (id, user_id, kind, workspace, position, size, config_json, grid_x, grid_y, grid_w, grid_h, created_at, updated_at) \
             VALUES (?, ?, 'search', 0, 99, 'standard', '{}', 0, 5, 6, 4, datetime('now'), datetime('now'))",
        )
        .bind(format!("{}-search", user.id))
        .bind(&user.id)
        .execute(&pool)
        .await
        .expect("legacy search widget inserts");

        sqlx::raw_sql(MIGRATIONS[35].1)
            .execute(&pool)
            .await
            .expect("search widget removal migration applies");

        let widgets = queries::list_dashboard_widgets(&pool, &user.id)
            .await
            .expect("widgets load");
        assert!(widgets.iter().all(|widget| widget.kind != "search"));

        let preserved = queries::get_dashboard_widget(&pool, &user.id, &kept.id)
            .await
            .expect("existing widget loads")
            .expect("existing widget remains");
        assert_eq!(preserved.kind, "youtube");
        assert_eq!(
            queries::get_widget_secret(&pool, &user.id, &kept.id)
                .await
                .expect("existing widget secret loads")
                .as_deref(),
            Some("encrypted-token")
        );

        let mut positions: Vec<i64> = widgets.iter().map(|widget| widget.position).collect();
        positions.sort_unstable();
        assert_eq!(
            positions,
            (0..i64::try_from(widgets.len()).expect("widget count fits i64")).collect::<Vec<_>>()
        );

        assert!(
            queries::create_dashboard_widget(&pool, &user.id, "search", 0, "standard")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn yearless_contact_birthday_notes_are_recovered() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("database connects");
        sqlx::raw_sql(
            "CREATE TABLE contacts (birthday TEXT, notes TEXT NOT NULL); \
             INSERT INTO contacts (birthday, notes) \
             VALUES (NULL, 'Birthday (year unknown): 08-20');",
        )
        .execute(&pool)
        .await
        .expect("legacy contact creates");

        sqlx::raw_sql(include_str!(
            "../migrations/029_yearless_contact_birthdays.sql"
        ))
        .execute(&pool)
        .await
        .expect("yearless birthday migration runs");

        let birthday: Option<String> = sqlx::query_scalar("SELECT birthday FROM contacts LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("birthday loads");
        assert_eq!(birthday.as_deref(), Some("--08-20"));
    }

    #[tokio::test]
    async fn partial_youtube_thumbnail_migration_is_repaired() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("database connects");
        sqlx::raw_sql(
            "CREATE TABLE _migrations (name TEXT PRIMARY KEY, applied_at TEXT NOT NULL); \
             CREATE TABLE youtube_channels (channel_id TEXT PRIMARY KEY, \
             thumbnail_url TEXT NOT NULL DEFAULT '', \
             thumbnail_fetched_at TEXT)",
        )
        .execute(&pool)
        .await
        .expect("partial channel schema creates");

        sqlx::query("INSERT INTO youtube_channels (channel_id) VALUES (?)")
            .bind("UCabcdefghijklmnopqrstuv")
            .execute(&pool)
            .await
            .expect("existing channel stores");

        for (name, _) in MIGRATIONS.iter().take(MIGRATIONS.len()) {
            sqlx::query("INSERT INTO _migrations (name, applied_at) VALUES (?, ?)")
                .bind(*name)
                .bind(chrono::Utc::now().to_rfc3339())
                .execute(&pool)
                .await
                .expect("existing migration records");
        }

        run_migrations(&pool)
            .await
            .expect("partial schema repairs through migration runner");

        let columns = sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_table_info('youtube_channels')",
        )
        .fetch_all(&pool)
        .await
        .expect("channel columns load");
        for expected in [
            "thumbnail_url",
            "thumbnail_fetched_at",
            "thumbnail_content_type",
            "thumbnail_data",
        ] {
            assert!(columns.iter().any(|column| column == expected));
        }
        let preserved: String =
            sqlx::query_scalar("SELECT channel_id FROM youtube_channels LIMIT 1")
                .fetch_one(&pool)
                .await
                .expect("existing channel remains");
        assert_eq!(preserved, "UCabcdefghijklmnopqrstuv");
    }

    #[tokio::test]
    async fn legacy_wallpaper_migration_name_is_reconciled() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("database connects");
        sqlx::raw_sql("CREATE TABLE _migrations (name TEXT PRIMARY KEY, applied_at TEXT NOT NULL)")
            .execute(&pool)
            .await
            .expect("migration ledger creates");

        for (name, migration_sql) in MIGRATIONS.iter().take(20) {
            let mut transaction = pool.begin().await.expect("migration starts");
            sqlx::raw_sql(*migration_sql)
                .execute(&mut *transaction)
                .await
                .expect("legacy migration applies");
            sqlx::query("INSERT INTO _migrations (name, applied_at) VALUES (?, ?)")
                .bind(*name)
                .bind(chrono::Utc::now().to_rfc3339())
                .execute(&mut *transaction)
                .await
                .expect("legacy migration records");
            transaction.commit().await.expect("migration commits");
        }

        let (user, _) = queries::create_account(
            &pool,
            "wallpaper-upgrade@example.com",
            "$argon2id$upgrade",
            "Wallpaper Upgrade",
        )
        .await
        .expect("account creates");
        sqlx::raw_sql(MIGRATIONS[21].1)
            .execute(&pool)
            .await
            .expect("legacy wallpaper migration applies");
        sqlx::query("INSERT INTO _migrations (name, applied_at) VALUES (?, ?)")
            .bind("021_wallpaper_slots")
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(&pool)
            .await
            .expect("legacy wallpaper migration records");
        // Written as the build of that era wrote it. Today's `upsert_user_wallpaper` also
        // clears `user_wallpaper_selections`, which does not exist at this schema version.
        sqlx::query(
            "INSERT INTO user_wallpapers (user_id, slot, mime_type, image_data, updated_at) \
             VALUES (?, 'dashboard', 'image/png', ?, ?)",
        )
        .bind(&user.id)
        .bind(b"legacy-wallpaper".as_slice())
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&pool)
        .await
        .expect("legacy wallpaper stores");

        run_migrations(&pool)
            .await
            .expect("renumbered migrations apply");

        let canonical_recorded: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM _migrations WHERE name = ?)")
                .bind("022_wallpaper_slots")
                .fetch_one(&pool)
                .await
                .expect("canonical migration record loads");
        assert!(canonical_recorded);
        let stored_wallpaper = queries::find_user_wallpaper(&pool, &user.id, "dashboard")
            .await
            .expect("migrated wallpaper loads")
            .expect("migrated wallpaper remains");
        assert_eq!(stored_wallpaper.image_data, b"legacy-wallpaper");
        queries::upsert_user_wallpaper(
            &pool,
            &user.id,
            "loading",
            "image/png",
            b"loading-wallpaper",
        )
        .await
        .expect("loading wallpaper stores after upgrade");
    }

    #[tokio::test]
    async fn legacy_read_later_migration_name_is_reconciled() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("database connects");
        sqlx::raw_sql("CREATE TABLE _migrations (name TEXT PRIMARY KEY, applied_at TEXT NOT NULL)")
            .execute(&pool)
            .await
            .expect("migration ledger creates");

        let read_later_index = MIGRATIONS
            .iter()
            .position(|(name, _)| *name == "035_read_later")
            .expect("Read Later migration exists");
        for (name, migration_sql) in MIGRATIONS.iter().take(read_later_index) {
            let mut transaction = pool.begin().await.expect("migration starts");
            sqlx::raw_sql(*migration_sql)
                .execute(&mut *transaction)
                .await
                .expect("migration applies");
            sqlx::query("INSERT INTO _migrations (name, applied_at) VALUES (?, ?)")
                .bind(*name)
                .bind(chrono::Utc::now().to_rfc3339())
                .execute(&mut *transaction)
                .await
                .expect("migration records");
            transaction.commit().await.expect("migration commits");
        }

        sqlx::raw_sql(MIGRATIONS[read_later_index].1)
            .execute(&pool)
            .await
            .expect("legacy Read Later migration applies");
        sqlx::query("INSERT INTO _migrations (name, applied_at) VALUES (?, ?)")
            .bind("034_read_later")
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(&pool)
            .await
            .expect("legacy Read Later migration records");

        run_migrations(&pool)
            .await
            .expect("renamed Read Later migration reconciles");

        let canonical_recorded: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM _migrations WHERE name = ?)")
                .bind("035_read_later")
                .fetch_one(&pool)
                .await
                .expect("canonical migration record loads");
        assert!(canonical_recorded);
        let tables: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' \
             AND name IN ('rss_read_later', 'youtube_watch_later')",
        )
        .fetch_one(&pool)
        .await
        .expect("Read Later tables remain");
        assert_eq!(tables, 2);
    }

    #[tokio::test]
    async fn imported_avatar_never_replaces_an_existing_avatar() {
        let pool = connect("sqlite::memory:").await.expect("database connects");
        let (user, _) = queries::create_account(
            &pool,
            "avatar-import@example.com",
            "$argon2id$avatar-import",
            "Avatar Import",
        )
        .await
        .expect("account creates");

        assert!(!queries::has_user_avatar(&pool, &user.id).await.unwrap());
        assert!(
            queries::insert_user_avatar_if_absent(
                &pool,
                &user.id,
                "image/png",
                b"provider-avatar",
            )
            .await
            .expect("provider avatar stores")
        );
        assert!(queries::has_user_avatar(&pool, &user.id).await.unwrap());
        assert!(
            !queries::insert_user_avatar_if_absent(
                &pool,
                &user.id,
                "image/jpeg",
                b"replacement-avatar",
            )
            .await
            .expect("duplicate provider avatar is ignored")
        );

        let avatar = queries::find_user_avatar(&pool, &user.id)
            .await
            .expect("avatar loads")
            .expect("avatar exists");
        assert_eq!(avatar.mime_type, "image/png");
        assert_eq!(avatar.image_data, b"provider-avatar");
    }

    #[tokio::test]
    async fn lines_visibility_replies_reactions_and_attachments_are_scoped() {
        let pool = connect("sqlite::memory:").await.expect("database connects");
        let (alice, alice_settings) = queries::create_account(
            &pool,
            "alice-lines@example.com",
            "$argon2id$alice-lines",
            "Alice Lines",
        )
        .await
        .expect("Alice account creates");
        let (bob, _) = queries::create_account(
            &pool,
            "bob-lines@example.com",
            "$argon2id$bob-lines",
            "Bob Lines",
        )
        .await
        .expect("Bob account creates");
        assert_eq!(alice_settings.lines_default_visibility, "private");

        let public = queries::create_line_post(
            &pool,
            &alice.id,
            &entities::LinePostDraft {
                content: "Public note #rust".to_owned(),
                visibility: "public".to_owned(),
                reply_to_post_id: None,
                tags: vec!["rust".to_owned()],
            },
        )
        .await
        .expect("public post creates");
        let private = queries::create_line_post(
            &pool,
            &alice.id,
            &entities::LinePostDraft {
                content: "Private note #secret".to_owned(),
                visibility: "private".to_owned(),
                reply_to_post_id: None,
                tags: vec!["secret".to_owned()],
            },
        )
        .await
        .expect("private post creates");

        let bob_feed = queries::list_line_posts(&pool, &bob.id, "instance", "", "")
            .await
            .expect("Bob feed loads");
        assert_eq!(bob_feed.len(), 1);
        assert_eq!(bob_feed[0].id, public.id);
        assert!(
            queries::get_line_post(&pool, &bob.id, &private.id)
                .await
                .expect("private lookup completes")
                .is_none()
        );

        assert!(
            queries::add_line_post_reaction(&pool, &bob.id, &public.id, "👍")
                .await
                .expect("reaction stores")
        );
        assert!(
            !queries::add_line_post_reaction(&pool, &bob.id, &private.id, "👍")
                .await
                .expect("private reaction is rejected")
        );
        let attachment = queries::create_line_post_attachment(
            &pool,
            &alice.id,
            &private.id,
            "secret.txt",
            "text/plain",
            b"private bytes",
        )
        .await
        .expect("attachment stores")
        .expect("Alice owns post");
        assert!(
            queries::get_line_post_attachment(&pool, &bob.id, &private.id, &attachment.id)
                .await
                .expect("private attachment lookup completes")
                .is_none()
        );

        assert!(
            queries::delete_line_post(&pool, &bob.id, &public.id, true)
                .await
                .expect("administrator public delete completes")
        );
        assert!(
            !queries::delete_line_post(&pool, &bob.id, &private.id, true)
                .await
                .expect("administrator private delete is rejected")
        );
    }

    #[tokio::test]
    async fn custom_color_migration_maps_existing_presets() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("database connects");
        sqlx::raw_sql("CREATE TABLE _migrations (name TEXT PRIMARY KEY, applied_at TEXT NOT NULL)")
            .execute(&pool)
            .await
            .expect("migration ledger creates");
        for (name, migration_sql) in MIGRATIONS.iter().take(16) {
            let mut transaction = pool.begin().await.expect("migration starts");
            sqlx::raw_sql(*migration_sql)
                .execute(&mut *transaction)
                .await
                .expect("legacy migration applies");
            sqlx::query("INSERT INTO _migrations (name, applied_at) VALUES (?, ?)")
                .bind(*name)
                .bind(chrono::Utc::now().to_rfc3339())
                .execute(&mut *transaction)
                .await
                .expect("legacy migration records");
            transaction.commit().await.expect("migration commits");
        }
        let (user, _) = queries::create_account(
            &pool,
            "calendar-upgrade@example.com",
            "$argon2id$upgrade",
            "Calendar Upgrade",
        )
        .await
        .expect("account creates");
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO calendar_subscriptions \
             (id, user_id, url, name, color, created_at, updated_at) \
             VALUES ('legacy-calendar', ?, 'https://example.com/calendar.ics', \
             'Legacy calendar', 'amber', ?, ?)",
        )
        .bind(&user.id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("legacy calendar inserts");

        run_migrations(&pool)
            .await
            .expect("custom color migration applies");

        let color: String = sqlx::query_scalar(
            "SELECT color_value FROM calendar_subscriptions WHERE id = 'legacy-calendar'",
        )
        .fetch_one(&pool)
        .await
        .expect("migrated color loads");
        assert_eq!(color, "#FBBF24");
    }

    #[tokio::test]
    async fn metadata_can_be_upserted() {
        let pool = connect("sqlite::memory:").await.expect("database connects");

        queries::upsert_metadata(&pool, "example", "first")
            .await
            .expect("metadata inserts");
        let updated = queries::upsert_metadata(&pool, "example", "second")
            .await
            .expect("metadata updates");
        let stored = queries::fetch_metadata(&pool, "example")
            .await
            .expect("metadata loads");

        assert_eq!(stored, Some(updated));
    }

    #[tokio::test]
    async fn rss_items_are_user_owned_and_pruned_by_read_state() {
        let pool = connect("sqlite::memory:").await.expect("database connects");
        let (owner, _) =
            queries::create_account(&pool, "reader@example.com", "$argon2id$reader", "Reader")
                .await
                .expect("reader creates");
        let (other, _) =
            queries::create_account(&pool, "other@example.com", "$argon2id$other", "Other")
                .await
                .expect("other reader creates");
        let subscription = queries::create_rss_subscription(
            &pool,
            &owner.id,
            &entities::RssSubscriptionDraft {
                url: "https://example.com/feed.xml".to_owned(),
                base_url: "https://example.com".to_owned(),
                title: "Example feed".to_owned(),
                category: "Research".to_owned(),
                auto_delete_days: None,
                auto_delete_mode: "read".to_owned(),
            },
            &[entities::RssItemDraft {
                external_id: "old-entry".to_owned(),
                url: "https://example.com/old".to_owned(),
                title: "Old entry".to_owned(),
                summary: String::new(),
                published_at: "2000-01-01T00:00:00Z".to_owned(),
            }],
        )
        .await
        .expect("subscription creates");

        assert_eq!(
            queries::list_rss_items(&pool, &owner.id)
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(
            queries::list_rss_items(&pool, &other.id)
                .await
                .unwrap()
                .is_empty()
        );
        let item = queries::list_rss_items(&pool, &owner.id)
            .await
            .unwrap()
            .remove(0);
        assert!(
            queries::set_rss_item_read(&pool, &other.id, &item.id, true)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            queries::set_rss_item_read(&pool, &owner.id, &item.id, true)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            queries::set_rss_item_saved(&pool, &other.id, &item.id, true)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            queries::set_rss_item_saved(&pool, &owner.id, &item.id, true)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(
            queries::prune_rss_items(&pool, &owner.id, 30, "read")
                .await
                .unwrap(),
            0
        );
        assert!(
            queries::set_rss_item_saved(&pool, &owner.id, &item.id, false)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(
            queries::prune_rss_items(&pool, &owner.id, 30, "read")
                .await
                .unwrap(),
            1
        );
        assert!(
            queries::list_rss_items(&pool, &owner.id)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            queries::get_rss_subscription(&pool, &owner.id, &subscription.id)
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn due_rss_subscriptions_are_claimed_once_per_refresh_window() {
        let pool = connect("sqlite::memory:").await.expect("database connects");
        let (owner, _) =
            queries::create_account(&pool, "schedule@example.com", "$argon2id$sched", "Reader")
                .await
                .expect("reader creates");
        let subscription = queries::create_rss_subscription(
            &pool,
            &owner.id,
            &entities::RssSubscriptionDraft {
                url: "https://example.com/feed.xml".to_owned(),
                base_url: "https://example.com".to_owned(),
                title: "Example feed".to_owned(),
                category: "Research".to_owned(),
                auto_delete_days: None,
                auto_delete_mode: "read".to_owned(),
            },
            &[],
        )
        .await
        .expect("subscription creates");

        let aged = (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
        sqlx::query(
            "UPDATE rss_subscriptions SET last_fetched_at = ?, last_attempted_at = ? WHERE id = ?",
        )
        .bind(&aged)
        .bind(&aged)
        .bind(&subscription.id)
        .execute(&pool)
        .await
        .expect("subscription ages");

        let not_due_before = (chrono::Utc::now() - chrono::Duration::hours(3)).to_rfc3339();
        assert!(
            queries::list_due_rss_subscriptions(&pool, &not_due_before, 10)
                .await
                .expect("due subscriptions load")
                .is_empty()
        );

        let due_before = (chrono::Utc::now() - chrono::Duration::minutes(30)).to_rfc3339();
        let due = queries::list_due_rss_subscriptions(&pool, &due_before, 10)
            .await
            .expect("due subscriptions load");
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, subscription.id);
        assert_eq!(due[0].user_id, owner.id);
        assert_eq!(due[0].url, "https://example.com/feed.xml");

        assert!(
            queries::claim_rss_subscription_refresh(&pool, &subscription.id, &due_before)
                .await
                .expect("first claim succeeds")
        );
        assert!(
            !queries::claim_rss_subscription_refresh(&pool, &subscription.id, &due_before)
                .await
                .expect("second claim is refused")
        );

        queries::set_rss_refresh_error(&pool, &owner.id, &subscription.id, "feed unavailable")
            .await
            .expect("refresh error stores");
        assert!(
            queries::list_due_rss_subscriptions(&pool, &due_before, 10)
                .await
                .expect("due subscriptions load")
                .is_empty(),
            "a failed refresh backs off for a full window"
        );
    }

    #[tokio::test]
    async fn destructive_content_cleanup_is_scoped_to_one_account() {
        let pool = connect("sqlite::memory:").await.expect("database connects");
        let (owner, _) =
            queries::create_account(&pool, "cleanup@example.com", "$argon2id$owner", "Owner")
                .await
                .expect("owner creates");
        let (other, _) = queries::create_account(
            &pool,
            "cleanup-other@example.com",
            "$argon2id$other",
            "Other",
        )
        .await
        .expect("other account creates");
        let now = chrono::Utc::now().to_rfc3339();
        for (id, user_id, title) in [
            ("owner-task", owner.id.as_str(), "Owner task"),
            ("other-task", other.id.as_str(), "Other task"),
        ] {
            sqlx::query(
                "INSERT INTO tasks (id, user_id, title, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(user_id)
            .bind(title)
            .bind(&now)
            .bind(&now)
            .execute(&pool)
            .await
            .expect("task inserts");
        }
        let owner_task_count = queries::list_tasks(&pool, &owner.id)
            .await
            .expect("owner tasks load")
            .len() as u64;
        let other_task_count = queries::list_tasks(&pool, &other.id)
            .await
            .expect("other tasks load")
            .len();

        assert_eq!(
            queries::delete_user_content(&pool, &owner.id, "tasks")
                .await
                .expect("owner tasks delete"),
            owner_task_count
        );
        assert!(
            queries::list_tasks(&pool, &owner.id)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            queries::list_tasks(&pool, &other.id).await.unwrap().len(),
            other_task_count
        );
    }

    #[tokio::test]
    async fn journal_nodes_are_nested_private_and_cascade_deleted() {
        let pool = connect("sqlite::memory:").await.expect("database connects");
        let (owner, _) =
            queries::create_account(&pool, "writer@example.com", "$argon2id$writer", "Writer")
                .await
                .expect("writer creates");
        let (other, _) = queries::create_account(
            &pool,
            "journal-other@example.com",
            "$argon2id$other",
            "Other Writer",
        )
        .await
        .expect("other writer creates");

        let root =
            queries::create_journal_node(&pool, &owner.id, None, "Projects.md", "# Projects")
                .await
                .expect("root document creates");
        let nested =
            queries::create_journal_node(&pool, &owner.id, Some(&root.id), "Pandan.md", "# Pandan")
                .await
                .expect("nested parent document creates");
        let document = queries::create_journal_node(
            &pool,
            &owner.id,
            Some(&nested.id),
            "decisions.md",
            "# Decisions",
        )
        .await
        .expect("nested document creates");

        assert_eq!(
            queries::list_journal_nodes(&pool, &owner.id)
                .await
                .unwrap()
                .len(),
            3
        );
        assert!(
            queries::list_journal_nodes(&pool, &other.id)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            queries::journal_move_would_cycle(&pool, &owner.id, &root.id, &nested.id)
                .await
                .unwrap()
        );

        let updated = queries::update_journal_node(
            &pool,
            &owner.id,
            &document.id,
            Some(&nested.id),
            &document.name,
            "# Decisions\n\nUse SQLite.",
            None,
        )
        .await
        .expect("document saves")
        .expect("document exists");
        assert_eq!(updated.position, document.position);
        assert!(updated.content.contains("Use SQLite"));

        let root_updated = queries::update_journal_node(
            &pool,
            &owner.id,
            &root.id,
            None,
            &root.name,
            "# Projects\n\nThis document also contains subdocuments.",
            None,
        )
        .await
        .expect("parent document saves")
        .expect("parent document exists");
        assert!(root_updated.content.contains("subdocuments"));

        assert!(
            queries::delete_journal_node(&pool, &owner.id, &root.id)
                .await
                .unwrap()
        );
        assert!(
            queries::list_journal_nodes(&pool, &owner.id)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn journal_nodes_persist_user_defined_sibling_order() {
        let pool = connect("sqlite::memory:").await.expect("database connects");
        let (owner, _) =
            queries::create_account(&pool, "ordered@example.com", "$argon2id$ordered", "Writer")
                .await
                .expect("writer creates");
        let first = queries::create_journal_node(&pool, &owner.id, None, "First", "")
            .await
            .expect("first document creates");
        let second = queries::create_journal_node(&pool, &owner.id, None, "Second", "")
            .await
            .expect("second document creates");
        let third = queries::create_journal_node(&pool, &owner.id, None, "Third", "")
            .await
            .expect("third document creates");

        queries::update_journal_node(
            &pool,
            &owner.id,
            &third.id,
            None,
            &third.name,
            &third.content,
            Some(0),
        )
        .await
        .expect("third document reorders");
        let ordered = queries::list_journal_nodes(&pool, &owner.id)
            .await
            .expect("ordered documents load");
        assert_eq!(
            ordered
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            vec![third.id.as_str(), first.id.as_str(), second.id.as_str()]
        );
        assert_eq!(
            ordered.iter().map(|node| node.position).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );

        queries::update_journal_node(
            &pool,
            &owner.id,
            &first.id,
            Some(&third.id),
            &first.name,
            &first.content,
            Some(0),
        )
        .await
        .expect("first document nests");
        let moved = queries::list_journal_nodes(&pool, &owner.id)
            .await
            .expect("moved documents load");
        let roots = moved
            .iter()
            .filter(|node| node.parent_id.is_none())
            .collect::<Vec<_>>();
        assert_eq!(
            roots
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            vec![third.id.as_str(), second.id.as_str()]
        );
        assert_eq!(
            roots.iter().map(|node| node.position).collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(
            moved
                .iter()
                .find(|node| node.id == first.id)
                .expect("nested document remains")
                .position,
            0
        );
    }

    #[tokio::test]
    async fn journal_folder_migration_preserves_paths_and_content() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("memory database URL parses")
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("database connects");
        for (_, migration) in MIGRATIONS.iter().take(13) {
            sqlx::raw_sql(*migration)
                .execute(&pool)
                .await
                .expect("legacy migration applies");
        }
        let (user, _) = queries::create_account(
            &pool,
            "legacy-journal@example.com",
            "$argon2id$legacy",
            "Legacy Writer",
        )
        .await
        .expect("legacy account creates");
        sqlx::query(
            "INSERT INTO journal_nodes \
             (id, user_id, parent_id, kind, name, content, position, created_at, updated_at) \
             VALUES ('folder', ?, NULL, 'directory', 'Research', '', 0, 'now', 'now'), \
                    ('file', ?, 'folder', 'file', 'notes.md', '# Preserved', 0, 'now', 'now')",
        )
        .bind(&user.id)
        .bind(&user.id)
        .execute(&pool)
        .await
        .expect("legacy journal tree inserts");

        sqlx::raw_sql(MIGRATIONS[13].1)
            .execute(&pool)
            .await
            .expect("document migration applies");

        let nodes = queries::list_journal_nodes(&pool, &user.id)
            .await
            .expect("migrated journal loads");
        assert_eq!(nodes.len(), 2);
        let former_folder = nodes
            .iter()
            .find(|node| node.id == "folder")
            .expect("former folder remains");
        let child = nodes
            .iter()
            .find(|node| node.id == "file")
            .expect("child file remains");
        assert!(former_folder.content.is_empty());
        assert_eq!(child.parent_id.as_deref(), Some("folder"));
        assert_eq!(child.content, "# Preserved");
        let kind_columns: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('journal_nodes') WHERE name = 'kind'",
        )
        .fetch_one(&pool)
        .await
        .expect("journal columns inspect");
        assert_eq!(kind_columns, 0);
    }

    #[tokio::test]
    async fn calendars_and_payment_subscriptions_are_user_owned() {
        let pool = connect("sqlite::memory:").await.expect("database connects");
        let (owner, _) = queries::create_account(
            &pool,
            "calendar@example.com",
            "$argon2id$calendar",
            "Calendar Owner",
        )
        .await
        .expect("owner creates");
        let (other, _) = queries::create_account(
            &pool,
            "calendar-other@example.com",
            "$argon2id$other",
            "Other User",
        )
        .await
        .expect("other user creates");

        let source = queries::create_calendar_subscription(
            &pool,
            &owner.id,
            "https://example.com/work.ics",
            "Work",
            "#2DD4BF",
            &[entities::CalendarEventDraft {
                external_id: "meeting".to_owned(),
                title: "Planning".to_owned(),
                description: String::new(),
                location: String::new(),
                url: String::new(),
                start_at: "2026-08-14T09:00:00+00:00".to_owned(),
                end_at: Some("2026-08-14T10:00:00+00:00".to_owned()),
                all_day: false,
            }],
        )
        .await
        .expect("calendar creates");
        assert_eq!(
            queries::list_calendar_events(&pool, &owner.id)
                .await
                .expect("events load")
                .len(),
            1
        );
        assert!(
            queries::list_calendar_events(&pool, &other.id)
                .await
                .expect("other events load")
                .is_empty()
        );
        assert!(
            !queries::delete_calendar_subscription(&pool, &other.id, &source.id)
                .await
                .expect("cross-user delete is checked")
        );

        let payment = queries::create_payment_subscription(
            &pool,
            &owner.id,
            "Example Service",
            "Team plan",
            "Monthly",
            2_500_000,
            "USD",
            "2026-01-15",
        )
        .await
        .expect("payment subscription creates");
        assert!(
            queries::list_payment_subscriptions(&pool, &other.id)
                .await
                .expect("other payments load")
                .is_empty()
        );
        assert!(
            queries::update_payment_subscription(
                &pool,
                &other.id,
                &payment.id,
                "Changed",
                "",
                "Yearly",
                30_000_000,
                "USD",
                "2026-01-15",
            )
            .await
            .expect("cross-user update is checked")
            .is_none()
        );
        assert_eq!(payment.amount_micros, 2_500_000);
        assert_eq!(payment.currency, "USD");
    }

    #[tokio::test]
    async fn new_accounts_start_with_one_dashboard() {
        let pool = connect("sqlite::memory:").await.expect("database connects");
        let (user, _) = queries::create_account(
            &pool,
            "workspace@example.com",
            "$argon2id$test",
            "Dashboard Owner",
        )
        .await
        .expect("account creates");

        let dashboards = queries::list_workspaces(&pool, &user.id)
            .await
            .expect("dashboard loads");
        assert_eq!(
            dashboards
                .iter()
                .map(|workspace| workspace.name.as_str())
                .collect::<Vec<_>>(),
            ["Dashboard"]
        );
        assert!(
            queries::list_dashboard_widgets(&pool, &user.id)
                .await
                .expect("widgets load")
                .iter()
                .all(|widget| widget.workspace == 0)
        );
        let appearance = queries::find_user_appearance(&pool, &user.id)
            .await
            .expect("appearance loads");
        assert_eq!(appearance.background_brightness, 78);
    }

    #[tokio::test]
    async fn coding_projects_and_credentials_remain_user_owned() {
        let pool = connect("sqlite::memory:").await.expect("database connects");
        let (owner, _) = queries::create_account(
            &pool,
            "coding-owner@example.com",
            "$argon2id$owner",
            "Coding Owner",
        )
        .await
        .expect("owner creates");
        let (other, _) = queries::create_account(
            &pool,
            "coding-other@example.com",
            "$argon2id$other",
            "Coding Other",
        )
        .await
        .expect("other account creates");

        let project = queries::create_coding_project(
            &pool,
            &owner.id,
            "gitlab",
            "gitlab.com",
            "team/service",
        )
        .await
        .expect("project creates");
        assert!(!project.has_credential);
        assert!(
            queries::list_coding_projects(&pool, &other.id)
                .await
                .expect("other projects load")
                .is_empty()
        );
        assert!(
            !queries::delete_coding_project(&pool, &other.id, &project.id)
                .await
                .expect("cross-user delete is checked")
        );

        queries::upsert_coding_credential(
            &pool,
            &owner.id,
            "gitlab",
            "gitlab.com",
            "encrypted-token",
        )
        .await
        .expect("credential stores");
        let projects = queries::list_coding_projects(&pool, &owner.id)
            .await
            .expect("owner projects load");
        assert_eq!(projects.len(), 1);
        assert!(projects[0].has_credential);
        assert_eq!(
            queries::list_coding_credentials(&pool, &owner.id)
                .await
                .expect("owner credentials load")[0]
                .ciphertext,
            "encrypted-token"
        );
    }

    #[tokio::test]
    async fn oidc_state_is_single_use_and_identity_links_by_verified_email() {
        let pool = connect("sqlite::memory:").await.expect("database connects");
        let expiry = (chrono::Utc::now() + chrono::Duration::minutes(10)).to_rfc3339();
        queries::create_oidc_authorization(&pool, "state", "verifier", "nonce", &expiry)
            .await
            .expect("OIDC state stores");
        let consumed = queries::consume_oidc_authorization(&pool, "state")
            .await
            .expect("OIDC state consumes")
            .expect("OIDC state exists");
        assert_eq!(consumed.pkce_verifier, "verifier");
        assert!(
            queries::consume_oidc_authorization(&pool, "state")
                .await
                .expect("second state consume completes")
                .is_none()
        );

        let (user, _) = queries::create_account(
            &pool,
            "linked@example.com",
            "$argon2id$existing",
            "Linked User",
        )
        .await
        .expect("password account creates");
        let linked_user_id = queries::find_or_create_oidc_user(
            &pool,
            "https://issuer.example",
            "subject-1",
            "linked@example.com",
            "Provider Name",
            "$argon2id$unusable",
            false,
        )
        .await
        .expect("OIDC identity links")
        .expect("existing account is eligible");
        assert_eq!(linked_user_id, user.id);

        assert!(
            queries::find_or_create_oidc_user(
                &pool,
                "https://issuer.example",
                "subject-2",
                "new@example.com",
                "New User",
                "$argon2id$unusable",
                false,
            )
            .await
            .expect("disabled OIDC registration resolves")
            .is_none()
        );

        let new_user_id = queries::find_or_create_oidc_user(
            &pool,
            "https://issuer.example",
            "subject-3",
            "new@example.com",
            "New User",
            "$argon2id$unusable",
            true,
        )
        .await
        .expect("enabled OIDC registration resolves")
        .expect("new OIDC account is created");
        assert!(!new_user_id.is_empty());

        let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&pool)
            .await
            .expect("user count loads");
        assert_eq!(user_count, 2);
    }

    #[tokio::test]
    async fn verified_oidc_identity_can_claim_initial_administrator_setup() {
        let pool = connect("sqlite::memory:").await.expect("database connects");
        let user_id = queries::create_initial_oidc_administrator(
            &pool,
            "https://issuer.example/application/o/pandan/",
            "initial-subject",
            "owner@example.com",
            "OIDC Owner",
            "$argon2id$unusable",
        )
        .await
        .expect("OIDC setup completes")
        .expect("OIDC identity claims setup");

        assert!(
            queries::is_onboarding_complete(&pool)
                .await
                .expect("setup status loads")
        );
        let (email, role): (String, String) =
            sqlx::query_as("SELECT email, role FROM users WHERE id = ?")
                .bind(&user_id)
                .fetch_one(&pool)
                .await
                .expect("administrator loads");
        assert_eq!(email, "owner@example.com");
        assert_eq!(role, "administrator");

        let resolved_user_id = queries::find_or_create_oidc_user(
            &pool,
            "https://issuer.example/application/o/pandan/",
            "initial-subject",
            "owner@example.com",
            "OIDC Owner",
            "$argon2id$another-unusable",
            false,
        )
        .await
        .expect("OIDC identity resolves")
        .expect("persisted identity is linked");
        assert_eq!(resolved_user_id, user_id);

        assert!(
            queries::create_initial_administrator(
                &pool,
                "second@example.com",
                "$argon2id$password",
                "Second Owner",
            )
            .await
            .expect("competing setup resolves")
            .is_none()
        );
        let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&pool)
            .await
            .expect("user count loads");
        assert_eq!(user_count, 1);
    }

    #[tokio::test]
    async fn kanban_defaults_permissions_and_final_admin_are_preserved() {
        let pool = connect("sqlite::memory:").await.expect("database connects");
        let (owner, _) = queries::create_account(
            &pool,
            "kanban-owner@example.com",
            "$argon2id$owner",
            "Kanban Owner",
        )
        .await
        .expect("owner creates");
        let (guest, _) = queries::create_account(
            &pool,
            "kanban-guest@example.com",
            "$argon2id$guest",
            "Kanban Guest",
        )
        .await
        .expect("guest creates");

        let workspace =
            queries::create_kanban_workspace(&pool, &owner.id, "Product", "Shared delivery work")
                .await
                .expect("workspace creates");
        assert_eq!(workspace.role, "admin");
        assert_eq!(workspace.permissions.len(), 24);
        assert_eq!(
            queries::update_kanban_member_role(&pool, &workspace.id, &owner.id, "member")
                .await
                .expect("demotion is checked"),
            Some(false)
        );
        assert_eq!(
            queries::remove_kanban_member(&pool, &workspace.id, &owner.id)
                .await
                .expect("removal is checked"),
            Some(false)
        );

        assert!(
            queries::invite_kanban_member(&pool, &workspace.id, &guest.id, "guest", &owner.id,)
                .await
                .expect("guest is invited")
        );
        assert!(
            queries::respond_to_kanban_invitation(&pool, &workspace.id, &guest.id, true)
                .await
                .expect("guest accepts")
        );
        let guest_permissions =
            queries::kanban_effective_permissions(&pool, &workspace.id, &guest.id)
                .await
                .expect("guest permissions load");
        assert_eq!(guest_permissions.len(), 6);
        assert!(!guest_permissions.contains(&"board:create".to_owned()));
        assert!(
            queries::set_kanban_member_permission(
                &pool,
                &workspace.id,
                &guest.id,
                "board:create",
                true,
            )
            .await
            .expect("override saves")
        );
        assert!(
            queries::kanban_has_permission(&pool, &workspace.id, &guest.id, "board:create")
                .await
                .expect("override resolves")
        );

        let board_id =
            queries::create_kanban_board(&pool, &workspace.id, &owner.id, "Launch", "", "private")
                .await
                .expect("board creates");
        let board = queries::get_kanban_board(&pool, &board_id, &owner.id)
            .await
            .expect("board loads")
            .expect("board exists");
        assert_eq!(
            board
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Todo", "In Progress", "Finished"]
        );
    }

    #[tokio::test]
    async fn podcast_requests_gate_the_catalogue_and_downloads_survive_their_requester() {
        let pool = connect("sqlite::memory:").await.expect("database connects");
        let (admin, _) = queries::create_account(
            &pool,
            "admin-podcasts@example.com",
            "$argon2id$admin-podcasts",
            "Admin Podcasts",
        )
        .await
        .expect("administrator account creates");
        let (member, _) = queries::create_account(
            &pool,
            "member-podcasts@example.com",
            "$argon2id$member-podcasts",
            "Member Podcasts",
        )
        .await
        .expect("member account creates");

        let settings = queries::get_podcast_settings(&pool)
            .await
            .expect("podcast policy loads");
        assert!(settings.requests_enabled);
        assert_eq!(settings.max_pending_requests_per_user, 5);

        // A member request does not create a catalogue entry on its own.
        let request = queries::insert_podcast_request(
            &pool,
            &entities::PodcastRequestDraft {
                user_id: member.id.clone(),
                feed_url: "https://example.com/Feed.xml".to_owned(),
                normalized_url: "https://example.com/Feed.xml".to_owned(),
                resolved_title: "Example Show".to_owned(),
                resolved_author: "Example Author".to_owned(),
                resolved_artwork_url: String::new(),
                note: "Please add this".to_owned(),
            },
        )
        .await
        .expect("request records");
        assert_eq!(request.status, "pending");
        assert_eq!(request.requester_name, "Member Podcasts");
        assert!(
            queries::list_podcast_summaries(&pool, &member.id)
                .await
                .expect("catalogue loads")
                .is_empty(),
            "a pending request must not publish a podcast"
        );
        assert!(
            queries::has_open_podcast_request(&pool, &member.id, "https://example.com/Feed.xml")
                .await
                .expect("open request lookup completes")
        );

        // Approval publishes the podcast and subscribes the requester in one step.
        let podcast = queries::insert_podcast(
            &pool,
            &entities::PodcastDraft {
                feed_url: "https://example.com/Feed.xml".to_owned(),
                normalized_url: "https://example.com/Feed.xml".to_owned(),
                preview: entities::PodcastFeedPreview {
                    title: "Example Show".to_owned(),
                    description: "A show".to_owned(),
                    author: "Example Author".to_owned(),
                    site_url: "https://example.com".to_owned(),
                    language: "en".to_owned(),
                    artwork_url: "https://example.com/art.jpg".to_owned(),
                },
                added_by: admin.id.clone(),
                auto_download_count: 3,
            },
        )
        .await
        .expect("catalogue entry creates");
        assert!(
            queries::approve_podcast_request(&pool, &request.id, &admin.id, &podcast.id, "")
                .await
                .expect("approval completes")
        );
        let summaries = queries::list_podcast_summaries(&pool, &member.id)
            .await
            .expect("catalogue loads");
        assert_eq!(summaries.len(), 1);
        assert!(summaries[0].subscribed, "approval subscribes the requester");
        assert!(
            !queries::approve_podcast_request(&pool, &request.id, &admin.id, &podcast.id, "")
                .await
                .expect("second approval completes"),
            "a decided request cannot be approved twice"
        );

        // The same feed is never requestable twice.
        assert!(
            queries::find_podcast_by_normalized_url(&pool, "https://example.com/Feed.xml")
                .await
                .expect("catalogue lookup completes")
                .is_some()
        );

        let new_ids = queries::upsert_podcast_episodes(
            &pool,
            &podcast.id,
            &[
                entities::PodcastEpisodeDraft {
                    guid: "ep-1".to_owned(),
                    title: "Episode one".to_owned(),
                    description: "First".to_owned(),
                    episode_url: "https://example.com/1".to_owned(),
                    enclosure_url: "https://cdn.example.com/1.mp3".to_owned(),
                    enclosure_type: "audio/mpeg".to_owned(),
                    enclosure_bytes: Some(1024),
                    duration_seconds: Some(600),
                    published_at: "2026-08-01T00:00:00Z".to_owned(),
                },
                entities::PodcastEpisodeDraft {
                    guid: "ep-2".to_owned(),
                    title: "Episode two".to_owned(),
                    description: "Second".to_owned(),
                    episode_url: "https://example.com/2".to_owned(),
                    enclosure_url: "https://cdn.example.com/2.mp3".to_owned(),
                    enclosure_type: "audio/mpeg".to_owned(),
                    enclosure_bytes: Some(2048),
                    duration_seconds: Some(900),
                    published_at: "2026-08-02T00:00:00Z".to_owned(),
                },
            ],
        )
        .await
        .expect("episodes index");
        assert_eq!(new_ids.len(), 2);

        // Re-indexing the same feed discovers nothing new.
        assert!(
            queries::upsert_podcast_episodes(
                &pool,
                &podcast.id,
                &[entities::PodcastEpisodeDraft {
                    guid: "ep-1".to_owned(),
                    title: "Episode one, retitled".to_owned(),
                    description: "First".to_owned(),
                    episode_url: "https://example.com/1".to_owned(),
                    enclosure_url: "https://cdn.example.com/1.mp3".to_owned(),
                    enclosure_type: "audio/mpeg".to_owned(),
                    enclosure_bytes: Some(1024),
                    duration_seconds: Some(600),
                    published_at: "2026-08-01T00:00:00Z".to_owned(),
                }],
            )
            .await
            .expect("re-index completes")
            .is_empty()
        );

        let episodes = queries::list_podcast_episodes(&pool, &member.id, &podcast.id, 50, 0)
            .await
            .expect("episodes load");
        assert_eq!(episodes.len(), 2);
        assert_eq!(episodes[0].title, "Episode two", "newest first");
        assert_eq!(episodes[1].title, "Episode one, retitled");
        assert!(episodes[0].download_status.is_none());

        let newest = episodes[0].id.clone();
        assert!(
            queries::user_can_access_episode(&pool, &member.id, &newest)
                .await
                .expect("access check completes")
        );
        assert!(
            !queries::user_can_access_episode(&pool, &admin.id, &newest)
                .await
                .expect("access check completes"),
            "an unsubscribed administrator has no episode access"
        );

        // A cached file outlives the account that asked for it.
        queries::enqueue_podcast_download(&pool, &newest, Some(&member.id))
            .await
            .expect("download enqueues");
        let job = queries::claim_podcast_download(&pool, "1970-01-01T00:00:00Z", 3)
            .await
            .expect("claim completes")
            .expect("a job is available");
        assert_eq!(job.episode_id, newest);
        assert_eq!(job.enclosure_url, "https://cdn.example.com/2.mp3");
        assert!(
            queries::claim_podcast_download(&pool, "1970-01-01T00:00:00Z", 3)
                .await
                .expect("second claim completes")
                .is_none(),
            "a leased job is not handed out twice"
        );
        queries::mark_podcast_download_ready(&pool, &newest, "cached.mp3", "audio/mpeg", 2048)
            .await
            .expect("download publishes");
        assert_eq!(
            queries::podcast_storage_used_bytes(&pool)
                .await
                .expect("usage loads"),
            2048
        );

        queries::delete_user_podcast_content(&pool, &member.id)
            .await
            .expect("member content clears");
        assert!(
            queries::get_podcast_cached_file(&pool, &newest)
                .await
                .expect("cached lookup completes")
                .is_some(),
            "shared cached audio survives clearing one listener's content"
        );
        assert!(
            queries::get_podcast(&pool, &podcast.id)
                .await
                .expect("catalogue lookup completes")
                .is_some(),
            "the shared catalogue survives clearing one listener's content"
        );
    }

    #[tokio::test]
    async fn podcast_queue_reorders_without_tripping_its_unique_position() {
        let pool = connect("sqlite::memory:").await.expect("database connects");
        let (owner, _) = queries::create_account(
            &pool,
            "queue-podcasts@example.com",
            "$argon2id$queue-podcasts",
            "Queue Owner",
        )
        .await
        .expect("account creates");
        let podcast = queries::insert_podcast(
            &pool,
            &entities::PodcastDraft {
                feed_url: "https://example.com/queue.xml".to_owned(),
                normalized_url: "https://example.com/queue.xml".to_owned(),
                preview: entities::PodcastFeedPreview {
                    title: "Queue Show".to_owned(),
                    description: String::new(),
                    author: String::new(),
                    site_url: String::new(),
                    language: String::new(),
                    artwork_url: String::new(),
                },
                added_by: owner.id.clone(),
                auto_download_count: 0,
            },
        )
        .await
        .expect("catalogue entry creates");
        queries::subscribe_to_podcast(&pool, &owner.id, &podcast.id)
            .await
            .expect("subscription creates");

        let drafts = (0..4)
            .map(|index| entities::PodcastEpisodeDraft {
                guid: format!("ep-{index}"),
                title: format!("Episode {index}"),
                description: String::new(),
                episode_url: String::new(),
                enclosure_url: format!("https://cdn.example.com/{index}.mp3"),
                enclosure_type: "audio/mpeg".to_owned(),
                enclosure_bytes: None,
                duration_seconds: None,
                published_at: format!("2026-08-0{}T00:00:00Z", index + 1),
            })
            .collect::<Vec<_>>();
        let ids = queries::upsert_podcast_episodes(&pool, &podcast.id, &drafts)
            .await
            .expect("episodes index");
        assert_eq!(ids.len(), 4);

        for id in &ids {
            assert!(
                queries::append_to_podcast_queue(&pool, &owner.id, id, 100)
                    .await
                    .expect("queue append completes")
            );
        }
        let queued = queries::list_podcast_queue(&pool, &owner.id)
            .await
            .expect("queue loads");
        assert_eq!(
            queued
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<_>>(),
            ids
        );

        // Full reversal is the case a naive single-pass rewrite cannot survive.
        let reversed = ids.iter().rev().cloned().collect::<Vec<_>>();
        assert!(
            queries::reorder_podcast_queue(&pool, &owner.id, &reversed)
                .await
                .expect("reorder completes")
        );
        assert_eq!(
            queries::list_podcast_queue(&pool, &owner.id)
                .await
                .expect("queue loads")
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<_>>(),
            reversed
        );

        // A reorder that does not name exactly the queued set is refused.
        assert!(
            !queries::reorder_podcast_queue(&pool, &owner.id, &reversed[..2])
                .await
                .expect("partial reorder completes")
        );

        assert!(
            queries::remove_from_podcast_queue(&pool, &owner.id, &reversed[1])
                .await
                .expect("removal completes")
        );
        let remaining = queries::list_podcast_queue(&pool, &owner.id)
            .await
            .expect("queue loads");
        assert_eq!(remaining.len(), 3);
        assert_eq!(
            remaining
                .iter()
                .filter_map(|item| item.queue_position)
                .collect::<Vec<_>>(),
            vec![0, 1, 2],
            "removal closes the gap it leaves behind"
        );
    }
}
