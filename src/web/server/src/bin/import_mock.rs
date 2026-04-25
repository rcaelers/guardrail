// Imports src/web/server/mock/seed.json into a SurrealDB instance matching
// the schema in database/schema/guardrail.surql.
//
//   cargo run -p web --bin import_mock -- \
//     --host ws://localhost:8000 --user root --pass root \
//     --ns guardrail --db guardrail
//
// The script is idempotent: it deletes all rows from the application tables
// before inserting, so re-running it resets the database back to the seed.
//
// The schema is assumed to already be applied (`surrealkit sync` before
// running this). The script only touches row data, never DDL.

use clap::Parser;
use serde_json::Value;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::Root;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value = "ws://localhost:8000")]
    host: String,
    #[arg(long, default_value = "root")]
    user: String,
    #[arg(long, default_value = "root")]
    pass: String,
    #[arg(long, default_value = "guardrail")]
    ns: String,
    #[arg(long, default_value = "guardrail")]
    db: String,
    /// Path to the seed JSON. Default resolves next to the binary.
    #[arg(long)]
    seed: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let seed: Value = if let Some(path) = args.seed.as_deref() {
        let raw = std::fs::read_to_string(path)?;
        serde_json::from_str(&raw)?
    } else {
        serde_json::from_str(include_str!("../../mock/seed.json"))?
    };

    let db = surrealdb::engine::any::connect(&args.host).await?;
    db.signin(Root {
        username: args.user.clone(),
        password: args.pass.clone(),
    })
    .await?;
    db.use_ns(&args.ns).use_db(&args.db).await?;

    println!("Clearing existing rows…");
    clear_tables(&db).await?;

    let products = seed["products"].as_array().cloned().unwrap_or_default();
    let users = seed["users"].as_array().cloned().unwrap_or_default();
    let memberships = seed["memberships"].as_array().cloned().unwrap_or_default();
    let crashes = seed["crashes"].as_array().cloned().unwrap_or_default();
    let symbols = seed["symbols"].as_array().cloned().unwrap_or_default();
    let api_tokens = seed["api_tokens"].as_array().cloned().unwrap_or_default();

    println!("Importing {} products…", products.len());
    for p in &products {
        import_product(&db, p).await?;
    }

    println!("Importing {} users…", users.len());
    for u in &users {
        import_user(&db, u).await?;
    }

    println!("Importing {} memberships…", memberships.len());
    for m in &memberships {
        import_membership(&db, m).await?;
    }

    let mut group_count = 0usize;
    let mut crash_count = 0usize;
    let mut note_count = 0usize;
    println!("Importing crash groups + crashes…");
    for g in &crashes {
        let (gc, cc, nc) = import_group(&db, g).await?;
        group_count += gc;
        crash_count += cc;
        note_count += nc;
    }
    println!("  {group_count} groups, {crash_count} crashes, {note_count} notes");

    println!("Importing {} symbols…", symbols.len());
    for s in &symbols {
        import_symbol(&db, s).await?;
    }

    println!("Importing {} API tokens…", api_tokens.len());
    for token in &api_tokens {
        import_api_token(&db, token).await?;
    }

    println!("Done.");
    Ok(())
}

// ----------------------------------------------------------------------

async fn clear_tables(db: &Surreal<Any>) -> Result<()> {
    // Delete in FK-safe order. Start with rows that reference the core data,
    // then remove the imported entities themselves.
    let tables = [
        "sessions",
        "credentials",
        "api_tokens",
        "attachments",
        "annotations",
        "crashes",
        "crash_groups",
        "user_access",
        "symbols",
        "users",
        "products",
    ];
    for t in tables {
        db.query(format!("DELETE {t}")).await?;
    }
    Ok(())
}

// ----------------------------------------------------------------------

async fn import_product(db: &Surreal<Any>, p: &Value) -> Result<()> {
    let id = s(p, "id");
    let public = p.get("public").and_then(|v| v.as_bool()).unwrap_or(false);
    db.query(
        "CREATE type::record('products', $id) CONTENT {
            name: $name,
            slug: $slug,
            description: $description,
            color: $color,
            public: $public
        }",
    )
    .bind(("id", id.to_string()))
    .bind(("name", s(p, "name").to_string()))
    .bind(("slug", s(p, "slug").to_string()))
    .bind(("description", s(p, "description").to_string()))
    .bind(("color", s(p, "color").to_string()))
    .bind(("public", public))
    .await?;
    Ok(())
}

