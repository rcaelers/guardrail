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

/// Converts a queue's finished-job sets to the format apalis-redis >= rc.9 uses.
///
/// Older versions stored `{queue}:done` and `{queue}:dead` as sorted sets; newer
/// ones `SADD` to them, which fails with WRONGTYPE on the old keys and kills the
/// worker the first time it finishes a job. The conversion keeps every member,
/// is atomic per key and does nothing once a key is a plain set.
pub async fn upgrade_legacy_queue_keys(redis: &mut ConnectionManager, queues: &[&str]) {
    let script = redis::Script::new(UPGRADE_SORTED_SET_SCRIPT);
    for queue_name in queues {
        for suffix in ["done", "dead"] {
            let key = format!("{queue_name}:{suffix}");
            match script.key(&key).invoke_async::<i64>(redis).await {
                Ok(-1) => debug!("{} needs no upgrade", key),
                Ok(n) => warn!("Converted legacy sorted set {} to a set ({} entries)", key, n),
                Err(e) => error!("Failed to upgrade legacy queue key {}: {}", key, e),
            }
        }
    }
}

/// Returns the number of converted members, or -1 when the key is not a
/// sorted set (missing, or already converted).
const UPGRADE_SORTED_SET_SCRIPT: &str = r#"
if redis.call('TYPE', KEYS[1]).ok ~= 'zset' then
    return -1
end
local members = redis.call('ZRANGE', KEYS[1], 0, -1)
redis.call('DEL', KEYS[1])
for i = 1, #members, 1000 do
    redis.call('SADD', KEYS[1], unpack(members, i, math.min(i + 999, #members)))
end
return #members
"#;
