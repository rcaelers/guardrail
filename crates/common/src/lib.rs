pub mod settings;


#[cfg(feature = "ssr")]
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
#[cfg(feature = "ssr")]
use rand::{Rng, distr::Alphanumeric, rng};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "ssr")]
pub fn hash_token(token: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(token.as_bytes(), &salt)
        .map(|hash| hash.to_string())
}

#[cfg(feature = "ssr")]
pub fn verify_token(token: &str, hash: &str) -> Result<bool, argon2::password_hash::Error> {
    let parsed_hash = PasswordHash::new(hash)?;
    let argon2 = Argon2::default();

    match argon2.verify_password(token.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(feature = "ssr")]
#[allow(dead_code)]
pub fn generate_token() -> String {
    let mut rng = rng();
    let token: String = std::iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(36)
        .collect();
    token
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub id: Uuid,
    pub username: String,
    pub is_admin: bool,
}

impl AuthenticatedUser {
    pub fn new(id: uuid::Uuid, username: String, is_admin: bool) -> Self {
        Self {
            id,
            username,
            is_admin,
        }
    }
}

use std::{collections::VecDeque, ops::Range};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    pub fn to_sql(&self) -> &'static str {
        match self {
            SortOrder::Ascending => "ASC",
            SortOrder::Descending => "DESC",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct QueryParams {
    #[serde(default)]
    pub sorting: VecDeque<(String, SortOrder)>,
    pub range: Option<Range<usize>>,
    pub filter: Option<String>,
}

