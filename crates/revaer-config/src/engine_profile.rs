//! Engine profile validation and normalization helpers shared across the API and runtime paths.
//!
//! # Design
//! - Applies immutable-key checks and type validation for engine profile patches.
//! - Normalises persisted values to safe defaults before they are stored or forwarded to runtime.
//! - Surfaces an "effective" view with clamped values plus guard-rail warnings for observability.

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::model::{EngineProfile, Toggle};
use crate::validate::{ConfigError, ensure_mutable, parse_port};

/// Upper bound guard rail for rate limits (≈5 Gbps).
pub const MAX_RATE_LIMIT_BPS: i64 = 5_000_000_000;
const DEFAULT_DOWNLOAD_ROOT: &str = "/data/staging";
const DEFAULT_RESUME_DIR: &str = "/var/lib/revaer/state";
const MAX_INLINE_IP_FILTER_ENTRIES: usize = 5_000;

/// Effective engine configuration after applying guard rails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineProfileEffective {
    /// Engine implementation identifier.
    pub implementation: String,
    /// Network-facing options (listen, DHT, encryption).
    pub network: EngineNetworkConfig,
    /// Throughput and concurrency limits.
    pub limits: EngineLimitsConfig,
    /// Storage paths used by the engine.
    pub storage: EngineStorageConfig,
    /// Behavioural toggles that affect per-torrent defaults.
    pub behavior: EngineBehaviorConfig,
    /// Tracker configuration payload (validated to an object).
    pub tracker: Value,
    /// Guard-rail or normalisation warnings applied to the profile.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Network-centric knobs for the engine session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineNetworkConfig {
    /// Optional listen port override (None when clamped/disabled).
    pub listen_port: Option<i32>,
    /// Explicit listen interfaces (host/device/IP + port).
    #[serde(default)]
    pub listen_interfaces: Vec<String>,
    /// IPv6 preference for listening and outbound behaviour.
    pub ipv6_mode: EngineIpv6Mode,
    /// Whether DHT is enabled for peer discovery.
    pub enable_dht: bool,
    /// DHT bootstrap nodes (host:port entries).
    pub dht_bootstrap_nodes: Vec<String>,
    /// DHT router endpoints (host:port entries).
    pub dht_router_nodes: Vec<String>,
    /// Encryption policy applied to inbound/outbound peers.
    pub encryption: EngineEncryptionPolicy,
    /// Whether local service discovery is enabled.
    pub enable_lsd: Toggle,
    /// Whether `UPnP` is enabled.
    pub enable_upnp: Toggle,
    /// Whether NAT-PMP is enabled.
    pub enable_natpmp: Toggle,
    /// Whether peer exchange (PEX) is enabled.
    pub enable_pex: Toggle,
    /// IP filter and blocklist configuration.
    pub ip_filter: IpFilterConfig,
}

/// IPv6 preference policy applied to engine networking.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum EngineIpv6Mode {
    /// Disable IPv6 listeners and prefer IPv4.
    #[default]
    Disabled,
    /// Enable IPv6 alongside IPv4.
    Enabled,
    /// Prefer IPv6 addresses while keeping IPv4 listeners.
    PreferV6,
}

/// Canonical IP filter configuration (inline + optional blocklist URL/metadata).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct IpFilterConfig {
    /// Canonical CIDR entries to block.
    #[serde(default)]
    pub cidrs: Vec<String>,
    /// Optional remote blocklist URL to download and cache.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocklist_url: Option<String>,
    /// Optional `ETag` returned by the last successful blocklist fetch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// Timestamp of the last successful blocklist refresh.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated_at: Option<DateTime<Utc>>,
    /// Last error encountered when refreshing the blocklist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl IpFilterConfig {
    /// Convert to a JSON value for persistence.
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!(self)
    }

    /// Convert canonical CIDR strings into address ranges.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if canonical entries unexpectedly fail to parse.
    pub fn rules(&self) -> Result<Vec<IpFilterRule>, ConfigError> {
        self.cidrs
            .iter()
            .map(|cidr| canonicalize_ip_filter_entry(cidr, "ip_filter.cidrs").map(|(_, rule)| rule))
            .collect()
    }
}

/// Inclusive IP range used for native session filters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpFilterRule {
    /// Start address of the blocked range.
    pub start: IpAddr,
    /// End address of the blocked range.
    pub end: IpAddr,
}

/// Throughput and concurrency limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineLimitsConfig {
    /// Optional maximum number of active torrents.
    pub max_active: Option<i32>,
    /// Optional global download cap in bytes per second.
    pub download_rate_limit: Option<i64>,
    /// Optional global upload cap in bytes per second.
    pub upload_rate_limit: Option<i64>,
}

/// Storage paths for active downloads and resume data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStorageConfig {
    /// Root directory for active downloads.
    pub download_root: String,
    /// Directory for fast-resume payloads.
    pub resume_dir: String,
}

/// Behavioural defaults applied when admitting torrents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineBehaviorConfig {
    /// Whether sequential mode is the default.
    pub sequential_default: bool,
}

/// Canonical encryption policies accepted by the engine.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EngineEncryptionPolicy {
    /// Enforce encrypted peers exclusively.
    Require,
    /// Prefer encryption but allow plaintext peers.
    Prefer,
    /// Disable encrypted peers entirely.
    Disable,
}

impl EngineEncryptionPolicy {
    #[must_use]
    /// Render the policy as its canonical string representation.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Require => "require",
            Self::Prefer => "prefer",
            Self::Disable => "disable",
        }
    }
}

/// Outcome of validating and normalising an engine profile patch.
#[derive(Debug, Clone)]
pub struct EngineProfileMutation {
    /// Sanitised profile ready for persistence.
    pub stored: EngineProfile,
    /// Effective profile after clamping/normalisation.
    pub effective: EngineProfileEffective,
    /// Whether any fields changed after validation.
    pub mutated: bool,
}

/// Proxy kinds supported for tracker announces.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrackerProxyType {
    /// HTTP proxy.
    #[default]
    Http,
    /// HTTPS proxy.
    Https,
    /// SOCKS5 proxy.
    Socks5,
}

/// Proxy configuration used when announcing to trackers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TrackerProxyConfig {
    /// Proxy host or IP.
    pub host: String,
    /// Proxy port.
    pub port: u16,
    /// Optional username secret reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username_secret: Option<String>,
    /// Optional password secret reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_secret: Option<String>,
    /// Proxy type.
    #[serde(default)]
    pub kind: TrackerProxyType,
    /// Whether peer connections should also use the proxy.
    #[serde(default)]
    pub proxy_peers: bool,
}

