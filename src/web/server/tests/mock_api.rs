// End-to-end smoke test for the mock REST API. Binds the mock_api router to
// a random local port and exercises a representative slice of the endpoints
// to confirm wiring (paths, method dispatch, JSON shapes, basic mutations).

use std::net::SocketAddr;

use serde_json::Value;
use tokio::net::TcpListener;

#[path = "../src/mock_api.rs"]
mod mock_api;

async fn spawn_server() -> (String, reqwest::Client) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let app = mock_api::router().with_state(mock_api::MockState::new());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{addr}/api/v1");
    let client = reqwest::Client::new();
    // Wrap in /api/v1 to mirror production mounting.
    (base.replace("/api/v1", ""), client)
}

#[tokio::test]
async fn signin_and_users() {
    let (base, c) = spawn_server().await;

    let r = c
        .post(format!("{base}/auth/signin"))
        .json(&serde_json::json!({ "email": "you@studio.co" }))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "signin status: {}", r.status());
    let user: Value = r.json().await.unwrap();
    assert_eq!(user["id"], "u-you");
    assert_eq!(user["isAdmin"], true);

    let r = c
        .post(format!("{base}/auth/signin"))
        .json(&serde_json::json!({ "email": "nobody@nowhere" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);

    let r: Vec<Value> = c
        .get(format!("{base}/users"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r.len(), 7);
}

#[tokio::test]
async fn list_and_filter_groups() {
    let (base, c) = spawn_server().await;

    let r: Value = c
        .get(format!("{base}/crashes?productId=guardrail"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(r["total"].as_u64().unwrap() > 0);
    assert!(r["versions"].as_array().unwrap().len() >= 1);
    let groups = r["groups"].as_array().unwrap();
    // First field should be the highest count (default sort).
    let first = groups[0]["count"].as_u64().unwrap();
    let second = groups.get(1).map(|g| g["count"].as_u64().unwrap()).unwrap_or(0);
    assert!(first >= second);
    // Each summary should NOT carry the heavy detail fields.
    assert!(groups[0].get("dump").is_none());
    assert!(groups[0].get("notes").is_none());

    // status filter narrows results
    let id = groups[0]["id"].as_str().unwrap().to_string();
    let r: Value = c
        .get(format!("{base}/crashes?productId=guardrail&status=resolved"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    for g in r["groups"].as_array().unwrap() {
        assert_eq!(g["status"], "resolved");
    }

    // group detail carries the crashes array (per-crash detail lives there
    // now) plus notes / related, but no longer the heavy minidump blobs.
    let detail: Value = c
        .get(format!("{base}/crashes/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(detail["crashes"].is_array(), "group should expose crashes");
    let first_crash = &detail["crashes"][0];
    assert!(first_crash["dump"].is_object(), "crash should hold the dump");
    assert!(first_crash["stack"].is_array(), "crash should hold the stack");
    assert!(detail.get("dump").is_none(), "group must not carry dump");
    assert!(detail.get("stack").is_none(), "group must not carry stack");

    // by-crash lookup returns both the crash and its parent group.
    let crash_id = first_crash["id"].as_str().unwrap().to_string();
    let bundle: Value = c
        .get(format!("{base}/crashes/by-crash/{crash_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(bundle["crash"]["id"], crash_id);
    assert_eq!(bundle["group"]["id"], detail["id"]);
}

#[tokio::test]
async fn set_status_then_add_note() {
    let (base, c) = spawn_server().await;

    let list: Value = c
        .get(format!("{base}/crashes?productId=guardrail&limit=1"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = list["groups"][0]["id"].as_str().unwrap().to_string();

    let r = c
        .post(format!("{base}/crashes/{id}/status"))
        .json(&serde_json::json!({ "status": "triaged" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 204);

    let updated: Value = c
        .get(format!("{base}/crashes/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["status"], "triaged");

    let notes_before = updated["notes"].as_array().map(|a| a.len()).unwrap_or(0);
    let r = c
        .post(format!("{base}/crashes/{id}/notes"))
        .json(&serde_json::json!({ "body": "smoke test", "author": "tester" }))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());
    let note: Value = r.json().await.unwrap();
    assert_eq!(note["body"], "smoke test");

    let after: Value = c
        .get(format!("{base}/crashes/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(after["notes"].as_array().unwrap().len(), notes_before + 1);
}

#[tokio::test]
async fn members_grant_revoke() {
    let (base, c) = spawn_server().await;

    // grant a fresh role
    let r = c
        .post(format!("{base}/products/harpoon/members/u-sofia"))
        .json(&serde_json::json!({ "role": "readwrite" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 204);

    let members: Vec<Value> = c
        .get(format!("{base}/products/harpoon/members"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(members
        .iter()
        .any(|m| m["userId"] == "u-sofia" && m["role"] == "readwrite"
            && m["user"]["email"] == "sofia@guardrail.co"));

    // revoke
    let r = c
        .delete(format!("{base}/products/harpoon/members/u-sofia"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 204);

    let members: Vec<Value> = c
        .get(format!("{base}/products/harpoon/members"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!members.iter().any(|m| m["userId"] == "u-sofia"));
}

#[tokio::test]
async fn symbols_upload_delete() {
    let (base, c) = spawn_server().await;

    let before: Vec<Value> = c
        .get(format!("{base}/products/guardrail/symbols"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let initial = before.len();

    let r = c
        .post(format!("{base}/products/guardrail/symbols"))
        .json(&serde_json::json!({
            "name": "test.pdb", "version": "9.9.9", "arch": "x86_64",
            "format": "PDB", "size": "5.0 MB", "uploadedBy": "u-you"
        }))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());
    let created: Value = r.json().await.unwrap();
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["productId"], "guardrail");

    let after: Vec<Value> = c
        .get(format!("{base}/products/guardrail/symbols"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(after.len(), initial + 1);

    let r = c.delete(format!("{base}/symbols/{id}")).send().await.unwrap();
    assert_eq!(r.status(), 204);

    let final_rows: Vec<Value> = c
        .get(format!("{base}/products/guardrail/symbols"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(final_rows.len(), initial);
}

#[tokio::test]
async fn memberships_for_user() {
    let (base, c) = spawn_server().await;

    let r: Vec<Value> = c
        .get(format!("{base}/users/u-you/memberships"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(r.iter().any(|m| m["productId"] == "guardrail" && m["product"]["name"] == "Guardrail"));
}
