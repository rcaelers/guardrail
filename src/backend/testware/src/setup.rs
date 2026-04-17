use std::sync::OnceLock;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tracing_subscriber::EnvFilter;

/// The Ed25519 public key used in tests (matches create_settings() in lib.rs).
const TEST_PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----\
                               MCowBQYDK2VwAyEAJuN0TiFkCg0HnTjpisG1gfVY7XjKsFGuRm1JVmqkt74=\
                               -----END PUBLIC KEY-----";

pub struct TestSetup;

impl TestSetup {
    pub fn init() {
        static INIT: OnceLock<()> = OnceLock::new();

        INIT.get_or_init(|| {
            tracing::info!("Initializing test environment...");
            Self::init_logging();
        });
    }

    fn init_logging() {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .with_target(true)
            .with_level(true)
            .try_init()
            .ok();
    }

    /// Create an in-memory SurrealDB instance with schema applied.
    /// Each call creates a fresh, isolated database.
    pub async fn create_db() -> Surreal<Any> {
        Self::init();

        let db = surrealdb::engine::any::connect("mem://")
            .await
            .expect("Failed to connect to in-memory SurrealDB");

        db.use_ns("test")
            .use_db("test")
            .await
            .expect("Failed to select namespace/database");

        // Apply schema
        let schema = include_str!("../../../../database/schema/guardrail.surql");
        db.query(schema)
            .await
            .expect("Failed to apply SurrealDB schema");

        // Define the JWT-based record access method (mirrors init_guardrail_db)
        db.query(format!(
            r#"DEFINE ACCESS OVERWRITE guardrail_api ON DATABASE TYPE RECORD
                WITH JWT ALGORITHM EDDSA KEY '{TEST_PUBLIC_KEY}'
                DURATION FOR SESSION 1h"#
        ))
        .await
        .expect("Failed to define JWT access method");

        db
    }
}