/// Normalised tracker configuration derived from the persisted payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TrackerConfig {
    /// Default tracker list applied to all torrents.
    #[serde(default)]
    pub default: Vec<String>,
    /// Extra trackers appended to the defaults.
    #[serde(default)]
    pub extra: Vec<String>,
    /// Whether to replace defaults with request-provided trackers.
    #[serde(default)]
    pub replace: bool,
    /// Optional custom user-agent to send to trackers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// Optional announce IP override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub announce_ip: Option<String>,
    /// Optional listen interface override for tracker announces.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen_interface: Option<String>,
    /// Optional request timeout in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_timeout_ms: Option<i64>,
    /// Whether to announce to all trackers.
    #[serde(default)]
    pub announce_to_all: bool,
    /// Optional proxy configuration for tracker announces.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<TrackerProxyConfig>,
}

impl TrackerConfig {
    /// Convert the typed config into a JSON object for persistence/effective views.
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!(self)
    }
}

/// Validate and normalise an engine profile patch against the current snapshot.
///
/// Returns a ready-to-persist profile alongside its effective view and whether the
/// patch produced a change.
///
/// # Errors
///
/// Returns `ConfigError` when the patch contains unknown fields, immutable fields,
/// or type violations.
pub(crate) fn validate_engine_profile_patch(
    current: &EngineProfile,
    patch: &Value,
    immutable_keys: &HashSet<String>,
) -> Result<EngineProfileMutation, ConfigError> {
    let Some(map) = patch.as_object() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "<root>".to_string(),
            message: "changeset must be a JSON object".to_string(),
        });
    };

    let mut working = current.clone();
    let mut touched = false;
    let mut warnings = Vec::new();

    for (key, value) in map {
        ensure_mutable(immutable_keys, "engine_profile", key)?;
        touched |= apply_field(&mut working, key, value, &mut warnings)?;
    }

    let effective = normalize_engine_profile_with_warnings(&working, warnings);
    let stored = EngineProfile {
        id: working.id,
        implementation: working.implementation,
        listen_port: effective.network.listen_port,
        listen_interfaces: effective.network.listen_interfaces.clone(),
        ipv6_mode: ipv6_mode_label(effective.network.ipv6_mode).to_string(),
        dht: effective.network.enable_dht,
        encryption: effective.network.encryption.as_str().to_string(),
        max_active: effective.limits.max_active,
        max_download_bps: effective.limits.download_rate_limit,
        max_upload_bps: effective.limits.upload_rate_limit,
        sequential_default: effective.behavior.sequential_default,
        resume_dir: effective.storage.resume_dir.clone(),
        download_root: effective.storage.download_root.clone(),
        tracker: effective.tracker.clone(),
        enable_lsd: effective.network.enable_lsd,
        enable_upnp: effective.network.enable_upnp,
        enable_natpmp: effective.network.enable_natpmp,
        enable_pex: effective.network.enable_pex,
        dht_bootstrap_nodes: effective.network.dht_bootstrap_nodes.clone(),
        dht_router_nodes: effective.network.dht_router_nodes.clone(),
        ip_filter: effective.network.ip_filter.to_value(),
    };
    let mutated = touched || stored != *current;

    Ok(EngineProfileMutation {
        stored,
        effective,
        mutated,
    })
}

/// Produce the effective engine configuration for inspection and runtime application.
#[must_use]
pub fn normalize_engine_profile(profile: &EngineProfile) -> EngineProfileEffective {
    normalize_engine_profile_with_warnings(profile, Vec::new())
}

fn normalize_engine_profile_with_warnings(
    profile: &EngineProfile,
    mut warnings: Vec<String>,
) -> EngineProfileEffective {
    let listen_port = match profile.listen_port {
        Some(port) if (1..=65_535).contains(&port) => Some(port),
        Some(port) => {
            warnings.push(format!(
                "listen_port {port} is out of range; disabling listen override"
            ));
            None
        }
        None => None,
    };
    let listen_interfaces = sanitize_listen_interfaces(
        &profile.listen_interfaces,
        "listen_interfaces",
        &mut warnings,
    );
    let ipv6_mode = canonicalize_ipv6_mode(&profile.ipv6_mode, &mut warnings);

    let max_active = match profile.max_active {
        Some(value) if value > 0 => Some(value),
        Some(_) => {
            warnings.push("max_active <= 0 requested; leaving unlimited".to_string());
            None
        }
        None => None,
    };

    let download_rate_limit =
        clamp_rate_limit("max_download_bps", profile.max_download_bps, &mut warnings);
    let upload_rate_limit =
        clamp_rate_limit("max_upload_bps", profile.max_upload_bps, &mut warnings);

    let encryption = canonical_encryption(&profile.encryption, &mut warnings);

    let download_root = sanitize_path(
        &profile.download_root,
        DEFAULT_DOWNLOAD_ROOT,
        "download_root",
        &mut warnings,
    );
    let resume_dir = sanitize_path(
        &profile.resume_dir,
        DEFAULT_RESUME_DIR,
        "resume_dir",
        &mut warnings,
    );
    let tracker = sanitize_tracker(&profile.tracker, &mut warnings);
    let dht_bootstrap_nodes = sanitize_endpoints(
        &profile.dht_bootstrap_nodes,
        "dht_bootstrap_nodes",
        &mut warnings,
    );
    let dht_router_nodes =
        sanitize_endpoints(&profile.dht_router_nodes, "dht_router_nodes", &mut warnings);
    let ip_filter = sanitize_ip_filter(&profile.ip_filter, &mut warnings);

    EngineProfileEffective {
        implementation: profile.implementation.clone(),
        network: EngineNetworkConfig {
            listen_port,
            listen_interfaces,
            ipv6_mode,
            enable_dht: profile.dht,
            dht_bootstrap_nodes,
            dht_router_nodes,
            encryption,
            enable_lsd: profile.enable_lsd,
            enable_upnp: profile.enable_upnp,
            enable_natpmp: profile.enable_natpmp,
            enable_pex: profile.enable_pex,
            ip_filter,
        },
        limits: EngineLimitsConfig {
            max_active,
            download_rate_limit,
            upload_rate_limit,
        },
        storage: EngineStorageConfig {
            download_root,
            resume_dir,
        },
        behavior: EngineBehaviorConfig {
            sequential_default: profile.sequential_default,
        },
        tracker,
        warnings,
    }
}

