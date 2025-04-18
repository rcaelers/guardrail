use std::sync::OnceLock;
use tracing_subscriber::EnvFilter;

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
}
