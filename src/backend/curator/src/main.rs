use clap::Parser;
use std::sync::Arc;
use tracing::info;

use common::{init_logging, settings::Settings};

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

    info!("Starting maintenance worker");
    let app = curator::app::GuardrailCuratorApp::from_settings(settings).await;
    app.run(async {
        tokio::signal::ctrl_c().await?;
        info!("Shutting down the system");
        Ok(())
    })
    .await;
    info!("Maintenance worker has stopped");
}
