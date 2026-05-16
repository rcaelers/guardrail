use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand};
use common::{QueryParams, token::generate_api_token};
use data::user::NewUser;
use data::{
    api_token::{ApiToken, ENTITLEMENT_INVITATION_CREATE, NewApiToken},
    invitation::{Invitation, InvitationGrant, NewInvitation},
    product::{NewProduct, Product},
};
use email::{Email, EmailSender, LogEmailSender, ResendEmailSender};
use repos::{
    api_token::ApiTokenRepo, invitation::InvitationRepo, product::ProductRepo, user::UserRepo,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use surrealdb::{Surreal, engine::any::Any, opt::auth::Root};
use web::settings::Settings;

type AnyErr = Box<dyn std::error::Error + Send + Sync>;

// --- Pocket ID API types ---

#[derive(Debug, Deserialize, Serialize)]
struct PocketIdUser {
    id: String,
    username: String,
    email: Option<String>,
    #[serde(rename = "firstName")]
    first_name: Option<String>,
    #[serde(rename = "lastName")]
    last_name: Option<String>,
    #[serde(rename = "isAdmin")]
    is_admin: bool,
}

#[derive(Deserialize)]
struct PocketIdUserList {
    data: Vec<PocketIdUser>,
}

#[derive(Serialize)]
struct UpdateUserBody<'a> {
    username: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<&'a str>,
    #[serde(rename = "firstName", skip_serializing_if = "Option::is_none")]
    first_name: Option<&'a str>,
    #[serde(rename = "lastName", skip_serializing_if = "Option::is_none")]
    last_name: Option<&'a str>,
    #[serde(rename = "isAdmin")]
    is_admin: bool,
}

struct PocketIdClient {
    api_url: url::Url,
    public_url: url::Url,
    setup_path: String,
    api_key: String,
    client: reqwest::Client,
}

impl PocketIdClient {
    fn new(
        api_url: &str,
        public_url: &str,
        setup_path: &str,
        api_key: &str,
    ) -> Result<Self, AnyErr> {
        Ok(Self {
            api_url: api_url.parse()?,
            public_url: public_url.parse()?,
            setup_path: setup_path.to_string(),
            api_key: api_key.to_string(),
            client: reqwest::Client::new(),
        })
    }

    async fn list_users(&self) -> Result<Vec<PocketIdUser>, AnyErr> {
        let url = self.api_url.join("/api/users")?;
        let response = self
            .client
            .get(url)
            .header("X-API-KEY", &self.api_key)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(format!("list users failed: {}", response.status()).into());
        }
        let body: PocketIdUserList = response.json().await?;
        Ok(body.data)
    }

    async fn set_admin(&self, user: &PocketIdUser, is_admin: bool) -> Result<(), AnyErr> {
        let url = self.api_url.join(&format!("/api/users/{}", user.id))?;
        let body = UpdateUserBody {
            username: &user.username,
            email: user.email.as_deref(),
            first_name: user.first_name.as_deref(),
            last_name: user.last_name.as_deref(),
            is_admin,
        };
        let response = self
            .client
            .put(url)
            .header("X-API-KEY", &self.api_key)
            .json(&body)
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!("update user failed: {status}: {text}").into());
        }
        Ok(())
    }

    fn find_user<'a>(
        &self,
        users: &'a [PocketIdUser],
        identifier: &str,
    ) -> Option<&'a PocketIdUser> {
        users.iter().find(|u| {
            u.id == identifier || u.username == identifier || u.email.as_deref() == Some(identifier)
        })
    }

    async fn create_one_time_token(&self, user_id: &str, ttl: &str) -> Result<String, AnyErr> {
        let url = self
            .api_url
            .join(&format!("/api/users/{user_id}/one-time-access-token"))?;
        let response = self
            .client
            .post(url)
            .header("X-API-KEY", &self.api_key)
            .json(&serde_json::json!({ "ttl": ttl }))
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!("one-time-access-token failed: {status}: {text}").into());
        }
        #[derive(Deserialize)]
        struct TokenResponse {
            token: String,
        }
        let data: TokenResponse = response.json().await?;
        Ok(data.token)
    }

    fn build_login_url(&self, token: &str) -> Result<url::Url, AnyErr> {
        let path = format!("{}/{}", self.setup_path.trim_end_matches('/'), token);
        Ok(self.public_url.join(&path)?)
    }
}

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
    User(UserCommand),
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

    /// Send the invitation link to this email address (requires email to be configured in settings).
    #[arg(long)]
    email: Option<String>,
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

