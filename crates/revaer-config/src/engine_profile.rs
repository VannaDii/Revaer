//! Engine profile validation and normalization helpers shared across the API and runtime paths.
//!
//! # Design
//! - Applies immutable-key checks and type validation for engine profile patches.
//! - Normalises persisted values to safe defaults before they are stored or forwarded to runtime.
//! - Surfaces an "effective" view with clamped values plus guard-rail warnings for observability.

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use chrono::{DateTime, NaiveTime, Timelike, Utc, Weekday};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::model::{EngineProfile, Toggle};
use crate::validate::{ConfigError, ensure_mutable, parse_port};

/// Upper bound guard rail for rate limits (≈5 Gbps).
pub const MAX_RATE_LIMIT_BPS: i64 = 5_000_000_000;
const DEFAULT_DOWNLOAD_ROOT: &str = "/data/staging";
const DEFAULT_RESUME_DIR: &str = "/var/lib/revaer/state";
const DEFAULT_STORAGE_MODE: StorageMode = StorageMode::Sparse;
const MAX_INLINE_IP_FILTER_ENTRIES: usize = 5_000;
const MINUTES_PER_DAY: u16 = 24 * 60;

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
    /// Alternate speed configuration and schedule.
    pub alt_speed: AltSpeedConfig,
    /// Tracker configuration payload (validated to an object).
    pub tracker: Value,
    /// Peer class configuration applied to the engine.
    pub peer_classes: PeerClassesConfig,
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
    /// Optional outgoing port range for peer connections.
    pub outgoing_ports: Option<OutgoingPortRange>,
    /// Optional DSCP/TOS codepoint (0-63) applied to peer sockets.
    pub peer_dscp: Option<u8>,
    /// Whether anonymous mode is enabled.
    pub anonymous_mode: Toggle,
    /// Whether peers must be proxied.
    pub force_proxy: Toggle,
    /// Whether RC4 should be preferred for encryption.
    pub prefer_rc4: Toggle,
    /// Whether multiple connections per IP are allowed.
    pub allow_multiple_connections_per_ip: Toggle,
    /// Whether outgoing uTP is enabled.
    pub enable_outgoing_utp: Toggle,
    /// Whether incoming uTP is enabled.
    pub enable_incoming_utp: Toggle,
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

/// Outgoing port range used for peer connections.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutgoingPortRange {
    /// Start of the port range (inclusive).
    pub start: u16,
    /// End of the port range (inclusive).
    pub end: u16,
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
    /// Optional share ratio threshold before stopping seeding.
    pub seed_ratio_limit: Option<f64>,
    /// Optional seeding time limit in seconds.
    pub seed_time_limit: Option<i64>,
    /// Optional global peer connection limit.
    pub connections_limit: Option<i32>,
    /// Optional per-torrent peer connection limit.
    pub connections_limit_per_torrent: Option<i32>,
    /// Optional unchoke slot limit.
    pub unchoke_slots: Option<i32>,
    /// Optional half-open connection limit.
    pub half_open_limit: Option<i32>,
    /// Optional stats alert interval in milliseconds.
    #[serde(default)]
    pub stats_interval_ms: Option<i32>,
    /// Choking strategy used for downloads.
    pub choking_algorithm: ChokingAlgorithm,
    /// Choking strategy used while seeding.
    pub seed_choking_algorithm: SeedChokingAlgorithm,
    /// Whether strict super-seeding is enforced.
    pub strict_super_seeding: Toggle,
    /// Optional optimistic unchoke slot override.
    pub optimistic_unchoke_slots: Option<i32>,
    /// Optional maximum queued disk bytes override.
    pub max_queued_disk_bytes: Option<i64>,
}

/// Alternate speed caps and optional schedule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AltSpeedConfig {
    /// Alternate download cap in bytes per second.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_bps: Option<i64>,
    /// Alternate upload cap in bytes per second.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_bps: Option<i64>,
    /// Optional recurring schedule when the alternate caps should apply.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<AltSpeedSchedule>,
}

impl AltSpeedConfig {
    /// Convert to a JSON object for persistence and API responses.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut map = Map::new();
        if let Some(download) = self.download_bps {
            map.insert("download_bps".to_string(), json!(download));
        }
        if let Some(upload) = self.upload_bps {
            map.insert("upload_bps".to_string(), json!(upload));
        }
        if let Some(schedule) = &self.schedule {
            map.insert("schedule".to_string(), schedule.to_value());
        }
        Value::Object(map)
    }

    const fn has_caps(&self) -> bool {
        self.download_bps.is_some() || self.upload_bps.is_some()
    }
}

/// Recurring schedule describing when alternate speeds should take effect.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AltSpeedSchedule {
    /// Days of the week the schedule applies to (UTC).
    pub days: Vec<Weekday>,
    /// Start time expressed as minutes from midnight UTC.
    pub start_minutes: u16,
    /// End time expressed as minutes from midnight UTC.
    pub end_minutes: u16,
}

impl AltSpeedSchedule {
    /// Convert the schedule into a JSON object.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let days = self
            .days
            .iter()
            .map(|day| weekday_label(*day).to_string())
            .collect::<Vec<_>>();
        json!({
            "days": days,
            "start": format_minutes(self.start_minutes),
            "end": format_minutes(self.end_minutes),
        })
    }
}

/// Storage behaviour and paths for active downloads and resume data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStorageConfig {
    /// Root directory for active downloads.
    pub download_root: String,
    /// Directory for fast-resume payloads.
    pub resume_dir: String,
    /// Allocation strategy for new torrents.
    pub storage_mode: StorageMode,
    /// Whether partfiles should be used for incomplete pieces.
    pub use_partfile: Toggle,
    /// Optional disk read mode.
    pub disk_read_mode: Option<DiskIoMode>,
    /// Optional disk write mode.
    pub disk_write_mode: Option<DiskIoMode>,
    /// Whether piece hashes should be verified.
    pub verify_piece_hashes: Toggle,
    /// Optional disk cache size in MiB.
    pub cache_size: Option<i32>,
    /// Optional cache expiry in seconds.
    pub cache_expiry: Option<i32>,
    /// Whether disk reads should be coalesced.
    pub coalesce_reads: Toggle,
    /// Whether disk writes should be coalesced.
    pub coalesce_writes: Toggle,
    /// Whether the shared disk cache pool should be used.
    pub use_disk_cache_pool: Toggle,
}

/// Allocation modes supported by the engine.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StorageMode {
    /// Sparse file allocation (default).
    Sparse,
    /// Pre-allocate full files.
    Allocate,
}

/// Disk IO cache policy exposed to the engine.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiskIoMode {
    /// Allow the operating system to cache disk IO.
    EnableOsCache,
    /// Disable OS caching for disk IO.
    DisableOsCache,
    /// Write through without caching.
    WriteThrough,
}

impl DiskIoMode {
    #[must_use]
    /// String representation used for persistence and API payloads.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EnableOsCache => "enable_os_cache",
            Self::DisableOsCache => "disable_os_cache",
            Self::WriteThrough => "write_through",
        }
    }
}

/// Behavioural defaults applied when admitting torrents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineBehaviorConfig {
    /// Whether sequential mode is the default.
    pub sequential_default: bool,
    /// Whether torrents are auto-managed by default.
    pub auto_managed: Toggle,
    /// Whether to prefer seeds when allocating queue slots.
    pub auto_manage_prefer_seeds: Toggle,
    /// Whether idle torrents should be excluded from slot accounting.
    pub dont_count_slow_torrents: Toggle,
    /// Whether torrents default to super-seeding.
    pub super_seeding: Toggle,
}

/// Choking strategy applied to downloading torrents.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChokingAlgorithm {
    /// Fixed number of unchoke slots.
    FixedSlots,
    /// Rate-based choking with dynamic slot count.
    RateBased,
}

impl ChokingAlgorithm {
    #[must_use]
    /// Render the algorithm as its canonical string representation.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FixedSlots => "fixed_slots",
            Self::RateBased => "rate_based",
        }
    }
}

/// Choking strategy applied while seeding.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SeedChokingAlgorithm {
    /// Rotate unchoked peers evenly.
    RoundRobin,
    /// Prioritise peers we can upload to the fastest.
    FastestUpload,
    /// Bias toward peers starting or finishing a download.
    AntiLeech,
}

impl SeedChokingAlgorithm {
    #[must_use]
    /// Render the algorithm as its canonical string representation.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RoundRobin => "round_robin",
            Self::FastestUpload => "fastest_upload",
            Self::AntiLeech => "anti_leech",
        }
    }
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    /// Optional client certificate path for tracker TLS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_cert: Option<String>,
    /// Optional client private key path for tracker TLS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_private_key: Option<String>,
    /// Optional CA certificate bundle path for tracker TLS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_ca_cert: Option<String>,
    /// Whether to verify tracker TLS certificates.
    #[serde(default = "TrackerConfig::default_ssl_tracker_verify")]
    pub ssl_tracker_verify: bool,
    /// Optional proxy configuration for tracker announces.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<TrackerProxyConfig>,
    /// Optional authentication material for tracker announces.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<TrackerAuthConfig>,
}

