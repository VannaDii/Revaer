//! Label policy parsing helpers.
//!
//! # Design
//! - Treat empty inputs as unset values.
//! - Validate numeric ranges client-side to match API expectations.
//! - Keep formatting/parsing centralized for consistency.

#[cfg(target_arch = "wasm32")]
use crate::models::TorrentLabelPolicy;

pub(crate) fn parse_optional_i32(field: &str, value: &str) -> Result<Option<i32>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed = trimmed
        .parse::<i32>()
        .map_err(|_| format!("{field} must be an integer"))?;
    if parsed < 0 {
        return Err(format!("{field} must be zero or a positive integer"));
    }
    Ok(Some(parsed))
}

pub(crate) fn parse_optional_u64(field: &str, value: &str) -> Result<Option<u64>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed = trimmed
        .parse::<u64>()
        .map_err(|_| format!("{field} must be an integer"))?;
    Ok(Some(parsed))
}

pub(crate) fn parse_optional_f64(field: &str, value: &str) -> Result<Option<f64>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed = trimmed
        .parse::<f64>()
        .map_err(|_| format!("{field} must be a number"))?;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(format!("{field} must be a non-negative number"));
    }
    Ok(Some(parsed))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn policy_badges(policy: &TorrentLabelPolicy) -> Vec<&'static str> {
    let mut badges = Vec::new();
    if policy.download_dir.is_some() {
        badges.push("download dir");
    }
    if policy.rate_limit.is_some() {
        badges.push("rate limit");
    }
    if policy.queue_position.is_some() {
        badges.push("queue");
    }
    if policy.auto_managed.is_some() {
        badges.push("auto-managed");
    }
    if policy.seed_ratio_limit.is_some() || policy.seed_time_limit.is_some() {
        badges.push("seeding");
    }
    if policy.cleanup.is_some() {
        badges.push("cleanup");
    }
    if badges.is_empty() {
        badges.push("default");
    }
    badges
}
