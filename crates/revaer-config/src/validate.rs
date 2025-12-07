//! Validation helpers and parsing utilities for configuration documents.

use std::net::IpAddr;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::ApiKeyRateLimit;

/// Structured errors emitted during configuration validation/mutation.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Attempted to modify a field marked as immutable.
    #[error("immutable field '{field}' in '{section}' cannot be modified")]
    ImmutableField {
        /// Section containing the immutable field.
        section: String,
        /// Name of the immutable field.
        field: String,
    },

    /// Field contained an invalid value.
    #[error("invalid value for '{field}' in '{section}': {message}")]
    InvalidField {
        /// Section that failed validation.
        section: String,
        /// Field that failed validation.
        field: String,
        /// Human-readable error description.
        message: String,
    },

    /// Field did not exist in the target section.
    #[error("unknown field '{field}' in '{section}' settings")]
    UnknownField {
        /// Section where the unknown field was encountered.
        section: String,
        /// Name of the unexpected field.
        field: String,
    },
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn parse_port(value: &Value, section: &str, field: &str) -> Result<i32> {
    let port = value.as_i64().ok_or_else(|| ConfigError::InvalidField {
        section: section.to_string(),
        field: field.to_string(),
        message: "must be an integer".to_string(),
    })?;

    if !(1..=65_535).contains(&port) {
        return Err(ConfigError::InvalidField {
            section: section.to_string(),
            field: field.to_string(),
            message: "must be between 1 and 65535".to_string(),
        }
        .into());
    }

    let port_i32 = i32::try_from(port).map_err(|_| ConfigError::InvalidField {
        section: section.to_string(),
        field: field.to_string(),
        message: "must fit within 32-bit signed integer range".to_string(),
    })?;

    Ok(port_i32)
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn parse_api_key_rate_limit(value: &Value) -> Result<Option<ApiKeyRateLimit>> {
    let map = value.as_object().ok_or_else(|| ConfigError::InvalidField {
        section: "api_keys".to_string(),
        field: "rate_limit".to_string(),
        message: "must be an object".to_string(),
    })?;

    if map.is_empty() {
        return Ok(None);
    }

    let burst =
        map.get("burst")
            .and_then(Value::as_i64)
            .ok_or_else(|| ConfigError::InvalidField {
                section: "api_keys".to_string(),
                field: "rate_limit.burst".to_string(),
                message: "must be an integer".to_string(),
            })?;
    let per_seconds = map
        .get("per_seconds")
        .and_then(Value::as_u64)
        .ok_or_else(|| ConfigError::InvalidField {
            section: "api_keys".to_string(),
            field: "rate_limit.per_seconds".to_string(),
            message: "must be an integer".to_string(),
        })?;

    if burst <= 0 {
        return Err(ConfigError::InvalidField {
            section: "api_keys".to_string(),
            field: "rate_limit.burst".to_string(),
            message: "must be positive".to_string(),
        }
        .into());
    }

    let burst_u32 = u32::try_from(burst).map_err(|_| ConfigError::InvalidField {
        section: "api_keys".to_string(),
        field: "rate_limit.burst".to_string(),
        message: "must be between 1 and 4_294_967_295".to_string(),
    })?;

    Ok(Some(ApiKeyRateLimit {
        burst: burst_u32,
        replenish_period: Duration::from_secs(per_seconds),
    }))
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn parse_api_key_rate_limit_for_config(
    value: &Value,
) -> Result<Option<ApiKeyRateLimit>> {
    match value {
        Value::Null => Ok(None),
        Value::Object(map) if map.is_empty() => Ok(None),
        Value::Object(_) => parse_api_key_rate_limit(value),
        _ => Err(ConfigError::InvalidField {
            section: "api_keys".to_string(),
            field: "rate_limit".to_string(),
            message: "must be an object".to_string(),
        }
        .into()),
    }
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn parse_uuid(value: &str) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|err| anyhow!("invalid UUID '{value}': {err}"))
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn parse_bind_addr(value: &str) -> Result<IpAddr> {
    let Some(host) = value.split('/').next() else {
        return Err(anyhow!("invalid bind address '{value}'"));
    };

    host.parse::<IpAddr>()
        .with_context(|| format!("invalid bind address '{value}'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Map, json};
    use std::str::FromStr;

    #[test]
    fn parse_rate_limit_accepts_empty_object() {
        let value = Value::Object(Map::new());
        let parsed =
            parse_api_key_rate_limit_for_config(&value).expect("empty object should be accepted");
        assert!(parsed.is_none());
    }

    #[test]
    fn parse_rate_limit_valid_configuration() {
        let value = json!({ "burst": 10, "per_seconds": 60 });
        let parsed =
            parse_api_key_rate_limit_for_config(&value).expect("valid configuration should parse");
        let limit = parsed.expect("limit should be present");
        assert_eq!(limit.burst, 10);
        assert_eq!(limit.replenish_period, Duration::from_secs(60));
    }

    #[test]
    fn parse_rate_limit_rejects_zero_burst() {
        let value = json!({ "burst": 0, "per_seconds": 30 });
        let err = parse_api_key_rate_limit_for_config(&value).unwrap_err();
        assert!(err.downcast_ref::<ConfigError>().is_some());
    }

    #[test]
    fn parse_rate_limit_rejects_missing_fields() {
        let value = json!({ "burst": 5 });
        let err = parse_api_key_rate_limit_for_config(&value).unwrap_err();
        assert!(err.downcast_ref::<ConfigError>().is_some());
    }

    #[test]
    fn app_mode_parses_and_formats() {
        assert_eq!(
            crate::AppMode::from_str("setup").unwrap(),
            crate::AppMode::Setup
        );
        assert_eq!(
            crate::AppMode::from_str("active").unwrap(),
            crate::AppMode::Active
        );
        assert!(crate::AppMode::from_str("invalid").is_err());
        assert_eq!(crate::AppMode::Setup.as_str(), "setup");
        assert_eq!(crate::AppMode::Active.as_str(), "active");
    }

    #[test]
    fn parse_port_accepts_valid_range() {
        let value = json!(8080);
        let port = parse_port(&value, "app_profile", "http_port").expect("port should parse");
        assert_eq!(port, 8080);
    }

    #[test]
    fn parse_port_rejects_out_of_range_and_non_numeric() {
        let value = json!(0);
        let err = parse_port(&value, "app_profile", "http_port").unwrap_err();
        assert!(err.to_string().contains("between 1 and 65535"));

        let non_numeric = json!("not-a-port");
        let err = parse_port(&non_numeric, "app_profile", "http_port").unwrap_err();
        assert!(err.to_string().contains("must be an integer"));
    }

    #[test]
    fn parse_uuid_validates_format() {
        assert!(parse_uuid("00000000-0000-0000-0000-000000000001").is_ok());
        assert!(parse_uuid("invalid").is_err());
    }
}
