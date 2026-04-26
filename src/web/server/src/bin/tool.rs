use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand};
use common::{QueryParams, settings::Settings, token::generate_api_token};
use data::{
    api_token::{ApiToken, ENTITLEMENT_INVITATION_CREATE, NewApiToken},
    invitation::{Invitation, InvitationGrant, NewInvitation},
    product::{NewProduct, Product},
};
use repos::{api_token::ApiTokenRepo, invitation::InvitationRepo, product::ProductRepo};
use serde::Serialize;
use serde_json::Value;
use surrealdb::{Surreal, engine::any::Any, opt::auth::Root};

type AnyErr = Box<dyn std::error::Error + Send + Sync>;

#[derive(Parser, Debug)]
#[command(author, version, about = "Guardrail administration tool")]
struct Cli {
    #[arg(short = 'C', long, default_value = "config")]
    config_dir: String,

    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Invite(InviteCommand),
    Token(TokenCommand),
    Product(ProductCommand),
}

#[derive(Args, Debug)]
struct InviteCommand {
    #[command(subcommand)]
    command: InviteSubcommand,
}

#[derive(Subcommand, Debug)]
enum InviteSubcommand {
    List,
    Create(InviteCreateArgs),
    #[command(alias = "revoke")]
    Remove(IdArgs),
}

#[derive(Args, Debug)]
struct InviteCreateArgs {
    #[arg(long)]
    admin: bool,

    #[arg(long = "grant", value_name = "PRODUCT_ID:ROLE")]
    grants: Vec<String>,

    #[arg(long)]
    expires_at: Option<DateTime<Utc>>,

    #[arg(long)]
    max_uses: Option<u32>,

    #[arg(long, default_value = "guardrailctl")]
    created_by: String,

    #[arg(long)]
    create_api_key: bool,

    #[arg(long, default_value = "Invite CLI")]
    api_key_description: String,

    #[arg(long)]
    api_key_product_id: Option<String>,
}

#[derive(Args, Debug)]
struct TokenCommand {
    #[command(subcommand)]
    command: TokenSubcommand,
}

#[derive(Subcommand, Debug)]
enum TokenSubcommand {
    List,
    Create(TokenCreateArgs),
    #[command(alias = "delete")]
    Remove(IdArgs),
    Revoke(IdArgs),
}

#[derive(Args, Debug)]
struct TokenCreateArgs {
    #[arg(long, default_value = "Tool-created API token")]
    description: String,

    #[arg(long = "entitlement")]
    entitlements: Vec<String>,

    #[arg(long)]
    product_id: Option<String>,

    #[arg(long)]
    user_id: Option<String>,

    #[arg(long)]
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Args, Debug)]
struct ProductCommand {
    #[command(subcommand)]
    command: ProductSubcommand,
}

#[derive(Subcommand, Debug)]
enum ProductSubcommand {
    List,
    Create(ProductCreateArgs),
    #[command(alias = "delete")]
    Remove(IdArgs),
}

#[derive(Args, Debug)]
struct ProductCreateArgs {
    #[arg(long)]
    name: String,

    #[arg(long, default_value = "")]
    description: String,

    #[arg(long)]
    public: bool,

    #[arg(long)]
    metadata: Option<String>,
}

#[derive(Args, Debug)]
struct IdArgs {
    id: String,
}

#[derive(Serialize)]
struct CreatedToken {
    id: String,
    token: String,
    entitlements: Vec<String>,
}

