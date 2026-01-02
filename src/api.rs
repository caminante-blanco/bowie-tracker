use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use chrono::Utc;
use gloo_timers::future::sleep;
use std::time::Duration;
use web_sys::console;

static RATELIMIT_REMAINING: AtomicUsize = AtomicUsize::new(30);
static RATELIMIT_RESET_AT: AtomicI64 = AtomicI64::new(0);

pub async fn fetch_with_rate_limit(url: &str, token: &str) -> Result<reqwest::Response, String> {
    let now = Utc::now().timestamp();
    let reset_at = RATELIMIT_RESET_AT.load(Ordering::Relaxed);
    let remaining = RATELIMIT_REMAINING.load(Ordering::Relaxed);

    // If we are getting close to the limit (e.g., < 2 remaining), 
    // and the reset time is in the future, wait a bit.
    if remaining < 2 && now < reset_at {
        let wait_ms = ((reset_at - now) * 1000).max(100);
        console::log_1(&format!("API Limit close. Throttling for {}ms...", wait_ms).into());
        sleep(Duration::from_millis(wait_ms as u64)).await;
    }

    let client = reqwest::Client::new();
    let mut req = client.get(url);
    if !token.is_empty() {
        req = req.header("Authorization", format!("Token {}", token));
    }

    let resp = req.send().await.map_err(|e| e.to_string())?;

    // Update rate limit state from headers
    if let Some(rem) = resp.headers().get("x-ratelimit-remaining") {
        if let Ok(val) = rem.to_str().unwrap_or_default().parse::<usize>() {
            RATELIMIT_REMAINING.store(val, Ordering::Relaxed);
        }
    }
    if let Some(reset) = resp.headers().get("x-ratelimit-reset") {
        if let Ok(val) = reset.to_str().unwrap_or_default().parse::<i64>() {
            // Note: ListenBrainz reset header is a Unix timestamp
            RATELIMIT_RESET_AT.store(val, Ordering::Relaxed);
        }
    }

    Ok(resp)
}

