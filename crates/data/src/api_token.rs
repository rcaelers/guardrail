use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct ApiToken {
    pub id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub description: String,
    pub token_id: Uuid,
    pub token_hash: String,
    pub product_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub entitlements: Vec<String>,
    pub last_used_at: Option<NaiveDateTime>,
    pub expires_at: Option<NaiveDateTime>,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewApiToken {
    pub description: String,
    pub token_id: Uuid,
    pub token_hash: String,
    pub product_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub entitlements: Vec<String>,
    pub expires_at: Option<NaiveDateTime>,
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
            let now = Utc::now().naive_utc();
            if expires_at < now {
                return false;
            }
        }

        tracing::info!("Token {} is valid with entitlements: {:?}", self.id, self.entitlements,);

        true
    }
}
