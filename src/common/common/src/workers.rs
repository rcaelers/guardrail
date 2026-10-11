use redis::aio::ConnectionManager;
use tracing::{debug, error, warn};

/// Removes the worker registrations a previous process left behind.
///
/// apalis-redis refuses to register a worker whose name was seen within the
/// orphan threshold (heartbeat interval × missed heartbeats), so a pod that
/// replaces one that was killed would fail to start its workers. Each worker
/// runs once per deployment, so a registration with its name is always stale
/// when a new process starts.
pub async fn clear_stale_registrations(redis: &mut ConnectionManager, workers: &[(&str, &str)]) {
    for (queue_name, worker_name) in workers {
        let workers_key = format!("{queue_name}:workers");
        let inflight_key = format!("{queue_name}:inflight:{worker_name}");
        match redis::cmd("ZREM")
            .arg(&workers_key)
            // apalis-redis >= rc.11 registers the plain worker name; older
            // versions registered the worker's inflight key.
            .arg(*worker_name)
            .arg(&inflight_key)
            .query_async::<i64>(redis)
            .await
        {
            Ok(0) => debug!("No stale registration to clear for {}", worker_name),
            Ok(_) => warn!("Cleared stale worker registration for {}", worker_name),
            Err(e) => {
                error!("Failed to clear stale worker registration for {}: {}", worker_name, e)
            }
        }
    }
}