async fn import_user(db: &Surreal<Any>, u: &Value) -> Result<()> {
    let id = s(u, "id");
    let email = s(u, "email").to_string();
    db.query(
        "CREATE type::record('users', $id) CONTENT {
            username: $username,
            email: $email,
            name: $name,
            avatar: $avatar,
            is_admin: $is_admin,
            created_at: <datetime>$joined_at
        }",
    )
    .bind(("id", id.to_string()))
    .bind(("username", email.clone())) // email doubles as username for the mock
    .bind(("email", email))
    .bind(("name", s(u, "name").to_string()))
    .bind(("avatar", s(u, "avatar").to_string()))
    .bind(("is_admin", u["isAdmin"].as_bool().unwrap_or(false)))
    .bind(("joined_at", s(u, "joinedAt").to_string()))
    .await?;
    Ok(())
}

async fn import_membership(db: &Surreal<Any>, m: &Value) -> Result<()> {
    let user_id = s(m, "userId");
    let product_id = s(m, "productId");
    let role = s(m, "role");
    db.query(
        "CREATE user_access CONTENT {
            user_id: type::record('users', $user_id),
            product_id: type::record('products', $product_id),
            role: $role
        }",
    )
    .bind(("user_id", user_id.to_string()))
    .bind(("product_id", product_id.to_string()))
    .bind(("role", role.to_string()))
    .await?;
    Ok(())
}

// Each mock group becomes one crash_groups row plus one crashes row per
// member crash plus annotations for each note.
async fn import_group(db: &Surreal<Any>, g: &Value) -> Result<(usize, usize, usize)> {
    let group_id = s(g, "id");
    let product_id = s(g, "productId");
    let fingerprint = group_id.to_string(); // mock uses the group id as the fingerprint
    let count = g["count"].as_u64().unwrap_or(0) as i64;

    let assignee = g["assignee"].as_str().map(|a| format!("u-{a}"));

    db.query(
        "CREATE type::record('crash_groups', $id) CONTENT {
            product_id: type::record('products', $product_id),
            fingerprint: $fingerprint,
            signal: $signal,
            count: $count,
            status: $status,
            assignee: IF $assignee_id != NONE
                THEN type::record('users', $assignee_id)
                ELSE NONE END,
            first_seen: <datetime>$first_seen,
            last_seen: <datetime>$last_seen
        }",
    )
    .bind(("id", group_id.to_string()))
    .bind(("product_id", product_id.to_string()))
    .bind(("fingerprint", fingerprint))
    .bind(("signal", s(g, "signal").to_string()))
    .bind(("count", count))
    .bind(("status", s(g, "status").to_string()))
    .bind(("assignee_id", assignee))
    .bind(("first_seen", s(g, "firstSeen").to_string()))
    .bind(("last_seen", s(g, "lastSeen").to_string()))
    .await?;

    let crashes = g["crashes"].as_array().cloned().unwrap_or_default();
    let mut crash_n = 0usize;
    for c in &crashes {
        import_crash(db, group_id, product_id, c).await?;
        crash_n += 1;
    }

    let notes = g["notes"].as_array().cloned().unwrap_or_default();
    let mut note_n = 0usize;
    for n in &notes {
        import_note(db, group_id, product_id, n).await?;
        note_n += 1;
    }

    Ok((1, crash_n, note_n))
}

// A crash keeps the full UI-facing detail (stack/threads/modules/env/
// breadcrumbs/logs/userDescription/dump/derived plus per-crash metadata)
// inside `report`. The db_api reads this back and hands it to the UI as-is.
async fn import_crash(
    db: &Surreal<Any>,
    group_id: &str,
    product_id: &str,
    c: &Value,
) -> Result<()> {
    let crash_id = s(c, "id");
    // Keep everything EXCEPT id/groupId/productId/at (those are first-class
    // columns) inside the `report` blob.
    let mut report = c.as_object().cloned().unwrap_or_default();
    for k in ["id", "groupId", "productId"] {
        report.remove(k);
    }
    let report_value = Value::Object(report);

    db.query(
        "CREATE type::record('crashes', $id) CONTENT {
            product_id: type::record('products', $product_id),
            group_id: type::record('crash_groups', $group_id),
            fingerprint: $fingerprint,
            report: $report,
            created_at: <datetime>$at,
            updated_at: <datetime>$at
        }",
    )
    .bind(("id", crash_id.to_string()))
    .bind(("product_id", product_id.to_string()))
    .bind(("group_id", group_id.to_string()))
    .bind(("fingerprint", group_id.to_string()))
    .bind(("report", report_value))
    .bind(("at", s(c, "at").to_string()))
    .await?;
    Ok(())
}