/// Peer class definition applied to the engine session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerClassConfig {
    /// Stable class identifier (0–31).
    pub id: u8,
    /// Optional human-readable label.
    pub label: String,
    /// Download bandwidth allocation priority (1–255).
    pub download_priority: u8,
    /// Upload bandwidth allocation priority (1–255).
    pub upload_priority: u8,
    /// Connection limit factor applied to this class (percentage multiplier).
    pub connection_limit_factor: u16,
    /// Whether unchoke slots should be ignored for this class.
    #[serde(default)]
    pub ignore_unchoke_slots: bool,
}

impl Default for PeerClassConfig {
    fn default() -> Self {
        Self {
            id: 0,
            label: "class_0".to_string(),
            download_priority: 1,
            upload_priority: 1,
            connection_limit_factor: 100,
            ignore_unchoke_slots: false,
        }
    }
}

/// Aggregated peer class configuration including defaults.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PeerClassesConfig {
    /// Class definitions keyed by id.
    #[serde(default)]
    pub classes: Vec<PeerClassConfig>,
    /// Default class ids applied when none are specified.
    #[serde(default)]
    pub default: Vec<u8>,
}

impl PeerClassesConfig {
    /// Convert the typed configuration into a JSON payload for persistence.
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!(self)
    }
}

impl TrackerConfig {
    /// Convert the typed config into a JSON object for persistence/effective views.
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!(self)
    }

    const fn default_ssl_tracker_verify() -> bool {
        true
    }
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            default: Vec::new(),
            extra: Vec::new(),
            replace: false,
            user_agent: None,
            announce_ip: None,
            listen_interface: None,
            request_timeout_ms: None,
            announce_to_all: false,
            ssl_cert: None,
            ssl_private_key: None,
            ssl_ca_cert: None,
            ssl_tracker_verify: true,
            proxy: None,
            auth: None,
        }
    }
}

/// Authentication material for tracker requests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TrackerAuthConfig {
    /// Optional username secret reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username_secret: Option<String>,
    /// Optional password secret reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_secret: Option<String>,
    /// Optional cookie secret reference (trackerid or HTTP cookie value).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie_secret: Option<String>,
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
        seed_ratio_limit: effective.limits.seed_ratio_limit,
        seed_time_limit: effective.limits.seed_time_limit,
        connections_limit: effective.limits.connections_limit,
        connections_limit_per_torrent: effective.limits.connections_limit_per_torrent,
        unchoke_slots: effective.limits.unchoke_slots,
        half_open_limit: effective.limits.half_open_limit,
        stats_interval_ms: effective.limits.stats_interval_ms.map(i64::from),
        alt_speed: effective.alt_speed.to_value(),
        sequential_default: effective.behavior.sequential_default,
        auto_managed: effective.behavior.auto_managed,
        auto_manage_prefer_seeds: effective.behavior.auto_manage_prefer_seeds,
        dont_count_slow_torrents: effective.behavior.dont_count_slow_torrents,
        super_seeding: effective.behavior.super_seeding,
        choking_algorithm: effective.limits.choking_algorithm.as_str().to_string(),
        seed_choking_algorithm: effective.limits.seed_choking_algorithm.as_str().to_string(),
        strict_super_seeding: effective.limits.strict_super_seeding,
        optimistic_unchoke_slots: effective.limits.optimistic_unchoke_slots,
        max_queued_disk_bytes: effective.limits.max_queued_disk_bytes,
        resume_dir: effective.storage.resume_dir.clone(),
        download_root: effective.storage.download_root.clone(),
        storage_mode: storage_mode_label(effective.storage.storage_mode).to_string(),
        use_partfile: effective.storage.use_partfile,
        disk_read_mode: effective
            .storage
            .disk_read_mode
            .map(|mode| mode.as_str().to_string()),
        disk_write_mode: effective
            .storage
            .disk_write_mode
            .map(|mode| mode.as_str().to_string()),
        verify_piece_hashes: effective.storage.verify_piece_hashes,
        cache_size: effective.storage.cache_size,
        cache_expiry: effective.storage.cache_expiry,
        coalesce_reads: effective.storage.coalesce_reads,
        coalesce_writes: effective.storage.coalesce_writes,
        use_disk_cache_pool: effective.storage.use_disk_cache_pool,
        tracker: effective.tracker.clone(),
        enable_lsd: effective.network.enable_lsd,
        enable_upnp: effective.network.enable_upnp,
        enable_natpmp: effective.network.enable_natpmp,
        enable_pex: effective.network.enable_pex,
        dht_bootstrap_nodes: effective.network.dht_bootstrap_nodes.clone(),
        dht_router_nodes: effective.network.dht_router_nodes.clone(),
        ip_filter: effective.network.ip_filter.to_value(),
        anonymous_mode: effective.network.anonymous_mode,
        force_proxy: effective.network.force_proxy,
        prefer_rc4: effective.network.prefer_rc4,
        allow_multiple_connections_per_ip: effective.network.allow_multiple_connections_per_ip,
        enable_outgoing_utp: effective.network.enable_outgoing_utp,
        enable_incoming_utp: effective.network.enable_incoming_utp,
        outgoing_port_min: effective
            .network
            .outgoing_ports
            .map(|range| i32::from(range.start)),
        outgoing_port_max: effective
            .network
            .outgoing_ports
            .map(|range| i32::from(range.end)),
        peer_dscp: effective.network.peer_dscp.map(i32::from),
        peer_classes: effective.peer_classes.to_value(),
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
    let (listen_port, listen_interfaces, ipv6_mode) =
        sanitize_listen_config(profile, &mut warnings);
    let limits = sanitize_limits(profile, &mut warnings);
    let alt_speed = sanitize_alt_speed(&profile.alt_speed, &mut warnings);
    let storage = sanitize_storage(profile, &mut warnings);
    let tracker = sanitize_tracker(&profile.tracker, &mut warnings);
    let (dht_bootstrap_nodes, dht_router_nodes) = sanitize_dht_endpoints(profile, &mut warnings);
    let ip_filter = sanitize_ip_filter(&profile.ip_filter, &mut warnings);
    let peer_classes = sanitize_peer_classes(&profile.peer_classes, &mut warnings);
    let (outgoing_ports, peer_dscp) = sanitize_network_overrides(profile, &mut warnings);
    let encryption = canonical_encryption(&profile.encryption, &mut warnings);
    let tracker_proxy_present = serde_json::from_value::<TrackerConfig>(tracker.clone())
        .ok()
        .and_then(|cfg| cfg.proxy)
        .is_some();
    let anonymous_mode = profile.anonymous_mode;
    let force_proxy = enforce_proxy_requirements(
        anonymous_mode,
        profile.force_proxy,
        tracker_proxy_present,
        &mut warnings,
    );

    EngineProfileEffective {
        implementation: profile.implementation.clone(),
        network: EngineNetworkConfig {
            listen_port,
            listen_interfaces,
            ipv6_mode,
            outgoing_ports,
            peer_dscp,
            anonymous_mode,
            force_proxy,
            prefer_rc4: profile.prefer_rc4,
            allow_multiple_connections_per_ip: profile.allow_multiple_connections_per_ip,
            enable_outgoing_utp: profile.enable_outgoing_utp,
            enable_incoming_utp: profile.enable_incoming_utp,
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
        limits,
        storage,
        behavior: EngineBehaviorConfig {
            sequential_default: profile.sequential_default,
            auto_managed: profile.auto_managed,
            auto_manage_prefer_seeds: profile.auto_manage_prefer_seeds,
            dont_count_slow_torrents: profile.dont_count_slow_torrents,
            super_seeding: profile.super_seeding,
        },
        alt_speed,
        tracker,
        peer_classes,
        warnings,
    }
}

fn sanitize_listen_config(
    profile: &EngineProfile,
    warnings: &mut Vec<String>,
) -> (Option<i32>, Vec<String>, EngineIpv6Mode) {
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
    let listen_interfaces =
        sanitize_listen_interfaces(&profile.listen_interfaces, "listen_interfaces", warnings);
    let ipv6_mode = canonicalize_ipv6_mode(&profile.ipv6_mode, warnings);
    (listen_port, listen_interfaces, ipv6_mode)
}

fn sanitize_limits(profile: &EngineProfile, warnings: &mut Vec<String>) -> EngineLimitsConfig {
    let max_active = sanitize_positive_limit(profile.max_active, "max_active", warnings);
    let connections_limit =
        sanitize_positive_limit(profile.connections_limit, "connections_limit", warnings);
    let connections_limit_per_torrent = sanitize_positive_limit(
        profile.connections_limit_per_torrent,
        "connections_limit_per_torrent",
        warnings,
    );
    let unchoke_slots = sanitize_positive_limit(profile.unchoke_slots, "unchoke_slots", warnings);
    let half_open_limit =
        sanitize_positive_limit(profile.half_open_limit, "half_open_limit", warnings);
    let stats_interval_ms =
        sanitize_stats_interval(profile.stats_interval_ms, "stats_interval_ms", warnings);
    let download_rate_limit =
        clamp_rate_limit("max_download_bps", profile.max_download_bps, warnings);
    let upload_rate_limit = clamp_rate_limit("max_upload_bps", profile.max_upload_bps, warnings);
    let seed_ratio_limit = sanitize_seed_ratio_limit(profile.seed_ratio_limit, warnings);
    let seed_time_limit = sanitize_seed_time_limit(profile.seed_time_limit, warnings);
    let choking_algorithm = canonical_choking_algorithm(&profile.choking_algorithm, warnings);
    let seed_choking_algorithm =
        canonical_seed_choking_algorithm(&profile.seed_choking_algorithm, warnings);
    let optimistic_unchoke_slots =
        sanitize_optimistic_unchoke_slots(profile.optimistic_unchoke_slots, warnings);
    let max_queued_disk_bytes =
        sanitize_max_queued_disk_bytes(profile.max_queued_disk_bytes, warnings);

    EngineLimitsConfig {
        max_active,
        download_rate_limit,
        upload_rate_limit,
        seed_ratio_limit,
        seed_time_limit,
        connections_limit,
        connections_limit_per_torrent,
        unchoke_slots,
        half_open_limit,
        stats_interval_ms,
        choking_algorithm,
        seed_choking_algorithm,
        strict_super_seeding: profile.strict_super_seeding,
        optimistic_unchoke_slots,
        max_queued_disk_bytes,
    }
}

