use std::time::Duration;

use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::Root;
use tracing::{error, warn};

use common::settings::Database;

const CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// Opens a new SurrealDB connection with its own authenticated session.
pub async fn connect(settings: &Database) -> Result<Surreal<Any>, surrealdb::Error> {
    let db = surrealdb::engine::any::connect(&settings.endpoint).await?;
    db.signin(Root {
        username: settings.username.clone(),
        password: settings.password.clone(),
    })
    .await?;
    db.use_ns(&settings.namespace)
        .use_db(&settings.database)
        .await?;
    Ok(db)
}

/// Whether `db` can no longer reach SurrealDB while a new connection can.
///
/// After some SurrealDB outages the SDK keeps failing on an existing handle
/// (for example "Session not found") even though the server is back. Only a
/// new connection recovers from that, so the process must be restarted. When
/// SurrealDB itself is down, a restart would not help, so that is not broken.
pub async fn is_handle_broken(db: &Surreal<Any>, settings: &Database) -> bool {
    let handle_error = match tokio::time::timeout(CHECK_TIMEOUT, db.query("RETURN 1")).await {
        Ok(Ok(_)) => return false,
        Ok(Err(e)) => e.to_string(),
        Err(_) => "timed out".to_string(),
    };

    match tokio::time::timeout(CHECK_TIMEOUT, connect(settings)).await {
        Ok(Ok(_)) => {
            error!(
                error = %handle_error,
                "SurrealDB connection is broken while a new connection works"
            );
            true
        }
        Ok(Err(e)) => {
            warn!(error = %handle_error, fresh_error = %e, "SurrealDB is unavailable");
            false
        }
        Err(_) => {
            warn!(error = %handle_error, "SurrealDB is unavailable (connect timed out)");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Child, Command};

    fn start_server(bin: &str, port: u16) -> Child {
        Command::new(bin)
            .args([
                "start", "--log", "error", "--user", "root", "--pass", "root",
            ])
            .args(["--bind", &format!("127.0.0.1:{port}"), "memory"])
            .spawn()
            .expect("start surreal")
    }

    fn settings(port: u16) -> Database {
        Database {
            endpoint: format!("ws://127.0.0.1:{port}"),
            namespace: "test".to_string(),
            database: "test".to_string(),
            username: "root".to_string(),
            password: "root".to_string(),
            ..Default::default()
        }
    }

    /// Needs the SurrealDB binary, e.g. GUARDRAIL_TEST_SURREAL_BIN=/usr/local/bin/surreal
    #[tokio::test]
    #[ignore = "requires GUARDRAIL_TEST_SURREAL_BIN"]
    async fn detects_broken_handle_only_when_a_new_connection_works() {
        let bin = std::env::var("GUARDRAIL_TEST_SURREAL_BIN").expect("GUARDRAIL_TEST_SURREAL_BIN");
        let mut first = start_server(&bin, 18771);
        let mut second = start_server(&bin, 18772);
        tokio::time::sleep(Duration::from_secs(2)).await;

        let db = connect(&settings(18771)).await.unwrap();
        assert!(!is_handle_broken(&db, &settings(18771)).await, "healthy handle");

        // The handle's server goes away while the configured one stays up.
        first.kill().unwrap();
        first.wait().unwrap();
        assert!(is_handle_broken(&db, &settings(18772)).await, "handle broken, server up");

        // Nothing is reachable: SurrealDB is down, a restart would not help.
        second.kill().unwrap();
        second.wait().unwrap();
        assert!(!is_handle_broken(&db, &settings(18772)).await, "server down");
    }
}
