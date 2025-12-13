//! Typed configuration models and change payloads.
//!
//! # Design
//! - Pure data carriers used by the configuration service and API.
//! - Keeps domain types separate from IO/wiring code in `lib.rs`.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::net::IpAddr;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

use crate::engine_profile::EngineProfileEffective;

/// High-level view of the application profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppProfile {
    /// Primary key for the application profile row.
    pub id: Uuid,
    /// Friendly identifier displayed in user interfaces.
    pub instance_name: String,
    /// Operating mode (`setup` or `active`).
    pub mode: AppMode,
    /// Monotonic version used to detect concurrent updates.
    pub version: i64,
    /// HTTP port the API server should bind to.
    pub http_port: i32,
    /// IP address (and interface) the API server should bind to.
    pub bind_addr: IpAddr,
    /// Structured telemetry configuration (JSON object).
    pub telemetry: Value,
    /// Feature flags exposed to the application (JSON object).
    pub features: Value,
    /// Immutable keys that must not be edited by clients.
    pub immutable_keys: Value,
}

/// Setup or active mode flag recorded in `app_profile.mode`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AppMode {
    /// Provisioning mode that restricts APIs to setup operations.
    Setup,
    /// Normal operational mode.
    Active,
}

impl FromStr for AppMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "setup" => Ok(Self::Setup),
            "active" => Ok(Self::Active),
            other => Err(anyhow!("invalid app mode '{other}'")),
        }
    }
}

impl AppMode {
    #[must_use]
    /// Render the mode as its lowercase string representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Active => "active",
        }
    }
}

/// Transparent wrapper for boolean feature toggles to avoid pedantic lint churn.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(transparent)]
pub struct Toggle(pub bool);

impl Toggle {
    #[must_use]
    /// Whether the toggle is enabled.
    pub const fn is_enabled(self) -> bool {
        self.0
    }
}

impl From<bool> for Toggle {
    fn from(value: bool) -> Self {
        Self(value)
    }
}

impl From<Toggle> for bool {
    fn from(toggle: Toggle) -> Self {
        toggle.0
    }
}

/// Engine configuration surfaced to consumers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineProfile {
    /// Primary key for the engine profile row.
    pub id: Uuid,
    /// Engine implementation identifier (e.g., `stub`, `libtorrent`).
    pub implementation: String,
    /// Optional TCP port the engine should listen on.
    pub listen_port: Option<i32>,
    /// Whether the engine enables the DHT subsystem.
    pub dht: bool,
    /// Encryption policy string forwarded to the engine.
    pub encryption: String,
    /// Maximum number of concurrent active torrents.
    pub max_active: Option<i32>,
    /// Global download cap in bytes per second.
    pub max_download_bps: Option<i64>,
    /// Global upload cap in bytes per second.
    pub max_upload_bps: Option<i64>,
    /// Whether torrents default to sequential download.
    pub sequential_default: bool,
    /// Filesystem path for storing resume data.
    pub resume_dir: String,
    /// Root directory for active downloads.
    pub download_root: String,
    /// Arbitrary tracker configuration payload (JSON object).
    pub tracker: Value,
    /// Enable local service discovery (mDNS).
    pub enable_lsd: Toggle,
    /// Enable `UPnP` port mapping.
    pub enable_upnp: Toggle,
    /// Enable NAT-PMP port mapping.
    pub enable_natpmp: Toggle,
    /// Enable peer exchange (PEX).
    pub enable_pex: Toggle,
}

/// Filesystem policy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsPolicy {
    /// Primary key for the filesystem policy row.
    pub id: Uuid,
    /// Destination directory for completed artifacts.
    pub library_root: String,
    /// Whether archives should be extracted automatically.
    pub extract: bool,
    /// PAR2 verification policy (`disabled`, `verify`, etc.).
    pub par2: String,
    /// Whether nested directory structures should be flattened.
    pub flatten: bool,
    /// Move mode (`copy`, `move`, `hardlink`).
    pub move_mode: String,
    /// Cleanup rules describing paths to retain (JSON array).
    pub cleanup_keep: Value,
    /// Cleanup rules describing paths to purge (JSON array).
    pub cleanup_drop: Value,
    /// Optional chmod value applied to files.
    pub chmod_file: Option<String>,
    /// Optional chmod value applied to directories.
    pub chmod_dir: Option<String>,
    /// Optional owner applied to moved files.
    pub owner: Option<String>,
    /// Optional group applied to moved files.
    pub group: Option<String>,
    /// Optional umask enforced during filesystem operations.
    pub umask: Option<String>,
    /// Allow-list of destination paths (JSON array).
    pub allow_paths: Value,
}

