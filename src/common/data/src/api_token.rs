use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const ENTITLEMENT_INVITATION_CREATE: &str = "invitation-create";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiToken {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub description: String,
    pub token_id: Uuid,
    pub token_hash: String,
    #[serde(default)]
    pub product_id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    pub entitlements: Vec<String>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewApiToken {
    pub description: String,
    pub token_id: Uuid,
    pub token_hash: String,
    pub product_id: Option<String>,
    pub user_id: Option<String>,
    pub entitlements: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
}

impl ApiToken {
    pub fn has_entitlement(&self, required_entitlement: &str) -> bool {
        tracing::info!(
            "Checking entitlement for token {} {:?} : {}",
            self.id,
            self.entitlements,
            required_entitlement,
        );
        if !self
            .entitlements
            .contains(&required_entitlement.to_string())
        {
            return false;
        }

        tracing::info!("Token {} has required entitlement: {:?}", self.id, required_entitlement,);
        true
    }

    pub fn is_valid(&self) -> bool {
        if !self.is_active {
            return false;
        }

        if let Some(expires_at) = self.expires_at {
            let now = Utc::now();
            if expires_at < now {
                return false;
            }
        }

        tracing::info!("Token {} is valid with entitlements: {:?}", self.id, self.entitlements,);

        true
    }
}
