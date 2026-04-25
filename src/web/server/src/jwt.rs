use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};

use common::settings::Settings;

pub const SURREAL_ACCESS_METHOD: &str = "guardrail_api";

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub username: String,
    pub user_id: Option<String>,
    pub is_admin: bool,
    pub sub: String,
    pub iss: String,
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    pub ac: String,
    pub ns: String,
    pub db: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

pub fn make_jwt(
    username: &str,
    user_id: Option<&str>,
    is_admin: bool,
    settings: &Settings,
) -> Result<String, jsonwebtoken::errors::Error> {
    let validity = settings.auth.jwk.token_validity_in_minutes;
    let validity = if validity > 0 { validity } else { 60 };
    let now = Utc::now();
    let claims = JwtClaims {
        username: username.to_string(),
        user_id: user_id.map(String::from),
        is_admin,
        sub: username.to_string(),
        iss: settings.auth.id.clone(),
        aud: "guardrail".to_string(),
        exp: (now + Duration::minutes(validity)).timestamp(),
        iat: now.timestamp(),
        ac: SURREAL_ACCESS_METHOD.to_string(),
        ns: settings.database.namespace.clone(),
        db: settings.database.database.clone(),
        id: user_id.map(String::from),
    };
    let key = EncodingKey::from_ed_pem(settings.auth.jwk.private_key.as_bytes())?;
    encode(&Header::new(Algorithm::EdDSA), &claims, &key)
}

pub fn make_anon_jwt(settings: &Settings) -> Result<String, jsonwebtoken::errors::Error> {
    make_jwt("anonymous", None, false, settings)
}
