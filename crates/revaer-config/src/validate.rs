//! Validation helpers and parsing utilities for configuration documents.

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::error::{ConfigError, ConfigResult};
use uuid::Uuid;

use crate::ApiKeyRateLimit;

/// Canonical CIDR entry with parsed range bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CidrEntry {
    /// Canonical CIDR string (network/prefix).
    pub cidr: String,
    /// Parsed range covered by the CIDR.
    pub range: CidrRange,
}

/// Inclusive IP address range derived from a CIDR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CidrRange {
    /// Start address of the CIDR range.
    pub start: IpAddr,
    /// End address of the CIDR range.
    pub end: IpAddr,
}

impl CidrRange {
    /// Returns true when the IP address is within the CIDR range.
    #[must_use]
    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.start, self.end, ip) {
            (IpAddr::V4(start), IpAddr::V4(end), IpAddr::V4(addr)) => {
                let start = u32::from(start);
                let end = u32::from(end);
                let addr = u32::from(addr);
                addr >= start && addr <= end
            }
            (IpAddr::V6(start), IpAddr::V6(end), IpAddr::V6(addr)) => {
                let start = u128::from(start);
                let end = u128::from(end);
                let addr = u128::from(addr);
                addr >= start && addr <= end
            }
            _ => false,
        }
    }
}

/// Parse and canonicalize CIDR entries with deduplication.
///
/// # Errors
///
/// Returns `ConfigError::InvalidField` when any entry is invalid.
pub fn canonicalize_cidr_entries(
    entries: &[String],
    section: &str,
    field: &str,
) -> ConfigResult<Vec<CidrEntry>> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for entry in entries {
        let parsed = parse_cidr_entry(entry, section, field)?;
        if seen.insert(parsed.cidr.clone()) {
            normalized.push(parsed);
        }
    }
    Ok(normalized)
}