fn sanitize_alt_speed(value: &Value, warnings: &mut Vec<String>) -> AltSpeedConfig {
    match normalize_alt_speed_payload(value) {
        Ok(mut config) => {
            config.download_bps =
                clamp_rate_limit("alt_speed.download_bps", config.download_bps, warnings);
            config.upload_bps =
                clamp_rate_limit("alt_speed.upload_bps", config.upload_bps, warnings);

            if config.schedule.is_none() {
                if config.has_caps() {
                    warnings.push(
                        "alt_speed.schedule missing; alternate caps will not be applied"
                            .to_string(),
                    );
                }
                AltSpeedConfig::default()
            } else if !config.has_caps() {
                warnings.push(
                    "alt_speed requires download_bps or upload_bps; disabling alternate speeds"
                        .to_string(),
                );
                AltSpeedConfig::default()
            } else {
                config
            }
        }
        Err(err) => {
            warnings.push(format!(
                "alt_speed payload invalid ({err}); disabling alternate speeds"
            ));
            AltSpeedConfig::default()
        }
    }
}

fn sanitize_positive_limit(
    value: Option<i32>,
    label: &str,
    warnings: &mut Vec<String>,
) -> Option<i32> {
    match value {
        Some(v) if v > 0 => Some(v),
        Some(v) => {
            warnings.push(format!("{label} {v} is non-positive; disabling override"));
            None
        }
        None => None,
    }
}

fn sanitize_seed_ratio_limit(value: Option<f64>, warnings: &mut Vec<String>) -> Option<f64> {
    value.and_then(|ratio| {
        if !ratio.is_finite() || ratio < 0.0 {
            warnings.push(format!(
                "seed_ratio_limit {ratio} is invalid; disabling ratio stop"
            ));
            None
        } else {
            Some(ratio)
        }
    })
}

fn sanitize_seed_time_limit(value: Option<i64>, warnings: &mut Vec<String>) -> Option<i64> {
    value.and_then(|seconds| {
        if seconds < 0 {
            warnings.push(format!(
                "seed_time_limit {seconds} is negative; disabling seeding timeout"
            ));
            None
        } else {
            Some(seconds)
        }
    })
}

fn canonical_choking_algorithm(raw: &str, warnings: &mut Vec<String>) -> ChokingAlgorithm {
    match raw.to_ascii_lowercase().as_str() {
        "fixed" | "fixed_slots" => ChokingAlgorithm::FixedSlots,
        "rate_based" | "rate-based" | "rate" => ChokingAlgorithm::RateBased,
        other => {
            warnings.push(format!(
                "unknown choking_algorithm '{other}'; defaulting to 'fixed_slots'"
            ));
            ChokingAlgorithm::FixedSlots
        }
    }
}

fn sanitize_stats_interval(
    value: Option<i64>,
    label: &str,
    warnings: &mut Vec<String>,
) -> Option<i32> {
    match value {
        Some(v) if (100..=600_000).contains(&v) => i32::try_from(v).ok(),
        Some(v) => {
            warnings.push(format!(
                "{label} {v} is out of range; using default cadence"
            ));
            None
        }
        None => None,
    }
}

fn canonical_seed_choking_algorithm(raw: &str, warnings: &mut Vec<String>) -> SeedChokingAlgorithm {
    match raw.to_ascii_lowercase().as_str() {
        "round_robin" | "round-robin" | "roundrobin" => SeedChokingAlgorithm::RoundRobin,
        "fastest_upload" | "fastest-upload" | "fastest" => SeedChokingAlgorithm::FastestUpload,
        "anti_leech" | "anti-leech" | "antileech" => SeedChokingAlgorithm::AntiLeech,
        other => {
            warnings.push(format!(
                "unknown seed_choking_algorithm '{other}'; defaulting to 'round_robin'"
            ));
            SeedChokingAlgorithm::RoundRobin
        }
    }
}

fn sanitize_optimistic_unchoke_slots(
    value: Option<i32>,
    warnings: &mut Vec<String>,
) -> Option<i32> {
    value.and_then(|slots| {
        if slots <= 0 {
            warnings.push(format!(
                "optimistic_unchoke_slots {slots} is non-positive; disabling override"
            ));
            None
        } else {
            Some(slots)
        }
    })
}

fn sanitize_max_queued_disk_bytes(value: Option<i64>, warnings: &mut Vec<String>) -> Option<i64> {
    value.and_then(|limit| {
        if limit <= 0 {
            warnings.push(format!(
                "max_queued_disk_bytes {limit} is non-positive; disabling override"
            ));
            None
        } else if limit > i64::from(i32::MAX) {
            warnings.push(format!(
                "max_queued_disk_bytes {limit} exceeds i32::MAX; clamping to {}",
                i32::MAX
            ));
            Some(i64::from(i32::MAX))
        } else {
            Some(limit)
        }
    })
}

fn sanitize_storage(profile: &EngineProfile, warnings: &mut Vec<String>) -> EngineStorageConfig {
    let download_root = sanitize_path(
        &profile.download_root,
        DEFAULT_DOWNLOAD_ROOT,
        "download_root",
        warnings,
    );
    let resume_dir = sanitize_path(
        &profile.resume_dir,
        DEFAULT_RESUME_DIR,
        "resume_dir",
        warnings,
    );
    let storage_mode = sanitize_storage_mode(&profile.storage_mode, warnings);
    let use_partfile = profile.use_partfile;
    let disk_read_mode = parse_disk_mode(
        profile.disk_read_mode.as_deref(),
        "disk_read_mode",
        warnings,
    );
    let disk_write_mode = parse_disk_mode(
        profile.disk_write_mode.as_deref(),
        "disk_write_mode",
        warnings,
    );
    let verify_piece_hashes = profile.verify_piece_hashes;
    let cache_size = sanitize_cache_value("cache_size", profile.cache_size, warnings);
    let cache_expiry = sanitize_cache_value("cache_expiry", profile.cache_expiry, warnings);
    let coalesce_reads = profile.coalesce_reads;
    let coalesce_writes = profile.coalesce_writes;
    let use_disk_cache_pool = profile.use_disk_cache_pool;

    EngineStorageConfig {
        download_root,
        resume_dir,
        storage_mode,
        use_partfile,
        disk_read_mode,
        disk_write_mode,
        verify_piece_hashes,
        cache_size,
        cache_expiry,
        coalesce_reads,
        coalesce_writes,
        use_disk_cache_pool,
    }
}

fn sanitize_storage_mode(value: &str, warnings: &mut Vec<String>) -> StorageMode {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "" => DEFAULT_STORAGE_MODE,
        "sparse" => StorageMode::Sparse,
        "allocate" => StorageMode::Allocate,
        other => {
            warnings.push(format!(
                "storage_mode '{other}' is not supported; defaulting to sparse allocation"
            ));
            DEFAULT_STORAGE_MODE
        }
    }
}

fn parse_disk_mode(
    value: Option<&str>,
    field: &str,
    warnings: &mut Vec<String>,
) -> Option<DiskIoMode> {
    let mode = value.map(|raw| raw.trim().to_ascii_lowercase())?;

    match mode.as_str() {
        "" => None,
        "enable_os_cache" => Some(DiskIoMode::EnableOsCache),
        "disable_os_cache" => Some(DiskIoMode::DisableOsCache),
        "write_through" => Some(DiskIoMode::WriteThrough),
        other => {
            warnings.push(format!(
                "{field} '{other}' is not supported; ignoring override"
            ));
            None
        }
    }
}

fn sanitize_cache_value(
    field: &str,
    value: Option<i32>,
    warnings: &mut Vec<String>,
) -> Option<i32> {
    value.and_then(|val| {
        if val <= 0 {
            warnings.push(format!(
                "{field} {val} is non-positive; leaving libtorrent defaults in place"
            ));
            None
        } else {
            Some(val)
        }
    })
}

fn validate_storage_mode(mode: &str) -> Result<(), ConfigError> {
    let normalized = mode.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "sparse" | "allocate" => Ok(()),
        other => Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "storage_mode".to_string(),
            message: format!("must be sparse or allocate (got {other})"),
        }),
    }
}

