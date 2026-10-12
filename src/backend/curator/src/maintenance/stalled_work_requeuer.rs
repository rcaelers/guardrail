use apalis::prelude::*;
use apalis_redis::RedisStorage;
use chrono::{DateTime, Duration, Utc};
use common::import_failure::ImportKind;
use common::jobs::MinidumpJob;
use futures::stream::TryStreamExt;
use object_store::{ObjectStore, path::Path};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::import_failure;
use crate::jobs::{ImportCrashJob, ImportSymbolJob};
use crate::state::AppState;

/// Work older than this is assumed to have lost its queued job.
const STALE_AFTER: Duration = Duration::hours(1);

/// A crash that keeps failing is left alone after this many requeues.
const MAX_REQUEUES: i64 = 3;

/// How long a requeue counter is remembered.
const COUNTER_TTL_SECONDS: i64 = 30 * 24 * 3600;

/// Queues the sweeper can push recovered work onto.
#[derive(Clone)]
pub struct RequeueTargets {
    pub minidump: RedisStorage<MinidumpJob>,
    pub import_crash: RedisStorage<ImportCrashJob>,
    pub import_symbol: RedisStorage<ImportSymbolJob>,
    pub counters: redis::aio::ConnectionManager,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Minidump,
    ImportCrash,
    ImportSymbol,
}

impl Stage {
    fn prefix(self) -> &'static str {
        match self {
            Stage::Minidump => "crashes/",
            Stage::ImportCrash => "processed-crashes/",
            Stage::ImportSymbol => "processed-symbols/",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Stage::Minidump => "minidump",
            Stage::ImportCrash => "import-crash",
            Stage::ImportSymbol => "import-symbol",
        }
    }

    fn import_kind(self) -> Option<ImportKind> {
        match self {
            Stage::Minidump => None,
            Stage::ImportCrash => Some(ImportKind::Crash),
            Stage::ImportSymbol => Some(ImportKind::Symbol),
        }
    }
}

/// Requeues work whose queued job was lost.
///
/// Every pipeline stage keeps its input in object storage until the stage
/// completes: `crashes/<id>.json` until the processor is done, and
/// `processed-{crashes,symbols}/<id>.json` until the curator has imported it. If
/// Valkey loses its queues, or a worker dies, those files stay behind and
/// nothing picks them up again. This sweeper pushes a fresh job for every file
/// that has been waiting longer than [`STALE_AFTER`]. The workers skip jobs
/// whose work is already done, so requeuing a job that is still queued is safe.
enum Requeue {
    Pushed,
    /// Still waiting after the maximum number of requeues.
    GaveUp,
    /// A recorded import failure, which is retried from the UI.
    RetriedFromUi,
}

pub struct StalledWorkRequeuer;

impl StalledWorkRequeuer {
    pub async fn run(
        app_state: &AppState,
        targets: &RequeueTargets,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting requeue of stalled work");
        let cutoff = Utc::now() - STALE_AFTER;
        let mut targets = targets.clone();
        // Work that should have been done by now; reported by the curator's
        // /pipeline check. Recorded import failures wait for a retry from the UI.
        let mut stuck = 0usize;

        for stage in [Stage::Minidump, Stage::ImportCrash, Stage::ImportSymbol] {
            let stalled = Self::find_stalled(&app_state.storage, stage, cutoff).await?;
            let mut requeued = 0usize;
            for id in &stalled {
                match Self::requeue(&app_state.storage, &mut targets, stage, id).await {
                    Ok(Requeue::Pushed) => {
                        requeued += 1;
                        stuck += 1;
                    }
                    Ok(Requeue::GaveUp) => stuck += 1,
                    Ok(Requeue::RetriedFromUi) => {}
                    Err(e) => {
                        stuck += 1;
                        error!(stage = stage.name(), id = %id, error = %e, "Failed to requeue stalled work")
                    }
                }
            }
            info!(
                stage = stage.name(),
                stalled = stalled.len(),
                requeued,
                "Checked for stalled work"
            );
        }

        crate::pipeline::record_stuck_work(&mut targets.counters, stuck).await;
        info!(stuck, "Completed requeue of stalled work");
        Ok(())
    }