fn apply_field(
    working: &mut EngineProfile,
    key: &str,
    value: &Value,
    warnings: &mut Vec<String>,
) -> Result<bool, ConfigError> {
    match key {
        "implementation" => Ok(assign_if_changed(
            &mut working.implementation,
            required_string(value, "implementation")?,
        )),
        "listen_port" => Ok(assign_if_changed(
            &mut working.listen_port,
            parse_optional_port(value)?,
        )),
        "listen_interfaces" => Ok(assign_if_changed(
            &mut working.listen_interfaces,
            parse_listen_interfaces(value)?,
        )),
        "ipv6_mode" => Ok(assign_if_changed(
            &mut working.ipv6_mode,
            parse_ipv6_mode(value)?,
        )),
        "dht" => Ok(assign_if_changed(
            &mut working.dht,
            required_bool(value, "dht")?,
        )),
        "encryption" => {
            let raw = required_string(value, "encryption")?;
            let policy = canonical_encryption(&raw, warnings);
            Ok(assign_if_changed(
                &mut working.encryption,
                policy.as_str().to_string(),
            ))
        }
        "max_active" => Ok(assign_if_changed(
            &mut working.max_active,
            parse_optional_i32(value, "max_active")?,
        )),
        "max_download_bps" => apply_rate_limit_field(
            working,
            value,
            "max_download_bps",
            |profile| profile.max_download_bps,
            |profile, limit| {
                profile.max_download_bps = limit;
            },
        ),
        "max_upload_bps" => apply_rate_limit_field(
            working,
            value,
            "max_upload_bps",
            |profile| profile.max_upload_bps,
            |profile, limit| {
                profile.max_upload_bps = limit;
            },
        ),
        "sequential_default" => Ok(assign_if_changed(
            &mut working.sequential_default,
            required_bool(value, "sequential_default")?,
        )),
        "resume_dir" => Ok(assign_if_changed(
            &mut working.resume_dir,
            required_string(value, "resume_dir")?,
        )),
        "download_root" => Ok(assign_if_changed(
            &mut working.download_root,
            required_string(value, "download_root")?,
        )),
        "enable_lsd" => Ok(assign_if_changed(
            &mut working.enable_lsd,
            Toggle::from(required_bool(value, "enable_lsd")?),
        )),
        "enable_upnp" => Ok(assign_if_changed(
            &mut working.enable_upnp,
            Toggle::from(required_bool(value, "enable_upnp")?),
        )),
        "enable_natpmp" => Ok(assign_if_changed(
            &mut working.enable_natpmp,
            Toggle::from(required_bool(value, "enable_natpmp")?),
        )),
        "enable_pex" => Ok(assign_if_changed(
            &mut working.enable_pex,
            Toggle::from(required_bool(value, "enable_pex")?),
        )),
        "dht_bootstrap_nodes" => Ok(assign_if_changed(
            &mut working.dht_bootstrap_nodes,
            parse_endpoint_array(value, "dht_bootstrap_nodes")?,
        )),
        "dht_router_nodes" => Ok(assign_if_changed(
            &mut working.dht_router_nodes,
            parse_endpoint_array(value, "dht_router_nodes")?,
        )),
        "ip_filter" => apply_ip_filter_field(working, value),
        "tracker" => {
            let normalized = normalize_tracker_payload(value)?;
            Ok(assign_if_changed(&mut working.tracker, normalized))
        }
        other => Err(ConfigError::UnknownField {
            section: "engine_profile".to_string(),
            field: other.to_string(),
        }),
    }
}

fn apply_ip_filter_field(working: &mut EngineProfile, value: &Value) -> Result<bool, ConfigError> {
    let (updated_at_specified, etag_specified, error_specified) =
        value.as_object().map_or((false, false, false), |map| {
            (
                map.contains_key("last_updated_at"),
                map.contains_key("etag"),
                map.contains_key("last_error"),
            )
        });
    let mut next = normalize_ip_filter_payload(value)?;
    let previous = decode_ip_filter(&working.ip_filter);
    if next.cidrs != previous.cidrs || next.blocklist_url != previous.blocklist_url {
        next.last_updated_at = None;
        next.etag = None;
        next.last_error = None;
    } else {
        if next.last_updated_at.is_none() && !updated_at_specified {
            next.last_updated_at = previous.last_updated_at;
        }
        if next.etag.is_none() && !etag_specified {
            next.etag = previous.etag;
        }
        if next.last_error.is_none() && !error_specified {
            next.last_error = previous.last_error;
        }
    }
    let normalized = next.to_value();
    Ok(assign_if_changed(&mut working.ip_filter, normalized))
}

fn required_string(value: &Value, field: &str) -> Result<String, ConfigError> {
    let Some(text) = value.as_str() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be a string".to_string(),
        });
    };
    Ok(text.to_string())
}

fn required_bool(value: &Value, field: &str) -> Result<bool, ConfigError> {
    let Some(flag) = value.as_bool() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be a boolean".to_string(),
        });
    };
    Ok(flag)
}

fn parse_optional_port(value: &Value) -> Result<Option<i32>, ConfigError> {
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(parse_port(value, "engine_profile", "listen_port")?))
    }
}

fn parse_optional_i32(value: &Value, field: &str) -> Result<Option<i32>, ConfigError> {
    if value.is_null() {
        return Ok(None);
    }
    let Some(raw_value) = value.as_i64() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be an integer".to_string(),
        });
    };
    if !(0..=i64::from(i32::MAX)).contains(&raw_value) {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be within 0..=i32::MAX".to_string(),
        });
    }
    let value_i32 = i32::try_from(raw_value).map_err(|_| ConfigError::InvalidField {
        section: "engine_profile".to_string(),
        field: field.to_string(),
        message: "must fit within 32-bit signed integer range".to_string(),
    })?;
    Ok(Some(value_i32))
}

fn parse_endpoint_array(value: &Value, field: &str) -> Result<Vec<String>, ConfigError> {
    let Some(array) = value.as_array() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be an array of host:port entries".to_string(),
        });
    };

    let mut seen = HashSet::new();
    let mut endpoints = Vec::new();
    for entry in array {
        let Some(text) = entry.as_str() else {
            return Err(ConfigError::InvalidField {
                section: "engine_profile".to_string(),
                field: field.to_string(),
                message: "entries must be strings".to_string(),
            });
        };
        let normalized =
            normalize_endpoint(text, field).map_err(|message| ConfigError::InvalidField {
                section: "engine_profile".to_string(),
                field: field.to_string(),
                message,
            })?;
        let key = normalized.to_ascii_lowercase();
        if seen.insert(key) {
            endpoints.push(normalized);
        }
    }

    Ok(endpoints)
}

