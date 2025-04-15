use std::sync::Arc;

use chrono::Utc;
use common::settings::Settings;
use serde_json::json;
use sqlx::PgPool;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

// Data models
use data::api_token::NewApiToken;
use data::attachment::NewAttachment;
use data::crash::NewCrash;
use data::credentials::NewCredential;
use data::product::NewProduct;
use data::symbols::NewSymbols;
use data::user::NewUser;
use data::version::NewVersion;

// Repos
use repos::api_token::ApiTokenRepo;
use repos::attachment::AttachmentsRepo;
use repos::crash::CrashRepo;
use repos::credentials::CredentialsRepo;
use repos::product::ProductRepo;
use repos::symbols::SymbolsRepo;
use repos::user::UserRepo;
use repos::version::VersionRepo;

// Entity types
use data::api_token::ApiToken;
use data::attachment::Attachment;
use data::crash::Crash;
use data::credentials::Credential;
use data::product::Product;
use data::symbols::Symbols;
use data::user::User;
use data::version::Version;
use webauthn_rs::prelude::Url;
use webauthn_rs::{Webauthn, WebauthnBuilder};

/// Create a test product with a random name
pub async fn create_test_product(pool: &PgPool) -> Uuid {
    let new_product = NewProduct {
        name: format!("TestProduct_{}", Uuid::new_v4()),
        description: "Test Product Description".to_string(),
    };

    ProductRepo::create(pool, new_product)
        .await
        .expect("Failed to insert test product")
}

/// Create a test product with a specific name and description
pub async fn create_test_product_with_details(
    pool: &PgPool,
    name: &str,
    description: &str,
) -> Product {
    let new_product = NewProduct {
        name: name.to_string(),
        description: description.to_string(),
    };

    let product_id = ProductRepo::create(pool, new_product)
        .await
        .expect("Failed to insert test product");

    ProductRepo::get_by_id(pool, product_id)
        .await
        .expect("Failed to retrieve created product")
        .expect("Created product not found")
}

/// Create a test version and its associated product if needed
pub async fn create_test_version(
    pool: &PgPool,
    name: &str,
    hash: &str,
    tag: &str,
    product_id: Option<Uuid>,
) -> Version {
    let product_id = match product_id {
        Some(id) => id,
        None => create_test_product(pool).await,
    };

    let new_version = NewVersion {
        name: name.to_string(),
        hash: hash.to_string(),
        tag: tag.to_string(),
        product_id,
    };

    let version_id = VersionRepo::create(pool, new_version)
        .await
        .expect("Failed to insert test version");

    VersionRepo::get_by_id(pool, version_id)
        .await
        .expect("Failed to retrieve created version")
        .expect("Created version not found")
}

/// Set up common test dependencies - create a product and version
pub async fn setup_test_dependencies(pool: &PgPool) -> (Uuid, Uuid) {
    // Create product first
    let new_product = NewProduct {
        name: format!("TestProduct_{}", Uuid::new_v4()),
        description: "Test Product Description".to_string(),
    };

    let product_id = ProductRepo::create(pool, new_product)
        .await
        .expect("Failed to insert test product");

    // Then create version
    let new_version = NewVersion {
        name: format!("Version_{}", Uuid::new_v4()),
        hash: format!("Hash_{}", Uuid::new_v4()),
        tag: format!("Tag_{}", Uuid::new_v4()),
        product_id,
    };

    let version_id = VersionRepo::create(pool, new_version)
        .await
        .expect("Failed to insert test version");

    (product_id, version_id)
}

/// Create a test crash and its associated product and version if needed
pub async fn create_test_crash(
    pool: &PgPool,
    summary: &str,
    report_data: serde_json::Value,
    product_id: Option<Uuid>,
    version_id: Option<Uuid>,
) -> Crash {
    let (product_id, version_id) = match (product_id, version_id) {
        (Some(pid), Some(vid)) => (pid, vid),
        _ => setup_test_dependencies(pool).await,
    };

    let new_crash = NewCrash {
        summary: summary.to_string(),
        report: report_data,
        product_id,
        version_id,
    };

    let crash_id = CrashRepo::create(pool, new_crash)
        .await
        .expect("Failed to insert test crash");

    CrashRepo::get_by_id(pool, crash_id)
        .await
        .expect("Failed to retrieve created crash")
        .expect("Created crash not found")
}

/// Create a test attachment and its associated crash if needed
pub async fn create_test_attachment(
    pool: &PgPool,
    name: &str,
    mime_type: &str,
    file_size: i64,
    filename: &str,
    product_id: Option<Uuid>,
    crash_id: Option<Uuid>,
) -> Attachment {
    let crash_id = match crash_id {
        Some(id) => id,
        None => {
            let (product_id, version_id) = setup_test_dependencies(pool).await;

            let new_crash = NewCrash {
                summary: "Test Crash".to_string(),
                report: json!({"test": "data"}),
                version_id,
                product_id,
            };

            CrashRepo::create(pool, new_crash)
                .await
                .expect("Failed to insert test crash")
        }
    };

    let product_id = match product_id {
        Some(id) => id,
        None => {
            // Use crash's product_id if not provided
            let crash = CrashRepo::get_by_id(pool, crash_id)
                .await
                .expect("Failed to get crash")
                .expect("Crash not found");
            crash.product_id
        }
    };

    let new_attachment = NewAttachment {
        name: name.to_string(),
        mime_type: mime_type.to_string(),
        size: file_size,
        filename: filename.to_string(),
        crash_id,
        product_id,
    };

    let attachment_id = AttachmentsRepo::create(pool, new_attachment)
        .await
        .expect("Failed to insert test attachment");

    AttachmentsRepo::get_by_id(pool, attachment_id)
        .await
        .expect("Failed to retrieve created attachment")
        .expect("Created attachment not found")
}

