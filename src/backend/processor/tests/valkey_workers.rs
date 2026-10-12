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

#[tokio::test]
#[ignore = "requires GUARDRAIL_TEST_VALKEY_URI"]
async fn worker_finishes_jobs_after_legacy_done_set_is_upgraded() {
    let conn = connect().await;
    let mut redis = conn.clone();
    let queue = format!("guardrail::test::{}", uuid::Uuid::new_v4());
    let done_key = format!("{queue}:done");

    // apalis-redis rc.8 recorded finished jobs in a sorted set.
    let _: () = redis::cmd("ZADD")
        .arg(&done_key)
        .arg(1)
        .arg("old-job-1")
        .arg(2)
        .arg("old-job-2")
        .query_async(&mut redis)
        .await
        .unwrap();

    // The current version cannot record a finished job in it, and the worker dies.
    let (failing, _processed) = start_worker(&conn, &queue);
    push(&conn, &queue, "with-legacy-key").await;
    let exit = tokio::time::timeout(Duration::from_secs(10), failing)
        .await
        .expect("worker exits")
        .expect("worker task completes");
    let error = exit
        .expect_err("worker fails on the legacy key")
        .to_string();
    assert!(error.contains("WRONGTYPE"), "unexpected worker error: {error}");

    common::workers::upgrade_legacy_queue_keys(&mut redis, &[queue.as_str()]).await;
    let key_type: String = redis::cmd("TYPE")
        .arg(&done_key)
        .query_async(&mut redis)
        .await
        .unwrap();
    assert_eq!(key_type, "set");
    let members: Vec<String> = redis::cmd("SMEMBERS")
        .arg(&done_key)
        .query_async(&mut redis)
        .await
        .unwrap();
    assert!(
        members.contains(&"old-job-1".to_string()) && members.contains(&"old-job-2".to_string())
    );
    // Running it again changes nothing.
    common::workers::upgrade_legacy_queue_keys(&mut redis, &[queue.as_str()]).await;

    common::workers::clear_stale_registrations(&mut redis, &[(queue.as_str(), WORKER)]).await;
    let (worker, mut processed) = start_worker(&conn, &queue);
    push(&conn, &queue, "after-upgrade").await;
    assert!(processes(&mut processed, "after-upgrade").await, "worker runs");
    // Give the worker time to record the job as done, then make sure it is still alive.
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(!worker.is_finished(), "worker survives recording a finished job");
    worker.abort();
}
