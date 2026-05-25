use std::path::PathBuf;

use chrono::Utc;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

pub mod mockall_object_store;
pub mod setup;

// Data models
use common::token::generate_api_token;
use data::api_token::NewApiToken;
use data::attachment::NewAttachment;
use data::crash::NewCrash;
use data::product::NewProduct;
use data::symbols::NewSymbols;
use data::user::NewUser;

// Repos
use repos::api_token::ApiTokenRepo;
use repos::attachment::AttachmentsRepo;
use repos::crash::CrashRepo;
use repos::product::ProductRepo;
use repos::symbols::SymbolsRepo;
use repos::user::UserRepo;

// Entity types
use data::api_token::ApiToken;
use data::attachment::Attachment;
use data::crash::Crash;
use data::product::Product;
use data::symbols::Symbols;
use data::user::User;

/// Create a test product with a random name
pub async fn create_test_product(db: &Surreal<Any>) -> Product {
    let new_product = NewProduct {
        name: format!("TestProduct_{}", Uuid::new_v4()),
        description: "Test Product Description".to_string(),
        ..Default::default()
    };

    let product_id = ProductRepo::create(db, new_product)
        .await
        .expect("Failed to insert test product");

    ProductRepo::get_by_id(db, product_id)
        .await
        .expect("Failed to retrieve created product")
        .expect("Created product not found")
}

/// Create a test product with a specific name and description
pub async fn create_test_product_with_details(
    db: &Surreal<Any>,
    name: &str,
    description: &str,
) -> Product {
    let new_product = NewProduct {
        name: name.to_string(),
        description: description.to_string(),
        ..Default::default()
    };

    let product_id = ProductRepo::create(db, new_product)
        .await
        .expect("Failed to insert test product");

    ProductRepo::get_by_id(db, product_id)
        .await
        .expect("Failed to retrieve created product")
        .expect("Created product not found")
}

/// Create a test crash and its associated product if needed
pub async fn create_test_crash(
    db: &Surreal<Any>,
    fingerprint: Option<&str>,
    product_id: Option<String>,
) -> Crash {
    let product_id = match product_id {
        Some(pid) => pid,
        None => create_test_product(db).await.id,
    };

    let new_crash = NewCrash {
        id: None,
        minidump: None,
        product_id,
        report: Some(serde_json::json!({
            "error": "Test error",
            "stacktrace": "Test stack trace"
        })),
        fingerprint: fingerprint.map(|s| s.to_string()),
        group_id: None,
    };

    let crash_id = CrashRepo::create(db, new_crash)
        .await
        .expect("Failed to insert test crash");

    CrashRepo::get_by_id(db, crash_id)
        .await
        .expect("Failed to retrieve created crash")
        .expect("Created crash not found")
}

/// Create a test attachment and its associated crash if needed
pub async fn create_test_attachment(
    db: &Surreal<Any>,
    name: &str,
    mime_type: &str,
    file_size: i64,
    filename: &str,
    product_id: Option<String>,
    crash_id: Option<String>,
) -> Attachment {
    let crash_id = match crash_id {
        Some(id) => id,
        None => {
            let product = create_test_product(db).await;

            let new_crash = NewCrash {
                id: None,
                minidump: None,
                product_id: product.id,
                report: Some(serde_json::json!({
                    "error": "Test error",
                    "stacktrace": "Test stack trace"
                })),
                fingerprint: Some("test_signature".to_string()),
                group_id: None,
            };

            CrashRepo::create(db, new_crash)
                .await
                .expect("Failed to insert test crash")
        }
    };

    let product_id = match product_id {
        Some(id) => id,
        None => {
            // Use crash's product_id if not provided
            let crash = CrashRepo::get_by_id(db, &crash_id)
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
        storage_path: format!("test_storage/{}.{}", Uuid::new_v4(), filename),
        crash_id,
        product_id,
    };

    let attachment_id = AttachmentsRepo::create(db, new_attachment)
        .await
        .expect("Failed to insert test attachment");

    AttachmentsRepo::get_by_id(db, attachment_id)
        .await
        .expect("Failed to retrieve created attachment")
        .expect("Created attachment not found")
}

#[allow(clippy::too_many_arguments)]
pub async fn create_test_symbols(
    db: &Surreal<Any>,
    os: &str,
    arch: &str,
    build_id: &str,
    module_id: &str,
    storage_path: &str,
    product_id: Option<String>,
) -> Symbols {
    let product_id = match product_id {
        Some(p) => p,
        _ => create_test_product(db).await.id.to_string(),
    };

    let new_symbols = NewSymbols {
        os: os.to_string(),
        arch: arch.to_string(),
        build_id: build_id.to_string(),
        module_id: module_id.to_string(),
        storage_path: storage_path.to_string(),
        product_id,
        version: String::new(),
        channel: String::new(),
        commit: String::new(),
        build_tag: String::new(),
    };

    let symbols_id = SymbolsRepo::create(db, new_symbols)
        .await
        .expect("Failed to insert test symbols");

    SymbolsRepo::get_by_id(db, symbols_id)
        .await
        .expect("Failed to retrieve created symbols")
        .expect("Created symbols not found")
}

/// Create a test user
pub async fn create_test_user(db: &Surreal<Any>, username: &str, is_admin: bool) -> User {
    let new_user = NewUser {
        username: username.to_string(),
        email: None,
        name: None,
        is_admin,
    };

    let user_id = UserRepo::create(db, new_user)
        .await
        .expect("Failed to insert test user");

    UserRepo::get_by_id(db, user_id)
        .await
        .expect("Failed to retrieve created user")
        .expect("Created user not found")
}

/// Create a test user with a random username
pub async fn create_random_test_user(db: &Surreal<Any>) -> String {
    let username = format!("testuser_{}", Uuid::new_v4());
    let new_user = NewUser {
        username,
        email: None,
        name: None,
        is_admin: false,
    };

    UserRepo::create(db, new_user)
        .await
        .expect("Failed to create test user")
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

pub fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(true)
        .with_level(true)
        .init();
}

/// Returns the workspace root config directory path, suitable for use in test settings.
pub fn workspace_config_dir() -> String {
    std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().expect("Failed to get current directory"))
        .ancestors()
        .nth(3)
        .expect("Failed to find workspace root")
        .join("config")
        .to_string_lossy()
        .to_string()
}

pub async fn create_test_token(
    db: &Surreal<Any>,
    description: &str,
    product: Option<String>,
    user: Option<String>,
    entitlements: &[&str],
) -> (String, ApiToken) {
    let (token_id, token, token_hash) = generate_api_token().expect("Failed to generate API token");

    let entitlements: Vec<String> = entitlements.iter().map(|&s| s.to_string()).collect();
    let new_token = NewApiToken {
        description: description.to_string(),
        token_id,
        token_hash,
        product_id: product,
        user_id: user,
        entitlements,
        expires_at: Some(Utc::now() + chrono::Duration::days(30)), // Default expiry of 30 days
        is_active: true,
    };

    let id = ApiTokenRepo::create(db, new_token)
        .await
        .expect("Failed to insert test API token");

    let api_token = ApiTokenRepo::get_by_id(db, id)
        .await
        .expect("Failed to retrieve the created API token")
        .expect("Created API token not found");

    (token, api_token)
}
