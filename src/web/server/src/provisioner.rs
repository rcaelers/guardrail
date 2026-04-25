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
    /// Called when a user returns to redeem an invite they previously abandoned.
    async fn create_setup_url(
        &self,
        external_id: &str,
    ) -> Result<Url, ProvisionerError>;
}
