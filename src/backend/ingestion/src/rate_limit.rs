//! Valkey-backed abuse protection for the public crash-upload endpoint.
//!
//! Fixed-window counters keyed by client IP and by product token are stored in
//! Valkey (Redis protocol) so the limits hold across replicas. The limiter
//! deliberately *fails open*: if Valkey is unavailable we log and allow the
//! request rather than turning a cache hiccup into an ingestion outage (the
//! product token still gates uploads).

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, State};
use axum::extract::Request;
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::Response;
use redis::aio::ConnectionManager;
use tracing::warn;

use crate::error::ApiError;
use crate::settings::RateLimit;

const WINDOW_SECS: i64 = 60;

#[derive(Clone)]
pub struct RateLimiter {
    manager: ConnectionManager,
    config: RateLimit,
}

impl std::fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimiter")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl RateLimiter {
    pub fn new(manager: ConnectionManager, config: RateLimit) -> Self {
        Self { manager, config }
    }

    /// Increment the fixed-window counter for `key` and return the new count.
    /// The TTL is set on first increment so the window is anchored to the first
    /// request and resets cleanly. INCR and EXPIRE run as one server-side
    /// script: issued as two separate commands, a failure between them would
    /// leave a counter with no TTL that grows forever, rate-limiting that
    /// client permanently.
    async fn incr(&self, key: &str) -> redis::RedisResult<u64> {
        const INCR_WITH_TTL: &str = r"
            local count = redis.call('INCR', KEYS[1])
            if count == 1 then
                redis.call('EXPIRE', KEYS[1], ARGV[1])
            end
            return count";
        let mut conn = self.manager.clone();
        redis::cmd("EVAL")
            .arg(INCR_WITH_TTL)
            .arg(1)
            .arg(key)
            .arg(WINDOW_SECS)
            .query_async(&mut conn)
            .await
    }

    /// Returns `Err(RateLimited)` when either the per-IP or per-token window is
    /// exhausted. Valkey errors are logged and treated as "allow".
    pub async fn check(&self, ip: &str, token: &str) -> Result<(), ApiError> {
        if self.config.per_ip_per_minute > 0 {
            self.enforce(&format!("rl:ip:{ip}"), self.config.per_ip_per_minute)
                .await?;
        }
        if self.config.per_token_per_minute > 0 {
            self.enforce(&format!("rl:token:{token}"), self.config.per_token_per_minute)
                .await?;
        }
        Ok(())
    }

    async fn enforce(&self, key: &str, limit: u64) -> Result<(), ApiError> {
        match self.incr(key).await {
            Ok(count) if count > limit => {
                warn!(key, count, limit, "rate limit exceeded");
                Err(ApiError::RateLimited)
            }
            Ok(_) => Ok(()),
            Err(e) => {
                // Fail open: do not let a Valkey outage block ingestion.
                warn!(key, error = ?e, "rate limiter unavailable, allowing request");
                Ok(())
            }
        }
    }
}

/// Extract the client IP, preferring the left-most `X-Forwarded-For` entry when
/// the deployment is configured to trust it, otherwise the socket peer address.
fn client_ip(headers: &HeaderMap, peer: Option<&SocketAddr>, trust_forwarded_for: bool) -> String {
    if trust_forwarded_for
        && let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok())
        && let Some(first) = xff.split(',').next()
    {
        let ip = first.trim();
        if !ip.is_empty() {
            return ip.to_string();
        }
    }
    peer.map(|p| p.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Extract the product token from a `/api/minidump/{token}/upload` path.
fn token_from_path(path: &str) -> Option<&str> {
    let mut segments = path.split('/').filter(|s| !s.is_empty());
    while let Some(seg) = segments.next() {
        if seg == "minidump" {
            return segments.next();
        }
    }
    None
}

/// Axum middleware applied to the upload route. No-ops when no limiter is
/// configured (e.g. in tests) or when rate limiting is disabled.
pub async fn rate_limit(
    State(limiter): State<Option<Arc<RateLimiter>>>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let Some(limiter) = limiter else {
        return Ok(next.run(request).await);
    };
    if !limiter.config.enabled {
        return Ok(next.run(request).await);
    }

    let ip = client_ip(
        request.headers(),
        request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| &ci.0),
        limiter.config.trust_forwarded_for,
    );
    let token = token_from_path(request.uri().path())
        .unwrap_or("unknown")
        .to_string();

    limiter.check(&ip, &token).await?;
    Ok(next.run(request).await)
}

// `AppState` exposes `Option<Arc<RateLimiter>>` to the middleware's `State`
// extractor: the `#[derive(FromRef)]` on `AppState` generates the impl from the
// `rate_limiter` field.

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn token_from_path_extracts_upload_token() {
        assert_eq!(token_from_path("/api/minidump/abc123/upload"), Some("abc123"));
        assert_eq!(token_from_path("/minidump/xyz/upload"), Some("xyz"));
        assert_eq!(token_from_path("/api/live"), None);
    }

    #[test]
    fn client_ip_prefers_forwarded_for_when_trusted() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("1.2.3.4, 5.6.7.8"));
        let peer: SocketAddr = "10.0.0.1:9000".parse().unwrap();

        assert_eq!(client_ip(&headers, Some(&peer), true), "1.2.3.4");
        // When not trusted, fall back to the socket peer.
        assert_eq!(client_ip(&headers, Some(&peer), false), "10.0.0.1");
        // No header, no peer.
        assert_eq!(client_ip(&HeaderMap::new(), None, true), "unknown");
    }
}