fn sanitize_dht_endpoints(
    profile: &EngineProfile,
    warnings: &mut Vec<String>,
) -> (Vec<String>, Vec<String>) {
    let dht_bootstrap_nodes = sanitize_endpoints(
        &profile.dht_bootstrap_nodes,
        "dht_bootstrap_nodes",
        warnings,
    );
    let dht_router_nodes =
        sanitize_endpoints(&profile.dht_router_nodes, "dht_router_nodes", warnings);
    (dht_bootstrap_nodes, dht_router_nodes)
}

fn sanitize_network_overrides(
    profile: &EngineProfile,
    warnings: &mut Vec<String>,
) -> (Option<OutgoingPortRange>, Option<u8>) {
    let outgoing_ports = sanitize_outgoing_ports(
        profile.outgoing_port_min,
        profile.outgoing_port_max,
        warnings,
    );
    let peer_dscp = sanitize_peer_dscp(profile.peer_dscp, warnings);
    (outgoing_ports, peer_dscp)
}

fn enforce_proxy_requirements(
    anonymous_mode: Toggle,
    mut force_proxy: Toggle,
    tracker_proxy_present: bool,
    warnings: &mut Vec<String>,
) -> Toggle {
    if anonymous_mode.is_enabled() && tracker_proxy_present && !force_proxy.is_enabled() {
        warnings
            .push("anonymous_mode requested with a tracker proxy; forcing peer proxy".to_string());
        force_proxy = Toggle(true);
    }
    force_proxy
}

fn apply_field(
    working: &mut EngineProfile,
    key: &str,
    value: &Value,
    warnings: &mut Vec<String>,
) -> Result<bool, ConfigError> {
    if let Some(applied) = apply_privacy_field(working, key, value)? {
        return Ok(applied);
    }
    if let Some(applied) = apply_network_field(working, key, value, warnings)? {
        return Ok(applied);
    }
    if let Some(applied) = apply_limit_field(working, key, value)? {
        return Ok(applied);
    }
    if let Some(applied) = apply_alt_speed_field(working, key, value)? {
        return Ok(applied);
    }
    if let Some(applied) = apply_behavior_field(working, key, value)? {
        return Ok(applied);
    }
    if let Some(applied) = apply_toggle_field(working, key, value)? {
        return Ok(applied);
    }
    if let Some(applied) = apply_dht_field(working, key, value)? {
        return Ok(applied);
    }
    if let Some(applied) = apply_tracker_field(working, key, value)? {
        return Ok(applied);
    }
    if let Some(applied) = apply_peer_class_field(working, key, value, warnings)? {
        return Ok(applied);
    }
    Err(ConfigError::UnknownField {
        section: "engine_profile".to_string(),
        field: key.to_string(),
    })
}

fn apply_privacy_field(
    working: &mut EngineProfile,
    key: &str,
    value: &Value,
) -> Result<Option<bool>, ConfigError> {
    let applied = match key {
        "anonymous_mode" => Some(assign_if_changed(
            &mut working.anonymous_mode,
            Toggle::from(required_bool(value, "anonymous_mode")?),
        )),
        "force_proxy" => Some(assign_if_changed(
            &mut working.force_proxy,
            Toggle::from(required_bool(value, "force_proxy")?),
        )),
        "prefer_rc4" => Some(assign_if_changed(
            &mut working.prefer_rc4,
            Toggle::from(required_bool(value, "prefer_rc4")?),
        )),
        "allow_multiple_connections_per_ip" => Some(assign_if_changed(
            &mut working.allow_multiple_connections_per_ip,
            Toggle::from(required_bool(value, "allow_multiple_connections_per_ip")?),
        )),
        "enable_outgoing_utp" => Some(assign_if_changed(
            &mut working.enable_outgoing_utp,
            Toggle::from(required_bool(value, "enable_outgoing_utp")?),
        )),
        "enable_incoming_utp" => Some(assign_if_changed(
            &mut working.enable_incoming_utp,
            Toggle::from(required_bool(value, "enable_incoming_utp")?),
        )),
        _ => None,
    };
    Ok(applied)
}

fn apply_peer_class_field(
    working: &mut EngineProfile,
    key: &str,
    value: &Value,
    warnings: &mut Vec<String>,
) -> Result<Option<bool>, ConfigError> {
    if key != "peer_classes" {
        return Ok(None);
    }
    if !value.is_object() {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "peer_classes".to_string(),
            message: "peer_classes must be an object".to_string(),
        });
    }
    let sanitized = sanitize_peer_classes(value, warnings).to_value();
    Ok(Some(assign_if_changed(
        &mut working.peer_classes,
        sanitized,
    )))
}

fn apply_network_field(
    working: &mut EngineProfile,
    key: &str,
    value: &Value,
    warnings: &mut Vec<String>,
) -> Result<Option<bool>, ConfigError> {
    let applied = match key {
        "implementation" => Some(assign_if_changed(
            &mut working.implementation,
            required_string(value, "implementation")?,
        )),
        "listen_port" => Some(assign_if_changed(
            &mut working.listen_port,
            parse_optional_port(value)?,
        )),
        "listen_interfaces" => Some(assign_if_changed(
            &mut working.listen_interfaces,
            parse_listen_interfaces(value)?,
        )),
        "ipv6_mode" => Some(assign_if_changed(
            &mut working.ipv6_mode,
            parse_ipv6_mode(value)?,
        )),
        "dht" => Some(assign_if_changed(
            &mut working.dht,
            required_bool(value, "dht")?,
        )),
        "outgoing_port_min" => Some(assign_if_changed(
            &mut working.outgoing_port_min,
            parse_optional_i32(value, "outgoing_port_min")?,
        )),
        "outgoing_port_max" => Some(assign_if_changed(
            &mut working.outgoing_port_max,
            parse_optional_i32(value, "outgoing_port_max")?,
        )),
        "peer_dscp" => Some(assign_if_changed(
            &mut working.peer_dscp,
            parse_optional_i32(value, "peer_dscp")?,
        )),
        "encryption" => {
            let raw = required_string(value, "encryption")?;
            let policy = canonical_encryption(&raw, warnings);
            Some(assign_if_changed(
                &mut working.encryption,
                policy.as_str().to_string(),
            ))
        }
        _ => None,
    };
    Ok(applied)
}

fn apply_limit_field(
    working: &mut EngineProfile,
    key: &str,
    value: &Value,
) -> Result<Option<bool>, ConfigError> {
    let applied = match key {
        "max_active" => Some(assign_if_changed(
            &mut working.max_active,
            parse_optional_i32(value, "max_active")?,
        )),
        "connections_limit" => Some(assign_if_changed(
            &mut working.connections_limit,
            parse_optional_i32(value, "connections_limit")?,
        )),
        "connections_limit_per_torrent" => Some(assign_if_changed(
            &mut working.connections_limit_per_torrent,
            parse_optional_i32(value, "connections_limit_per_torrent")?,
        )),
        "unchoke_slots" => Some(assign_if_changed(
            &mut working.unchoke_slots,
            parse_optional_i32(value, "unchoke_slots")?,
        )),
        "half_open_limit" => Some(assign_if_changed(
            &mut working.half_open_limit,
            parse_optional_i32(value, "half_open_limit")?,
        )),
        "choking_algorithm" => Some(assign_if_changed(
            &mut working.choking_algorithm,
            required_string(value, "choking_algorithm")?,
        )),
        "seed_choking_algorithm" => Some(assign_if_changed(
            &mut working.seed_choking_algorithm,
            required_string(value, "seed_choking_algorithm")?,
        )),
        "strict_super_seeding" => Some(assign_if_changed(
            &mut working.strict_super_seeding,
            Toggle::from(required_bool(value, "strict_super_seeding")?),
        )),
        "optimistic_unchoke_slots" => Some(assign_if_changed(
            &mut working.optimistic_unchoke_slots,
            parse_optional_i32(value, "optimistic_unchoke_slots")?,
        )),
        "max_queued_disk_bytes" => Some(assign_if_changed(
            &mut working.max_queued_disk_bytes,
            parse_optional_non_negative_i64(value, "max_queued_disk_bytes")?,
        )),
        "max_download_bps" => Some(apply_rate_limit_field(
            working,
            value,
            "max_download_bps",
            |profile| profile.max_download_bps,
            |profile, limit| {
                profile.max_download_bps = limit;
            },
        )?),
        "max_upload_bps" => Some(apply_rate_limit_field(
            working,
            value,
            "max_upload_bps",
            |profile| profile.max_upload_bps,
            |profile, limit| {
                profile.max_upload_bps = limit;
            },
        )?),
        "seed_ratio_limit" => Some(assign_if_changed(
            &mut working.seed_ratio_limit,
            parse_optional_non_negative_f64(value, "seed_ratio_limit")?,
        )),
        "seed_time_limit" => Some(assign_if_changed(
            &mut working.seed_time_limit,
            parse_optional_non_negative_i64(value, "seed_time_limit")?,
        )),
        "stats_interval_ms" => Some(assign_if_changed(
            &mut working.stats_interval_ms,
            parse_optional_non_negative_i64(value, "stats_interval_ms")?,
        )),
        _ => None,
    };
    Ok(applied)
}

