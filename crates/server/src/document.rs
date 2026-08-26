//! Serves the single-page application document with absolute link-preview URLs.
//!
//! Pandan renders entirely in the browser, so a link preview crawler never runs
//! the application: the Open Graph tags in `ui/src/app.html` are the whole story
//! it gets. Those tags need absolute URLs, which only exist at runtime, so the
//! document carries an [`ORIGIN_PLACEHOLDER`] that is substituted here.
//!
//! Every route resolves to the same document, so every Pandan link previews as
//! the same generic card. That is deliberate: the pages behind it are private.

use actix_web::{HttpRequest, HttpResponse, error::ErrorInternalServerError, http::header, web};
use tracing::warn;

use crate::{AppState, UI_BUILD_DIR};

/// Written into `ui/src/app.html` wherever a preview tag needs an absolute URL.
const ORIGIN_PLACEHOLDER: &str = "__PANDAN_ORIGIN__";

/// The public origin that link preview tags are written against.
///
/// `PANDAN_BASE_URL` is authoritative when configured, because a deployment
/// behind a proxy is the only thing that knows its own public address. Without
/// it the origin is reconstructed from the request, which means trusting
/// forwarding headers; the value is therefore only ever used to build preview
/// URLs, and never to make an authorization or redirect decision.
#[derive(Clone, Debug, Default)]
pub struct SiteOrigin {
    configured: Option<String>,
}

impl SiteOrigin {
    /// Reads `PANDAN_BASE_URL`, keeping any path prefix and dropping the trailing slash.
    #[must_use]
    pub fn from_env() -> Self {
        let configured = std::env::var("PANDAN_BASE_URL")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .and_then(|value| {
                let normalized = normalize_base_url(&value);
                if normalized.is_none() {
                    warn!(
                        "PANDAN_BASE_URL is not an absolute HTTP(S) URL; link preview URLs will \
                         be derived from each request instead"
                    );
                }
                normalized
            });

        Self { configured }
    }

    /// The origin to write into the served document, or an empty string when none is safe.
    ///
    /// An empty origin leaves the preview URLs root-relative, which the major
    /// crawlers resolve against the page they fetched.
    #[must_use]
    pub fn resolve(&self, request: &HttpRequest) -> String {
        if let Some(configured) = &self.configured {
            return configured.clone();
        }

        let connection = request.connection_info();
        let scheme = connection.scheme();
        if !matches!(scheme, "http" | "https") {
            return String::new();
        }

        let derived = format!("{scheme}://{}", connection.host());
        if is_attribute_safe(&derived) {
            derived
        } else {
            String::new()
        }
    }
}

/// Accepts a base URL only when it is absolute, HTTP(S), and carries no query or fragment.
fn normalize_base_url(value: &str) -> Option<String> {
    let parsed = url::Url::parse(value).ok()?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host().is_none()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return None;
    }

    let normalized = parsed.as_str().trim_end_matches('/').to_owned();
    is_attribute_safe(&normalized).then_some(normalized)
}

/// Rejects anything that could break out of the HTML attribute it is written into.
///
/// With `PANDAN_BASE_URL` unset the origin is rebuilt from request headers, so
/// this guards attacker-controlled input on its way into the served document.
fn is_attribute_safe(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 300
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(
                    character,
                    '-' | '.' | ':' | '/' | '[' | ']' | '_' | '~' | '%'
                )
        })
}

/// Serves the static application's fallback document for client-side routes.
///
/// # Errors
///
/// Returns an Actix error when the UI has not been built or the fallback file cannot be read.
pub async fn spa_document(
    request: HttpRequest,
    state: web::Data<AppState>,
) -> actix_web::Result<HttpResponse> {
    let document = tokio::fs::read_to_string(format!("{UI_BUILD_DIR}/200.html"))
        .await
        .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok()
        .content_type(header::ContentType::html())
        // The shell references hashed assets and an injected origin, so it must
        // be revalidated rather than reused from a stale cache.
        .insert_header((header::CACHE_CONTROL, "no-cache"))
        .body(render(&document, &state.site_origin.resolve(&request))))
}

/// Serves the root-scoped service worker without allowing an intermediary to
/// reuse a stale worker script across deployments.
///
/// # Errors
///
/// Returns an Actix error when the UI has not been built or the worker file
/// cannot be read.
pub async fn service_worker() -> actix_web::Result<HttpResponse> {
    let script = tokio::fs::read(format!("{UI_BUILD_DIR}/service-worker.js"))
        .await
        .map_err(ErrorInternalServerError)?;

    Ok(service_worker_response(script))
}

