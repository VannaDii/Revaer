//! Engine profile validation and normalization helpers shared across the API and runtime paths.
//!
//! # Design
//! - Applies immutable-key checks and type validation for engine profile patches.
//! - Normalises persisted values to safe defaults before they are stored or forwarded to runtime.
//! - Surfaces an "effective" view with clamped values plus guard-rail warnings for observability.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::model::EngineProfile;
use crate::validate::{ConfigError, ensure_mutable, ensure_object, parse_port};

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
            ensure_object(value, "engine_profile", "tracker")?;
            Ok(assign_if_changed(&mut working.tracker, value.clone()))
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
    if value.is_object() {
        value.clone()
    } else {
        warnings.push("tracker payload was not an object; replacing with {}".to_string());
        Value::Object(Map::new())
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
}
