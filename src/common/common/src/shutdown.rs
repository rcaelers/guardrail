use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Tells a deliberate shutdown apart from workers that stopped on their own.
///
/// An apalis worker whose backend stream ends (for example because Valkey was
/// restarted) stops cleanly, but `Monitor::run_with_signal` keeps waiting for the
/// shutdown signal, so the process stays up with no workers and its health
/// endpoint keeps reporting healthy. Exiting lets Kubernetes restart the pod.
#[derive(Clone, Default)]
pub struct ShutdownGuard {
    requested: Arc<AtomicBool>,
}

impl ShutdownGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Wraps the shutdown signal so the guard knows when a stop was requested.
    pub fn watch<F, T>(&self, signal: F) -> impl Future<Output = T> + Send
    where
        F: Future<Output = T> + Send,
    {
        let requested = self.requested.clone();
        async move {
            let result = signal.await;
            requested.store(true, Ordering::SeqCst);
            result
        }
    }

    pub fn is_requested(&self) -> bool {
        self.requested.load(Ordering::SeqCst)
    }

    /// Exits the process when a worker stops without a shutdown request.
    pub fn worker_stopped(&self, worker: &str) {
        if !self.is_requested() {
            tracing::error!(
                worker = %worker,
                "Worker stopped without a shutdown request; exiting so the process is restarted"
            );
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn watch_marks_shutdown_as_requested_once_signal_completes() {
        let guard = ShutdownGuard::new();
        assert!(!guard.is_requested());

        let value = guard.watch(async { 42 }).await;

        assert_eq!(value, 42);
        assert!(guard.is_requested());
        // Must not exit once shutdown was requested.
        guard.worker_stopped("test");
    }
}