fn apply_alt_speed_field(
    working: &mut EngineProfile,
    key: &str,
    value: &Value,
) -> Result<Option<bool>, ConfigError> {
    let applied = match key {
        "alt_speed" => {
            let config = normalize_alt_speed_payload(value)?;
            Some(assign_if_changed(&mut working.alt_speed, config.to_value()))
        }
        _ => None,
    };
    Ok(applied)
}

fn apply_behavior_field(
    working: &mut EngineProfile,
    key: &str,
    value: &Value,
) -> Result<Option<bool>, ConfigError> {
    let applied = match key {
        "sequential_default" => Some(assign_if_changed(
            &mut working.sequential_default,
            required_bool(value, "sequential_default")?,
        )),
        "auto_managed" => Some(assign_if_changed(
            &mut working.auto_managed,
            Toggle::from(required_bool(value, "auto_managed")?),
        )),
        "auto_manage_prefer_seeds" => Some(assign_if_changed(
            &mut working.auto_manage_prefer_seeds,
            Toggle::from(required_bool(value, "auto_manage_prefer_seeds")?),
        )),
        "dont_count_slow_torrents" => Some(assign_if_changed(
            &mut working.dont_count_slow_torrents,
            Toggle::from(required_bool(value, "dont_count_slow_torrents")?),
        )),
        "super_seeding" => Some(assign_if_changed(
            &mut working.super_seeding,
            Toggle::from(required_bool(value, "super_seeding")?),
        )),
        "resume_dir" => Some(assign_if_changed(
            &mut working.resume_dir,
            required_string(value, "resume_dir")?,
        )),
        "download_root" => Some(assign_if_changed(
            &mut working.download_root,
            required_string(value, "download_root")?,
        )),
        "storage_mode" => {
            let mode = required_string(value, "storage_mode")?;
            validate_storage_mode(&mode)?;
            Some(assign_if_changed(&mut working.storage_mode, mode))
        }
        "use_partfile" => Some(assign_if_changed(
            &mut working.use_partfile,
            Toggle::from(required_bool(value, "use_partfile")?),
        )),
        "disk_read_mode" => Some(assign_if_changed(
            &mut working.disk_read_mode,
            Some(required_string(value, "disk_read_mode")?),
        )),
        "disk_write_mode" => Some(assign_if_changed(
            &mut working.disk_write_mode,
            Some(required_string(value, "disk_write_mode")?),
        )),
        "verify_piece_hashes" => Some(assign_if_changed(
            &mut working.verify_piece_hashes,
            Toggle::from(required_bool(value, "verify_piece_hashes")?),
        )),
        "cache_size" => Some(assign_if_changed(
            &mut working.cache_size,
            parse_optional_i32(value, "cache_size")?,
        )),
        "cache_expiry" => Some(assign_if_changed(
            &mut working.cache_expiry,
            parse_optional_i32(value, "cache_expiry")?,
        )),
        "coalesce_reads" => Some(assign_if_changed(
            &mut working.coalesce_reads,
            Toggle::from(required_bool(value, "coalesce_reads")?),
        )),
        "coalesce_writes" => Some(assign_if_changed(
            &mut working.coalesce_writes,
            Toggle::from(required_bool(value, "coalesce_writes")?),
        )),
        "use_disk_cache_pool" => Some(assign_if_changed(
            &mut working.use_disk_cache_pool,
            Toggle::from(required_bool(value, "use_disk_cache_pool")?),
        )),
        _ => None,
    };
    Ok(applied)
}

fn apply_toggle_field(
    working: &mut EngineProfile,
    key: &str,
    value: &Value,
) -> Result<Option<bool>, ConfigError> {
    let applied = match key {
        "enable_lsd" => Some(assign_if_changed(
            &mut working.enable_lsd,
            Toggle::from(required_bool(value, "enable_lsd")?),
        )),
        "enable_upnp" => Some(assign_if_changed(
            &mut working.enable_upnp,
            Toggle::from(required_bool(value, "enable_upnp")?),
        )),
        "enable_natpmp" => Some(assign_if_changed(
            &mut working.enable_natpmp,
            Toggle::from(required_bool(value, "enable_natpmp")?),
        )),
        "enable_pex" => Some(assign_if_changed(
            &mut working.enable_pex,
            Toggle::from(required_bool(value, "enable_pex")?),
        )),
        _ => None,
    };
    Ok(applied)
}

fn apply_dht_field(
    working: &mut EngineProfile,
    key: &str,
    value: &Value,
) -> Result<Option<bool>, ConfigError> {
    let applied = match key {
        "dht_bootstrap_nodes" => Some(assign_if_changed(
            &mut working.dht_bootstrap_nodes,
            parse_endpoint_array(value, "dht_bootstrap_nodes")?,
        )),
        "dht_router_nodes" => Some(assign_if_changed(
            &mut working.dht_router_nodes,
            parse_endpoint_array(value, "dht_router_nodes")?,
        )),
        _ => None,
    };
    Ok(applied)
}

fn apply_tracker_field(
    working: &mut EngineProfile,
    key: &str,
    value: &Value,
) -> Result<Option<bool>, ConfigError> {
    let applied = match key {
        "ip_filter" => Some(apply_ip_filter_field(working, value)?),
        "tracker" => {
            let normalized = normalize_tracker_payload(value)?;
            Some(assign_if_changed(&mut working.tracker, normalized))
        }
        _ => None,
    };
    Ok(applied)
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

fn parse_optional_non_negative_i64(value: &Value, field: &str) -> Result<Option<i64>, ConfigError> {
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
    if raw_value < 0 {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be non-negative".to_string(),
        });
    }
    Ok(Some(raw_value))
}

fn parse_optional_non_negative_f64(value: &Value, field: &str) -> Result<Option<f64>, ConfigError> {
    if value.is_null() {
        return Ok(None);
    }
    let Some(raw_value) = value.as_f64() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be a number".to_string(),
        });
    };
    if !raw_value.is_finite() || raw_value < 0.0 {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be a non-negative finite number".to_string(),
        });
    }
    Ok(Some(raw_value))
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

fn sanitize_outgoing_ports(
    start: Option<i32>,
    end: Option<i32>,
    warnings: &mut Vec<String>,
) -> Option<OutgoingPortRange> {
    match (start, end) {
        (None, None) => None,
        (Some(_), None) | (None, Some(_)) => {
            warnings.push(
                "outgoing_port_min/outgoing_port_max must both be set; ignoring range".to_string(),
            );
            None
        }
        (Some(min), Some(max)) => {
            if !(1..=65_535).contains(&min) || !(1..=65_535).contains(&max) || min > max {
                warnings.push(format!(
                    "invalid outgoing port range {min}..={max}; disabling override"
                ));
                None
            } else if let (Ok(start), Ok(end)) = (u16::try_from(min), u16::try_from(max)) {
                Some(OutgoingPortRange { start, end })
            } else {
                warnings.push(
                    "outgoing port range could not be normalized; disabling override".to_string(),
                );
                None
            }
        }
    }
}

