use actix_files::Files;
use actix_web::{App, HttpServer, web};
use server::{AppState, SiteOrigin, UI_BUILD_DIR, configure_api, spa_document};
use tracing::info;
use tracing_actix_web::TracingLogger;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> miette::Result<()> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    std::fs::create_dir_all("data")
        .map_err(|error| miette::miette!("failed to create data directory: {error}"))?;

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data/pandan.db".to_owned());
    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(9651);

    info!(%database_url, "connecting to database");
    let pool = db::connect(&database_url)
        .await
        .map_err(|error| database_startup_error(&database_url, error))?;
    let cookie_secure = std::env::var("COOKIE_SECURE")
        .is_ok_and(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"));
    let oidc = server::oidc::OidcProvider::from_env()
        .await
        .map_err(|error| miette::miette!("OIDC configuration error: {error}"))?;
    info!(enabled = oidc.is_some(), "OIDC provider configured");
    let widget_integrations = server::widget_integrations::WidgetIntegrationService::from_env()
        .map_err(|error| miette::miette!("widget integration configuration error: {error}"))?;
    info!(
        secret_storage_enabled = widget_integrations.secrets_enabled(),
        invidious_enabled = widget_integrations.invidious_enabled(),
        "widget integrations configured"
    );
    let podcast_media = server::PodcastMedia::from_env()
        .map_err(|error| miette::miette!("podcast media configuration error: {error}"))?;
    info!(
        media_dir = %podcast_media.root().display(),
        "podcast media storage configured"
    );
    let state = web::Data::new(AppState {
        pool,
        cookie_secure,
        oidc,
        widget_integrations,
        podcast_media,
        site_origin: SiteOrigin::from_env(),
    });
    server::spawn_youtube_refresh_worker(state.clone());
    server::spawn_podcast_workers(state.clone());
    server::spawn_rss_refresh_worker(state.clone());

    info!(port, "server listening");
    HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .app_data(state.clone())
            .configure(configure_api)
            // The application document is rendered rather than served from
            // disk, so the root route cannot be left to `Files::index_file`.
            .service(web::resource("/").to(spa_document))
            .service(
                Files::new("/", UI_BUILD_DIR)
                    .prefer_utf8(true)
                    .default_handler(web::to(spa_document)),
            )
    })
    .bind(("0.0.0.0", port))
    .map_err(|error| miette::miette!("bind failed: {error}"))?
    .run()
    .await
    .map_err(|error| miette::miette!("server error: {error}"))
}

fn database_startup_error(database_url: &str, error: sqlx::Error) -> miette::Report {
    if is_sqlite_access_error(&error) {
        return miette::miette!(
            "failed to open SQLite database configured by DATABASE_URL (`{database_url}`): \
             {error}. Ensure the database's parent directory exists and is readable and writable \
             by the user running Pandan"
        );
    }

    miette::miette!("database error: {error}")
}

fn is_sqlite_access_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Io(error) => matches!(
            error.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
        ),
        sqlx::Error::Database(error) => error
            .code()
            .is_some_and(|code| is_sqlite_access_code(&code)),
        _ => false,
    }
}

fn is_sqlite_access_code(code: &str) -> bool {
    code.parse::<i32>()
        .is_ok_and(|code| matches!(code & 0xff, 8 | 14))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_access_error_explains_parent_directory_permissions() {
        let error = sqlx::Error::Io(std::io::Error::from(std::io::ErrorKind::PermissionDenied));

        let message = database_startup_error("sqlite:///data/pandan.db", error).to_string();

        assert!(message.contains("sqlite:///data/pandan.db"));
        assert!(message.contains("parent directory"));
        assert!(message.contains("readable and writable"));
    }

    #[test]
    fn unrelated_database_errors_keep_the_generic_message() {
        let error = sqlx::Error::Io(std::io::Error::from(std::io::ErrorKind::ConnectionReset));

        let message = database_startup_error("sqlite:///data/pandan.db", error).to_string();

        assert!(message.starts_with("database error:"));
        assert!(!message.contains("parent directory"));
    }

    #[test]
    fn sqlite_read_only_and_cannot_open_codes_are_access_errors() {
        assert!(is_sqlite_access_code("8"));
        assert!(is_sqlite_access_code("14"));
        assert!(is_sqlite_access_code("526"));
        assert!(is_sqlite_access_code("1544"));
        assert!(!is_sqlite_access_code("1"));
        assert!(!is_sqlite_access_code("not-a-number"));
    }
}
