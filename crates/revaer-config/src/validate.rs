//! Validation helpers and parsing utilities for configuration documents.

use std::collections::HashSet;
use std::net::IpAddr;

use anyhow::{Context, Result, anyhow};
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
pub(crate) fn validate_port(value: i32, section: &str, field: &str) -> Result<(), ConfigError> {
    if !(1..=65_535).contains(&value) {
        return Err(ConfigError::InvalidField {
            section: section.to_string(),
            field: field.to_string(),
            message: "must be between 1 and 65535".to_string(),
        });
    }
    Ok(())
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn validate_api_key_rate_limit(limit: &ApiKeyRateLimit) -> Result<(), ConfigError> {
    if limit.burst == 0 {
        return Err(ConfigError::InvalidField {
            section: "api_keys".to_string(),
            field: "rate_limit.burst".to_string(),
            message: "must be positive".to_string(),
        });
    }

    if limit.replenish_period.as_secs() == 0 {
        return Err(ConfigError::InvalidField {
            section: "api_keys".to_string(),
            field: "rate_limit.per_seconds".to_string(),
            message: "must be at least 1 second".to_string(),
        });
    }

    Ok(())
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

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn ensure_mutable(
    immutable_keys: &HashSet<String>,
    section: &str,
    field: &str,
) -> Result<(), ConfigError> {
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
        let err = validate_port(0, "app_profile", "http_port").unwrap_err();
        assert!(matches!(err, ConfigError::InvalidField { .. }));
    }

    #[test]
    fn allows_valid_ports() {
        assert!(validate_port(7070, "app_profile", "http_port").is_ok());
    }
}