#[derive(Serialize)]
struct InviteCreateOutput {
    invitation: Invitation,
    invite_url: String,
    api_token: Option<CreatedToken>,
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("guardrailctl failed: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), AnyErr> {
    let cli = Cli::parse();
    let settings = Settings::with_config_dir(&cli.config_dir)?;
    let db = connect_db(&settings).await?;

    match &cli.command {
        Command::Invite(command) => run_invite(&cli, &settings, &db, command).await?,
        Command::Token(command) => run_token(&cli, &db, command).await?,
        Command::Product(command) => run_product(&cli, &db, command).await?,
    }

    Ok(())
}

async fn connect_db(settings: &Settings) -> Result<Surreal<Any>, AnyErr> {
    let db = surrealdb::engine::any::connect(&settings.database.endpoint).await?;
    db.signin(Root {
        username: settings.database.username.clone(),
        password: settings.database.password.clone(),
    })
    .await?;
    db.use_ns(&settings.database.namespace)
        .use_db(&settings.database.database)
        .await?;
    Ok(db)
}

async fn run_invite(
    cli: &Cli,
    settings: &Settings,
    db: &Surreal<Any>,
    command: &InviteCommand,
) -> Result<(), AnyErr> {
    match &command.command {
        InviteSubcommand::List => {
            let invitations = InvitationRepo::get_all(db, QueryParams::default()).await?;
            print_invites(cli, &invitations)?;
        }
        InviteSubcommand::Create(args) => {
            let grants = parse_grants(&args.grants)?;
            if !args.admin && grants.is_empty() {
                return Err("non-admin invitations need at least one --grant".into());
            }

            let invitation = InvitationRepo::create(
                db,
                NewInvitation {
                    created_by: args.created_by.trim().to_string(),
                    expires_at: args.expires_at,
                    max_uses: args.max_uses,
                    is_admin: args.admin,
                    grants,
                },
            )
            .await?;

            let api_token = if args.create_api_key {
                Some(
                    create_token(
                        db,
                        &args.api_key_description,
                        vec![ENTITLEMENT_INVITATION_CREATE.to_string()],
                        args.api_key_product_id.clone(),
                        None,
                        None,
                    )
                    .await?,
                )
            } else {
                None
            };

            let invite_url = format!(
                "{}/invite/{}",
                settings.auth.origin.trim_end_matches('/'),
                invitation.code
            );
            let output = InviteCreateOutput {
                invitation,
                invite_url,
                api_token,
            };
            print_invite_create(cli, &output)?;
        }
        InviteSubcommand::Remove(args) => {
            InvitationRepo::revoke(db, &args.id).await?;
            print_status(cli, "revoked", "invitation", &args.id)?;
        }
    }
    Ok(())
}

async fn run_token(cli: &Cli, db: &Surreal<Any>, command: &TokenCommand) -> Result<(), AnyErr> {
    match &command.command {
        TokenSubcommand::List => {
            let tokens = ApiTokenRepo::get_all(db).await?;
            print_tokens(cli, &tokens)?;
        }
        TokenSubcommand::Create(args) => {
            let entitlements = if args.entitlements.is_empty() {
                vec![ENTITLEMENT_INVITATION_CREATE.to_string()]
            } else {
                args.entitlements
                    .iter()
                    .map(|entitlement| entitlement.trim().to_string())
                    .filter(|entitlement| !entitlement.is_empty())
                    .collect()
            };
            let created = create_token(
                db,
                &args.description,
                entitlements,
                args.product_id.clone(),
                args.user_id.clone(),
                args.expires_at,
            )
            .await?;
            print_created_token(cli, &created)?;
        }
        TokenSubcommand::Remove(args) => {
            ApiTokenRepo::delete(db, &args.id).await?;
            print_status(cli, "deleted", "token", &args.id)?;
        }
        TokenSubcommand::Revoke(args) => {
            ApiTokenRepo::revoke(db, &args.id).await?;
            print_status(cli, "revoked", "token", &args.id)?;
        }
    }
    Ok(())
}

async fn run_product(cli: &Cli, db: &Surreal<Any>, command: &ProductCommand) -> Result<(), AnyErr> {
    match &command.command {
        ProductSubcommand::List => {
            let products = ProductRepo::get_all(db, QueryParams::default()).await?;
            print_products(cli, &products)?;
        }
        ProductSubcommand::Create(args) => {
            let metadata = match args.metadata.as_deref() {
                Some(raw) => serde_json::from_str(raw)?,
                None => Value::Object(Default::default()),
            };
            let id = ProductRepo::create(
                db,
                NewProduct {
                    name: args.name.trim().to_string(),
                    description: args.description.trim().to_string(),
                    public: args.public,
                    metadata,
                },
            )
            .await?;
            print_status(cli, "created", "product", &id)?;
        }
        ProductSubcommand::Remove(args) => {
            ProductRepo::remove(db, &args.id).await?;
            print_status(cli, "removed", "product", &args.id)?;
        }
    }
    Ok(())
}

async fn create_token(
    db: &Surreal<Any>,
    description: &str,
    entitlements: Vec<String>,
    product_id: Option<String>,
    user_id: Option<String>,
    expires_at: Option<DateTime<Utc>>,
) -> Result<CreatedToken, AnyErr> {
    if entitlements.is_empty() {
        return Err("at least one entitlement is required".into());
    }

    let (token_id, token, token_hash) =
        generate_api_token().map_err(|err| format!("failed to generate API token: {err}"))?;
    let new_token = NewApiToken {
        description: description.trim().to_string(),
        token_id,
        token_hash,
        product_id: product_id
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        user_id: user_id
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        entitlements: entitlements.clone(),
        expires_at,
        is_active: true,
    };

    let id = ApiTokenRepo::create(db, new_token).await?;
    Ok(CreatedToken {
        id,
        token,
        entitlements,
    })
}

fn parse_grants(raw_grants: &[String]) -> Result<Vec<InvitationGrant>, String> {
    raw_grants
        .iter()
        .map(|raw| {
            let (product_id, role) = raw
                .split_once(':')
                .ok_or_else(|| format!("invalid grant '{raw}', expected product_id:role"))?;
            let product_id = product_id.trim();
            let role = role.trim();
            if product_id.is_empty() {
                return Err(format!("invalid grant '{raw}', product_id is empty"));
            }
            if !matches!(role, "readonly" | "readwrite" | "maintainer") {
                return Err(format!(
                    "invalid role '{role}', expected readonly, readwrite, or maintainer"
                ));
            }
            Ok(InvitationGrant {
                product_id: product_id.to_string(),
                role: role.to_string(),
            })
        })
        .collect()
}

fn print_json<T: Serialize>(value: &T) -> Result<(), AnyErr> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn print_status(cli: &Cli, action: &str, kind: &str, id: &str) -> Result<(), AnyErr> {
    if cli.json {
        print_json(&serde_json::json!({ "status": action, "kind": kind, "id": id }))
    } else {
        println!("{action} {kind} {id}");
        Ok(())
    }
}

fn print_invite_create(cli: &Cli, output: &InviteCreateOutput) -> Result<(), AnyErr> {
    if cli.json {
        print_json(output)
    } else {
        if let Some(token) = &output.api_token {
            println!("Created API token {}", token.id);
            println!("API token: {}", token.token);
        }
        println!("Created invitation {}", output.invitation.id);
        println!("Code: {}", output.invitation.code);
        println!("URL: {}", output.invite_url);
        Ok(())
    }
}

fn print_created_token(cli: &Cli, created: &CreatedToken) -> Result<(), AnyErr> {
    if cli.json {
        print_json(created)
    } else {
        println!("Created API token {}", created.id);
        println!("API token: {}", created.token);
        println!("Entitlements: {}", created.entitlements.join(","));
        Ok(())
    }
}

fn print_invites(cli: &Cli, invitations: &[Invitation]) -> Result<(), AnyErr> {
    if cli.json {
        return print_json(&invitations);
    }
    println!("id\tstatus\tadmin\tuses\tcreated_by\tcode");
    for invitation in invitations {
        let max_uses = invitation
            .max_uses
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{}\t{:?}\t{}\t{}/{}\t{}\t{}",
            invitation.id,
            invitation.status,
            invitation.is_admin,
            invitation.use_count,
            max_uses,
            invitation.created_by,
            invitation.code
        );
    }
    Ok(())
}

