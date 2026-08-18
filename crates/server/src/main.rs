use actix_files::Files;
use actix_web::{App, HttpServer, web};
use server::{AppState, UI_BUILD_DIR, configure_api, spa_fallback};
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
        .map_err(|error| miette::miette!("database error: {error}"))?;
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
    let state = web::Data::new(AppState {
        pool,
        cookie_secure,
        oidc,
        widget_integrations,
    });
    server::spawn_youtube_refresh_worker(state.clone());

    info!(port, "server listening");
    HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .app_data(state.clone())
            .configure(configure_api)
            .service(
                Files::new("/", UI_BUILD_DIR)
                    .index_file("200.html")
                    .prefer_utf8(true)
                    .default_handler(web::to(spa_fallback)),
            )
    })
    .bind(("0.0.0.0", port))
    .map_err(|error| miette::miette!("bind failed: {error}"))?
    .run()
    .await
    .map_err(|error| miette::miette!("server error: {error}"))
}
