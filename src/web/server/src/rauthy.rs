use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::provisioner::{
    CreateUserRequest, IdentityProvisioner, ProvisionedUser, ProvisionerError,
};

pub struct RauthyProvisioner {
    pub api_url: Url,
    pub public_url: Url,
    /// "name$secret" format, used as `Authorization: API-Key {api_key}`
    pub api_key: String,
    pub client: reqwest::Client,
}

#[derive(Serialize)]
struct NewUserRequest<'a> {
    email: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    given_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    family_name: Option<&'a str>,
    language: &'a str,
    roles: Vec<String>,
}

#[derive(Deserialize)]
struct RauthyUser {
    id: String,
}

#[async_trait]
impl IdentityProvisioner for RauthyProvisioner {
    async fn create_user(
        &self,
        req: CreateUserRequest,
    ) -> Result<ProvisionedUser, ProvisionerError> {
        let user_id = self.api_create_user(&req).await?;
        // Rauthy auto-sends a setup email on creation; no immediate setup URL needed.
        Ok(ProvisionedUser {
            external_id: user_id,
            setup_url: None,
        })
    }

    async fn create_setup_url(&self, _external_id: &str) -> Result<Option<Url>, ProvisionerError> {
        // Rauthy has no admin API for one-time setup links — it emails the user automatically
        // on account creation. Return the public URL so the frontend opens a popup there;
        // the user follows the emailed link inside the popup to register their passkey,
        // then closes it manually.
        Ok(Some(self.public_url.clone()))
    }

    async fn find_user_id(
        &self,
        email: &str,
        _username: &str,
    ) -> Result<Option<String>, ProvisionerError> {
        self.api_find_user_by_email(email).await
    }

    async fn create_recovery_url(&self, _external_id: &str) -> Result<Url, ProvisionerError> {
        Ok(self.public_url.clone())
    }
}

impl RauthyProvisioner {
    fn auth_header(&self) -> String {
        format!("API-Key {}", self.api_key)
    }

    async fn api_create_user(
        &self,
        req: &CreateUserRequest,
    ) -> Result<String, ProvisionerError> {
        let url = self
            .api_url
            .join("/auth/v1/users")
            .map_err(|e| ProvisionerError::ApiError(e.to_string()))?;

        let body = NewUserRequest {
            email: &req.email,
            given_name: req.first_name.as_deref(),
            family_name: req.last_name.as_deref(),
            language: "en",
            roles: vec![],
        };

        let response = self
            .client
            .post(url)
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await
            .map_err(|e| ProvisionerError::HttpError(e.to_string()))?;

        let status = response.status();
        if status.as_u16() == 409 {
            return Err(ProvisionerError::UserAlreadyExists(req.email.clone()));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProvisionerError::ApiError(format!(
                "create user returned {status}: {body}"
            )));
        }

        let user: RauthyUser = response
            .json()
            .await
            .map_err(|e| ProvisionerError::ApiError(format!("parse create-user response: {e}")))?;

        Ok(user.id)
    }

    async fn api_find_user_by_email(
        &self,
        email: &str,
    ) -> Result<Option<String>, ProvisionerError> {
        let mut url = self.api_url.clone();
        url.path_segments_mut()
            .map_err(|_| ProvisionerError::ApiError("cannot-be-a-base API URL".into()))?
            .clear()
            .extend(&["auth", "v1", "users", "email", email]);

        let response = self
            .client
            .get(url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| ProvisionerError::HttpError(e.to_string()))?;

        let status = response.status();
        if status.as_u16() == 404 {
            return Ok(None);
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProvisionerError::ApiError(format!(
                "find user by email returned {status}: {body}"
            )));
        }

        let user: RauthyUser = response
            .json()
            .await
            .map_err(|e| ProvisionerError::ApiError(format!("parse user response: {e}")))?;

        tracing::info!(email, user_id = %user.id, "found Rauthy user by email");
        Ok(Some(user.id))
    }
}
