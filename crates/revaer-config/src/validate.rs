//! Validation helpers and parsing utilities for configuration documents.

use std::collections::HashSet;
use std::net::IpAddr;

use crate::error::{ConfigError, ConfigResult};
use uuid::Uuid;

use crate::ApiKeyRateLimit;

/// Ensure a port number is within the valid TCP/UDP range.
///
/// # Errors
///
/// Returns `ConfigError::InvalidField` when the value is out of range.
pub fn validate_port(value: i32, section: &str, field: &str) -> ConfigResult<()> {
    if !(1..=65_535).contains(&value) {
        return Err(ConfigError::InvalidField {
            section: section.to_string(),
            field: field.to_string(),
            value: Some(value.to_string()),
            reason: "must be between 1 and 65535",
        });
    }
    Ok(())
}

/// Validate the rate limit settings for an API key.
///
/// # Errors
///
/// Returns `ConfigError::InvalidField` when the limits are invalid.
pub fn validate_api_key_rate_limit(limit: &ApiKeyRateLimit) -> ConfigResult<()> {
    if limit.burst == 0 {
        return Err(ConfigError::InvalidField {
            section: "api_keys".to_string(),
            field: "rate_limit.burst".to_string(),
            value: Some(limit.burst.to_string()),
            reason: "must be positive",
        });
    }

    if limit.replenish_period.as_secs() == 0 {
        return Err(ConfigError::InvalidField {
            section: "api_keys".to_string(),
            field: "rate_limit.per_seconds".to_string(),
            value: Some(limit.replenish_period.as_secs().to_string()),
            reason: "must be at least 1 second",
        });
    }

    Ok(())
}

/// Parse a UUID from a string value.
///
/// # Errors
///
/// Returns `ConfigError::InvalidUuid` when parsing fails.
pub fn parse_uuid(value: &str) -> ConfigResult<Uuid> {
    Uuid::parse_str(value).map_err(|_err| ConfigError::InvalidUuid {
        value: value.to_string(),
    })
}

/// Parse a bind address from a string value.
///
/// # Errors
///
/// Returns `ConfigError::InvalidBindAddr` when parsing fails.
pub fn parse_bind_addr(value: &str) -> ConfigResult<IpAddr> {
    let host = value.split('/').next().unwrap_or("");
    host.parse::<IpAddr>()
        .map_err(|_err| ConfigError::InvalidBindAddr {
            value: value.to_string(),
        })
}

/// Ensure a field is not marked as immutable.
///
/// # Errors
///
/// Returns `ConfigError::ImmutableField` when the field is immutable.
pub fn ensure_mutable<S: std::hash::BuildHasher>(
    immutable_keys: &HashSet<String, S>,
    section: &str,
    field: &str,
) -> ConfigResult<()> {
    if field != "immutable_keys" {
        let scoped = format!("{section}.{field}");
        let scoped_wildcard = format!("{section}.*");
        if immutable_keys.contains(section)
            || immutable_keys.contains(field)
            || immutable_keys.contains(&scoped)
            || immutable_keys.contains(&scoped_wildcard)
        {
            return Err(ConfigError::ImmutableField {
                section: section.to_string(),
                field: field.to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_ports() {
        assert!(matches!(
            validate_port(0, "app_profile", "http_port"),
            Err(ConfigError::InvalidField { .. })
        ));
    }

    #[test]
    fn allows_valid_ports() {
        assert!(validate_port(7070, "app_profile", "http_port").is_ok());
    }
}