/// Patch description for API keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum ApiKeyPatch {
    /// Insert or update an API key record.
    Upsert {
        /// Identifier for the API key.
        key_id: String,
        /// Optional human-readable label.
        label: Option<String>,
        /// Optional enabled flag override.
        enabled: Option<bool>,
        /// Optional new secret value.
        secret: Option<String>,
        /// Optional rate limit configuration payload.
        rate_limit: Option<Value>,
    },
    /// Remove an API key record.
    Delete {
        /// Identifier for the API key to remove.
        key_id: String,
    },
}

/// Patch description for secrets stored in `settings_secret`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum SecretPatch {
    /// Insert or update a secret value.
    Set {
        /// Secret key identifier.
        name: String,
        /// Secret value material.
        value: String,
    },
    /// Remove a secret entry.
    Delete {
        /// Secret key identifier to remove.
        name: String,
    },
}

/// Context returned after applying a changeset.
#[derive(Debug, Clone, Serialize)]
pub struct AppliedChanges {
    /// Revision recorded after the changeset was applied.
    pub revision: i64,
    /// New application profile snapshot when relevant.
    pub app_profile: Option<AppProfile>,
    /// Updated engine profile snapshot when relevant.
    pub engine_profile: Option<EngineProfile>,
    /// Updated filesystem policy snapshot when relevant.
    pub fs_policy: Option<FsPolicy>,
}

/// Structured change payload emitted by LISTEN/NOTIFY.
#[derive(Debug, Clone)]
pub struct SettingsChange {
    /// Database table that triggered the notification.
    pub table: String,
    /// Revision recorded after applying the change.
    pub revision: i64,
    /// Operation descriptor (`insert`, `update`, `delete`).
    pub operation: String,
    /// Optional payload describing the updated document.
    pub payload: SettingsPayload,
}

/// Optional rich payload associated with a `SettingsChange`.
#[derive(Debug, Clone)]
pub enum SettingsPayload {
    /// Application profile document that changed.
    AppProfile(AppProfile),
    /// Engine profile document that changed.
    EngineProfile(EngineProfile),
    /// Filesystem policy document that changed.
    FsPolicy(FsPolicy),
    /// Notification that did not include a payload.
    None,
}

/// Authentication context returned for a validated API key.
#[derive(Debug, Clone)]
pub struct ApiKeyAuth {
    /// Unique identifier associated with the API key record.
    pub key_id: String,
    /// Optional human-readable label for the key.
    pub label: Option<String>,
    /// Optional token-bucket rate limit applied to requests.
    pub rate_limit: Option<ApiKeyRateLimit>,
}

/// Token-bucket rate limit configuration applied per API key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeyRateLimit {
    /// Maximum number of requests allowed within a replenishment window.
    pub burst: u32,
    /// Duration between token replenishments.
    pub replenish_period: Duration,
}

impl ApiKeyRateLimit {
    /// Serialise the rate limit into a stable JSON representation.
    #[must_use]
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "burst": self.burst,
            "per_seconds": self.replenish_period.as_secs(),
        })
    }
}

/// Token representation surfaced to the caller. The plaintext value is only
/// available at issuance time.
#[derive(Debug, Clone)]
pub struct SetupToken {
    /// HMAC-safe API token value (plaintext).
    pub plaintext: String,
    /// Expiry instant for the token.
    pub expires_at: DateTime<Utc>,
}

/// Snapshot of the full configuration state at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    /// Monotonic revision identifier.
    pub revision: i64,
    /// Application profile configuration.
    pub app_profile: AppProfile,
    /// Torrent engine profile configuration.
    pub engine_profile: EngineProfile,
    /// Effective engine profile after clamping and normalisation.
    pub engine_profile_effective: EngineProfileEffective,
    /// Filesystem policy configuration.
    pub fs_policy: FsPolicy,
}

/// Changeset payload applied to the configuration service.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SettingsChangeset {
    /// Optional application profile update payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_profile: Option<Value>,
    /// Optional engine profile update payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_profile: Option<Value>,
    /// Optional filesystem policy update payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fs_policy: Option<Value>,
    /// API key upserts/deletions included in the changeset.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub api_keys: Vec<ApiKeyPatch>,
    /// Secret store mutations included in the changeset.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<SecretPatch>,
}

impl SettingsChangeset {
    #[must_use]
    /// Whether the changeset is empty (no patch operations specified).
    pub const fn is_empty(&self) -> bool {
        self.app_profile.is_none()
            && self.engine_profile.is_none()
            && self.fs_policy.is_none()
            && self.api_keys.is_empty()
            && self.secrets.is_empty()
    }
}
