use clap::{Parser, Subcommand};
use std::sync::Arc;
use tracing::info;

use common::{init_logging, settings::Settings};

#[derive(Parser, Debug)]
#[command(author, version, about = "Guardrail unified service binary")]
struct Cli {
    #[arg(short = 'C', long, default_value = "config", global = true)]
    config_dir: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run the REST API server
    Api,
    /// Run the crash ingestion server
    Ingestion,
    /// Run the minidump processor worker
    Processor,
    /// Run the curator / maintenance worker
    Curator,
    /// Run the web UI server
    Web,
    /// Run all backend services in a single process
    All,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let settings =
        Arc::new(Settings::with_config_dir(&cli.config_dir).expect("Failed to load settings"));

    init_logging().await;
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    match cli.command {
        Command::Api => run_api(settings).await,
        Command::Ingestion => run_ingestion(settings).await,
        Command::Processor => run_processor(settings).await,
        Command::Curator => run_curator(settings).await,
        Command::Web => run_web(settings).await,
        Command::All => run_all(settings).await,
    }
}

async fn run_api(settings: Arc<Settings>) {
    info!("Starting API server on port {}", settings.api_server.port);
    let app = api::app::GuardrailApiApp::from_settings(settings).await;
    if let Err(err) = app.ensure_default_api_token().await {
        tracing::warn!("Failed to ensure default API token: {}", err);
    }
    app.serve().await;
}

async fn run_ingestion(settings: Arc<Settings>) {
    info!("Starting ingestion server on port {}", settings.ingestion_server.port);
    let app = ingestion::app::GuardrailIngestionApp::from_settings(settings).await;
    app.serve().await;
}

async fn run_processor(settings: Arc<Settings>) {
    info!("Starting minidump processor");
    let app = processor::app::GuardrailProcessorApp::from_settings(settings).await;
    app.run(async {
        tokio::signal::ctrl_c().await?;
        info!("Shutting down processor");
        Ok(())
    })
    .await;
}

async fn run_curator(settings: Arc<Settings>) {
    info!("Starting curator");
    let app = curator::app::GuardrailCuratorApp::from_settings(settings).await;
    app.run(async {
        tokio::signal::ctrl_c().await?;
        info!("Shutting down curator");
        Ok(())
    })
    .await;
}

async fn run_web(settings: Arc<Settings>) {
    info!("Starting web server on port {}", settings.web_server.port);
    web::app::GuardrailWebApp::from_settings(settings).await.serve().await;
}

async fn run_all(settings: Arc<Settings>) {
    info!("Starting all services");

    let api_settings = settings.clone();
    let ingestion_settings = settings.clone();
    let processor_settings = settings.clone();
    let curator_settings = settings.clone();
    let web_settings = settings.clone();

    let api_handle = tokio::spawn(async move {
        let app = api::app::GuardrailApiApp::from_settings(api_settings).await;
        if let Err(err) = app.ensure_default_api_token().await {
            tracing::warn!("Failed to ensure default API token: {}", err);
        }
        app.serve().await;
    });

    let ingestion_handle = tokio::spawn(async move {
        let app = ingestion::app::GuardrailIngestionApp::from_settings(ingestion_settings).await;
        app.serve().await;
    });

    let processor_handle = tokio::spawn(async move {
        let app = processor::app::GuardrailProcessorApp::from_settings(processor_settings).await;
        app.run(async {
            tokio::signal::ctrl_c().await?;
            Ok(())
        })
        .await;
    });

    let curator_handle = tokio::spawn(async move {
        let app = curator::app::GuardrailCuratorApp::from_settings(curator_settings).await;
        app.run(async {
            tokio::signal::ctrl_c().await?;
            Ok(())
        })
        .await;
    });

    let web_handle = tokio::spawn(async move {
        web::app::GuardrailWebApp::from_settings(web_settings).await.serve().await;
    });

    let _ = tokio::join!(api_handle, ingestion_handle, processor_handle, curator_handle, web_handle);
}
