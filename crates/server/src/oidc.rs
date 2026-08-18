use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet,
    EndpointNotSet, EndpointSet, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
    reqwest,
};
use std::fmt;

const OIDC_CALLBACK_PATH: &str = "api/auth/oidc/callback";

type DiscoveredClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

#[derive(Clone)]
pub struct OidcProvider {
    client: DiscoveredClient,
    http_client: reqwest::Client,
    pub issuer: String,
    pub name: String,
}

pub struct AuthorizationAttempt {
    pub url: String,
    pub state: String,
    pub nonce: String,
    pub pkce_verifier: String,
}

pub struct VerifiedIdentity {
    pub issuer: String,
    pub subject: String,
    pub email: String,
    pub display_name: String,
    pub picture_url: Option<String>,
}

#[derive(Debug)]
pub struct OidcError(String);

impl fmt::Display for OidcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for OidcError {}

impl OidcProvider {
    /// Discovers and configures an OIDC provider when all required environment values exist.
    ///
    /// # Errors
    ///
    /// Returns a configuration or discovery error when OIDC was requested but is invalid.
    pub async fn from_env() -> Result<Option<Self>, OidcError> {
        let issuer = optional_env("OIDC_ISSUER");
        let client_id = optional_env("OIDC_CLIENT_ID");
        let client_secret = optional_env("OIDC_CLIENT_SECRET");
        let base_url = optional_env("PANDAN_BASE_URL");
        let (issuer, client_id, client_secret, base_url) = match (
            issuer,
            client_id,
            client_secret,
            base_url,
        ) {
            (None, None, None, _) => return Ok(None),
            (Some(issuer), Some(client_id), Some(client_secret), Some(base_url)) => {
                (issuer, client_id, client_secret, base_url)
            }
            _ => return Err(OidcError(
                "OIDC_ISSUER, OIDC_CLIENT_ID, and OIDC_CLIENT_SECRET must be set together, and PANDAN_BASE_URL is required when OIDC is enabled"
                    .to_owned(),
            )),
        };
        let redirect_url = oidc_redirect_url(&base_url)?;

        let http_client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| OidcError(format!("failed to build OIDC HTTP client: {error}")))?;
        let provider_metadata = CoreProviderMetadata::discover_async(
            IssuerUrl::new(issuer.clone())
                .map_err(|error| OidcError(format!("invalid OIDC issuer: {error}")))?,
            &http_client,
        )
        .await
        .map_err(|error| OidcError(format!("OIDC discovery failed: {error}")))?;
        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(client_id),
            Some(ClientSecret::new(client_secret)),
        )
        .set_redirect_uri(
            RedirectUrl::new(redirect_url)
                .map_err(|error| OidcError(format!("invalid OIDC redirect URL: {error}")))?,
        );

        Ok(Some(Self {
            client,
            http_client,
            issuer,
            name: optional_env("OIDC_PROVIDER_NAME").unwrap_or_else(|| "Single sign-on".to_owned()),
        }))
    }

    pub fn authorization_attempt(&self) -> AuthorizationAttempt {
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state, nonce) = self
            .client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("email".to_owned()))
            .add_scope(Scope::new("profile".to_owned()))
            .set_pkce_challenge(challenge)
            .url();
        AuthorizationAttempt {
            url: url.to_string(),
            state: state.secret().to_owned(),
            nonce: nonce.secret().to_owned(),
            pkce_verifier: verifier.secret().to_owned(),
        }
    }

    /// Exchanges and validates one authorization code against the discovered provider keys.
    ///
    /// # Errors
    ///
    /// Returns an OIDC protocol error when exchange, signature, nonce, token hash, or claims fail.
    pub async fn verify_code(
        &self,
        code: String,
        pkce_verifier: String,
        nonce: String,
    ) -> Result<VerifiedIdentity, OidcError> {
        let token_response = self
            .client
            .exchange_code(AuthorizationCode::new(code))
            .map_err(|error| OidcError(format!("OIDC token endpoint unavailable: {error}")))?
            .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
            .request_async(&self.http_client)
            .await
            .map_err(|error| OidcError(format!("OIDC code exchange failed: {error}")))?;
        let id_token = token_response
            .id_token()
            .ok_or_else(|| OidcError("OIDC provider omitted the ID token".to_owned()))?;
        let verifier = self.client.id_token_verifier();
        let claims = id_token
            .claims(&verifier, &Nonce::new(nonce))
            .map_err(|error| OidcError(format!("OIDC ID token validation failed: {error}")))?;

        if let Some(expected_hash) = claims.access_token_hash() {
            let actual_hash = AccessTokenHash::from_token(
                token_response.access_token(),
                id_token.signing_alg().map_err(|error| {
                    OidcError(format!("OIDC signing algorithm invalid: {error}"))
                })?,
                id_token
                    .signing_key(&verifier)
                    .map_err(|error| OidcError(format!("OIDC signing key invalid: {error}")))?,
            )
            .map_err(|error| OidcError(format!("OIDC access token hash failed: {error}")))?;
            if actual_hash != *expected_hash {
                return Err(OidcError("OIDC access token hash did not match".to_owned()));
            }
        }

        if claims.email_verified() != Some(true) {
            return Err(OidcError(
                "OIDC provider did not supply a verified email".to_owned(),
            ));
        }
        let email = claims
            .email()
            .map(|value| value.as_str().trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| OidcError("OIDC provider omitted the email claim".to_owned()))?;
        let display_name = claims
            .name()
            .and_then(|names| names.get(None))
            .map(|name| name.as_str().trim().to_owned())
            .filter(|name| !name.is_empty())
            .or_else(|| email.split('@').next().map(str::to_owned))
            .ok_or_else(|| OidcError("OIDC provider omitted a usable name".to_owned()))?;
        let picture_url = claims
            .picture()
            .and_then(|pictures| {
                pictures
                    .get(None)
                    .or_else(|| pictures.iter().next().map(|(_, picture)| picture))
            })
            .map(|picture| picture.as_str().trim().to_owned())
            .filter(|picture| !picture.is_empty());

        Ok(VerifiedIdentity {
            issuer: self.issuer.clone(),
            subject: claims.subject().as_str().to_owned(),
            email,
            display_name,
            picture_url,
        })
    }
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn oidc_redirect_url(base_url: &str) -> Result<String, OidcError> {
    let parsed = url::Url::parse(base_url)
        .map_err(|error| OidcError(format!("invalid Pandan base URL: {error}")))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host().is_none() {
        return Err(OidcError(
            "PANDAN_BASE_URL must be an absolute HTTP or HTTPS URL".to_owned(),
        ));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(OidcError(
            "PANDAN_BASE_URL must not include credentials".to_owned(),
        ));
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(OidcError(
            "PANDAN_BASE_URL must not include a query string or fragment".to_owned(),
        ));
    }

    Ok(format!(
        "{}/{OIDC_CALLBACK_PATH}",
        parsed.as_str().trim_end_matches('/')
    ))
}

#[cfg(test)]
mod tests {
    use super::oidc_redirect_url;

    #[test]
    fn derives_oidc_redirect_url_from_base_url() {
        assert_eq!(
            oidc_redirect_url("https://pandan.example.com").unwrap(),
            "https://pandan.example.com/api/auth/oidc/callback"
        );
        assert_eq!(
            oidc_redirect_url("https://example.com/pandan/").unwrap(),
            "https://example.com/pandan/api/auth/oidc/callback"
        );
    }

    #[test]
    fn rejects_invalid_base_urls() {
        for base_url in [
            "pandan.example.com",
            "ftp://pandan.example.com",
            "https://user:secret@pandan.example.com",
            "https://pandan.example.com?source=oidc",
            "https://pandan.example.com#login",
        ] {
            assert!(
                oidc_redirect_url(base_url).is_err(),
                "expected {base_url} to be rejected"
            );
        }
    }
}