fn parse_listen_interfaces(value: &Value) -> Result<Vec<String>, ConfigError> {
    let Some(array) = value.as_array() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "listen_interfaces".to_string(),
            message: "must be an array of host:port entries".to_string(),
        });
    };

    let mut seen = HashSet::new();
    let mut interfaces = Vec::new();
    for entry in array {
        let Some(text) = entry.as_str() else {
            return Err(ConfigError::InvalidField {
                section: "engine_profile".to_string(),
                field: "listen_interfaces".to_string(),
                message: "entries must be strings".to_string(),
            });
        };
        let normalized = canonicalize_listen_interface(text, "listen_interfaces")?;
        let key = normalized.to_ascii_lowercase();
        if seen.insert(key) {
            interfaces.push(normalized);
        }
    }
    Ok(interfaces)
}

fn canonicalize_listen_interface(entry: &str, field: &str) -> Result<String, ConfigError> {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "entries cannot be empty".to_string(),
        });
    }
    if trimmed.contains(char::is_whitespace) {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "entries cannot contain whitespace".to_string(),
        });
    }

    if let Some(stripped) = trimmed.strip_prefix('[') {
        let Some(closing) = stripped.find(']') else {
            return Err(ConfigError::InvalidField {
                section: "engine_profile".to_string(),
                field: field.to_string(),
                message: "IPv6 entries must be bracketed like [::1]:6881".to_string(),
            });
        };
        let host = stripped[..closing].trim();
        let remainder = stripped.get(closing + 1..).unwrap_or("").trim();
        if host.is_empty() || !remainder.starts_with(':') {
            return Err(ConfigError::InvalidField {
                section: "engine_profile".to_string(),
                field: field.to_string(),
                message: "IPv6 entries must be formatted as [addr]:port".to_string(),
            });
        }
        let port_text = remainder.trim_start_matches(':').trim();
        let port = port_text
            .parse::<i64>()
            .map_err(|_| ConfigError::InvalidField {
                section: "engine_profile".to_string(),
                field: field.to_string(),
                message: "port must be an integer between 1 and 65535".to_string(),
            })?;
        let _ = parse_port(&json!(port), "engine_profile", field)?;
        return Ok(format!("[{host}]:{port}"));
    }

    let Some((host, port_text)) = trimmed.rsplit_once(':') else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "entries must be host:port or [ipv6]:port".to_string(),
        });
    };
    let host = host.trim();
    if host.is_empty() {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "host component cannot be empty".to_string(),
        });
    }
    let port = port_text
        .parse::<i64>()
        .map_err(|_| ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "port must be an integer between 1 and 65535".to_string(),
        })?;
    let _ = parse_port(&json!(port), "engine_profile", field)?;
    Ok(format!("{host}:{port}"))
}

fn parse_ipv6_mode(value: &Value) -> Result<String, ConfigError> {
    let Some(text) = value.as_str() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "ipv6_mode".to_string(),
            message: "must be a string".to_string(),
        });
    };
    let normalized = text.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "" | "disabled" | "disable" | "off" => Ok("disabled".to_string()),
        "enabled" | "enable" | "on" | "v6" | "ipv6" => Ok("enabled".to_string()),
        "prefer_v6" | "prefer-v6" | "prefer6" | "prefer" => Ok("prefer_v6".to_string()),
        other => Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "ipv6_mode".to_string(),
            message: format!("must be one of disabled, enabled, or prefer_v6 (got {other})"),
        }),
    }
}

fn canonicalize_ipv6_mode(value: &str, warnings: &mut Vec<String>) -> EngineIpv6Mode {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "disabled" | "disable" | "off" => EngineIpv6Mode::Disabled,
        "enabled" | "enable" | "on" | "v6" | "ipv6" => EngineIpv6Mode::Enabled,
        "prefer_v6" | "prefer-v6" | "prefer6" | "prefer" => EngineIpv6Mode::PreferV6,
        other => {
            warnings.push(format!(
                "ipv6_mode '{other}' is invalid; defaulting to disabled"
            ));
            EngineIpv6Mode::Disabled
        }
    }
}

const fn ipv6_mode_label(mode: EngineIpv6Mode) -> &'static str {
    match mode {
        EngineIpv6Mode::Disabled => "disabled",
        EngineIpv6Mode::Enabled => "enabled",
        EngineIpv6Mode::PreferV6 => "prefer_v6",
    }
}
fn sanitize_endpoints(values: &[String], field: &str, warnings: &mut Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut endpoints = Vec::new();

    for value in values {
        match normalize_endpoint(value, field) {
            Ok(normalized) => {
                let key = normalized.to_ascii_lowercase();
                if seen.insert(key) {
                    endpoints.push(normalized);
                }
            }
            Err(message) => warnings.push(message),
        }
    }

    endpoints
}

fn sanitize_listen_interfaces(
    values: &[String],
    field: &str,
    warnings: &mut Vec<String>,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut interfaces = Vec::new();

    for value in values {
        match canonicalize_listen_interface(value, field) {
            Ok(normalized) => {
                let key = normalized.to_ascii_lowercase();
                if seen.insert(key) {
                    interfaces.push(normalized);
                }
            }
            Err(err) => warnings.push(err.to_string()),
        }
    }

    interfaces
}

fn sanitize_ip_filter(value: &Value, warnings: &mut Vec<String>) -> IpFilterConfig {
    match normalize_ip_filter_payload(value) {
        Ok(config) => config,
        Err(err) => {
            warnings.push(format!(
                "ip_filter payload invalid ({err}); replacing with {{}}"
            ));
            IpFilterConfig::default()
        }
    }
}

fn decode_ip_filter(value: &Value) -> IpFilterConfig {
    normalize_ip_filter_payload(value).unwrap_or_default()
}

fn normalize_ip_filter_payload(value: &Value) -> Result<IpFilterConfig, ConfigError> {
    if value.is_null() {
        return Ok(IpFilterConfig::default());
    }
    let map = value.as_object().ok_or_else(|| ConfigError::InvalidField {
        section: "engine_profile".to_string(),
        field: "ip_filter".to_string(),
        message: "must be an object".to_string(),
    })?;

    for key in map.keys() {
        match key.as_str() {
            "cidrs" | "blocklist_url" | "etag" | "last_updated_at" | "last_error" => {}
            other => {
                return Err(ConfigError::UnknownField {
                    section: "engine_profile".to_string(),
                    field: format!("ip_filter.{other}"),
                });
            }
        }
    }

    let cidrs = parse_ip_filter_cidrs(map.get("cidrs"))?;
    let blocklist_url = parse_blocklist_url(map.get("blocklist_url"))?;
    let etag = parse_optional_short_string(map.get("etag"), "ip_filter.etag", 512)?;
    let last_updated_at =
        parse_optional_timestamp(map.get("last_updated_at"), "ip_filter.last_updated_at")?;
    let last_error =
        parse_optional_short_string(map.get("last_error"), "ip_filter.last_error", 512)?;

    Ok(IpFilterConfig {
        cidrs,
        blocklist_url,
        etag,
        last_updated_at,
        last_error,
    })
}