async fn import_note(db: &Surreal<Any>, group_id: &str, product_id: &str, n: &Value) -> Result<()> {
    db.query(
        "CREATE annotations CONTENT {
            source: 'user',
            value: $body,
            author: $author,
            group_id: type::record('crash_groups', $group_id),
            product_id: type::record('products', $product_id),
            created_at: <datetime>$at,
            updated_at: <datetime>$at
        }",
    )
    .bind(("body", s(n, "body").to_string()))
    .bind(("author", s(n, "author").to_string()))
    .bind(("group_id", group_id.to_string()))
    .bind(("product_id", product_id.to_string()))
    .bind(("at", s(n, "at").to_string()))
    .await?;
    Ok(())
}

async fn import_symbol(db: &Surreal<Any>, s_: &Value) -> Result<()> {
    let id = s(s_, "id");
    let product_id = s(s_, "productId");
    db.query(
        "CREATE type::record('symbols', $id) CONTENT {
            product_id: type::record('products', $product_id),
            os: '',
            arch: $arch,
            build_id: $build_id,
            module_id: $module_id,
            storage_path: $storage_path,
            created_at: <datetime>$created_at,
            updated_at: <datetime>$created_at
        }",
    )
    .bind(("id", id.to_string()))
    .bind(("product_id", product_id.to_string()))
    .bind(("arch", s(s_, "arch").to_string()))
    .bind(("build_id", s(s_, "debugId").to_string()))
    .bind(("module_id", s(s_, "name").to_string()))
    .bind(("storage_path", format!("symbols/{}", id)))
    .bind(("created_at", s(s_, "uploadedAt").to_string()))
    .await?;
    Ok(())
}

async fn import_api_token(db: &Surreal<Any>, t: &Value) -> Result<()> {
    let id = s(t, "id");
    let product_id = s(t, "productId");
    let user_id = s(t, "userId");
    db.query(
        "CREATE type::record('api_tokens', $id) CONTENT {
            description: $description,
            token_id: <uuid>$token_id,
            token_hash: $token_hash,
            product_id: IF $product_id != ''
                THEN type::record('products', $product_id)
                ELSE NONE END,
            user_id: IF $user_id != ''
                THEN type::record('users', $user_id)
                ELSE NONE END,
            entitlements: $entitlements,
            last_used_at: IF $last_used_at != ''
                THEN <datetime>$last_used_at
                ELSE NONE END,
            expires_at: IF $expires_at != ''
                THEN <datetime>$expires_at
                ELSE NONE END,
            is_active: $is_active,
            created_at: IF $created_at != ''
                THEN <datetime>$created_at
                ELSE time::now() END,
            updated_at: IF $updated_at != ''
                THEN <datetime>$updated_at
                ELSE time::now() END
        }",
    )
    .bind(("id", id.to_string()))
    .bind(("description", s(t, "description").to_string()))
    .bind(("token_id", s(t, "tokenId").to_string()))
    .bind(("token_hash", s(t, "tokenHash").to_string()))
    .bind(("product_id", product_id.to_string()))
    .bind(("user_id", user_id.to_string()))
    .bind((
        "entitlements",
        t.get("entitlements")
            .cloned()
            .unwrap_or(Value::Array(vec![])),
    ))
    .bind(("last_used_at", s(t, "lastUsedAt").to_string()))
    .bind(("expires_at", s(t, "expiresAt").to_string()))
    .bind(("is_active", t["isActive"].as_bool().unwrap_or(true)))
    .bind(("created_at", s(t, "createdAt").to_string()))
    .bind(("updated_at", s(t, "updatedAt").to_string()))
    .await?;
    Ok(())
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("")
}