    async fn find_stalled(
        storage: &Arc<dyn ObjectStore>,
        stage: Stage,
        cutoff: DateTime<Utc>,
    ) -> Result<Vec<String>, object_store::Error> {
        let mut ids = Vec::new();
        let mut listing = storage.list(Some(&Path::from(stage.prefix())));
        while let Some(meta) = listing.try_next().await? {
            if meta.last_modified >= cutoff {
                continue;
            }
            if let Some(id) = stalled_id(stage, meta.location.as_ref()) {
                ids.push(id.to_string());
            }
        }
        Ok(ids)
    }

    async fn requeue(
        storage: &Arc<dyn ObjectStore>,
        targets: &mut RequeueTargets,
        stage: Stage,
        id: &str,
    ) -> Result<Requeue, String> {
        // A recorded import failure is retried from the UI, not by the sweeper.
        if let Some(kind) = stage.import_kind()
            && import_failure::load(storage, kind, id).await.is_some()
        {
            return Ok(Requeue::RetriedFromUi);
        }

        let attempt = Self::next_attempt(&mut targets.counters, stage, id).await?;
        if attempt > MAX_REQUEUES {
            if attempt == MAX_REQUEUES + 1 {
                warn!(
                    stage = stage.name(),
                    id = %id,
                    "Stalled work still not done after {MAX_REQUEUES} requeues; giving up"
                );
            }
            return Ok(Requeue::GaveUp);
        }

        match stage {
            Stage::Minidump => {
                let crash =
                    import_failure::read_json(storage, &format!("crashes/{id}.json")).await?;
                targets
                    .minidump
                    .push(MinidumpJob { crash })
                    .await
                    .map_err(|e| e.to_string())?;
            }
            Stage::ImportCrash => {
                targets
                    .import_crash
                    .push(ImportCrashJob {
                        crash_id: id.to_string(),
                    })
                    .await
                    .map_err(|e| e.to_string())?;
            }
            Stage::ImportSymbol => {
                targets
                    .import_symbol
                    .push(ImportSymbolJob {
                        symbol_upload_id: id.to_string(),
                    })
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        info!(stage = stage.name(), id = %id, attempt, "Requeued stalled work");
        Ok(Requeue::Pushed)
    }

    async fn next_attempt(
        counters: &mut redis::aio::ConnectionManager,
        stage: Stage,
        id: &str,
    ) -> Result<i64, String> {
        let key = format!("guardrail::requeues::{}:{id}", stage.name());
        let (attempt, _): (i64, i64) = redis::pipe()
            .atomic()
            .incr(&key, 1)
            .expire(&key, COUNTER_TTL_SECONDS)
            .query_async(counters)
            .await
            .map_err(|e| format!("update requeue counter {key}: {e}"))?;
        Ok(attempt)
    }
}

/// Extracts the work id from a stage's storage path, ignoring anything else
/// that lives under the prefix.
fn stalled_id(stage: Stage, location: &str) -> Option<&str> {
    let id = location
        .strip_prefix(stage.prefix())?
        .strip_suffix(".json")?;
    (!id.is_empty() && !id.contains('/')).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stalled_id_extracts_ids_for_each_stage() {
        assert_eq!(stalled_id(Stage::Minidump, "crashes/abc.json"), Some("abc"));
        assert_eq!(stalled_id(Stage::ImportCrash, "processed-crashes/abc.json"), Some("abc"));
        assert_eq!(stalled_id(Stage::ImportSymbol, "processed-symbols/abc.json"), Some("abc"));
    }

    #[test]
    fn stalled_id_ignores_unrelated_paths() {
        assert_eq!(stalled_id(Stage::Minidump, "crashes/abc.txt"), None);
        assert_eq!(stalled_id(Stage::Minidump, "crashes/nested/abc.json"), None);
        assert_eq!(stalled_id(Stage::Minidump, "crashes/.json"), None);
        assert_eq!(stalled_id(Stage::Minidump, "processed-crashes/abc.json"), None);
    }
}