#[derive(Args, Debug)]
struct UserCommand {
    #[command(subcommand)]
    command: UserSubcommand,
}

#[derive(Subcommand, Debug)]
enum UserSubcommand {
    List,
    SetAdmin(UserIdentifierArgs),
    UnsetAdmin(UserIdentifierArgs),
    GenerateLoginCode(UserGenerateLoginCodeArgs),
    Sync(UserSyncArgs),
}

#[derive(Args, Debug)]
struct UserIdentifierArgs {
    /// Pocket ID username, email address, or user ID
    identifier: String,
}

#[derive(Args, Debug)]
struct UserSyncArgs {
    /// Pocket ID username, email address, or user ID
    identifier: String,

    #[arg(long)]
    admin: bool,
}

#[derive(Args, Debug)]
struct UserGenerateLoginCodeArgs {
    /// Pocket ID username, email address, or user ID
    identifier: String,

    /// Token time-to-live (e.g. "15m", "1h", "168h")
    #[arg(long, default_value = "15m")]
    ttl: String,
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
    email_sent: Option<String>,
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
    let settings = Settings::load(&cli.config_dir)?;
    let db = connect_db(&settings).await?;

    match &cli.command {
        Command::Invite(command) => run_invite(&cli, &settings, &db, command).await?,
        Command::Token(command) => run_token(&cli, &db, command).await?,
        Command::Product(command) => run_product(&cli, &db, command).await?,
        Command::User(command) => run_user(&cli, &settings, &db, command).await?,
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
                settings.ingress.base_url.trim_end_matches('/'),
                invitation.code
            );

            let email_sent = if let Some(to) = args.email.as_deref() {
                if settings.email.from.is_empty() {
                    return Err("email.from is not configured in settings".into());
                }
                let sender = build_email_sender(settings);
                let email = Email {
                    from: settings.email.from.clone(),
                    to: to.to_string(),
                    subject: "You've been invited to Guardrail".to_string(),
                    html: format!(
                        "<p>You have been invited to join <strong>Guardrail</strong>.</p>\
                         <p><a href=\"{url}\">Accept invitation</a></p>\
                         <p>Or copy this link: {url}</p>",
                        url = invite_url,
                    ),
                    text: Some(format!(
                        "You have been invited to join Guardrail. Accept here: {url}",
                        url = invite_url,
                    )),
                };
                sender.send(email).await?;
                Some(to.to_string())
            } else {
                None
            };

