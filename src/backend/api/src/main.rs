use clap::Parser;
use k8s_openapi::api::core::v1::Secret;
use kube::{
    Api, Client,
    api::{ObjectMeta, PostParams},
};
use std::sync::Arc;
use tracing::info;

use api::app::GuardrailApiApp;
use common::token::generate_api_token;
use common::{init_logging, settings::Settings};
use repos::Repo;

const SECRET_NAME: &str = "guardrail-initial-admin-token";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[arg(short = 'C', long, default_value = "config")]
    config_dir: String,
}

async fn create_k8s_initial_token_secret(token: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::try_default().await?;
    let namespace =
        std::fs::read_to_string("/var/run/secrets/kubernetes.io/serviceaccount/namespace")
            .unwrap_or_else(|_| {
                tracing::warn!("Could not determine current namespace, using 'default'");
                "default".to_string()
            });

    let secrets: Api<Secret> = Api::namespaced(client, &namespace);

    if secrets.get_opt(SECRET_NAME).await?.is_some() {
        return Ok(());
    }

    let secret = Secret {
        metadata: ObjectMeta {
            name: Some(SECRET_NAME.to_string()),
            labels: Some(
                [("app.kubernetes.io/part-of".to_string(), "guardrail".to_string())].into(),
            ),
            ..Default::default()
        },
        string_data: Some([("token".to_string(), token.to_string())].into()),
        type_: Some("Opaque".to_string()),
        ..Default::default()
    };

    secrets
        .create(&PostParams::default(), &secret)
        .await
        .expect("Failed to create secret");
    Ok(())
}

async fn ensure_default_api_token(repo: &Repo) -> Result<(), Box<dyn std::error::Error>> {
    use data::api_token::NewApiToken;
    use repos::api_token::ApiTokenRepo;

    let tokens = ApiTokenRepo::get_all(&repo.db).await?;
    if !tokens.is_empty() {
        info!("API tokens already exist, skipping default token creation");
        return Ok(());
    }

    let (token_id, token, token_hash) =
        generate_api_token().map_err(|_| "Failed to generate API token")?;

    let new_token = NewApiToken {
        description: "Default API token".to_string(),
        token_id,
        token_hash,
        product_id: None,
        user_id: None,
        entitlements: vec!["token".to_string()],
        expires_at: None,
        is_active: true,
    };

    let _token_id = ApiTokenRepo::create(&repo.db, new_token).await?;
    info!("Created default API token");

    if let Err(err) = create_k8s_initial_token_secret(&token).await {
        tracing::warn!("Failed to create initial token secret: {}", err);
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    let settings =
        Arc::new(Settings::with_config_dir(&args.config_dir).expect("Failed to load settings"));

    init_logging().await;
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    info!("Starting server on port {}", settings.api_server.port);

    let app = GuardrailApiApp::from_settings(settings).await;

    if let Err(err) = ensure_default_api_token(app.repo()).await {
        tracing::warn!("Failed to ensure default API token: {}", err);
    }

    app.serve().await;
}
