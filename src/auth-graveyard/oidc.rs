use async_trait::async_trait;
use oauth2::PkceCodeVerifier;
use openidconnect::{
    core::{CoreAuthenticationFlow, CoreClient, CoreGenderClaim, CoreProviderMetadata},
    reqwest::async_http_client,
    AccessTokenHash, AdditionalClaims, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge, RedirectUrl, Scope,
    SubjectIdentifier, TokenResponse, UserInfoClaims,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};
use url::Url;

use super::error::AuthError;
use crate::settings::settings;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserClaims {
    pub id: SubjectIdentifier,
    pub email: String,
    pub real_name: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthenticationContext {
    pub auth_url: Url,
    pub csrf_token: CsrfToken,
    pub nonce: Nonce,
    pub pkce_verifier: PkceCodeVerifier,
}

#[derive(Debug, Deserialize, Serialize)]
struct ExtraClaims {
    scopes: String,
}
impl AdditionalClaims for ExtraClaims {}

#[async_trait]
pub trait OidcClientTrait {
    async fn authorize(&self) -> Result<AuthenticationContext, AuthError>;
    async fn exchange_code(
        &self,
        context: AuthenticationContext,
        code: String,
        state: String,
    ) -> Result<UserClaims, AuthError>;
}

pub type OidcClientTraitArc = Arc<dyn OidcClientTrait + Sync + Send>;

#[derive(Debug)]
pub struct OidcClient {
    pub client: CoreClient,
}

impl OidcClient {
    pub async fn new() -> Result<Self, AuthError> {
        let issuer_url = IssuerUrl::new(settings().auth.issuer.clone())
            .map_err(|err| AuthError::InvalidIssuerURL(err.to_string()))?;

        let redirect_uri = RedirectUrl::new(format!("{}/auth/callback", settings().server.site))
            .map_err(|err| AuthError::InvalidRedirectURL(err.to_string()))?;

        let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, async_http_client)
            .await
            .map_err(|err| AuthError::DiscoveryError(err.to_string()))?;

        let client_secret = settings().auth.client_secret.clone();
        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(settings().auth.client_id.clone()),
            client_secret.map(ClientSecret::new),
        )
        .set_redirect_uri(redirect_uri);

        Ok(Self { client })
    }
}

#[async_trait]
impl OidcClientTrait for OidcClient {
    async fn authorize(&self) -> Result<AuthenticationContext, AuthError> {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let mut request = self
            .client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .add_scope(Scope::new("admin".to_string()))
            .add_scope(Scope::new("symupload".to_string()))
            .add_scope(Scope::new("view".to_string()))
            .set_pkce_challenge(pkce_challenge);

        if let Some(ref scopes) = settings().auth.scopes {
            for scope in scopes.clone() {
                request = request.add_scope(Scope::new(scope));
            }
        }

        let (auth_url, csrf_token, nonce) = request.url();

        let context = AuthenticationContext {
            nonce,
            csrf_token,
            auth_url,
            pkce_verifier,
        };
        Ok(context)
    }

    async fn exchange_code(
        &self,
        context: AuthenticationContext,
        code: String,
        state: String,
    ) -> Result<UserClaims, AuthError> {
        let token_response = self
            .client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(context.pkce_verifier)
            .request_async(async_http_client)
            .await
            .map_err(|err| AuthError::TokenExchangeFailed(err.to_string()))?;

        let id_token = token_response
            .id_token()
            .ok_or(AuthError::TokenExchangeFailed("missing token".to_owned()))?;

        let claims = id_token
            .claims(
                &self
                    .client
                    .id_token_verifier()
                    .set_other_audience_verifier_fn(|aud| {
                        debug!("Audience: {}", aud.as_str());
                        settings().auth.audiences.iter().any(|i| i == aud.as_str())
                    }),
                &context.nonce,
            )
            .map_err(|err| AuthError::ClaimVerificationError(err.to_string()))?;

        if let Some(expected_access_token_hash) = claims.access_token_hash() {
            let actual_access_token_hash = AccessTokenHash::from_token(
                token_response.access_token(),
                &id_token
                    .signing_alg()
                    .map_err(|err| AuthError::TokenSigningError(err.to_string()))?,
            )
            .map_err(|_err| AuthError::Failure)?;

            if actual_access_token_hash != *expected_access_token_hash {
                return Err(AuthError::TokenMismatch);
            }
        }

        info!(
            "User {} with e-mail address {} has authenticated successfully",
            claims.subject().as_str(),
            claims
                .email()
                .map(|email| email.as_str())
                .unwrap_or("<not provided>"),
        );

        info!("Claims : {:?}", claims);
        info!("Scopes: {:?}", token_response.scopes());

        let user_claims: UserInfoClaims<ExtraClaims, CoreGenderClaim> = self
            .client
            .user_info(token_response.access_token().to_owned(), None)
            .map_err(|_err| AuthError::Failure)?
            .request_async(async_http_client)
            .await
            .map_err(|_err| AuthError::Failure)?;

        info!("User Claims : {:?}", user_claims);

        let validity = token_response
            .expires_in()
            .ok_or(AuthError::ResponseFieldError {
                field: "expired_in".to_string(),
                reason: "missing".to_string(),
            })?;

        // TODO: use validity
        info!("Token is valid for {} seconds", validity.as_secs());

        let user = UserClaims {
            id: claims.subject().clone(),
            email: claims
                .email()
                .ok_or(AuthError::ResponseFieldError {
                    field: "e-mail".to_string(),
                    reason: "missing".to_string(),
                })?
                .to_string(),
            real_name: claims
                .name()
                .ok_or(AuthError::ResponseFieldError {
                    field: "name".to_string(),
                    reason: "missing".to_string(),
                })?
                .get(None)
                .ok_or(AuthError::ResponseFieldError {
                    field: "name".to_string(),
                    reason: "missing".to_string(),
                })?
                .to_string(),
            scopes: user_claims
                .additional_claims()
                .scopes
                .split_whitespace()
                .map(|s| s.to_string())
                .collect(),
        };

        Ok(user)
    }
}

#[cfg(test)]
pub mod test_stubs {
    use async_trait::async_trait;
    use oauth2::CsrfToken;
    use openidconnect::Nonce;

    use crate::auth::oidc::{AuthenticationContext, OidcClientTrait};

    pub struct OidcClientStub;

    #[async_trait]
    impl OidcClientTrait for OidcClientStub {
        async fn authorize(&self) -> Result<AuthenticationContext, crate::auth::error::AuthError> {
            let context: AuthenticationContext = AuthenticationContext {
                nonce: Nonce::new_random(),
                csrf_token: CsrfToken::new_random(),
                auth_url: url::Url::parse("http://localhost").unwrap(),
                pkce_verifier: oauth2::PkceCodeVerifier::new("x".to_string()),
            };

            Ok(context)
        }

        async fn exchange_code(
            &self,
            _context: AuthenticationContext,
            _code: String,
            _state: String,
        ) -> Result<crate::auth::oidc::UserClaims, crate::auth::error::AuthError> {
            Err(crate::auth::error::AuthError::InvalidTokenExchange)
        }
    }
}