fn parse_ip_filter_cidrs(value: Option<&Value>) -> Result<Vec<String>, ConfigError> {
    let Some(raw) = value else {
        return Ok(Vec::new());
    };
    if raw.is_null() {
        return Ok(Vec::new());
    }
    let entries = raw.as_array().ok_or_else(|| ConfigError::InvalidField {
        section: "engine_profile".to_string(),
        field: "ip_filter.cidrs".to_string(),
        message: "must be an array of CIDR strings".to_string(),
    })?;
    if entries.len() > MAX_INLINE_IP_FILTER_ENTRIES {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "ip_filter.cidrs".to_string(),
            message: format!("must contain at most {MAX_INLINE_IP_FILTER_ENTRIES} entries"),
        });
    }

    let mut cidrs = Vec::new();
    let mut seen = HashSet::new();
    for entry in entries {
        let Some(text) = entry.as_str() else {
            return Err(ConfigError::InvalidField {
                section: "engine_profile".to_string(),
                field: "ip_filter.cidrs".to_string(),
                message: "entries must be strings".to_string(),
            });
        };
        let (canonical, _) = canonicalize_ip_filter_entry(text, "ip_filter.cidrs")?;
        if seen.insert(canonical.clone()) {
            cidrs.push(canonical);
        }
    }
    Ok(cidrs)
}

fn parse_blocklist_url(value: Option<&Value>) -> Result<Option<String>, ConfigError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    let Some(url) = raw.as_str() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "ip_filter.blocklist_url".to_string(),
            message: "must be a string".to_string(),
        });
    };
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > 2_048 {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "ip_filter.blocklist_url".to_string(),
            message: "must be shorter than 2048 characters".to_string(),
        });
    }
    let lowered = trimmed.to_ascii_lowercase();
    if !(lowered.starts_with("http://") || lowered.starts_with("https://")) {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "ip_filter.blocklist_url".to_string(),
            message: "must start with http:// or https://".to_string(),
        });
    }
    Ok(Some(trimmed.to_string()))
}

fn parse_optional_short_string(
    value: Option<&Value>,
    field: &str,
    max_len: usize,
) -> Result<Option<String>, ConfigError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    let Some(text) = raw.as_str() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be a string".to_string(),
        });
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > max_len {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: format!("must be at most {max_len} characters"),
        });
    }
    Ok(Some(trimmed.to_string()))
}

fn parse_optional_timestamp(
    value: Option<&Value>,
    field: &str,
) -> Result<Option<DateTime<Utc>>, ConfigError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    let Some(text) = raw.as_str() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be an RFC3339 timestamp string".to_string(),
        });
    };
    let parsed = DateTime::parse_from_rfc3339(text)
        .map_err(|_| ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be an RFC3339 timestamp string".to_string(),
        })?
        .with_timezone(&Utc);
    Ok(Some(parsed))
}

/// Canonicalize a CIDR or IP entry into a normalized string and address range.
///
/// # Errors
///
/// Returns `ConfigError` when the entry is empty or not a valid IPv4/IPv6
/// address with an in-bounds prefix length.
pub fn canonicalize_ip_filter_entry(
    entry: &str,
    field: &str,
) -> Result<(String, IpFilterRule), ConfigError> {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "entries cannot be empty".to_string(),
        });
    }
    let (addr_str, prefix) = match trimmed.split_once('/') {
        Some((addr, prefix)) => (addr.trim(), Some(prefix.trim())),
        None => (trimmed, None),
    };
    if addr_str.is_empty() {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "address component cannot be empty".to_string(),
        });
    }
    let address = IpAddr::from_str(addr_str).map_err(|_| ConfigError::InvalidField {
        section: "engine_profile".to_string(),
        field: field.to_string(),
        message: "must contain valid IPv4 or IPv6 addresses".to_string(),
    })?;
    let max_bits = match address {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    };
    let prefix = match prefix {
        Some("") | None => max_bits,
        Some(bits) => {
            let parsed = bits.parse::<u8>().map_err(|_| ConfigError::InvalidField {
                section: "engine_profile".to_string(),
                field: field.to_string(),
                message: format!("invalid prefix length '{bits}'"),
            })?;
            if parsed > max_bits {
                return Err(ConfigError::InvalidField {
                    section: "engine_profile".to_string(),
                    field: field.to_string(),
                    message: format!("prefix must be between 0 and {max_bits}"),
                });
            }
            parsed
        }
    };

    let range = canonical_range(address, prefix, field)?;
    let canonical = format!("{}/{}", range.start, prefix);
    Ok((canonical, range))
}

fn canonical_range(address: IpAddr, prefix: u8, field: &str) -> Result<IpFilterRule, ConfigError> {
    match address {
        IpAddr::V4(addr) => {
            if prefix > 32 {
                return Err(ConfigError::InvalidField {
                    section: "engine_profile".to_string(),
                    field: field.to_string(),
                    message: "IPv4 prefix must be between 0 and 32".to_string(),
                });
            }
            let host_bits = 32 - u32::from(prefix);
            let mask = if prefix == 0 {
                0
            } else {
                u32::MAX.checked_shl(host_bits).unwrap_or(0)
            };
            let start = u32::from(addr) & mask;
            let end = if prefix == 0 {
                u32::MAX
            } else {
                start | (!mask)
            };
            Ok(IpFilterRule {
                start: IpAddr::V4(Ipv4Addr::from(start)),
                end: IpAddr::V4(Ipv4Addr::from(end)),
            })
        }
        IpAddr::V6(addr) => {
            if prefix > 128 {
                return Err(ConfigError::InvalidField {
                    section: "engine_profile".to_string(),
                    field: field.to_string(),
                    message: "IPv6 prefix must be between 0 and 128".to_string(),
                });
            }
            let host_bits = 128 - u128::from(prefix);
            let mask = if prefix == 0 {
                0
            } else {
                u128::MAX
                    .checked_shl(u32::try_from(host_bits).unwrap_or(0))
                    .unwrap_or(0)
            };
            let start = u128::from(addr) & mask;
            let end = if prefix == 0 {
                u128::MAX
            } else {
                start | (!mask)
            };
            Ok(IpFilterRule {
                start: IpAddr::V6(Ipv6Addr::from(start)),
                end: IpAddr::V6(Ipv6Addr::from(end)),
            })
        }
    }
}