fn sanitize_peer_dscp(value: Option<i32>, warnings: &mut Vec<String>) -> Option<u8> {
    value.and_then(|raw| {
        if (0..=63).contains(&raw) {
            u8::try_from(raw).ok()
        } else {
            warnings.push(format!(
                "peer_dscp {raw} is out of range 0-63; disabling marking"
            ));
            None
        }
    })
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

const fn storage_mode_label(mode: StorageMode) -> &'static str {
    match mode {
        StorageMode::Sparse => "sparse",
        StorageMode::Allocate => "allocate",
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
            | "request_timeout_ms" | "announce_to_all" | "ssl_cert" | "ssl_private_key"
            | "ssl_ca_cert" | "ssl_tracker_verify" | "proxy" | "auth" => {}
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
    let ssl_cert = parse_optional_string(map.get("ssl_cert"), "tracker.ssl_cert")?;
    let ssl_private_key =
        parse_optional_string(map.get("ssl_private_key"), "tracker.ssl_private_key")?;
    let ssl_ca_cert = parse_optional_string(map.get("ssl_ca_cert"), "tracker.ssl_ca_cert")?;
    let ssl_tracker_verify =
        parse_optional_bool(map.get("ssl_tracker_verify"), "tracker.ssl_tracker_verify")?
            .unwrap_or(true);
    let proxy = parse_proxy(map.get("proxy"))?;
    let auth = parse_auth(map.get("auth"))?;

    let config = TrackerConfig {
        default,
        extra,
        replace,
        user_agent,
        announce_ip,
        listen_interface,
        request_timeout_ms,
        announce_to_all,
        ssl_cert,
        ssl_private_key,
        ssl_ca_cert,
        ssl_tracker_verify,
        proxy,
        auth,
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

fn parse_auth(value: Option<&Value>) -> Result<Option<TrackerAuthConfig>, ConfigError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    let Some(map) = raw.as_object() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "tracker.auth".to_string(),
            message: "must be an object".to_string(),
        });
    };

    let username_secret =
        parse_optional_string(map.get("username_secret"), "tracker.auth.username_secret")?;
    let password_secret =
        parse_optional_string(map.get("password_secret"), "tracker.auth.password_secret")?;
    let cookie_secret =
        parse_optional_string(map.get("cookie_secret"), "tracker.auth.cookie_secret")?;

    if username_secret.is_none() && password_secret.is_none() && cookie_secret.is_none() {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "tracker.auth".to_string(),
            message: "must include at least one secret reference".to_string(),
        });
    }

    Ok(Some(TrackerAuthConfig {
        username_secret,
        password_secret,
        cookie_secret,
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

fn normalize_alt_speed_payload(value: &Value) -> Result<AltSpeedConfig, ConfigError> {
    if value.is_null() {
        return Ok(AltSpeedConfig::default());
    }
    let map = value.as_object().ok_or_else(|| ConfigError::InvalidField {
        section: "engine_profile".to_string(),
        field: "alt_speed".to_string(),
        message: "must be an object".to_string(),
    })?;

    for key in map.keys() {
        match key.as_str() {
            "download_bps" | "upload_bps" | "schedule" => {}
            other => {
                return Err(ConfigError::UnknownField {
                    section: "engine_profile".to_string(),
                    field: format!("alt_speed.{other}"),
                });
            }
        }
    }

    let download_bps = parse_optional_alt_rate(map.get("download_bps"), "alt_speed.download_bps")?;
    let upload_bps = parse_optional_alt_rate(map.get("upload_bps"), "alt_speed.upload_bps")?;
    let schedule = parse_alt_speed_schedule(map.get("schedule"))?;

    Ok(AltSpeedConfig {
        download_bps,
        upload_bps,
        schedule,
    })
}

fn parse_optional_alt_rate(value: Option<&Value>, field: &str) -> Result<Option<i64>, ConfigError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    raw.as_i64()
        .ok_or_else(|| ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be an integer".to_string(),
        })
        .map(Some)
}

fn parse_alt_speed_schedule(
    value: Option<&Value>,
) -> Result<Option<AltSpeedSchedule>, ConfigError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }

    let map = raw.as_object().ok_or_else(|| ConfigError::InvalidField {
        section: "engine_profile".to_string(),
        field: "alt_speed.schedule".to_string(),
        message: "must be an object with days/start/end".to_string(),
    })?;

    for key in map.keys() {
        match key.as_str() {
            "days" | "start" | "end" => {}
            other => {
                return Err(ConfigError::UnknownField {
                    section: "engine_profile".to_string(),
                    field: format!("alt_speed.schedule.{other}"),
                });
            }
        }
    }

    let days_value = map.get("days").ok_or_else(|| ConfigError::InvalidField {
        section: "engine_profile".to_string(),
        field: "alt_speed.schedule.days".to_string(),
        message: "is required and must list weekdays".to_string(),
    })?;
    let mut days = parse_weekday_array(days_value, "alt_speed.schedule.days")?;
    if days.is_empty() {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "alt_speed.schedule.days".to_string(),
            message: "must contain at least one weekday".to_string(),
        });
    }
    sort_weekdays(&mut days);

    let start_minutes = parse_time_minutes(map.get("start"), "alt_speed.schedule.start", "HH:MM")?;
    let end_minutes = parse_time_minutes(map.get("end"), "alt_speed.schedule.end", "HH:MM")?;

    if start_minutes == end_minutes {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "alt_speed.schedule".to_string(),
            message: "start and end times cannot be identical".to_string(),
        });
    }

    Ok(Some(AltSpeedSchedule {
        days,
        start_minutes,
        end_minutes,
    }))
}

fn parse_time_minutes(
    value: Option<&Value>,
    field: &str,
    expected: &str,
) -> Result<u16, ConfigError> {
    let Some(raw) = value else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "is required".to_string(),
        });
    };
    let Some(text) = raw.as_str() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be a string".to_string(),
        });
    };
    let trimmed = text.trim();
    let parsed =
        NaiveTime::parse_from_str(trimmed, "%H:%M").map_err(|_| ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: format!("must be formatted as {expected} (00:00-23:59)"),
        })?;
    let minutes = u16::try_from(parsed.hour() * 60 + parsed.minute())
        .unwrap_or(MINUTES_PER_DAY.saturating_sub(1));
    Ok(minutes)
}

fn parse_weekday_array(value: &Value, field: &str) -> Result<Vec<Weekday>, ConfigError> {
    let Some(array) = value.as_array() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be an array of weekday strings".to_string(),
        });
    };

    let mut seen = HashSet::new();
    let mut days = Vec::new();
    for entry in array {
        let Some(text) = entry.as_str() else {
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
        let Some(day) = parse_weekday(trimmed) else {
            return Err(ConfigError::InvalidField {
                section: "engine_profile".to_string(),
                field: field.to_string(),
                message: format!("unsupported weekday '{trimmed}'"),
            });
        };
        let key = weekday_label(day).to_string();
        if seen.insert(key) {
            days.push(day);
        }
    }
    Ok(days)
}

fn parse_weekday(label: &str) -> Option<Weekday> {
    match label.to_ascii_lowercase().as_str() {
        "mon" | "monday" => Some(Weekday::Mon),
        "tue" | "tues" | "tuesday" => Some(Weekday::Tue),
        "wed" | "wednesday" => Some(Weekday::Wed),
        "thu" | "thur" | "thurs" | "thursday" => Some(Weekday::Thu),
        "fri" | "friday" => Some(Weekday::Fri),
        "sat" | "saturday" => Some(Weekday::Sat),
        "sun" | "sunday" => Some(Weekday::Sun),
        _ => None,
    }
}

fn sort_weekdays(days: &mut [Weekday]) {
    days.sort_by_key(Weekday::number_from_monday);
}

const fn weekday_label(day: Weekday) -> &'static str {
    match day {
        Weekday::Mon => "mon",
        Weekday::Tue => "tue",
        Weekday::Wed => "wed",
        Weekday::Thu => "thu",
        Weekday::Fri => "fri",
        Weekday::Sat => "sat",
        Weekday::Sun => "sun",
    }
}