/// Serves install metadata with the manifest media type and revalidation so
/// icon or display-mode changes are discovered promptly.
///
/// # Errors
///
/// Returns an Actix error when the UI has not been built or the manifest file
/// cannot be read.
pub async fn web_app_manifest() -> actix_web::Result<HttpResponse> {
    let manifest = tokio::fs::read(format!("{UI_BUILD_DIR}/app.webmanifest"))
        .await
        .map_err(ErrorInternalServerError)?;

    Ok(web_app_manifest_response(manifest))
}

fn service_worker_response(script: Vec<u8>) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "text/javascript; charset=utf-8"))
        .insert_header((header::CACHE_CONTROL, "no-cache"))
        .insert_header(("Service-Worker-Allowed", "/"))
        .body(script)
}

fn web_app_manifest_response(manifest: Vec<u8>) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "application/manifest+json"))
        .insert_header((header::CACHE_CONTROL, "no-cache"))
        .body(manifest)
}

/// Substitutes the resolved origin into the document.
fn render(document: &str, origin: &str) -> String {
    document.replace(ORIGIN_PLACEHOLDER, origin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;

    const DOCUMENT: &str = r#"<meta property="og:url" content="__PANDAN_ORIGIN__/" />
<meta property="og:image" content="__PANDAN_ORIGIN__/og-card.png" />"#;

    fn origin_from(request_builder: TestRequest) -> String {
        SiteOrigin::default().resolve(&request_builder.to_http_request())
    }

    #[test]
    fn renders_absolute_preview_urls_from_the_request_origin() {
        let rendered = render(
            DOCUMENT,
            &origin_from(TestRequest::default().insert_header(("host", "pandan.example.com"))),
        );

        assert!(rendered.contains(r#"content="http://pandan.example.com/""#));
        assert!(rendered.contains(r#"content="http://pandan.example.com/og-card.png""#));
        assert!(!rendered.contains(ORIGIN_PLACEHOLDER));
    }

    #[test]
    fn honours_the_forwarded_scheme_of_a_terminating_proxy() {
        let origin = origin_from(
            TestRequest::default()
                .insert_header(("host", "pandan.example.com"))
                .insert_header(("x-forwarded-proto", "https")),
        );

        assert_eq!(origin, "https://pandan.example.com");
    }

    #[test]
    fn configured_base_url_wins_over_the_request_and_keeps_its_path_prefix() {
        let origin = SiteOrigin {
            configured: normalize_base_url("https://example.com/pandan/"),
        }
        .resolve(
            &TestRequest::default()
                .insert_header(("host", "internal.local"))
                .to_http_request(),
        );

        assert_eq!(origin, "https://example.com/pandan");
    }

    #[test]
    fn rejects_a_base_url_that_is_not_absolute_http() {
        assert!(normalize_base_url("pandan.example.com").is_none());
        assert!(normalize_base_url("javascript:alert(1)").is_none());
        assert!(normalize_base_url("https://example.com/?next=1").is_none());
    }

    #[test]
    fn falls_back_to_relative_urls_when_the_host_header_is_hostile() {
        let rendered = render(
            DOCUMENT,
            &origin_from(
                TestRequest::default().insert_header(("host", r#"a.test"><script>x</script>"#)),
            ),
        );

        assert!(rendered.contains(r#"content="/og-card.png""#));
        assert!(!rendered.contains("<script>"));
    }

    #[test]
    fn service_worker_is_revalidated_and_can_control_the_root_scope() {
        let response = service_worker_response(Vec::new());

        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&header::HeaderValue::from_static("no-cache")),
        );
        assert_eq!(
            response.headers().get("Service-Worker-Allowed"),
            Some(&header::HeaderValue::from_static("/")),
        );
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE),
            Some(&header::HeaderValue::from_static(
                "text/javascript; charset=utf-8",
            )),
        );
    }

    #[test]
    fn web_app_manifest_uses_the_install_manifest_media_type() {
        let response = web_app_manifest_response(Vec::new());

        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&header::HeaderValue::from_static("no-cache")),
        );
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE),
            Some(&header::HeaderValue::from_static(
                "application/manifest+json",
            )),
        );
    }
}
