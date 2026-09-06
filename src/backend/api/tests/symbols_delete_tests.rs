#![cfg(test)]
//! Token-authenticated symbol deletion.
//!
//! The properties that matter are that a token cannot reach another product's
//! symbols, cannot delete without the entitlement, and that the stored object
//! goes with the row rather than being left behind.

use axum::http::{Request, StatusCode};
use axum::{Router, body::Body};
use object_store::{ObjectStore, ObjectStoreExt, path::Path};
use serde_json::Value;
use std::sync::Arc;
use testware::setup::TestSetup;
use tower::ServiceExt;

use api::routes::routes;
use api::state::AppState;
use api::worker::TestWorker;
use repos::Repo;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use testware::{create_test_product_with_details, create_test_token};

async fn seed_symbol(
    db: &Surreal<Any>,
    storage: &Arc<dyn ObjectStore>,
    product_id: &str,
    module_id: &str,
    build_id: &str,
    storage_path: &str,
) -> String {
    let storage_path = storage_path.to_string();
    storage
        .put(&Path::from(storage_path.as_str()), b"MODULE windows x86_64 X m\n".to_vec().into())
        .await
        .unwrap();
    repos::symbols::SymbolsRepo::create(
        db,
        data::symbols::NewSymbols {
            os: "windows".into(),
            arch: "x86_64".into(),
            build_id: build_id.into(),
            module_id: module_id.into(),
            storage_path: storage_path.clone(),
            product_id: product_id.into(),
            version: "1.11.1".into(),
            channel: "stable".into(),
            commit: "abc".into(),
            build_tag: "20260723-v1_11_1local".into(),
        },
    )
    .await
    .expect("seed symbol");
    storage_path
}

#[tokio::test]
async fn deleting_symbols_is_scoped_and_removes_the_object() {
    let db = &TestSetup::create_db().await;
    let storage: Arc<dyn ObjectStore> = Arc::new(object_store::memory::InMemory::new());
    let state = AppState {
        repo: Repo::new(db.clone()),
        settings: Arc::new(api::settings::Settings::default()),
        storage: storage.clone(),
        worker: Arc::new(TestWorker::new()),
    };
    let app: Router = Router::new().nest("/api", routes(state.clone()).await).with_state(state);

    let product = create_test_product_with_details(db, "Mine", "mine").await;
    let other = create_test_product_with_details(db, "Other", "not yours").await;

    let module = "libglib-2.0-0.dll";
    let build = "6A3F7C5516C000";
    let mine = format!("symbols/{module}-{build}");
    let theirs = format!("symbols/{module}-OTHERBUILD");

    // Two rows for the same module build: an upload creates rather than replaces.
    seed_symbol(db, &storage, &product.id, module, build, &mine).await;
    seed_symbol(db, &storage, &product.id, module, build, &mine).await;
    seed_symbol(db, &storage, &other.id, module, "OTHERBUILD", &theirs).await;

    let (uploader, _) =
        create_test_token(db, "up", Some(product.id.clone()), None, &["symbol-upload"]).await;
    let (reader, _) =
        create_test_token(db, "ro", Some(product.id.clone()), None, &["crash-read"]).await;

    let call = |token: Option<String>, ptoken: String| {
        let app = app.clone();
        async move {
            let mut req = Request::builder()
                .method("DELETE")
                .uri(format!("/api/symbols/{ptoken}/{module}/{build}"));
            if let Some(t) = token {
                req = req.header("Authorization", format!("Bearer {t}"));
            }
            let r = app.oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
            let status = r.status();
            let bytes = axum::body::to_bytes(r.into_body(), 65536).await.unwrap();
            (status, serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null))
        }
    };

    // --- authentication and entitlement ---
    let (status, _) = call(None, product.product_token.clone()).await;
    assert_ne!(status, StatusCode::OK, "no token must not delete symbols");

    let (status, _) = call(Some(reader.clone()), product.product_token.clone()).await;
    assert_ne!(status, StatusCode::OK, "crash-read must not delete symbols");

    // --- product scope ---
    let (status, _) = call(Some(uploader.clone()), other.product_token.clone()).await;
    assert_ne!(status, StatusCode::OK, "a token must not reach another product");

    // --- the delete itself ---
    let (status, body) = call(Some(uploader.clone()), product.product_token.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["deleted"].as_u64(), Some(2), "both rows for the build go");

    let left = repos::symbols::SymbolsRepo::get_all_by_module_and_build_id(
        db, &product.id, build, module,
    )
    .await
    .unwrap();
    assert!(left.is_empty(), "no rows may survive");
    assert!(
        storage.get(&Path::from(mine.as_str())).await.is_err(),
        "the object must go with the last row referencing it, or it leaks"
    );
    assert!(
        storage.get(&Path::from(theirs.as_str())).await.is_ok(),
        "the other product's symbols must be untouched"
    );

    // --- deleting nothing is not an error ---
    let (status, body) = call(Some(uploader), product.product_token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["deleted"].as_u64(), Some(0));
}

/// Storage paths carry no product, so two products can share one file. Deleting
/// one product's row must not pull the file out from under the other.
#[tokio::test]
async fn a_shared_object_survives_until_the_last_row_goes() {
    let db = &TestSetup::create_db().await;
    let storage: Arc<dyn ObjectStore> = Arc::new(object_store::memory::InMemory::new());
    let state = AppState {
        repo: Repo::new(db.clone()),
        settings: Arc::new(api::settings::Settings::default()),
        storage: storage.clone(),
        worker: Arc::new(TestWorker::new()),
    };
    let app: Router = Router::new().nest("/api", routes(state.clone()).await).with_state(state);

    let mine = create_test_product_with_details(db, "Mine", "mine").await;
    let theirs = create_test_product_with_details(db, "Theirs", "theirs").await;

    let module = "libglib-2.0-0.dll";
    let build = "6A3F7C5516C000";
    let shared = format!("symbols/{module}-{build}");
    seed_symbol(db, &storage, &mine.id, module, build, &shared).await;
    seed_symbol(db, &storage, &theirs.id, module, build, &shared).await;

    let (token, _) =
        create_test_token(db, "up", Some(mine.id.clone()), None, &["symbol-upload"]).await;

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/symbols/{}/{module}/{build}", mine.product_token))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let r = app.oneshot(req).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    assert!(
        storage.get(&Path::from(shared.as_str())).await.is_ok(),
        "the file is still referenced by the other product and must stay"
    );
}
