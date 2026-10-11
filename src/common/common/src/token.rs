use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{Error, PasswordHasher, PasswordVerifier, phc::PasswordHash},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE};
use rand::Rng;
use rand::RngExt;
use rand::rng;
use uuid::Uuid;

const UUID_LEN: usize = Uuid::from_u128(0).as_bytes().len(); // 16 bytes
const SECRET_LEN: usize = 48;
const TOKEN_LEN: usize = UUID_LEN + SECRET_LEN;
const MIN_TOKEN_LEN: usize = UUID_LEN + 16;

fn hash_secret(secret: &[u8]) -> Result<String, Error> {
    let mut salt = [0u8; 16];
    rng().fill_bytes(&mut salt);

    let params = Params::new(32768, 2, 2, None).map_err(|_| Error::ParamsInvalid)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    argon2
        .hash_password_with_salt(secret, &salt)
        .map(|hash| hash.to_string())
}

pub fn generate_product_token() -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rng();
    (0..24)
        .map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char)
        .collect()
}

pub fn generate_api_token() -> Result<(Uuid, String, String), Error> {
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

pub fn decode_api_token(token: &str) -> Result<(Uuid, Vec<u8>), Error> {
    let raw = URL_SAFE.decode(token).map_err(|_| Error::EncodingInvalid)?;

    if raw.len() < MIN_TOKEN_LEN {
        return Err(Error::EncodingInvalid);
    }

    let token_id = Uuid::from_slice(&raw[..UUID_LEN]).map_err(|_| Error::EncodingInvalid)?;
    let secret = raw[UUID_LEN..].to_vec();

    Ok((token_id, secret))
}

pub fn verify_api_secret(secret: &[u8], stored_hash: &str) -> Result<bool, Error> {
    let parsed_hash = PasswordHash::new(stored_hash).map_err(|_| Error::EncodingInvalid)?;

    // Use the same parameters for verification (though Argon2 will read them from the hash)
    let params = Params::new(32768, 2, 2, None).map_err(|_| Error::ParamsInvalid)?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let verified = argon2.verify_password(secret, &parsed_hash).is_ok();

    Ok(verified)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Made by the argon2 0.5 implementation. Tokens issued before the upgrade
    /// are stored in this form and must keep verifying.
    const ARGON2_05_HASH: &str = "$argon2id$v=19$m=32768,t=2,p=2$Aaf9/kMGwQExFEjYPHYQ+w$+HcHAP3+SdwL+xx8bEEiFXQ7mHwD60HV8l1S8+9yiFs";

    #[test]
    fn verifies_hashes_made_before_the_argon2_upgrade() {
        assert!(verify_api_secret(b"guardrail-fixture-secret", ARGON2_05_HASH).unwrap());
        assert!(!verify_api_secret(b"wrong-secret", ARGON2_05_HASH).unwrap());
    }

    #[test]
    fn generated_tokens_round_trip() {
        let (token_id, token, hash) = generate_api_token().unwrap();
        assert!(hash.starts_with("$argon2id$v=19$m=32768,t=2,p=2$"));

        let (decoded_id, secret) = decode_api_token(&token).unwrap();
        assert_eq!(decoded_id, token_id);
        assert!(verify_api_secret(&secret, &hash).unwrap());
        assert!(!verify_api_secret(b"not-the-secret", &hash).unwrap());
    }

    #[test]
    fn rejects_malformed_tokens_and_hashes() {
        assert!(decode_api_token("not base64!").is_err());
        assert!(decode_api_token(&URL_SAFE.encode([0u8; 8])).is_err());
        assert!(verify_api_secret(b"secret", "not a phc string").is_err());
    }
}
