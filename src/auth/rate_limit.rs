// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::api::router::AppState;
use crate::auth::{AuthContext, AuthPrincipal};

#[derive(Debug, Clone)]
struct WindowCounter {
    count: u32,
    window_start: Instant,
}

#[derive(Default)]
pub struct RateLimitStore {
    minute: Mutex<HashMap<String, WindowCounter>>,
    hour: Mutex<HashMap<String, WindowCounter>>,
}

impl RateLimitStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn check_window(
        map: &Mutex<HashMap<String, WindowCounter>>,
        key_id: &str,
        limit: u32,
        window: Duration,
    ) -> Result<(u32, u64), u64> {
        let mut guard = map.lock().unwrap();
        let now = Instant::now();
        let entry = guard.entry(key_id.to_string()).or_insert(WindowCounter {
            count: 0,
            window_start: now,
        });
        if now.duration_since(entry.window_start) >= window {
            entry.count = 0;
            entry.window_start = now;
        }
        if entry.count >= limit {
            let reset_secs = window
                .saturating_sub(now.duration_since(entry.window_start))
                .as_secs()
                .max(1);
            return Err(reset_secs);
        }
        entry.count += 1;
        let remaining = limit.saturating_sub(entry.count);
        let reset_secs = window
            .saturating_sub(now.duration_since(entry.window_start))
            .as_secs()
            .max(1);
        Ok((remaining, reset_secs))
    }

    pub fn check(&self, key_id: &str, rpm: Option<u32>, rph: Option<u32>) -> RateLimitStatus {
        let mut status = RateLimitStatus::default();
        if let Some(limit) = rpm {
            match Self::check_window(&self.minute, key_id, limit, Duration::from_secs(60)) {
                Ok((remaining, reset)) => status.observe(limit, remaining, reset),
                Err(reset) => return RateLimitStatus::limited(limit, reset),
            }
        }
        if let Some(limit) = rph {
            match Self::check_window(&self.hour, key_id, limit, Duration::from_secs(3600)) {
                Ok((remaining, reset)) => status.observe(limit, remaining, reset),
                Err(reset) => return RateLimitStatus::limited(limit, reset),
            }
        }
        status
    }
}

/// Outcome of a rate-limit check, carrying header data for the most constraining window.
#[derive(Debug, Default, Clone)]
pub struct RateLimitStatus {
    pub limited: bool,
    pub limit: Option<u32>,
    pub remaining: Option<u32>,
    pub reset: Option<u64>,
    pub retry_after: Option<u64>,
}

impl RateLimitStatus {
    fn limited(limit: u32, reset: u64) -> Self {
        Self {
            limited: true,
            limit: Some(limit),
            remaining: Some(0),
            reset: Some(reset),
            retry_after: Some(reset),
        }
    }

    /// Track the window with the fewest remaining requests for the response headers.
    fn observe(&mut self, limit: u32, remaining: u32, reset: u64) {
        let replace = match self.remaining {
            Some(current) => remaining < current,
            None => true,
        };
        if replace {
            self.limit = Some(limit);
            self.remaining = Some(remaining);
            self.reset = Some(reset);
        }
    }
}

fn reset_epoch(reset_secs: u64) -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + reset_secs
}

fn apply_rate_limit_headers(response: &mut Response, status: &RateLimitStatus) {
    let headers = response.headers_mut();
    if let Some(limit) = status.limit {
        if let Ok(value) = limit.to_string().parse() {
            headers.insert("X-RateLimit-Limit", value);
        }
    }
    if let Some(remaining) = status.remaining {
        if let Ok(value) = remaining.to_string().parse() {
            headers.insert("X-RateLimit-Remaining", value);
        }
    }
    if let Some(reset) = status.reset {
        if let Ok(value) = reset_epoch(reset).to_string().parse() {
            headers.insert("X-RateLimit-Reset", value);
        }
    }
}

pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    let auth = req.extensions().get::<AuthContext>().cloned();
    let Some(auth) = auth else {
        return Ok(next.run(req).await);
    };

    let AuthPrincipal::ApiKey { key_id, .. } = &auth.principal else {
        return Ok(next.run(req).await);
    };

    if auth.rpm.is_none() && auth.rph.is_none() {
        return Ok(next.run(req).await);
    }

    let status = state.rate_limits.check(key_id, auth.rpm, auth.rph);

    if status.limited {
        let retry_after = status.retry_after.unwrap_or(1);
        let body = Json(serde_json::json!({
            "error": "rate limit exceeded",
            "retry_after": retry_after,
        }));
        let mut response = (StatusCode::TOO_MANY_REQUESTS, body).into_response();
        if let Ok(value) = retry_after.to_string().parse() {
            response.headers_mut().insert("Retry-After", value);
        }
        apply_rate_limit_headers(&mut response, &status);
        return Err(response);
    }

    let mut response = next.run(req).await;
    apply_rate_limit_headers(&mut response, &status);
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_counter_enforces_limit() {
        let store = RateLimitStore::new();
        let key = "k1";
        for _ in 0..5 {
            assert!(!store.check(key, Some(5), None).limited);
        }
        assert!(store.check(key, Some(5), None).limited);
    }

    #[test]
    fn status_reports_remaining_for_most_constraining_window() {
        let store = RateLimitStore::new();
        let key = "k2";
        let status = store.check(key, Some(10), Some(2));
        assert!(!status.limited);
        // hour window (limit 2, remaining 1) is more constraining than minute (limit 10, remaining 9)
        assert_eq!(status.limit, Some(2));
        assert_eq!(status.remaining, Some(1));
    }

    #[test]
    fn rpm_one_blocks_second_request() {
        let store = RateLimitStore::new();
        let key = "k3";
        assert!(!store.check(key, Some(1), Some(1)).limited);
        assert!(store.check(key, Some(1), Some(1)).limited);
    }
}
