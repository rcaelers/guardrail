//! Worker behaviour against a real Valkey.
//!
//! Needs a scratch Valkey, e.g.
//! `GUARDRAIL_TEST_VALKEY_URI=redis://127.0.0.1:6379/15 cargo test -p processor --test valkey_workers -- --ignored`

use std::time::Duration;

use apalis::prelude::*;
use apalis_redis::{Config as RedisConfig, ConnectionManager, RedisStorage};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;

const WORKER: &str = "test-worker";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestJob {
    id: String,
}

async fn handle(job: TestJob, done: Data<UnboundedSender<String>>) -> Result<(), BoxDynError> {
    done.send(job.id)?;
    Ok(())
}

async fn connect() -> ConnectionManager {
    let uri = std::env::var("GUARDRAIL_TEST_VALKEY_URI").expect("GUARDRAIL_TEST_VALKEY_URI");
    apalis_redis::connect(uri).await.expect("connect to Valkey")
}

fn storage(conn: &ConnectionManager, queue: &str) -> RedisStorage<TestJob> {
    RedisStorage::new(conn.clone()).with_config(RedisConfig::default().queue(queue))
}

/// Starts a worker the way the processor does; returns its task and the ids of
/// the jobs it processes.
fn start_worker(
    conn: &ConnectionManager,
    queue: &str,
) -> (JoinHandle<Result<(), WorkerError>>, UnboundedReceiver<String>) {
    let (done, processed) = unbounded_channel();
    let worker = WorkerBuilder::new(WORKER)
        .backend(storage(conn, queue))
        .data(done)
        .build(handle);
    (tokio::spawn(worker.run()), processed)
}

async fn push(conn: &ConnectionManager, queue: &str, id: &str) {
    storage(conn, queue)
        .push(TestJob { id: id.to_string() })
        .await
        .expect("push job");
}

async fn processes(processed: &mut UnboundedReceiver<String>, id: &str) -> bool {
    matches!(
        tokio::time::timeout(Duration::from_secs(10), processed.recv()).await,
        Ok(Some(got)) if got == id
    )
}

#[tokio::test]
#[ignore = "requires GUARDRAIL_TEST_VALKEY_URI"]
async fn replacement_worker_starts_after_stale_registration_is_cleared() {
    let conn = connect().await;
    let queue = format!("guardrail::test::{}", uuid::Uuid::new_v4());

    // The old pod: registers, works, then is killed without deregistering.
    let (old, mut processed) = start_worker(&conn, &queue);
    push(&conn, &queue, "before-restart").await;
    assert!(processes(&mut processed, "before-restart").await, "old worker runs");
    old.abort();
    let _ = old.await;

    // Its registration blocks a replacement with the same name...
    let (blocked, mut processed) = start_worker(&conn, &queue);
    push(&conn, &queue, "while-blocked").await;
    assert!(
        !processes(&mut processed, "while-blocked").await,
        "replacement must not run while the stale registration exists"
    );
    let exit = tokio::time::timeout(Duration::from_secs(10), blocked)
        .await
        .expect("blocked worker exits")
        .expect("worker task completes");
    let error = exit
        .expect_err("blocked worker fails to register")
        .to_string();
    assert!(error.contains("still active"), "unexpected worker error: {error}");

    // ...until the startup cleanup removes it.
    common::workers::clear_stale_registrations(&mut conn.clone(), &[(queue.as_str(), WORKER)])
        .await;
    let (replacement, mut processed) = start_worker(&conn, &queue);
    assert!(
        processes(&mut processed, "while-blocked").await,
        "replacement picks up the job queued while it was blocked"
    );
    push(&conn, &queue, "after-restart").await;
    assert!(processes(&mut processed, "after-restart").await, "replacement runs");
    replacement.abort();
}
