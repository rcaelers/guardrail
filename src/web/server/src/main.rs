use clap::Parser;
use std::sync::Arc;
use tracing::info;

use common::{init_logging, settings::Settings};
use web::app::GuardrailWebApp;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[arg(short = 'C', long, default_value = "config")]
    config_dir: String,
}

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    let settings =
        Arc::new(Settings::with_config_dir(&args.config_dir).expect("Failed to load settings"));

    init_logging().await;

    info!("Starting web server");

    GuardrailWebApp::from_settings(settings).await.serve().await;
}
