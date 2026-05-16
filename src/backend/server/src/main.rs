use clap::{Parser, Subcommand};
use tracing::info;

use common::init_logging;

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

    init_logging().await;
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    match cli.command {
        Command::Api => run_api(cli.config_dir).await,
        Command::Ingestion => run_ingestion(cli.config_dir).await,
        Command::Processor => run_processor(cli.config_dir).await,
        Command::Curator => run_curator(cli.config_dir).await,
        Command::Web => run_web(cli.config_dir).await,
        Command::All => run_all(cli.config_dir).await,
    }
}

async fn run_api(config_dir: String) {
    let settings = std::sync::Arc::new(
        api::settings::Settings::load(&config_dir).expect("Failed to load API settings"),
    );
    info!("Starting API server on port {}", settings.api_server.port);
    let app = api::app::GuardrailApiApp::from_settings(settings).await;
    if let Err(err) = app.ensure_default_api_token().await {
        tracing::warn!("Failed to ensure default API token: {}", err);
    }
    app.serve().await;
}

async fn run_ingestion(config_dir: String) {
    let settings = std::sync::Arc::new(
        ingestion::settings::Settings::load(&config_dir)
            .expect("Failed to load ingestion settings"),
    );
    info!("Starting ingestion server on port {}", settings.ingestion_server.port);
    let app = ingestion::app::GuardrailIngestionApp::from_settings(settings).await;
    app.serve().await;
}

async fn run_processor(config_dir: String) {
    let settings = std::sync::Arc::new(
        processor::settings::Settings::load(&config_dir)
            .expect("Failed to load processor settings"),
    );
    info!("Starting minidump processor");
    let app = processor::app::GuardrailProcessorApp::from_settings(settings).await;
    app.run(async {
        tokio::signal::ctrl_c().await?;
        info!("Shutting down processor");
        Ok(())
    })
    .await;
}

async fn run_curator(config_dir: String) {
    let settings = std::sync::Arc::new(
        curator::settings::Settings::load(&config_dir).expect("Failed to load curator settings"),
    );
    info!("Starting curator");
    let app = curator::app::GuardrailCuratorApp::from_settings(settings).await;
    app.run(async {
        tokio::signal::ctrl_c().await?;
        info!("Shutting down curator");
        Ok(())
    })
    .await;
}

async fn run_web(config_dir: String) {
    let settings = std::sync::Arc::new(
        web::settings::Settings::load(&config_dir).expect("Failed to load web settings"),
    );
    info!("Starting web server on port {}", settings.web_server.port);
    web::app::GuardrailWebApp::from_settings(settings)
        .await
        .serve()
        .await;
}

async fn run_all(config_dir: String) {
    info!("Starting all services");

    let api_handle = tokio::spawn({
        let c = config_dir.clone();
        async move { run_api(c).await }
    });

    let ingestion_handle = tokio::spawn({
        let c = config_dir.clone();
        async move { run_ingestion(c).await }
    });

    let processor_handle = tokio::spawn({
        let c = config_dir.clone();
        async move { run_processor(c).await }
    });

    let curator_handle = tokio::spawn({
        let c = config_dir.clone();
        async move { run_curator(c).await }
    });

    let web_handle = tokio::spawn({
        let c = config_dir.clone();
        async move { run_web(c).await }
    });

    let _ =
        tokio::join!(api_handle, ingestion_handle, processor_handle, curator_handle, web_handle);
}
