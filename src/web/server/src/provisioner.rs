use async_trait::async_trait;
use thiserror::Error;
use url::Url;

#[derive(Debug, Clone)]
pub struct CreateUserRequest {
    pub username: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

#[derive(Debug)]
pub struct ProvisionedUser {
    /// The subject identifier that will appear in the OIDC `sub` claim.
    pub external_id: String,
    /// If `Some`, redirect the user here to complete credential setup (e.g. passkey registration).
    /// If `None`, redirect straight to OIDC login.
    pub setup_url: Option<Url>,
}

#[derive(Debug, Error)]
pub enum ProvisionerError {
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    #[error("user already exists: {0}")]
    UserAlreadyExists(String),

    #[error("identity provider API error: {0}")]
    ApiError(String),
}

#[async_trait]
pub trait IdentityProvisioner: Send + Sync {
    async fn create_user(
        &self,
        req: CreateUserRequest,
    ) -> Result<ProvisionedUser, ProvisionerError>;

    /// Issue a fresh credential-setup URL for an already-provisioned user.
    /// Returns `Some(url)` when the provider supports a direct popup link (e.g. PocketID
    /// one-time token). Returns `None` when no such link is available and the caller
    /// should proceed straight to OIDC login (e.g. Rauthy, which emails the link).
    async fn create_setup_url(&self, external_id: &str) -> Result<Option<Url>, ProvisionerError>;

    /// Look up the identity provider's internal user ID by email, falling back
    /// to username. Returns `None` if no matching user exists.
    async fn find_user_id(
        &self,
        email: &str,
        username: &str,
    ) -> Result<Option<String>, ProvisionerError>;

    /// Issue a short-lived one-time login URL for account recovery (lost passkey).
    /// The TTL is intentionally short — use `create_setup_url` for invitation flows.
    async fn create_recovery_url(&self, external_id: &str) -> Result<Url, ProvisionerError>;
}