fn print_tokens(cli: &Cli, tokens: &[ApiToken]) -> Result<(), AnyErr> {
    if cli.json {
        return print_json(&tokens);
    }
    println!("id\tactive\tproduct_id\tuser_id\tentitlements\tdescription");
    for token in tokens {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            token.id,
            token.is_active,
            token.product_id.as_deref().unwrap_or("-"),
            token.user_id.as_deref().unwrap_or("-"),
            token.entitlements.join(","),
            token.description
        );
    }
    Ok(())
}

fn print_products(cli: &Cli, products: &[Product]) -> Result<(), AnyErr> {
    if cli.json {
        return print_json(&products);
    }
    println!("id\tpublic\taccepting_crashes\tname");
    for product in products {
        println!(
            "{}\t{}\t{}\t{}",
            product.id, product.public, product.accepting_crashes, product.name
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_grants;

    #[test]
    fn parses_product_role_grants() {
        let grants = parse_grants(&["product-a:maintainer".to_string()]).unwrap();
        assert_eq!(grants[0].product_id, "product-a");
        assert_eq!(grants[0].role, "maintainer");
    }

    #[test]
    fn rejects_unknown_roles() {
        let err = parse_grants(&["product-a:owner".to_string()]).unwrap_err();
        assert!(err.contains("invalid role"));
    }
}