fn normalize_endpoint(value: &str, field: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} entries cannot be empty"));
    }

    let Some((host, port_str)) = trimmed.rsplit_once(':') else {
        return Err(format!("{field} entries must be host:port"));
    };
    if host.trim().is_empty() {
        return Err(format!("{field} host component cannot be empty"));
    }
    let port: i64 = port_str
        .trim()
        .parse()
        .map_err(|_| format!("{field} port must be an integer between 1 and 65535"))?;
    // Reuse the port parser for bounds checking.
    let _ = parse_port(&json!(port), "engine_profile", field).map_err(|err| err.to_string())?;

    Ok(format!("{}:{}", host.trim(), port))
}

fn normalize_tracker_payload(value: &Value) -> Result<Value, ConfigError> {
    if value.is_null() {
        return Ok(TrackerConfig::default().to_value());
    }
    let map = value.as_object().ok_or_else(|| ConfigError::InvalidField {
        section: "engine_profile".to_string(),
        field: "tracker".to_string(),
        message: "must be an object".to_string(),
    })?;

    for key in map.keys() {
        match key.as_str() {
            "default" | "extra" | "replace" | "user_agent" | "announce_ip" | "listen_interface"
            | "request_timeout_ms" | "announce_to_all" | "proxy" => {}
            other => {
                return Err(ConfigError::UnknownField {
                    section: "engine_profile".to_string(),
                    field: format!("tracker.{other}"),
                });
            }
        }
    }

    let default = parse_tracker_list(map.get("default"), "tracker.default")?;
    let extra = parse_tracker_list(map.get("extra"), "tracker.extra")?;
    let replace = parse_optional_bool(map.get("replace"), "tracker.replace")?.unwrap_or(false);
    let announce_to_all =
        parse_optional_bool(map.get("announce_to_all"), "tracker.announce_to_all")?
            .unwrap_or(false);
    let user_agent = parse_optional_string(map.get("user_agent"), "tracker.user_agent")?;
    let announce_ip = parse_optional_string(map.get("announce_ip"), "tracker.announce_ip")?;
    let listen_interface =
        parse_optional_string(map.get("listen_interface"), "tracker.listen_interface")?;
    let request_timeout_ms =
        parse_optional_timeout(map.get("request_timeout_ms"), "tracker.request_timeout_ms")?;
    let proxy = parse_proxy(map.get("proxy"))?;

    let config = TrackerConfig {
        default,
        extra,
        replace,
        user_agent,
        announce_ip,
        listen_interface,
        request_timeout_ms,
        announce_to_all,
        proxy,
    };

    Ok(config.to_value())
}

fn parse_optional_timeout(value: Option<&Value>, field: &str) -> Result<Option<i64>, ConfigError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    let Some(timeout) = raw.as_i64() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be an integer (milliseconds)".to_string(),
        });
    };
    if !(0..=900_000).contains(&timeout) {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be between 0 and 900000 milliseconds".to_string(),
        });
    }
    Ok(Some(timeout))
}

fn parse_optional_bool(value: Option<&Value>, field: &str) -> Result<Option<bool>, ConfigError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    raw.as_bool()
        .ok_or_else(|| ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be a boolean".to_string(),
        })
        .map(Some)
}

fn parse_optional_string(
    value: Option<&Value>,
    field: &str,
) -> Result<Option<String>, ConfigError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    let Some(text) = raw.as_str() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be a string".to_string(),
        });
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > 255 {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be at most 255 characters".to_string(),
        });
    }
    Ok(Some(trimmed.to_string()))
}

fn parse_tracker_list(raw: Option<&Value>, field: &str) -> Result<Vec<String>, ConfigError> {
    let Some(value) = raw else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let Some(items) = value.as_array() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be an array of strings".to_string(),
        });
    };
    let mut seen = HashSet::new();
    let mut trackers = Vec::new();
    for item in items {
        let Some(text) = item.as_str() else {
            return Err(ConfigError::InvalidField {
                section: "engine_profile".to_string(),
                field: field.to_string(),
                message: "entries must be strings".to_string(),
            });
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.len() > 512 {
            return Err(ConfigError::InvalidField {
                section: "engine_profile".to_string(),
                field: field.to_string(),
                message: "entries must be shorter than 512 characters".to_string(),
            });
        }
        if seen.insert(trimmed.to_ascii_lowercase()) {
            trackers.push(trimmed.to_string());
        }
    }
    Ok(trackers)
}

fn parse_proxy(value: Option<&Value>) -> Result<Option<TrackerProxyConfig>, ConfigError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    let Some(map) = raw.as_object() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "tracker.proxy".to_string(),
            message: "must be an object".to_string(),
        });
    };

    let host = parse_optional_string(map.get("host"), "tracker.proxy.host")?.ok_or_else(|| {
        ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "tracker.proxy.host".to_string(),
            message: "is required when proxy is set".to_string(),
        }
    })?;
    let port =
        map.get("port")
            .and_then(Value::as_i64)
            .ok_or_else(|| ConfigError::InvalidField {
                section: "engine_profile".to_string(),
                field: "tracker.proxy.port".to_string(),
                message: "is required and must be an integer".to_string(),
            })?;
    if !(1..=65_535).contains(&port) {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "tracker.proxy.port".to_string(),
            message: "must be between 1 and 65535".to_string(),
        });
    }

    let kind = map
        .get("kind")
        .and_then(Value::as_str)
        .map(|value| match value {
            "http" => Ok(TrackerProxyType::Http),
            "https" => Ok(TrackerProxyType::Https),
            "socks5" => Ok(TrackerProxyType::Socks5),
            other => Err(ConfigError::InvalidField {
                section: "engine_profile".to_string(),
                field: "tracker.proxy.kind".to_string(),
                message: format!("unsupported proxy kind '{other}'"),
            }),
        })
        .transpose()?
        .unwrap_or_default();
    let username_secret =
        parse_optional_string(map.get("username_secret"), "tracker.proxy.username_secret")?;
    let password_secret =
        parse_optional_string(map.get("password_secret"), "tracker.proxy.password_secret")?;
    let proxy_peers =
        parse_optional_bool(map.get("proxy_peers"), "tracker.proxy.proxy_peers")?.unwrap_or(false);

    Ok(Some(TrackerProxyConfig {
        host,
        port: u16::try_from(port).unwrap_or_default(),
        username_secret,
        password_secret,
        kind,
        proxy_peers,
    }))
}

