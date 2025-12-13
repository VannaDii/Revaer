//! Engine profile validation and normalization helpers shared across the API and runtime paths.
//!
//! # Design
//! - Applies immutable-key checks and type validation for engine profile patches.
//! - Normalises persisted values to safe defaults before they are stored or forwarded to runtime.
//! - Surfaces an "effective" view with clamped values plus guard-rail warnings for observability.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::model::EngineProfile;
use crate::validate::{ConfigError, ensure_mutable, parse_port};

/// Upper bound guard rail for rate limits (≈5 Gbps).
pub const MAX_RATE_LIMIT_BPS: i64 = 5_000_000_000;
const DEFAULT_DOWNLOAD_ROOT: &str = "/data/staging";
const DEFAULT_RESUME_DIR: &str = "/var/lib/revaer/state";

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
    /// Whether DHT is enabled for peer discovery.
    pub enable_dht: bool,
    /// Encryption policy applied to inbound/outbound peers.
    pub encryption: EngineEncryptionPolicy,
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
        dht: effective.network.enable_dht,
        encryption: effective.network.encryption.as_str().to_string(),
        max_active: effective.limits.max_active,
        max_download_bps: effective.limits.download_rate_limit,
        max_upload_bps: effective.limits.upload_rate_limit,
        sequential_default: effective.behavior.sequential_default,
        resume_dir: effective.storage.resume_dir.clone(),
        download_root: effective.storage.download_root.clone(),
        tracker: effective.tracker.clone(),
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

    EngineProfileEffective {
        implementation: profile.implementation.clone(),
        network: EngineNetworkConfig {
            listen_port,
            enable_dht: profile.dht,
            encryption,
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
            dht: true,
            encryption: "prefer".into(),
            max_active: Some(4),
            max_download_bps: Some(250_000),
            max_upload_bps: Some(125_000),
            sequential_default: false,
            resume_dir: "/tmp/resume".into(),
            download_root: "/tmp/downloads".into(),
            tracker: json!({}),
        }
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
}