#[allow(clippy::too_many_arguments)]
pub async fn create_test_symbols(
    pool: &PgPool,
    os: &str,
    arch: &str,
    build_id: &str,
    module_id: &str,
    file_location: &str,
    product_id: Option<Uuid>,
    version_id: Option<Uuid>,
) -> Symbols {
    let (product_id, version_id) = match (product_id, version_id) {
        (Some(p), Some(v)) => (p, v),
        _ => setup_test_dependencies(pool).await,
    };

    let new_symbols = NewSymbols {
        os: os.to_string(),
        arch: arch.to_string(),
        build_id: build_id.to_string(),
        module_id: module_id.to_string(),
        file_location: file_location.to_string(),
        product_id,
        version_id,
    };

    let symbols_id = SymbolsRepo::create(pool, new_symbols)
        .await
        .expect("Failed to insert test symbols");

    SymbolsRepo::get_by_id(pool, symbols_id)
        .await
        .expect("Failed to retrieve created symbols")
        .expect("Created symbols not found")
}

/// Create a test user
pub async fn create_test_user(pool: &PgPool, username: &str, is_admin: bool) -> User {
    let new_user = NewUser {
        username: username.to_string(),
        is_admin,
    };

    let user_id = UserRepo::create(pool, new_user)
        .await
        .expect("Failed to insert test user");

    UserRepo::get_by_id(pool, user_id)
        .await
        .expect("Failed to retrieve created user")
        .expect("Created user not found")
}

/// Create a test user with a random username
pub async fn create_random_test_user(pool: &PgPool) -> Uuid {
    let username = format!("testuser_{}", Uuid::new_v4());
    let new_user = NewUser {
        username,
        is_admin: false,
    };

    UserRepo::create(pool, new_user)
        .await
        .expect("Failed to create test user")
}

/// Create a test credential
pub async fn create_test_credential(
    pool: &PgPool,
    data: serde_json::Value,
    user_id: Option<Uuid>,
) -> Credential {
    let user_id = match user_id {
        Some(id) => id,
        None => create_random_test_user(pool).await,
    };

    let new_credential = NewCredential { user_id, data };

    let credential_id = CredentialsRepo::create(pool, new_credential)
        .await
        .expect("Failed to insert test credential");

    CredentialsRepo::get_by_id(pool, credential_id)
        .await
        .expect("Failed to retrieve created credential")
        .expect("Created credential not found")
}

/// Hash a token using argon2 (for API token tests)
pub fn hash_token(token: &str) -> String {
    use argon2::{
        Argon2,
        password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
    };

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(token.as_bytes(), &salt)
        .expect("Failed to hash token")
        .to_string()
}

/// Create a test API token
pub async fn create_test_api_token(
    pool: &PgPool,
    description: &str,
    token: &str,
    product_id: Option<Uuid>,
    user_id: Option<Uuid>,
    entitlements: &[&str],
) -> ApiToken {
    let product_id = match product_id {
        Some(id) => id,
        None => create_test_product(pool).await,
    };

    // Convert entitlements to a vector of strings
    let entitlements: Vec<String> = entitlements.iter().map(|&s| s.to_string()).collect();

    // Hash the token using argon2
    let token_hash = hash_token(token);

    // Create the new API token
    let new_token = NewApiToken {
        description: description.to_string(),
        token_hash,
        product_id: Some(product_id),
        user_id,
        entitlements,
        expires_at: Some(Utc::now().naive_utc() + chrono::Duration::days(30)), // Default expiry of 30 days
    };

    // Create the API token using the repository
    let token_id = ApiTokenRepo::create(pool, new_token)
        .await
        .expect("Failed to create API token");

    // Retrieve the created token
    ApiTokenRepo::get_by_id(pool, token_id)
        .await
        .expect("Failed to retrieve the created API token")
        .expect("Created API token not found")
}

pub fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(true)
        .with_level(true)
        .init();

    //    tracing_log::LogTracer::init().expect("Failed to set logger");
}

pub fn create_webauthn(settings: Arc<Settings>) -> Arc<Webauthn> {
    let rp_id = settings.auth.id.as_str();
    let rp_origin = Url::parse(settings.auth.origin.as_str()).expect("Invalid URL");
    let builder = WebauthnBuilder::new(rp_id, &rp_origin).expect("Invalid configuration");
    let builder = builder.rp_name(settings.auth.name.as_str());

    Arc::new(builder.build().expect("Invalid configuration"))
}

pub async fn create_token(
    pool: &PgPool,
    token: &str,
    product: Option<Uuid>,
    entitement: &str,
) -> Uuid {
    let token_hash = hash_token(token);
    let new_token = NewApiToken {
        description: "Test API token".to_string(),
        token_hash,
        product_id: product,
        user_id: None,
        entitlements: vec![entitement.to_string()],
        expires_at: None,
    };

    ApiTokenRepo::create(pool, new_token)
        .await
        .expect("Failed to insert test API token")
}

pub fn create_settings() -> Arc<Settings> {
    let mut settings = Settings::default();
    tracing::info!("Logging initialized");

    settings.auth.id = "localhost".to_string();
    settings.auth.origin = "http://localhost:3000".to_string();
    settings.auth.name = "TestApp".to_string();

    Arc::new(settings)
}
