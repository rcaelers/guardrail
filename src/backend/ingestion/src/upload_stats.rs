use redis::aio::ConnectionManager;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

use crate::error::ApiError;

/// Rejections needed in the window before the upload path counts as broken,
/// so a single bad client cannot trigger an alert.
const MIN_REJECTIONS: u64 = 5;

/// Buckets are kept a little longer than the two-hour window they serve.
const BUCKET_TTL_SECONDS: i64 = 3 * 3600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UploadOutcome {
    Accepted,
    /// The product token is unknown. Real clients only see this when the
    /// product cache is wrong, as happened when Valkey was wiped.
    ProductNotFound,
    /// The server could not take the upload (5xx).
    ServerError,
    /// Rejected for a client-side reason, e.g. validation or a bad request.
    Other,
}

impl UploadOutcome {
    pub fn from_error(error: &ApiError) -> Self {
        match error {
            ApiError::ProductNotFound(_) => Self::ProductNotFound,
            ApiError::ServiceUnavailable(_) | ApiError::InternalFailure() => Self::ServerError,
            _ => Self::Other,
        }
    }

    fn field(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::ProductNotFound => "product_not_found",
            Self::ServerError => "server_error",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
pub struct UploadCounts {
    pub accepted: u64,
    pub product_not_found: u64,
    pub server_error: u64,
    pub other: u64,
}

impl UploadCounts {
    fn add(&mut self, field: &str, n: u64) {
        match field {
            "accepted" => self.accepted += n,
            "product_not_found" => self.product_not_found += n,
            "server_error" => self.server_error += n,
            "other" => self.other += n,
            _ => {}
        }
    }

    /// Problems that mean real uploads are being lost.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.product_not_found >= MIN_REJECTIONS && self.accepted == 0 {
            problems.push(format!(
                "{} uploads rejected as unknown product and none accepted",
                self.product_not_found
            ));
        }
        if self.server_error >= MIN_REJECTIONS && self.server_error > self.accepted {
            problems.push(format!(
                "{} uploads failed with server errors, {} accepted",
                self.server_error, self.accepted
            ));
        }
        problems
    }
}

#[derive(Debug, Serialize)]
pub struct UploadHealth {
    pub healthy: bool,
    pub problems: Vec<String>,
    /// Counts for the current and previous hour.
    pub window: UploadCounts,
}

#[derive(Debug, Clone)]
enum Backend {
    Redis(ConnectionManager),
    Memory(Arc<Mutex<HashMap<u64, UploadCounts>>>),
}

/// Hourly upload outcome counters, used to detect that uploads are being
/// rejected while every service still looks healthy.
#[derive(Debug, Clone)]
pub struct UploadStats {
    backend: Backend,
}

impl UploadStats {
    pub fn new(redis: ConnectionManager) -> Self {
        Self {
            backend: Backend::Redis(redis),
        }
    }

    pub fn memory() -> Self {
        Self {
            backend: Backend::Memory(Arc::default()),
        }
    }

    /// Never fails: losing a counter must not fail an upload.
    pub async fn record(&self, outcome: UploadOutcome) {
        let hour = current_hour();
        match &self.backend {
            Backend::Redis(conn) => {
                let key = bucket_key(hour);
                let result: redis::RedisResult<()> = redis::pipe()
                    .hincr(&key, outcome.field(), 1)
                    .ignore()
                    .expire(&key, BUCKET_TTL_SECONDS)
                    .ignore()
                    .query_async(&mut conn.clone())
                    .await;
                if let Err(e) = result {
                    debug!(error = ?e, "Failed to record upload outcome");
                }
            }
            Backend::Memory(buckets) => {
                if let Ok(mut buckets) = buckets.lock() {
                    buckets.entry(hour).or_default().add(outcome.field(), 1);
                }
            }
        }
    }

    pub async fn health(&self) -> Result<UploadHealth, ApiError> {
        let hour = current_hour();
        let mut window = UploadCounts::default();
        match &self.backend {
            Backend::Redis(conn) => {
                for bucket in [hour.saturating_sub(1), hour] {
                    let fields: HashMap<String, u64> = redis::cmd("HGETALL")
                        .arg(bucket_key(bucket))
                        .query_async(&mut conn.clone())
                        .await
                        .map_err(|_| ApiError::ServiceUnavailable("cache unavailable".into()))?;
                    for (field, n) in fields {
                        window.add(&field, n);
                    }
                }
            }
            Backend::Memory(buckets) => {
                if let Ok(buckets) = buckets.lock() {
                    for bucket in [hour.saturating_sub(1), hour] {
                        if let Some(counts) = buckets.get(&bucket) {
                            window.accepted += counts.accepted;
                            window.product_not_found += counts.product_not_found;
                            window.server_error += counts.server_error;
                            window.other += counts.other;
                        }
                    }
                }
            }
        }
        let problems = window.problems();
        Ok(UploadHealth {
            healthy: problems.is_empty(),
            problems,
            window,
        })
    }
}

fn current_hour() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 3600)
        .unwrap_or(0)
}

fn bucket_key(hour: u64) -> String {
    format!("guardrail::upload-stats:{hour}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(accepted: u64, product_not_found: u64, server_error: u64) -> UploadCounts {
        UploadCounts {
            accepted,
            product_not_found,
            server_error,
            other: 0,
        }
    }

    #[test]
    fn unknown_product_rejections_without_accepted_uploads_are_a_problem() {
        assert_eq!(counts(0, 8, 0).problems().len(), 1);
        // A few stray bad tokens, or real uploads still getting through, are fine.
        assert!(counts(0, 4, 0).problems().is_empty());
        assert!(counts(3, 8, 0).problems().is_empty());
    }

    #[test]
    fn server_errors_outnumbering_accepted_uploads_are_a_problem() {
        assert_eq!(counts(2, 0, 6).problems().len(), 1);
        assert!(counts(0, 0, 4).problems().is_empty());
        assert!(counts(10, 0, 6).problems().is_empty());
    }

    #[test]
    fn outcome_is_derived_from_the_rejection_reason() {
        assert_eq!(
            UploadOutcome::from_error(&ApiError::ProductNotFound("t".into())),
            UploadOutcome::ProductNotFound
        );
        assert_eq!(
            UploadOutcome::from_error(&ApiError::ServiceUnavailable("x".into())),
            UploadOutcome::ServerError
        );
        assert_eq!(
            UploadOutcome::from_error(&ApiError::ValidationError("p".into(), "old".into())),
            UploadOutcome::Other
        );
    }

    #[tokio::test]
    async fn health_reports_rejected_uploads() {
        let stats = UploadStats::memory();
        assert!(stats.health().await.unwrap().healthy);

        for _ in 0..5 {
            stats.record(UploadOutcome::ProductNotFound).await;
        }
        let health = stats.health().await.unwrap();
        assert!(!health.healthy);
        assert_eq!(health.window.product_not_found, 5);

        stats.record(UploadOutcome::Accepted).await;
        assert!(stats.health().await.unwrap().healthy);
    }
}
