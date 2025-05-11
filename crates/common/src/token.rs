use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE};
use rand::RngCore;
use rand::rng;
use uuid::Uuid;

const UUID_LEN: usize = Uuid::from_u128(0).as_bytes().len(); // 16 bytes
const SECRET_LEN: usize = 48;
const TOKEN_LEN: usize = UUID_LEN + SECRET_LEN;
const MIN_TOKEN_LEN: usize = UUID_LEN + 16;

fn hash_secret(secret: &[u8]) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(secret, &salt)
        .map(|hash| hash.to_string())
}

pub fn generate_api_token() -> Result<(Uuid, String, String), argon2::password_hash::Error> {
    let token_id = Uuid::new_v4();

    let mut secret = [0u8; SECRET_LEN];
    let mut rng = rng();
    rng.fill_bytes(&mut secret);

    let mut raw = Vec::with_capacity(TOKEN_LEN);
    raw.extend_from_slice(token_id.as_bytes());
    raw.extend_from_slice(&secret);

    let hash = hash_secret(&secret)?;

    let token = URL_SAFE.encode(&raw);
    Ok((token_id, token, hash))
}

pub fn decode_api_token(token: &str) -> Result<(Uuid, Vec<u8>), argon2::password_hash::Error> {
    let raw = URL_SAFE
        .decode(token)
        .map_err(|_| argon2::password_hash::Error::PhcStringField)?;

    if raw.len() < MIN_TOKEN_LEN {
        return Err(argon2::password_hash::Error::PhcStringField);
    }

    let token_id = Uuid::from_slice(&raw[..UUID_LEN])
        .map_err(|_| argon2::password_hash::Error::PhcStringField)?;
    let secret = raw[UUID_LEN..].to_vec();

    Ok((token_id, secret))
}

pub fn verify_api_secret(
    secret: &[u8],
    stored_hash: &str,
) -> Result<bool, argon2::password_hash::Error> {
    let parsed_hash =
        PasswordHash::new(stored_hash).map_err(|_| argon2::password_hash::Error::Password)?;

    let verified = Argon2::default()
        .verify_password(secret, &parsed_hash)
        .is_ok();

    Ok(verified)
}
