use actix_web::{HttpRequest, HttpResponse, web};
use db::entities::NetworkAccessRule;
use reqwest::{Client, ClientBuilder, Url};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::lookup_host;
use url::Host;

use crate::{ApiError, AppState, authenticated_administrator};

const MAX_RULE_ORIGIN_LENGTH: usize = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkAccessScope {
    Rss,
    Calendar,
    Contacts,
    Podcasts,
    Notifications,
    Coding,
    Images,
    Youtube,
    Widgets,
    Jellyfin,
}

impl NetworkAccessScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rss => "rss",
            Self::Calendar => "calendar",
            Self::Contacts => "contacts",
            Self::Podcasts => "podcasts",
            Self::Notifications => "notifications",
            Self::Coding => "coding",
            Self::Images => "images",
            Self::Youtube => "youtube",
            Self::Widgets => "widgets",
            Self::Jellyfin => "jellyfin",
        }
    }

    fn valid_rule_scope(value: &str) -> bool {
        matches!(
            value,
            "all"
                | "rss"
                | "calendar"
                | "contacts"
                | "podcasts"
                | "notifications"
                | "coding"
                | "images"
                | "youtube"
                | "widgets"
                | "jellyfin"
        )
    }
}

#[derive(Debug, Clone)]
pub struct NetworkPolicy {
    pool: Option<SqlitePool>,
}

impl NetworkPolicy {
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool: Some(pool) }
    }

    #[cfg(test)]
    #[must_use]
    pub fn without_rules() -> Self {
        Self { pool: None }
    }

    /// Validates one outbound destination and pins the DNS result used by the HTTP client.
    pub async fn validate(
        &self,
        value: &str,
        scope: NetworkAccessScope,
    ) -> Result<ValidatedUrl, String> {
        self.validate_with_operator_override(value, scope, false)
            .await
    }

    /// Applies the normal rule set while allowing an operator-configured exact destination.
    /// Explicit administrator deny rules still take precedence over the operator override.
    pub async fn validate_with_operator_override(
        &self,
        value: &str,
        scope: NetworkAccessScope,
        operator_allows_private: bool,
    ) -> Result<ValidatedUrl, String> {
        let url = parse_outbound_url(value)?;
        let host = normalized_host(&url)?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| "URL port is missing".to_owned())?;
        let rules = if let Some(pool) = &self.pool {
            db::queries::find_network_access_rules(
                pool,
                url.scheme(),
                &host,
                i64::from(port),
                scope.as_str(),
            )
            .await
            .map_err(|error| {
                tracing::error!(%error, "outbound network policy could not be loaded");
                "network access policy could not be evaluated".to_owned()
            })?
        } else {
            Vec::new()
        };
        if rules.iter().any(|rule| rule.action == "deny") {
            return Err("destination is denied by the administrator network policy".to_owned());
        }
        let explicitly_allowed =
            operator_allows_private || rules.iter().any(|rule| rule.action == "allow");
        if url.scheme() == "http" && !explicitly_allowed {
            return Err(
                "only HTTPS URLs are allowed unless an administrator allows this exact HTTP origin"
                    .to_owned(),
            );
        }
        if !explicitly_allowed
            && (host.eq_ignore_ascii_case("localhost")
                || host
                    .rsplit('.')
                    .next()
                    .is_some_and(|suffix| suffix.eq_ignore_ascii_case("local")))
        {
            return Err("local network URLs are not allowed".to_owned());
        }

        let mut addresses = if let Ok(ip) = host.parse::<IpAddr>() {
            vec![SocketAddr::new(ip, port)]
        } else {
            lookup_host((host.as_str(), port))
                .await
                .map_err(|_| "URL host could not be resolved".to_owned())?
                .collect::<Vec<_>>()
        };
        addresses.sort_unstable();
        addresses.dedup();
        if addresses.is_empty() {
            return Err("URL host could not be resolved".to_owned());
        }
        if !explicitly_allowed && addresses.iter().any(|address| !public_ip(address.ip())) {
            return Err("private or reserved network URLs are not allowed".to_owned());
        }
        Ok(ValidatedUrl {
            url,
            host,
            addresses,
        })
    }
}