/// Default local network CIDRs used for anonymous access.
#[must_use]
pub fn default_local_networks() -> Vec<String> {
    vec![
        "127.0.0.0/8",
        "10.0.0.0/8",
        "172.16.0.0/12",
        "192.168.0.0/16",
        "169.254.0.0/16",
        "::1/128",
        "fe80::/10",
        "fd00::/8",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

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

fn parse_cidr_entry(entry: &str, section: &str, field: &str) -> ConfigResult<CidrEntry> {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::InvalidField {
            section: section.to_string(),
            field: field.to_string(),
            value: Some(entry.to_string()),
            reason: "CIDR entries cannot be empty",
        });
    }

    let (network, prefix) = trimmed
        .split_once('/')
        .ok_or_else(|| ConfigError::InvalidField {
            section: section.to_string(),
            field: field.to_string(),
            value: Some(trimmed.to_string()),
            reason: "CIDR entries must include a /prefix",
        })?;
    let network = network.trim();
    let prefix = prefix.trim();
    if network.is_empty() || prefix.is_empty() {
        return Err(ConfigError::InvalidField {
            section: section.to_string(),
            field: field.to_string(),
            value: Some(trimmed.to_string()),
            reason: "CIDR entries must include a network and prefix",
        });
    }

    let parsed_ip: IpAddr = network.parse().map_err(|_err| ConfigError::InvalidField {
        section: section.to_string(),
        field: field.to_string(),
        value: Some(network.to_string()),
        reason: "CIDR entries must include a valid IP address",
    })?;
    let prefix_len: u8 = prefix.parse().map_err(|_err| ConfigError::InvalidField {
        section: section.to_string(),
        field: field.to_string(),
        value: Some(prefix.to_string()),
        reason: "CIDR prefix must be numeric",
    })?;

    let (cidr, range) = match parsed_ip {
        IpAddr::V4(addr) => {
            if prefix_len > 32 {
                return Err(ConfigError::InvalidField {
                    section: section.to_string(),
                    field: field.to_string(),
                    value: Some(prefix_len.to_string()),
                    reason: "IPv4 prefix must be <= 32",
                });
            }
            let mask = if prefix_len == 0 {
                0
            } else {
                u32::MAX << (32 - prefix_len)
            };
            let network = u32::from(addr) & mask;
            let start = IpAddr::V4(Ipv4Addr::from(network));
            let end = IpAddr::V4(Ipv4Addr::from(network | !mask));
            (
                format!("{}/{}", Ipv4Addr::from(network), prefix_len),
                CidrRange { start, end },
            )
        }
        IpAddr::V6(addr) => {
            if prefix_len > 128 {
                return Err(ConfigError::InvalidField {
                    section: section.to_string(),
                    field: field.to_string(),
                    value: Some(prefix_len.to_string()),
                    reason: "IPv6 prefix must be <= 128",
                });
            }
            let addr_u128 = u128::from(addr);
            let mask = if prefix_len == 0 {
                0
            } else {
                u128::MAX << (128 - prefix_len)
            };
            let network = addr_u128 & mask;
            let start = IpAddr::V6(Ipv6Addr::from(network));
            let end = IpAddr::V6(Ipv6Addr::from(network | !mask));
            (
                format!("{}/{}", Ipv6Addr::from(network), prefix_len),
                CidrRange { start, end },
            )
        }
    };

    Ok(CidrEntry { cidr, range })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::net::{Ipv4Addr, Ipv6Addr};
    use std::time::Duration;
    use uuid::Uuid;

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

    #[test]
    fn canonicalize_cidr_entries_dedupes_and_normalizes() {
        let entries = vec!["10.0.0.1/24".to_string(), " 10.0.0.0/24 ".to_string()];
        let parsed = canonicalize_cidr_entries(&entries, "app_profile", "local_networks")
            .expect("cidr entries");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].cidr, "10.0.0.0/24");
        assert!(
            parsed[0]
                .range
                .contains(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)))
        );
    }

    #[test]
    fn canonicalize_cidr_entries_rejects_invalid_prefix() {
        let entries = vec!["10.0.0.0/40".to_string()];
        assert!(matches!(
            canonicalize_cidr_entries(&entries, "app_profile", "local_networks"),
            Err(ConfigError::InvalidField { .. })
        ));
    }

    #[test]
    fn cidr_range_contains_respects_ip_family() {
        let entry =
            parse_cidr_entry("10.0.0.0/24", "app_profile", "local_networks").expect("valid cidr");
        assert!(entry.range.contains(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(!entry.range.contains(IpAddr::V6(Ipv6Addr::LOCALHOST)));
    }

    #[test]
    fn parse_cidr_entry_rejects_empty_and_missing_prefix() {
        assert!(matches!(
            parse_cidr_entry("", "app_profile", "local_networks"),
            Err(ConfigError::InvalidField { .. })
        ));
        assert!(matches!(
            parse_cidr_entry("10.0.0.1", "app_profile", "local_networks"),
            Err(ConfigError::InvalidField { .. })
        ));
    }

    #[test]
    fn parse_cidr_entry_rejects_invalid_ip() {
        assert!(matches!(
            parse_cidr_entry("bad/24", "app_profile", "local_networks"),
            Err(ConfigError::InvalidField { .. })
        ));
    }

    #[test]
    fn parse_bind_addr_accepts_ip_and_rejects_invalid() {
        assert_eq!(
            parse_bind_addr("127.0.0.1").unwrap(),
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        );
        assert_eq!(
            parse_bind_addr("127.0.0.1/32").unwrap(),
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        );
        assert!(matches!(
            parse_bind_addr("invalid"),
            Err(ConfigError::InvalidBindAddr { .. })
        ));
    }

    #[test]
    fn parse_uuid_accepts_and_rejects_values() {
        let valid = Uuid::new_v4();
        assert_eq!(parse_uuid(&valid.to_string()).unwrap(), valid);
        assert!(matches!(
            parse_uuid("not-a-uuid"),
            Err(ConfigError::InvalidUuid { .. })
        ));
    }

    #[test]
    fn ensure_mutable_respects_scoped_immutability() {
        let mut immutables = HashSet::new();
        immutables.insert("app_profile.http_port".to_string());
        immutables.insert("engine_profile.*".to_string());
        ensure_mutable(&immutables, "app_profile", "bind_addr")
            .expect("unrelated field is mutable");
        assert!(matches!(
            ensure_mutable(&immutables, "app_profile", "http_port"),
            Err(ConfigError::ImmutableField { .. })
        ));
        assert!(matches!(
            ensure_mutable(&immutables, "engine_profile", "listen_port"),
            Err(ConfigError::ImmutableField { .. })
        ));
        assert!(ensure_mutable(&immutables, "app_profile", "immutable_keys").is_ok());
    }

    #[test]
    fn default_local_networks_cover_loopback_and_private_ranges() {
        let defaults = default_local_networks();
        assert!(defaults.contains(&"127.0.0.0/8".to_string()));
        assert!(defaults.contains(&"10.0.0.0/8".to_string()));
        assert!(defaults.contains(&"::1/128".to_string()));
        assert!(defaults.contains(&"fd00::/8".to_string()));
    }

    #[test]
    fn validate_api_key_rate_limit_rejects_zero_values() {
        let err = validate_api_key_rate_limit(&ApiKeyRateLimit {
            burst: 0,
            replenish_period: Duration::from_secs(5),
        })
        .expect_err("zero burst should fail");
        assert!(matches!(
            err,
            ConfigError::InvalidField { section, field, reason, .. }
            if section == "api_keys" && field == "rate_limit.burst" && reason == "must be positive"
        ));

        let err = validate_api_key_rate_limit(&ApiKeyRateLimit {
            burst: 5,
            replenish_period: Duration::from_secs(0),
        })
        .expect_err("zero replenish period should fail");
        assert!(matches!(
            err,
            ConfigError::InvalidField { section, field, reason, .. }
            if section == "api_keys" && field == "rate_limit.per_seconds" && reason == "must be at least 1 second"
        ));

        assert!(validate_api_key_rate_limit(&ApiKeyRateLimit {
            burst: 5,
            replenish_period: Duration::from_secs(10),
        })
        .is_ok());
    }

    #[test]
    fn ensure_mutable_rejects_section_and_field_shortcuts() {
        let mut section_immutable = HashSet::new();
        section_immutable.insert("app_profile".to_string());
        assert!(matches!(
            ensure_mutable(&section_immutable, "app_profile", "bind_addr"),
            Err(ConfigError::ImmutableField { .. })
        ));

        let mut field_immutable = HashSet::new();
        field_immutable.insert("http_port".to_string());
        assert!(matches!(
            ensure_mutable(&field_immutable, "app_profile", "http_port"),
            Err(ConfigError::ImmutableField { .. })
        ));
    }

    #[test]
    fn parse_cidr_entry_rejects_non_numeric_and_missing_components() {
        assert!(matches!(
            parse_cidr_entry("10.0.0.0/not-a-number", "app_profile", "local_networks"),
            Err(ConfigError::InvalidField { reason, .. }) if reason == "CIDR prefix must be numeric"
        ));
        assert!(matches!(
            parse_cidr_entry("/24", "app_profile", "local_networks"),
            Err(ConfigError::InvalidField { reason, .. }) if reason == "CIDR entries must include a network and prefix"
        ));
        assert!(matches!(
            parse_cidr_entry("10.0.0.0/", "app_profile", "local_networks"),
            Err(ConfigError::InvalidField { reason, .. }) if reason == "CIDR entries must include a network and prefix"
        ));
    }
}