fn apply_rate_limit_field(
    working: &mut EngineProfile,
    value: &Value,
    field: &str,
    getter: impl Fn(&EngineProfile) -> Option<i64>,
    setter: impl Fn(&mut EngineProfile, Option<i64>),
) -> Result<bool, ConfigError> {
    if value.is_null() {
        let before = getter(working);
        setter(working, None);
        return Ok(before.is_some());
    }
    let Some(limit) = value.as_i64() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be an integer".to_string(),
        });
    };
    if limit < 0 {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be non-negative".to_string(),
        });
    }
    let before = getter(working);
    setter(working, Some(limit));
    Ok(before != Some(limit))
}

fn clamp_rate_limit(field: &str, value: Option<i64>, warnings: &mut Vec<String>) -> Option<i64> {
    match value {
        Some(limit) if limit <= 0 => {
            warnings.push(format!("{field} <= 0 requested; disabling limit"));
            None
        }
        Some(limit) if limit > MAX_RATE_LIMIT_BPS => {
            warnings.push(format!(
                "{field} of {limit} exceeds guard rail; clamping to {MAX_RATE_LIMIT_BPS}"
            ));
            Some(MAX_RATE_LIMIT_BPS)
        }
        Some(limit) => Some(limit),
        None => None,
    }
}

fn canonical_encryption(raw: &str, warnings: &mut Vec<String>) -> EngineEncryptionPolicy {
    match raw.to_ascii_lowercase().as_str() {
        "require" | "required" => EngineEncryptionPolicy::Require,
        "disable" | "disabled" => EngineEncryptionPolicy::Disable,
        "prefer" => EngineEncryptionPolicy::Prefer,
        other => {
            warnings.push(format!(
                "unknown encryption policy '{other}'; defaulting to 'prefer'"
            ));
            EngineEncryptionPolicy::Prefer
        }
    }
}

fn sanitize_path(value: &str, fallback: &str, field: &str, warnings: &mut Vec<String>) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        warnings.push(format!("{field} was empty; using {fallback}"));
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn sanitize_tracker(value: &Value, warnings: &mut Vec<String>) -> Value {
    match normalize_tracker_payload(value) {
        Ok(normalized) => normalized,
        Err(err) => {
            warnings.push(format!(
                "tracker payload invalid ({err}); replacing with {{}}"
            ));
            Value::Object(Map::new())
        }
    }
}