#[derive(Debug)]
pub struct ValidatedUrl {
    url: Url,
    host: String,
    addresses: Vec<SocketAddr>,
}

impl ValidatedUrl {
    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    #[must_use]
    pub fn into_url(self) -> Url {
        self.url
    }

    /// Builds a client whose resolver is fixed to the addresses that passed policy validation.
    pub fn build_client(&self, builder: ClientBuilder) -> Result<Client, String> {
        let builder = if self.host.parse::<IpAddr>().is_ok() {
            builder
        } else {
            builder.resolve_to_addrs(&self.host, &self.addresses)
        };
        builder
            .build()
            .map_err(|error| format!("outbound HTTP client failed: {error}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuleOrigin {
    scheme: String,
    host: String,
    port: u16,
}

fn parse_rule_origin(value: &str) -> Result<RuleOrigin, String> {
    if value.len() > MAX_RULE_ORIGIN_LENGTH {
        return Err("network rule origin is too long".to_owned());
    }
    let url = parse_outbound_url(value)?;
    if !matches!(url.path(), "" | "/") || url.query().is_some() || url.fragment().is_some() {
        return Err(
            "network rules must name an origin without a path, query, or fragment".to_owned(),
        );
    }
    Ok(RuleOrigin {
        scheme: url.scheme().to_owned(),
        host: normalized_host(&url)?,
        port: url
            .port_or_known_default()
            .ok_or_else(|| "URL port is missing".to_owned())?,
    })
}

fn parse_outbound_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|_| "URL is invalid".to_owned())?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("only credential-free HTTP or HTTPS URLs are allowed".to_owned());
    }
    if url.host_str().is_none() {
        return Err("URL host is missing".to_owned());
    }
    Ok(url)
}

fn normalized_host(url: &Url) -> Result<String, String> {
    match url.host() {
        Some(Host::Domain(host)) => Ok(host.to_ascii_lowercase()),
        Some(Host::Ipv4(address)) => Ok(address.to_string()),
        Some(Host::Ipv6(address)) => Ok(address.to_string()),
        None => Err("URL host is missing".to_owned()),
    }
}

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => public_ipv4(ip),
        IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return public_ipv4(mapped);
            }
            let segments = ip.segments();
            if segments[..6].iter().all(|segment| *segment == 0) {
                return public_ipv4(Ipv4Addr::new(
                    (segments[6] >> 8) as u8,
                    segments[6] as u8,
                    (segments[7] >> 8) as u8,
                    segments[7] as u8,
                ));
            }
            if segments[0] == 0x0064
                && segments[1] == 0xff9b
                && segments[2..6].iter().all(|segment| *segment == 0)
            {
                return public_ipv4(Ipv4Addr::new(
                    (segments[6] >> 8) as u8,
                    segments[6] as u8,
                    (segments[7] >> 8) as u8,
                    segments[7] as u8,
                ));
            }
            let first = segments[0];
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                || (first & 0xffc0) == 0xfec0
                || (segments[0] == 0x0064 && segments[1] == 0xff9b && segments[2] == 1)
                || (segments[0] == 0x0100 && segments[1..4].iter().all(|segment| *segment == 0))
                || (segments[0] == 0x2001 && segments[1] <= 0x01ff)
                || (segments[0] == 0x2001 && segments[1] == 0x0db8)
                || segments[0] == 0x2002
                || (segments[0] & 0xfff0) == 0x3ff0
                || segments[0] == 0x5f00)
        }
    }
}

fn public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || ip.is_multicast()
        || a == 0
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0)
        || (a == 192 && b == 88 && ip.octets()[2] == 99)
        || (a == 198 && matches!(b, 18 | 19)))
}

#[derive(Debug, Deserialize, Serialize)]
struct CreateNetworkAccessRuleRequest {
    action: String,
    origin: String,
    integration: String,
}

pub fn configure(config: &mut web::ServiceConfig) {
    config
        .service(
            web::resource("/admin/network-access")
                .route(web::get().to(list_rules))
                .route(web::post().to(create_rule)),
        )
        .route(
            "/admin/network-access/{rule_id}",
            web::delete().to(delete_rule),
        );
}