            let output = InviteCreateOutput {
                invitation,
                invite_url,
                api_token,
                email_sent,
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

fn build_email_sender(settings: &Settings) -> Arc<dyn EmailSender> {
    if let Some(key) = settings
        .email
        .resend
        .as_ref()
        .map(|r| r.key.as_str())
        .filter(|k| !k.is_empty())
    {
        Arc::new(ResendEmailSender::new(key.to_string()))
    } else {
        Arc::new(LogEmailSender)
    }
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

async fn run_user(
    cli: &Cli,
    settings: &Settings,
    db: &Surreal<Any>,
    command: &UserCommand,
) -> Result<(), AnyErr> {
    let pocket_id = settings
        .provisioner
        .pocket_id
        .as_ref()
        .ok_or("provisioner.pocket_id is not configured")?;
    let public_url = pocket_id
        .public_url
        .as_deref()
        .unwrap_or(&pocket_id.api_url);
    let setup_path = pocket_id.setup_path.as_deref().unwrap_or("/lc/");
    let client =
        PocketIdClient::new(&pocket_id.api_url, public_url, setup_path, &pocket_id.api_key)?;

    match &command.command {
        UserSubcommand::List => {
            let users = client.list_users().await?;
            print_users(cli, &users)?;
        }
        UserSubcommand::SetAdmin(args) => {
            let users = client.list_users().await?;
            let user = client
                .find_user(&users, &args.identifier)
                .ok_or_else(|| format!("user not found: {}", args.identifier))?;
            if user.is_admin {
                print_status(cli, "already-admin", "user", &user.id)?;
            } else {
                client.set_admin(user, true).await?;
                print_status(cli, "set-admin", "user", &user.id)?;
            }
        }
        UserSubcommand::UnsetAdmin(args) => {
            let users = client.list_users().await?;
            let user = client
                .find_user(&users, &args.identifier)
                .ok_or_else(|| format!("user not found: {}", args.identifier))?;
            if !user.is_admin {
                print_status(cli, "not-admin", "user", &user.id)?;
            } else {
                client.set_admin(user, false).await?;
                print_status(cli, "unset-admin", "user", &user.id)?;
            }
        }
        UserSubcommand::GenerateLoginCode(args) => {
            let users = client.list_users().await?;
            let user = client
                .find_user(&users, &args.identifier)
                .ok_or_else(|| format!("user not found: {}", args.identifier))?;
            let token = client.create_one_time_token(&user.id, &args.ttl).await?;
            let url = client.build_login_url(&token)?;
            print_login_code(cli, &user.id, &url.to_string())?;
        }
        UserSubcommand::Sync(args) => {
            let users = client.list_users().await?;
            let pocket_user = client
                .find_user(&users, &args.identifier)
                .ok_or_else(|| format!("user not found: {}", args.identifier))?;

            let email = pocket_user.email.as_deref();
            let existing =
                UserRepo::get_by_name(db, &pocket_user.username)
                    .await?
                    .or(match email {
                        Some(e) => UserRepo::get_by_email(db, e).await?,
                        None => None,
                    });

            if let Some(local) = existing {
                print_sync(cli, "already-exists", &local.id, &pocket_user.username)?;
            } else {
                let id = UserRepo::create(
                    db,
                    NewUser {
                        username: pocket_user.username.clone(),
                        email: pocket_user.email.clone(),
                        is_admin: args.admin,
                    },
                )
                .await?;
                print_sync(cli, "created", &id, &pocket_user.username)?;
            }
        }
    }
    Ok(())
}

fn print_users(cli: &Cli, users: &[PocketIdUser]) -> Result<(), AnyErr> {
    if cli.json {
        return print_json(users);
    }
    println!("id\tadmin\tusername\temail");
    for user in users {
        println!(
            "{}\t{}\t{}\t{}",
            user.id,
            user.is_admin,
            user.username,
            user.email.as_deref().unwrap_or("-")
        );
    }
    Ok(())
}

fn print_login_code(cli: &Cli, user_id: &str, url: &str) -> Result<(), AnyErr> {
    if cli.json {
        return print_json(&serde_json::json!({ "user_id": user_id, "login_url": url }));
    }
    println!("Login URL: {url}");
    Ok(())
}

fn print_sync(cli: &Cli, status: &str, local_id: &str, username: &str) -> Result<(), AnyErr> {
    if cli.json {
        return print_json(
            &serde_json::json!({ "status": status, "local_id": local_id, "username": username }),
        );
    }
    println!("{status} user {username} (local id: {local_id})");
    Ok(())
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

fn print_json<T: Serialize + ?Sized>(value: &T) -> Result<(), AnyErr> {
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
        if let Some(to) = &output.email_sent {
            println!("Email sent to: {to}");
        }
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
