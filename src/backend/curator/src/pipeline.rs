use std::time::{Duration, SystemTime, UNIX_EPOCH};

use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use tracing::{debug, error};

/// Unix time of the last crash imported into the database.
const LAST_CRASH_IMPORT_KEY: &str = "guardrail::pipeline:last-crash-import";

/// Workrave sends several crashes a day, so two silent days mean crashes are
/// being lost somewhere between the client and the database.
const MAX_SILENCE: Duration = Duration::from_secs(48 * 3600);

pub async fn record_crash_import(redis: &mut ConnectionManager) {
    if let Err(e) = redis.set::<_, _, ()>(LAST_CRASH_IMPORT_KEY, now()).await {
        debug!(error = ?e, "Failed to record crash import time");
    }
}

/// Starts the silence clock on a fresh install, so a pipeline that never
/// imports anything is still reported after [`MAX_SILENCE`].
pub async fn start_clock(redis: &mut ConnectionManager) {
    if let Err(e) = redis.set_nx::<_, _, ()>(LAST_CRASH_IMPORT_KEY, now()).await {
        debug!(error = ?e, "Failed to initialise crash import time");
    }
}

/// Whether crashes still reach the database. Fails open when Valkey is
/// unreachable, which the Valkey and readiness checks already report.
pub async fn is_flowing(redis: &mut ConnectionManager) -> bool {
    match redis.get::<_, Option<u64>>(LAST_CRASH_IMPORT_KEY).await {
        Ok(Some(last)) => {
            let flowing = is_recent(last, now());
            if !flowing {
                error!(hours = now().saturating_sub(last) / 3600, "No crash imported for too long");
            }
            flowing
        }
        Ok(None) => true,
        Err(e) => {
            debug!(error = ?e, "Failed to read crash import time");
            true
        }
    }
}

fn is_recent(last_import: u64, now: u64) -> bool {
    now.saturating_sub(last_import) < MAX_SILENCE.as_secs()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crashes_flow_until_two_silent_days() {
        let now = 1_000_000;
        assert!(is_recent(now, now));
        assert!(is_recent(now - 47 * 3600, now));
        assert!(!is_recent(now - 48 * 3600, now));
        // A clock that moved backwards is not silence.
        assert!(is_recent(now + 60, now));
    }
}