async fn list_rules(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<Vec<NetworkAccessRule>>, ApiError> {
    authenticated_administrator(&state, &request).await?;
    Ok(web::Json(
        db::queries::list_network_access_rules(&state.pool).await?,
    ))
}

async fn create_rule(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CreateNetworkAccessRuleRequest>,
) -> Result<web::Json<NetworkAccessRule>, ApiError> {
    let administrator = authenticated_administrator(&state, &request).await?;
    if !matches!(payload.action.as_str(), "allow" | "deny") {
        return Err(ApiError::BadRequest(
            "network rule action must be allow or deny",
        ));
    }
    if !NetworkAccessScope::valid_rule_scope(&payload.integration) {
        return Err(ApiError::BadRequest("network rule integration is invalid"));
    }
    let origin = parse_rule_origin(payload.origin.trim()).map_err(|_| {
        ApiError::BadRequest("network rule must be a credential-free HTTP(S) origin")
    })?;
    let rule = db::queries::create_network_access_rule(
        &state.pool,
        &uuid::Uuid::new_v4().to_string(),
        &payload.action,
        &origin.scheme,
        &origin.host,
        i64::from(origin.port),
        &payload.integration,
        &administrator.id,
    )
    .await?
    .ok_or(ApiError::Conflict(
        "the network rule already exists or the 128-rule limit was reached",
    ))?;
    Ok(web::Json(rule))
}

async fn delete_rule(
    state: web::Data<AppState>,
    request: HttpRequest,
    rule_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    authenticated_administrator(&state, &request).await?;
    if db::queries::delete_network_access_rule(&state.pool, &rule_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("network access rule not found"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_ipv4_forms_are_not_public() {
        assert!(!public_ip("127.0.0.1".parse().expect("IPv4 parses")));
        assert!(!public_ip(
            "::ffff:192.168.1.9".parse().expect("mapped IPv6 parses")
        ));
        assert!(!public_ip(
            "64:ff9b::a00:1".parse().expect("NAT64 IPv6 parses")
        ));
        assert!(public_ip("8.8.8.8".parse().expect("public IPv4 parses")));
    }

    #[test]
    fn rule_origins_are_exact_and_credential_free() {
        let origin = parse_rule_origin("http://Example.COM:8080/").expect("origin parses");
        assert_eq!(origin.scheme, "http");
        assert_eq!(origin.host, "example.com");
        assert_eq!(origin.port, 8080);
        assert_eq!(
            parse_rule_origin("https://[fd00::1]:8443")
                .expect("IPv6 origin parses")
                .host,
            "fd00::1"
        );
        assert!(parse_rule_origin("http://example.com/private").is_err());
        assert!(parse_rule_origin("https://user@example.com").is_err());
    }

    #[tokio::test]
    async fn exact_allow_and_deny_rules_are_scoped_and_deny_wins() {
        let pool = db::connect("sqlite::memory:")
            .await
            .expect("database connects");
        let policy = NetworkPolicy::new(pool.clone());
        let origin = "http://127.0.0.1:8765";

        assert!(
            policy
                .validate(origin, NetworkAccessScope::Rss)
                .await
                .is_err()
        );
        sqlx::query(
            "INSERT INTO network_access_rules (\
                 id, action, scheme, host, port, integration, created_at, updated_at\
             ) VALUES ('allow-rss', 'allow', 'http', '127.0.0.1', 8765, 'rss', ?, ?)",
        )
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&pool)
        .await
        .expect("allow rule inserts");

        assert!(
            policy
                .validate(origin, NetworkAccessScope::Rss)
                .await
                .is_ok()
        );
        assert!(
            policy
                .validate(origin, NetworkAccessScope::Notifications)
                .await
                .is_err()
        );

        sqlx::query(
            "INSERT INTO network_access_rules (\
                 id, action, scheme, host, port, integration, created_at, updated_at\
             ) VALUES ('deny-rss', 'deny', 'http', '127.0.0.1', 8765, 'rss', ?, ?)",
        )
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&pool)
        .await
        .expect("deny rule inserts");

        let error = policy
            .validate(origin, NetworkAccessScope::Rss)
            .await
            .expect_err("deny wins");
        assert!(error.contains("denied by the administrator"));
    }
}