fn format_minutes(minutes: u16) -> String {
    let hours = minutes / 60;
    let mins = minutes % 60;
    format!("{hours:02}:{mins:02}")
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

fn sanitize_peer_classes(value: &Value, warnings: &mut Vec<String>) -> PeerClassesConfig {
    let Some(map) = value.as_object() else {
        warnings.push("peer_classes must be an object; ignoring payload".to_string());
        return PeerClassesConfig::default();
    };

    let classes = collect_peer_class_entries(map, warnings);
    let default = collect_default_peer_classes(map, &classes, warnings);

    PeerClassesConfig { classes, default }
}

fn collect_peer_class_entries(
    map: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> Vec<PeerClassConfig> {
    let mut classes = Vec::new();
    if let Some(entries) = map.get("classes").and_then(Value::as_array) {
        for entry in entries {
            if let Some(class) = parse_peer_class_entry(entry, warnings) {
                classes.push(class);
            }
        }
    }

    let mut seen = HashSet::new();
    classes.retain(|class| seen.insert(class.id));
    classes.sort_by_key(|class| class.id);
    classes
}

fn parse_peer_class_entry(entry: &Value, warnings: &mut Vec<String>) -> Option<PeerClassConfig> {
    let Some(obj) = entry.as_object() else {
        warnings.push("peer_classes.classes entry must be an object; skipping".to_string());
        return None;
    };
    let Some(id_value) = obj.get("id").and_then(Value::as_i64) else {
        warnings.push("peer_classes.classes entry missing id; skipping".to_string());
        return None;
    };
    if !(0..=31).contains(&id_value) {
        warnings.push(format!(
            "peer_classes id {id_value} out of range 0..31; skipping entry"
        ));
        return None;
    }
    let Ok(id) = u8::try_from(id_value) else {
        warnings.push(format!(
            "peer_classes id {id_value} could not be represented as u8; skipping entry"
        ));
        return None;
    };

    let label = obj
        .get("label")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let download_priority =
        clamp_priority(obj.get("download_priority"), "download_priority", warnings);
    let upload_priority = clamp_priority(obj.get("upload_priority"), "upload_priority", warnings);
    let connection_limit_factor =
        sanitize_connection_limit_factor(obj.get("connection_limit_factor"), warnings);
    let ignore_unchoke_slots = obj
        .get("ignore_unchoke_slots")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let label = if label.is_empty() {
        format!("class_{id}")
    } else {
        label
    };

    Some(PeerClassConfig {
        id,
        label,
        download_priority,
        upload_priority,
        connection_limit_factor,
        ignore_unchoke_slots,
    })
}

fn sanitize_connection_limit_factor(value: Option<&Value>, warnings: &mut Vec<String>) -> u16 {
    value
        .and_then(Value::as_i64)
        .map(|raw| raw.max(1))
        .map_or(100, |raw| {
            u16::try_from(raw).unwrap_or_else(|_| {
                warnings.push(format!(
                    "peer_classes.connection_limit_factor {raw} out of range; clamping to u16::MAX"
                ));
                u16::MAX
            })
        })
}

fn collect_default_peer_classes(
    map: &Map<String, Value>,
    classes: &[PeerClassConfig],
    warnings: &mut Vec<String>,
) -> Vec<u8> {
    let mut defaults = Vec::new();
    if let Some(entries) = map.get("default").and_then(Value::as_array) {
        for entry in entries {
            if let Some(id) = entry.as_i64() {
                if (0..=31).contains(&id) {
                    let Ok(id_u8) = u8::try_from(id) else {
                        warnings.push(format!(
                            "peer_classes.default entry {id} could not be represented as u8; skipping"
                        ));
                        continue;
                    };
                    if classes.iter().any(|class| class.id == id_u8) {
                        defaults.push(id_u8);
                    } else {
                        warnings.push(format!(
                            "peer_classes.default contains undefined id {id}; skipping"
                        ));
                    }
                } else {
                    warnings.push(format!(
                        "peer_classes.default contains invalid or undefined id {id}; skipping"
                    ));
                }
            }
        }
    }
    defaults.sort_unstable();
    defaults.dedup();
    defaults
}

fn clamp_priority(value: Option<&Value>, field: &str, warnings: &mut Vec<String>) -> u8 {
    match value.and_then(Value::as_i64) {
        Some(priority) if (1..=255).contains(&priority) => u8::try_from(priority).unwrap_or(1),
        Some(priority) => {
            warnings.push(format!(
                "peer_classes.{field} {priority} out of range 1..255; clamping to 1"
            ));
            1
        }
        None => 1,
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
    use serde_json::{Value, json};
    use std::collections::HashSet;
    use uuid::Uuid;

    fn sample_profile() -> EngineProfile {
        EngineProfile {
            id: Uuid::new_v4(),
            implementation: "libtorrent".into(),
            listen_port: Some(6_881),
            listen_interfaces: Vec::new(),
            ipv6_mode: "disabled".into(),
            anonymous_mode: false.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            outgoing_port_min: None,
            outgoing_port_max: None,
            peer_dscp: None,
            dht: true,
            encryption: "prefer".into(),
            max_active: Some(4),
            max_download_bps: Some(250_000),
            max_upload_bps: Some(125_000),
            seed_ratio_limit: None,
            seed_time_limit: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            stats_interval_ms: None,
            alt_speed: json!({}),
            sequential_default: false,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: false.into(),
            dont_count_slow_torrents: true.into(),
            super_seeding: false.into(),
            choking_algorithm: EngineProfile::default_choking_algorithm(),
            seed_choking_algorithm: EngineProfile::default_seed_choking_algorithm(),
            strict_super_seeding: false.into(),
            optimistic_unchoke_slots: None,
            max_queued_disk_bytes: None,
            resume_dir: "/tmp/resume".into(),
            download_root: "/tmp/downloads".into(),
            storage_mode: EngineProfile::default_storage_mode(),
            use_partfile: EngineProfile::default_use_partfile(),
            disk_read_mode: None,
            disk_write_mode: None,
            verify_piece_hashes: EngineProfile::default_verify_piece_hashes(),
            cache_size: None,
            cache_expiry: None,
            coalesce_reads: EngineProfile::default_coalesce_reads(),
            coalesce_writes: EngineProfile::default_coalesce_writes(),
            use_disk_cache_pool: EngineProfile::default_use_disk_cache_pool(),
            tracker: json!({}),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            ip_filter: json!({}),
            peer_classes: json!({}),
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
    fn privacy_flags_are_preserved() {
        let profile = sample_profile();
        let patch = json!({
            "anonymous_mode": true,
            "force_proxy": true,
            "prefer_rc4": true,
            "allow_multiple_connections_per_ip": true,
            "enable_outgoing_utp": true,
            "enable_incoming_utp": true
        });
        let mutation =
            validate_engine_profile_patch(&profile, &patch, &HashSet::new()).expect("valid patch");
        assert!(mutation.stored.anonymous_mode.is_enabled());
        assert!(mutation.stored.force_proxy.is_enabled());
        assert!(mutation.stored.prefer_rc4.is_enabled());
        assert!(
            mutation
                .stored
                .allow_multiple_connections_per_ip
                .is_enabled()
        );
        assert!(mutation.stored.enable_outgoing_utp.is_enabled());
        assert!(mutation.stored.enable_incoming_utp.is_enabled());
        assert!(mutation.effective.network.anonymous_mode.is_enabled());
        assert!(mutation.effective.network.force_proxy.is_enabled());
        assert!(mutation.effective.network.prefer_rc4.is_enabled());
        assert!(
            mutation
                .effective
                .network
                .allow_multiple_connections_per_ip
                .is_enabled()
        );
        assert!(mutation.effective.network.enable_outgoing_utp.is_enabled());
        assert!(mutation.effective.network.enable_incoming_utp.is_enabled());
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
    fn peer_classes_are_sanitized_and_sorted() {
        let profile = sample_profile();
        let patch = json!({
            "peer_classes": {
                "classes": [
                    { "id": 2, "label": "gold", "download_priority": 5, "upload_priority": 6, "connection_limit_factor": 150, "ignore_unchoke_slots": true },
                    { "id": 1, "label": " ", "download_priority": 300, "upload_priority": 0, "connection_limit_factor": 0 }
                ],
                "default": [2, 99, 1]
            }
        });
        let result = validate_engine_profile_patch(&profile, &patch, &HashSet::new())
            .expect("peer_classes patch valid");
        let effective = result.effective.peer_classes;
        assert_eq!(effective.classes.len(), 2);
        assert_eq!(effective.classes[0].id, 1);
        assert_eq!(effective.classes[0].label, "class_1");
        assert_eq!(effective.classes[0].download_priority, 1);
        assert_eq!(effective.classes[0].upload_priority, 1);
        assert_eq!(effective.classes[1].id, 2);
        assert!(effective.classes[1].ignore_unchoke_slots);
        assert_eq!(effective.default, vec![1, 2]);
    }

    #[test]
    fn queue_settings_are_honored() {
        let profile = sample_profile();
        let patch = json!({
            "auto_managed": false,
            "auto_manage_prefer_seeds": true,
            "dont_count_slow_torrents": false
        });

        let mutation =
            validate_engine_profile_patch(&profile, &patch, &HashSet::new()).expect("patch valid");
        assert!(!mutation.stored.auto_managed.is_enabled());
        assert!(mutation.stored.auto_manage_prefer_seeds.is_enabled());
        assert!(!mutation.stored.dont_count_slow_torrents.is_enabled());
        assert!(!mutation.effective.behavior.auto_managed.is_enabled());
        assert!(
            mutation
                .effective
                .behavior
                .auto_manage_prefer_seeds
                .is_enabled()
        );
        assert!(
            !mutation
                .effective
                .behavior
                .dont_count_slow_torrents
                .is_enabled()
        );
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
    fn tracker_auth_requires_secret_references() {
        let payload = json!({ "auth": {} });
        let err = normalize_tracker_payload(&payload).unwrap_err();
        assert!(err.to_string().contains("at least one secret reference"));

        let with_auth = json!({
            "auth": { "username_secret": "TRACKER_USER", "cookie_secret": "TRACKER_COOKIE" }
        });
        let normalized = normalize_tracker_payload(&with_auth).expect("valid tracker auth payload");
        let config: TrackerConfig =
            serde_json::from_value(normalized).expect("config should decode");
        let auth = config.auth.expect("auth present");
        assert_eq!(auth.username_secret.as_deref(), Some("TRACKER_USER"));
        assert_eq!(auth.cookie_secret.as_deref(), Some("TRACKER_COOKIE"));
        assert!(auth.password_secret.is_none());
    }

    #[test]
    fn tracker_ssl_fields_are_normalised() {
        let payload = json!({
            "ssl_cert": "/etc/certs/client.pem",
            "ssl_private_key": "/etc/certs/client.key",
            "ssl_ca_cert": "/etc/certs/ca.pem",
            "ssl_tracker_verify": false
        });

        let normalized = normalize_tracker_payload(&payload).expect("tracker payload");
        let config: TrackerConfig =
            serde_json::from_value(normalized).expect("config should decode");

        assert_eq!(config.ssl_cert.as_deref(), Some("/etc/certs/client.pem"));
        assert_eq!(
            config.ssl_private_key.as_deref(),
            Some("/etc/certs/client.key")
        );
        assert_eq!(config.ssl_ca_cert.as_deref(), Some("/etc/certs/ca.pem"));
        assert!(!config.ssl_tracker_verify);
    }

    #[test]
    fn tracker_ssl_verify_defaults_to_true() {
        let normalized = normalize_tracker_payload(&json!({})).expect("tracker payload defaults");
        let config: TrackerConfig =
            serde_json::from_value(normalized).expect("config should decode");
        assert!(config.ssl_tracker_verify);
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

    #[test]
    fn outgoing_ports_are_normalised_and_warn_on_invalid_ranges() {
        let mut profile = sample_profile();
        profile.outgoing_port_min = Some(6_000);
        profile.outgoing_port_max = Some(6_100);

        let effective = normalize_engine_profile(&profile);
        assert_eq!(
            effective.network.outgoing_ports,
            Some(OutgoingPortRange {
                start: 6_000,
                end: 6_100
            })
        );
        assert!(
            effective.warnings.is_empty(),
            "valid ranges should not emit warnings"
        );

        profile.outgoing_port_min = Some(70_000);
        profile.outgoing_port_max = Some(6_000);
        let clamped = normalize_engine_profile(&profile);
        assert!(clamped.network.outgoing_ports.is_none());
        assert!(
            clamped
                .warnings
                .iter()
                .any(|msg| msg.contains("outgoing port range")),
            "invalid ranges should be reported"
        );
    }

    #[test]
    fn peer_dscp_is_clamped_to_supported_values() {
        let mut profile = sample_profile();
        profile.peer_dscp = Some(8);

        let effective = normalize_engine_profile(&profile);
        assert_eq!(effective.network.peer_dscp, Some(8));

        profile.peer_dscp = Some(70);
        let clamped = normalize_engine_profile(&profile);
        assert!(clamped.network.peer_dscp.is_none());
        assert!(
            clamped.warnings.iter().any(|msg| msg.contains("peer_dscp")),
            "invalid values should be surfaced"
        );
    }

    #[test]
    fn connection_limits_are_sanitized() {
        let mut profile = sample_profile();
        profile.connections_limit = Some(400);
        profile.connections_limit_per_torrent = Some(80);
        profile.unchoke_slots = Some(0);
        profile.half_open_limit = Some(-5);
        profile.seed_ratio_limit = Some(-1.0);
        profile.seed_time_limit = Some(-10);

        let effective = normalize_engine_profile(&profile);
        assert_eq!(effective.limits.connections_limit, Some(400));
        assert_eq!(effective.limits.connections_limit_per_torrent, Some(80));
        assert!(effective.limits.unchoke_slots.is_none());
        assert!(effective.limits.half_open_limit.is_none());
        assert!(effective.limits.seed_ratio_limit.is_none());
        assert!(effective.limits.seed_time_limit.is_none());
        assert!(
            effective
                .warnings
                .iter()
                .any(|msg| msg.contains("unchoke_slots") && msg.contains("non-positive")),
            "non-positive unchoke slots should be reported"
        );
        assert!(
            effective
                .warnings
                .iter()
                .any(|msg| msg.contains("half_open_limit")),
            "non-positive half_open_limit should be reported"
        );
        assert!(
            effective
                .warnings
                .iter()
                .any(|msg| msg.contains("seed_ratio_limit")),
            "invalid seed_ratio_limit should be reported"
        );
        assert!(
            effective
                .warnings
                .iter()
                .any(|msg| msg.contains("seed_time_limit")),
            "invalid seed_time_limit should be reported"
        );
    }

    #[test]
    fn seed_limits_validate_and_preserve_positive_values() {
        let profile = sample_profile();
        let patch = json!({
            "seed_ratio_limit": 1.5,
            "seed_time_limit": 3600
        });
        let mutation =
            validate_engine_profile_patch(&profile, &patch, &HashSet::new()).expect("valid patch");
        assert_eq!(mutation.stored.seed_ratio_limit, Some(1.5));
        assert_eq!(mutation.stored.seed_time_limit, Some(3_600));
        assert_eq!(mutation.effective.limits.seed_ratio_limit, Some(1.5));
        assert_eq!(mutation.effective.limits.seed_time_limit, Some(3_600));
        assert!(mutation.effective.warnings.is_empty());
    }

    #[test]
    fn seed_limits_reject_negative_values() {
        let profile = sample_profile();
        let patch = json!({
            "seed_ratio_limit": -0.1,
            "seed_time_limit": -5
        });
        let err = validate_engine_profile_patch(&profile, &patch, &HashSet::new()).unwrap_err();
        assert!(
            err.to_string().contains("seed_ratio_limit")
                || err.to_string().contains("seed_time_limit")
        );
    }

    #[test]
    fn alt_speed_schedule_is_normalised() {
        let profile = sample_profile();
        let patch = json!({
            "alt_speed": {
                "download_bps": 1000,
                "upload_bps": 2000,
                "schedule": {
                    "days": ["sun", "Mon", "mon"],
                    "start": "22:15",
                    "end": "06:45"
                }
            }
        });

        let mutation =
            validate_engine_profile_patch(&profile, &patch, &HashSet::new()).expect("valid patch");
        assert!(mutation.mutated);
        let alt = mutation.effective.alt_speed;
        assert_eq!(alt.download_bps, Some(1_000));
        assert_eq!(alt.upload_bps, Some(2_000));
        let schedule = alt.schedule.expect("schedule present");
        assert_eq!(schedule.days, vec![Weekday::Mon, Weekday::Sun]);
        assert_eq!(schedule.start_minutes, 22 * 60 + 15);
        assert_eq!(schedule.end_minutes, 6 * 60 + 45);
        assert!(mutation.effective.warnings.is_empty());

        let stored_schedule = mutation
            .stored
            .alt_speed
            .get("schedule")
            .and_then(Value::as_object)
            .expect("schedule stored");
        assert_eq!(
            stored_schedule.get("start").and_then(Value::as_str),
            Some("22:15")
        );
        assert_eq!(
            stored_schedule.get("end").and_then(Value::as_str),
            Some("06:45")
        );
    }

    #[test]
    fn alt_speed_requires_schedule_and_caps() {
        let profile = sample_profile();
        let patch_missing_schedule = json!({
            "alt_speed": {
                "download_bps": 1000
            }
        });
        let mutation =
            validate_engine_profile_patch(&profile, &patch_missing_schedule, &HashSet::new())
                .expect("patch should validate");
        assert!(
            mutation.effective.alt_speed.schedule.is_none(),
            "missing schedule disables alt speed"
        );
        assert!(
            mutation
                .effective
                .warnings
                .iter()
                .any(|msg| msg.contains("alt_speed.schedule missing")),
            "missing schedule should warn"
        );

        let patch_missing_caps = json!({
            "alt_speed": {
                "schedule": {
                    "days": ["mon"],
                    "start": "10:00",
                    "end": "12:00"
                }
            }
        });
        let mutation =
            validate_engine_profile_patch(&profile, &patch_missing_caps, &HashSet::new())
                .expect("patch should validate");
        assert!(
            mutation.effective.alt_speed.schedule.is_none(),
            "missing caps should disable alt speed"
        );
        assert!(
            mutation
                .effective
                .warnings
                .iter()
                .any(|msg| msg.contains("alt_speed requires")),
            "missing caps should warn"
        );
    }

    #[test]
    fn alt_speed_rejects_invalid_schedule_shapes() {
        let profile = sample_profile();
        let patch = json!({
            "alt_speed": {
                "download_bps": 1000,
                "schedule": {
                    "days": ["tuesday"],
                    "start": "not-time",
                    "end": "10:00"
                }
            }
        });
        let err = validate_engine_profile_patch(&profile, &patch, &HashSet::new())
            .expect_err("invalid schedule should be rejected");
        assert!(err.to_string().contains("HH:MM"));

        let patch_bad_day = json!({
            "alt_speed": {
                "download_bps": 1000,
                "schedule": {
                    "days": ["funday"],
                    "start": "08:00",
                    "end": "12:00"
                }
            }
        });
        let err = validate_engine_profile_patch(&profile, &patch_bad_day, &HashSet::new())
            .expect_err("unsupported weekday should fail");
        assert!(err.to_string().contains("weekday"));
    }

    #[test]
    fn choking_fields_are_canonicalized() {
        let profile = sample_profile();
        let patch = json!({
            "choking_algorithm": "rate-based",
            "seed_choking_algorithm": "anti-leech",
            "strict_super_seeding": true,
            "optimistic_unchoke_slots": 7,
            "max_queued_disk_bytes": 123_456,
            "super_seeding": true
        });

        let mutation =
            validate_engine_profile_patch(&profile, &patch, &HashSet::new()).expect("patch valid");
        assert_eq!(mutation.stored.choking_algorithm, "rate_based");
        assert_eq!(mutation.stored.seed_choking_algorithm, "anti_leech");
        assert_eq!(mutation.stored.optimistic_unchoke_slots, Some(7));
        assert_eq!(mutation.stored.max_queued_disk_bytes, Some(123_456));
        assert!(bool::from(mutation.stored.strict_super_seeding));
        assert!(bool::from(mutation.stored.super_seeding));
        assert!(mutation.effective.warnings.is_empty());
    }

    #[test]
    fn choking_limits_clamp_and_warn() {
        let profile = sample_profile();
        let patch = json!({
            "choking_algorithm": "unknown",
            "seed_choking_algorithm": "unexpected",
            "optimistic_unchoke_slots": 0,
            "max_queued_disk_bytes": i64::from(i32::MAX) + 1000
        });

        let mutation =
            validate_engine_profile_patch(&profile, &patch, &HashSet::new()).expect("patch valid");
        assert_eq!(
            mutation.stored.choking_algorithm,
            EngineProfile::default_choking_algorithm()
        );
        assert_eq!(
            mutation.stored.seed_choking_algorithm,
            EngineProfile::default_seed_choking_algorithm()
        );
        assert_eq!(mutation.stored.optimistic_unchoke_slots, None);
        assert_eq!(
            mutation.stored.max_queued_disk_bytes,
            Some(i64::from(i32::MAX))
        );
        assert!(
            mutation
                .effective
                .warnings
                .iter()
                .any(|msg| msg.contains("choking_algorithm"))
        );
        assert!(
            mutation
                .effective
                .warnings
                .iter()
                .any(|msg| msg.contains("seed_choking_algorithm"))
        );
        assert!(
            mutation
                .effective
                .warnings
                .iter()
                .any(|msg| msg.contains("optimistic_unchoke_slots"))
        );
        assert!(
            mutation
                .effective
                .warnings
                .iter()
                .any(|msg| msg.contains("max_queued_disk_bytes"))
        );
    }
}
