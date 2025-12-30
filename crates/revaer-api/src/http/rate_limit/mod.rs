//! API key rate limiting primitives and HTTP header helpers.

use std::convert::TryFrom;
use std::fmt::{self, Display, Formatter};
use std::time::{Duration, Instant};

use crate::http::constants::{
    HEADER_RATE_LIMIT_LIMIT, HEADER_RATE_LIMIT_REMAINING, HEADER_RATE_LIMIT_RESET,
};
use axum::http::{HeaderMap, HeaderValue, header::RETRY_AFTER};
use revaer_config::ApiKeyRateLimit;

#[derive(Clone, Copy)]
pub(crate) struct RateLimitSnapshot {
    pub(crate) limit: u32,
    pub(crate) remaining: u32,
}

#[derive(Debug)]
pub(crate) struct RateLimitError {
    pub(crate) limit: u32,
    pub(crate) retry_after: Duration,
}

impl Display for RateLimitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("rate limit exceeded")
    }
}

impl std::error::Error for RateLimitError {}

pub(crate) struct RateLimiter {
    pub(crate) config: ApiKeyRateLimit,
    pub(crate) tokens: u128,
    pub(crate) last_refill: Instant,
}

pub(crate) struct RateLimitStatus {
    pub(crate) allowed: bool,
    pub(crate) remaining: u32,
    pub(crate) retry_after: Duration,
}

impl RateLimiter {
    const TOKEN_SCALE: u128 = 1_000_000;

    pub(crate) fn new(config: ApiKeyRateLimit) -> Self {
        let mut limiter = Self {
            config,
            tokens: 0,
            last_refill: Instant::now(),
        };
        limiter.tokens = limiter.capacity();
        limiter
    }

    pub(crate) fn capacity_for(config: &ApiKeyRateLimit) -> u128 {
        u128::from(config.burst) * Self::TOKEN_SCALE
    }

    fn capacity(&self) -> u128 {
        Self::capacity_for(&self.config)
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last_refill);
        if elapsed == Duration::ZERO {
            return;
        }

        let period_micros = self.config.replenish_period.as_micros();
        let capacity = self.capacity();
        if period_micros == 0 || capacity == 0 {
            self.tokens = capacity;
            self.last_refill = now;
            return;
        }

        let replenished = (capacity.saturating_mul(elapsed.as_micros())).checked_div(period_micros);

        if let Some(amount) = replenished
            && amount > 0
        {
            self.tokens = (self.tokens + amount).min(capacity);
            self.last_refill = now;
        }
    }

    pub(crate) fn evaluate(&mut self, config: &ApiKeyRateLimit, now: Instant) -> RateLimitStatus {
        if self.config != *config {
            self.config = config.clone();
            self.tokens = self.capacity();
            self.last_refill = now;
        }

        self.refill(now);

        if self.tokens >= Self::TOKEN_SCALE {
            self.tokens -= Self::TOKEN_SCALE;
            RateLimitStatus {
                allowed: true,
                remaining: self.remaining_tokens(),
                retry_after: Duration::ZERO,
            }
        } else {
            RateLimitStatus {
                allowed: false,
                remaining: 0,
                retry_after: self.retry_delay(),
            }
        }
    }

    fn remaining_tokens(&self) -> u32 {
        let tokens = self.tokens / Self::TOKEN_SCALE;
        u32::try_from(tokens).unwrap_or(u32::MAX)
    }

    fn retry_delay(&self) -> Duration {
        let capacity = self.capacity();
        if capacity == 0 {
            return Duration::MAX;
        }

        let period_micros = self.config.replenish_period.as_micros();
        if period_micros == 0 {
            return Duration::ZERO;
        }

        let deficit = Self::TOKEN_SCALE.saturating_sub(self.tokens);
        let needed = deficit.saturating_mul(period_micros);
        let retry_micros = needed.div_ceil(capacity);
        let clamped = retry_micros.min(u128::from(u64::MAX));
        let micros = u64::try_from(clamped).unwrap_or(u64::MAX);
        Duration::from_micros(micros)
    }
}

pub(crate) fn insert_rate_limit_headers(
    headers: &mut HeaderMap,
    limit: u32,
    remaining: u32,
    retry_after: Option<Duration>,
) {
    if let Ok(value) = HeaderValue::from_str(&limit.to_string()) {
        headers.insert(HEADER_RATE_LIMIT_LIMIT, value);
    }
    if let Ok(value) = HeaderValue::from_str(&remaining.to_string()) {
        headers.insert(HEADER_RATE_LIMIT_REMAINING, value);
    }
    if let Some(wait) = retry_after {
        let secs = wait.as_secs();
        let seconds = if secs == 0 && wait.subsec_nanos() > 0 {
            1
        } else {
            secs.max(1)
        };
        let text = seconds.to_string();
        if let Ok(value) = HeaderValue::from_str(&text) {
            headers.insert(RETRY_AFTER, value.clone());
            headers.insert(HEADER_RATE_LIMIT_RESET, value);
        }
    }
}
