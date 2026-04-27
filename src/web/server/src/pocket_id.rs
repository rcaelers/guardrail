use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::provisioner::{
    CreateUserRequest, IdentityProvisioner, ProvisionedUser, ProvisionerError,
};

pub struct PocketIdProvisioner {
    pub api_url: Url,
    pub public_url: Url,
    pub api_key: String,
    /// Path prefix for the passkey setup page; token is appended with a `/` separator.
    /// e.g. "/lc/" → "{public_url}/lc/{token}"
    pub setup_path: String,
    /// If set, appended as `?redirect=<url>` so PocketID sends the user here after passkey setup.
    pub post_setup_redirect: Option<String>,
    pub client: reqwest::Client,
}

// --- Pocket ID request / response shapes ---

#[derive(Serialize)]
struct CreateUserBody<'a> {
    username: &'a str,
    email: &'a str,
    #[serde(rename = "firstName", skip_serializing_if = "Option::is_none")]
    first_name: Option<&'a str>,
    #[serde(rename = "lastName", skip_serializing_if = "Option::is_none")]
    last_name: Option<&'a str>,
    #[serde(rename = "isAdmin")]
    is_admin: bool,
}

#[derive(Deserialize)]
struct CreateUserResponse {
    id: String,
}

#[derive(Deserialize)]
struct OneTimeAccessTokenResponse {
    token: String,
}

// --- Implementation ---

#[async_trait]
impl IdentityProvisioner for PocketIdProvisioner {
    async fn create_user(
        &self,
        req: CreateUserRequest,
    ) -> Result<ProvisionedUser, ProvisionerError> {
        let user_id = self.create_pocket_id_user(&req).await?;
        let setup_url = self.create_setup_url(&user_id).await?;
        Ok(ProvisionedUser {
            external_id: user_id,
            setup_url: Some(setup_url),
        })
    }

    async fn create_setup_url(&self, external_id: &str) -> Result<Url, ProvisionerError> {
        let token = self.create_one_time_token(external_id).await?;
        let path = format!("{}/{}", self.setup_path.trim_end_matches('/'), token);
        let mut url = self
            .public_url
            .join(&path)
            .map_err(|e| ProvisionerError::ApiError(e.to_string()))?;
        if let Some(redirect) = &self.post_setup_redirect {
            url.query_pairs_mut().append_pair("redirect", redirect);
        }
        Ok(url)
    }
}

impl PocketIdProvisioner {
    async fn create_pocket_id_user(
        &self,
        req: &CreateUserRequest,
    ) -> Result<String, ProvisionerError> {
        let url = self
            .api_url
            .join("/api/users")
            .map_err(|e| ProvisionerError::ApiError(e.to_string()))?;

        let body = CreateUserBody {
            username: &req.username,
            email: &req.email,
            first_name: req.first_name.as_deref(),
            last_name: req.last_name.as_deref(),
            is_admin: false,
        };

        let response = self
            .client
            .post(url)
            .header("X-API-KEY", &self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProvisionerError::HttpError(e.to_string()))?;

        let status = response.status();
        if status.as_u16() == 409 {
            return Err(ProvisionerError::UserAlreadyExists(req.username.clone()));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProvisionerError::ApiError(format!(
                "create user returned {status}: {body}"
            )));
        }

        let user: CreateUserResponse = response
            .json()
            .await
            .map_err(|e| ProvisionerError::ApiError(format!("parse create-user response: {e}")))?;

        Ok(user.id)
    }

    async fn create_one_time_token(&self, user_id: &str) -> Result<String, ProvisionerError> {
        let url = self
            .api_url
            .join(&format!("/api/users/{user_id}/one-time-access-token"))
            .map_err(|e| ProvisionerError::ApiError(e.to_string()))?;

        let response = self
            .client
            .post(url)
            .header("X-API-KEY", &self.api_key)
            .json(&serde_json::json!({ "ttl": "168h" }))
            .send()
            .await
            .map_err(|e| ProvisionerError::HttpError(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProvisionerError::ApiError(format!(
                "one-time-access-token returned {status}: {body}"
            )));
        }

        let token_data: OneTimeAccessTokenResponse = response
            .json()
            .await
            .map_err(|e| ProvisionerError::ApiError(format!("parse token response: {e}")))?;

        Ok(token_data.token)
    }
}
