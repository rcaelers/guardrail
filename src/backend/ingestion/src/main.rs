use clap::Parser;
use std::sync::Arc;
use tracing::info;

use common::init_logging;
use ingestion::app::GuardrailIngestionApp;
use ingestion::settings::Settings;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[arg(short = 'C', long, default_value = "config")]
    config_dir: String,
}

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    let settings = Arc::new(Settings::load(&args.config_dir).expect("Failed to load settings"));

    init_logging().await;
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    info!("Starting ingestion server on port {}", settings.ingress.port);

    let app = GuardrailIngestionApp::from_settings(settings).await;
    app.serve().await;
}
