use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tokio::sync::Mutex;

// How long a cached authenticated DB handle stays valid.  Kept well below the
// JWT session duration (60 min default) so admin-status changes propagate
// within a reasonable window.
const TTL: Duration = Duration::from_secs(300);

type Inner = HashMap<String, (Arc<Surreal<Any>>, Instant)>;

// Per-user cache of authenticated SurrealDB handles.
//
// Calling clone()+authenticate() on the underlying WebSocket connection for
// every HTTP request generates a flood of Attach/Detach session messages that
// can overload the single-task SurrealDB WebSocket event loop and cause
// subsequent requests to hang.  Caching authenticated handles eliminates this
// churn: authenticate() is called at most once per user per TTL window, and
// concurrent requests reuse the same Arc<Surreal<Any>> (which is Send+Sync and
// supports concurrent queries via independent request IDs).
#[derive(Clone, Default)]
pub(crate) struct AuthCache(Arc<Mutex<Inner>>);

impl AuthCache {
    pub(crate) async fn get(&self, key: &str) -> Option<Arc<Surreal<Any>>> {
        let cache = self.0.lock().await;
        cache.get(key).and_then(|(handle, inserted_at)| {
            if inserted_at.elapsed() < TTL {
                Some(Arc::clone(handle))
            } else {
                None
            }
        })
    }

    pub(crate) async fn insert(&self, key: String, handle: Arc<Surreal<Any>>) {
        let mut cache = self.0.lock().await;
        cache.insert(key, (handle, Instant::now()));
    }
}
