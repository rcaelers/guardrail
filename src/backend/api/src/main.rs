use clap::Parser;
use std::sync::Arc;
use tracing::info;

use api::app::GuardrailApiApp;
use api::settings::Settings;
use common::init_logging;

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

    info!("Starting server on port {}", settings.ingress.port);

    let app = GuardrailApiApp::from_settings(settings).await;

    if let Err(err) = app.ensure_default_api_token().await {
        tracing::warn!("Failed to ensure default API token: {}", err);
    }

    app.serve().await;
}