fn assign_if_changed<T>(target: &mut T, value: T) -> bool
where
    T: PartialEq + Clone,
{
    if *target == value {
        false
    } else {
        *target = value;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashSet;
    use uuid::Uuid;

    fn sample_profile() -> EngineProfile {
        EngineProfile {
            id: Uuid::new_v4(),
            implementation: "libtorrent".into(),
            listen_port: Some(6_881),
            listen_interfaces: Vec::new(),
            ipv6_mode: "disabled".into(),
            dht: true,
            encryption: "prefer".into(),
            max_active: Some(4),
            max_download_bps: Some(250_000),
            max_upload_bps: Some(125_000),
            sequential_default: false,
            resume_dir: "/tmp/resume".into(),
            download_root: "/tmp/downloads".into(),
            tracker: json!({}),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            ip_filter: json!({}),
        }
    }

    #[test]
    fn listen_interfaces_are_canonicalized_and_deduped() {
        let profile = sample_profile();
        let patch = json!({
            "listen_interfaces": ["0.0.0.0:7000", " [::]:7000 ", "0.0.0.0:7000"]
        });
        let mutation =
            validate_engine_profile_patch(&profile, &patch, &HashSet::new()).expect("valid patch");
        assert_eq!(
            mutation.stored.listen_interfaces,
            vec!["0.0.0.0:7000".to_string(), "[::]:7000".to_string()]
        );
        assert!(mutation.effective.warnings.is_empty());

        let invalid = json!({ "listen_interfaces": ["invalid-entry"] });
        assert!(matches!(
            validate_engine_profile_patch(&profile, &invalid, &HashSet::new()),
            Err(ConfigError::InvalidField { field, .. }) if field == "listen_interfaces"
        ));
    }

    #[test]
    fn ipv6_mode_is_parsed() {
        let profile = sample_profile();
        let patch = json!({ "ipv6_mode": "prefer_v6" });
        let mutation =
            validate_engine_profile_patch(&profile, &patch, &HashSet::new()).expect("valid patch");
        assert_eq!(mutation.stored.ipv6_mode, "prefer_v6");
        assert_eq!(
            mutation.effective.network.ipv6_mode,
            EngineIpv6Mode::PreferV6
        );

        let invalid = json!({ "ipv6_mode": "bogus" });
        assert!(matches!(
            validate_engine_profile_patch(&profile, &invalid, &HashSet::new()),
            Err(ConfigError::InvalidField { field, .. }) if field == "ipv6_mode"
        ));
    }

    #[test]
    fn patch_rejects_unknown_fields() {
        let profile = sample_profile();
        let patch = json!({ "unknown": true });
        let result = validate_engine_profile_patch(&profile, &patch, &HashSet::new());
        assert!(result.is_err(), "unknown fields should be rejected");
    }

    #[test]
    fn patch_normalises_and_clamps_values() {
        let profile = sample_profile();
        let patch = json!({
            "max_download_bps": MAX_RATE_LIMIT_BPS + 1,
            "download_root": "   ",
            "resume_dir": "",
            "tracker": {}
        });
        let mutation =
            validate_engine_profile_patch(&profile, &patch, &HashSet::new()).expect("patch valid");
        assert!(mutation.mutated);
        assert_eq!(
            mutation.stored.download_root, "/data/staging",
            "empty download roots should fall back to defaults"
        );
        assert_eq!(
            mutation.effective.limits.download_rate_limit,
            Some(MAX_RATE_LIMIT_BPS),
            "rate limits should be clamped to guard rail"
        );
        assert!(
            mutation
                .effective
                .warnings
                .iter()
                .any(|msg| msg.contains("guard rail")),
            "clamping should emit guard-rail warnings"
        );
    }

    #[test]
    fn patch_updates_nat_toggles() {
        let profile = sample_profile();
        let patch = json!({
            "enable_lsd": true,
            "enable_upnp": true,
            "enable_natpmp": true,
            "enable_pex": true
        });

        let mutation =
            validate_engine_profile_patch(&profile, &patch, &HashSet::new()).expect("patch valid");
        assert!(mutation.mutated);
        assert!(mutation.stored.enable_lsd.is_enabled());
        assert!(mutation.stored.enable_upnp.is_enabled());
        assert!(mutation.stored.enable_natpmp.is_enabled());
        assert!(mutation.stored.enable_pex.is_enabled());
        assert!(mutation.effective.network.enable_lsd.is_enabled());
        assert!(mutation.effective.network.enable_upnp.is_enabled());
        assert!(mutation.effective.network.enable_natpmp.is_enabled());
        assert!(mutation.effective.network.enable_pex.is_enabled());
    }

    #[test]
    fn patch_validates_dht_endpoints() {
        let profile = sample_profile();
        let invalid = json!({ "dht_bootstrap_nodes": ["bad-endpoint"] });
        let err = validate_engine_profile_patch(&profile, &invalid, &HashSet::new()).unwrap_err();
        assert!(
            err.to_string().contains("host:port"),
            "expected host:port validation"
        );

        let valid = json!({
            "dht_bootstrap_nodes": ["router.bittorrent.com:6881", "router.bittorrent.com:6881", " 1.2.3.4:15000 "],
            "dht_router_nodes": ["dht.transmissionbt.com:6881"]
        });
        let mutation =
            validate_engine_profile_patch(&profile, &valid, &HashSet::new()).expect("valid patch");
        assert_eq!(mutation.stored.dht_bootstrap_nodes.len(), 2);
        assert_eq!(
            mutation.stored.dht_bootstrap_nodes[0],
            "router.bittorrent.com:6881"
        );
        assert_eq!(mutation.stored.dht_bootstrap_nodes[1], "1.2.3.4:15000");
        assert_eq!(
            mutation.stored.dht_router_nodes,
            vec!["dht.transmissionbt.com:6881"]
        );
    }

    #[test]
    fn tracker_normalisation_rejects_invalid_shapes() {
        let bad_tracker = json!("not-an-object");
        let err = normalize_tracker_payload(&bad_tracker).unwrap_err();
        assert!(err.to_string().contains("must be an object"));

        let missing_port = json!({"proxy": {"host": "proxy"}}); // port missing
        let err = normalize_tracker_payload(&missing_port).unwrap_err();
        assert!(err.to_string().contains("port"));
    }

    #[test]
    fn tracker_normalisation_dedupes_lists() {
        let payload = json!({
            "default": [" https://tracker.example/announce ", "https://tracker.example/announce", "UDP://TRACKER"],
            "extra": ["", "https://extra/1"],
            "replace": true,
            "announce_ip": " 1.2.3.4 ",
            "listen_interface": " eth0 ",
            "request_timeout_ms": 5000,
            "announce_to_all": true,
            "proxy": {
                "host": "proxy.local",
                "port": 8080,
                "proxy_peers": true,
                "kind": "socks5"
            }
        });

        let normalized = normalize_tracker_payload(&payload).expect("valid tracker payload");
        let config: TrackerConfig =
            serde_json::from_value(normalized).expect("config should decode");
        assert_eq!(config.default.len(), 2);
        assert_eq!(config.default[0], "https://tracker.example/announce");
        assert_eq!(config.default[1], "UDP://TRACKER");
        assert_eq!(config.extra, vec!["https://extra/1".to_string()]);
        assert!(config.replace);
        assert_eq!(config.announce_ip.as_deref(), Some("1.2.3.4"));
        assert_eq!(config.listen_interface.as_deref(), Some("eth0"));
        assert_eq!(config.request_timeout_ms, Some(5_000));
        assert!(config.announce_to_all);
        let proxy = config.proxy.expect("proxy present");
        assert_eq!(proxy.host, "proxy.local");
        assert_eq!(proxy.port, 8080);
        assert!(proxy.proxy_peers);
        assert_eq!(proxy.kind, TrackerProxyType::Socks5);
    }

    #[test]
    fn ip_filter_rejects_invalid_entries() {
        let profile = sample_profile();
        let bad_cidr = json!({"ip_filter": {"cidrs": ["not-a-cidr"]}});
        let err = validate_engine_profile_patch(&profile, &bad_cidr, &HashSet::new()).unwrap_err();
        assert!(err.to_string().contains("must contain valid IPv4 or IPv6"));

        let bad_url = json!({"ip_filter": {"blocklist_url": "ftp://example.com/list"}});
        let err = validate_engine_profile_patch(&profile, &bad_url, &HashSet::new()).unwrap_err();
        assert!(err.to_string().contains("http:// or https://"));
    }

    #[test]
    fn ip_filter_canonicalises_and_resets_metadata() {
        let mut profile = sample_profile();
        profile.ip_filter = json!({
            "cidrs": ["10.0.0.0/8"],
            "blocklist_url": "https://example.com/blocklist.txt",
            "etag": "v1",
            "last_updated_at": "2024-01-01T00:00:00Z",
            "last_error": "stale"
        });

        // No change should preserve metadata.
        let noop = json!({"ip_filter": {"cidrs": ["10.0.0.0/8"], "blocklist_url": "https://example.com/blocklist.txt"}});
        let mutation =
            validate_engine_profile_patch(&profile, &noop, &HashSet::new()).expect("valid patch");
        let preserved: IpFilterConfig =
            serde_json::from_value(mutation.stored.ip_filter).expect("decode ip filter");
        assert_eq!(preserved.etag.as_deref(), Some("v1"));
        assert!(preserved.last_updated_at.is_some());
        assert_eq!(preserved.last_error.as_deref(), Some("stale"));

        // Changing CIDRs clears metadata and canonicalises.
        let patch = json!({"ip_filter": {"cidrs": ["192.168.1.1", "192.168.1.0/24", "2001:db8::1/64"], "blocklist_url": "https://example.com/blocklist.txt"}});
        let mutation =
            validate_engine_profile_patch(&profile, &patch, &HashSet::new()).expect("valid patch");
        let updated: IpFilterConfig =
            serde_json::from_value(mutation.stored.ip_filter).expect("decode ip filter");
        assert_eq!(
            updated.cidrs,
            vec![
                "192.168.1.1/32".to_string(),
                "192.168.1.0/24".to_string(),
                "2001:db8::/64".to_string()
            ]
        );
        assert!(updated.etag.is_none());
        assert!(updated.last_updated_at.is_none());
        assert!(updated.last_error.is_none());
    }
}
