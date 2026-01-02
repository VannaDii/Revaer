//! Engine profile validation and normalization helpers shared across the API and runtime paths.
//!
//! # Design
//! - Applies guard rails and normalizes stored values for runtime consumption.
//! - Surfaces an "effective" view with clamped values plus warnings for observability.

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use chrono::{DateTime, Utc, Weekday};
use serde::{Deserialize, Serialize};

use crate::error::ConfigError;
use crate::model::{EngineProfile, Toggle};

/// Upper bound guard rail for rate limits (≈5 Gbps).
pub const MAX_RATE_LIMIT_BPS: i64 = 5_000_000_000;
const DEFAULT_DOWNLOAD_ROOT: &str = ".server_root/downloads";
const DEFAULT_RESUME_DIR: &str = ".server_root/resume";
const DEFAULT_STORAGE_MODE: StorageMode = StorageMode::Sparse;
const MINUTES_PER_DAY: u16 = 24 * 60;
const ENGINE_SECTION: &str = "engine_profile";

fn engine_invalid_field(field: &str, value: Option<String>, reason: &'static str) -> ConfigError {
    ConfigError::InvalidField {
        section: ENGINE_SECTION.to_string(),
        field: field.to_string(),
        value,
        reason,
    }
}

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
    /// Tracker configuration payload (validated and normalized).
    pub tracker: TrackerConfig,
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

impl EngineIpv6Mode {
    #[must_use]
    /// String representation used for persistence and API payloads.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Enabled => "enabled",
            Self::PreferV6 => "prefer_v6",
        }
    }
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
    /// Optional disk cache expiry in seconds.
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

impl StorageMode {
    #[must_use]
    /// String representation used for persistence and API payloads.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sparse => "sparse",
            Self::Allocate => "allocate",
        }
    }
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

/// Behavioural toggles for per-torrent defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineBehaviorConfig {
    /// Whether torrents default to sequential download.
    pub sequential_default: bool,
    /// Whether torrents should be auto-managed by default.
    pub auto_managed: Toggle,
    /// Whether the queue manager should prefer seeds when assigning slots.
    pub auto_manage_prefer_seeds: Toggle,
    /// Whether slow torrents are exempt from active slot limits.
    pub dont_count_slow_torrents: Toggle,
    /// Whether torrents should default to super-seeding.
    pub super_seeding: Toggle,
}

/// Encryption policy applied to inbound/outbound peers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EngineEncryptionPolicy {
    /// Require encrypted peer connections.
    Require,
    /// Prefer encrypted peer connections.
    Prefer,
    /// Disable encrypted peer connections.
    Disable,
}

impl EngineEncryptionPolicy {
    #[must_use]
    /// String representation used for persistence and API payloads.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Require => "require",
            Self::Prefer => "prefer",
            Self::Disable => "disable",
        }
    }
}

/// Choking strategy used for downloads.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChokingAlgorithm {
    /// Fixed slot allocation strategy.
    FixedSlots,
    /// Rate-based allocation strategy.
    RateBased,
}

impl ChokingAlgorithm {
    #[must_use]
    /// String representation used for persistence and API payloads.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FixedSlots => "fixed_slots",
            Self::RateBased => "rate_based",
        }
    }
}

/// Choking strategy used while seeding.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SeedChokingAlgorithm {
    /// Round-robin seeding.
    RoundRobin,
    /// Prefer fastest uploaders.
    FastestUpload,
    /// Anti-leech strategy.
    AntiLeech,
}

impl SeedChokingAlgorithm {
    #[must_use]
    /// String representation used for persistence and API payloads.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RoundRobin => "round_robin",
            Self::FastestUpload => "fastest_upload",
            Self::AntiLeech => "anti_leech",
        }
    }
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

impl TrackerProxyType {
    #[must_use]
    /// String representation used for persistence and API payloads.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
            Self::Socks5 => "socks5",
        }
    }
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

impl TrackerConfig {
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
    let tracker_proxy_present = tracker.proxy.is_some();
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

fn sanitize_alt_speed(value: &AltSpeedConfig, warnings: &mut Vec<String>) -> AltSpeedConfig {
    let mut config = value.clone();
    config.download_bps = clamp_rate_limit("alt_speed.download_bps", config.download_bps, warnings);
    config.upload_bps = clamp_rate_limit("alt_speed.upload_bps", config.upload_bps, warnings);
    config.schedule = sanitize_alt_speed_schedule(config.schedule.take(), warnings);

    if config.schedule.is_none() {
        if config.has_caps() {
            warnings
                .push("alt_speed.schedule missing; alternate caps will not be applied".to_string());
        }
        AltSpeedConfig::default()
    } else if !config.has_caps() {
        warnings.push(
            "alt_speed requires download_bps or upload_bps; disabling alternate speeds".to_string(),
        );
        AltSpeedConfig::default()
    } else {
        config
    }
}

fn sanitize_alt_speed_schedule(
    schedule: Option<AltSpeedSchedule>,
    warnings: &mut Vec<String>,
) -> Option<AltSpeedSchedule> {
    let schedule = schedule?;

    if schedule.days.is_empty() {
        warnings.push("alt_speed.schedule.days empty; disabling schedule".to_string());
        return None;
    }

    let AltSpeedSchedule {
        mut days,
        start_minutes: raw_start_minutes,
        end_minutes: raw_end_minutes,
    } = schedule;
    let start_minutes = raw_start_minutes.min(MINUTES_PER_DAY - 1);
    let end_minutes = raw_end_minutes.min(MINUTES_PER_DAY - 1);
    if raw_start_minutes != start_minutes || raw_end_minutes != end_minutes {
        warnings.push("alt_speed.schedule times out of range; clamping".to_string());
    }

    if start_minutes == end_minutes {
        warnings.push("alt_speed.schedule start equals end; disabling schedule".to_string());
        return None;
    }

    days.sort_by_key(Weekday::number_from_monday);
    days.dedup();

    Some(AltSpeedSchedule {
        days,
        start_minutes,
        end_minutes,
    })
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

fn sanitize_stats_interval(
    value: Option<i64>,
    label: &str,
    warnings: &mut Vec<String>,
) -> Option<i32> {
    value.and_then(|raw| {
        if raw < 0 {
            warnings.push(format!("{label} {raw} is negative; disabling override"));
            None
        } else if raw > i64::from(i32::MAX) {
            warnings.push(format!("{label} {raw} exceeds max; clamping"));
            Some(i32::MAX)
        } else {
            i32::try_from(raw).ok()
        }
    })
}

fn sanitize_optimistic_unchoke_slots(
    value: Option<i32>,
    warnings: &mut Vec<String>,
) -> Option<i32> {
    match value {
        Some(v) if v > 0 => Some(v),
        Some(v) => {
            warnings.push(format!(
                "optimistic_unchoke_slots {v} is non-positive; disabling override"
            ));
            None
        }
        None => None,
    }
}

fn sanitize_max_queued_disk_bytes(value: Option<i64>, warnings: &mut Vec<String>) -> Option<i64> {
    value.and_then(|raw| {
        if raw < 0 {
            warnings.push(format!(
                "max_queued_disk_bytes {raw} is negative; disabling override"
            ));
            None
        } else {
            Some(raw)
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
    let storage_mode = canonical_storage_mode(&profile.storage_mode, warnings);
    let disk_read_mode = canonical_disk_mode(
        profile.disk_read_mode.as_deref(),
        "disk_read_mode",
        warnings,
    );
    let disk_write_mode = canonical_disk_mode(
        profile.disk_write_mode.as_deref(),
        "disk_write_mode",
        warnings,
    );

    EngineStorageConfig {
        download_root,
        resume_dir,
        storage_mode,
        use_partfile: profile.use_partfile,
        disk_read_mode,
        disk_write_mode,
        verify_piece_hashes: profile.verify_piece_hashes,
        cache_size: profile.cache_size,
        cache_expiry: profile.cache_expiry,
        coalesce_reads: profile.coalesce_reads,
        coalesce_writes: profile.coalesce_writes,
        use_disk_cache_pool: profile.use_disk_cache_pool,
    }
}

fn canonical_storage_mode(raw: &str, warnings: &mut Vec<String>) -> StorageMode {
    match raw.trim().to_ascii_lowercase().as_str() {
        "sparse" => StorageMode::Sparse,
        "allocate" => StorageMode::Allocate,
        other => {
            warnings.push(format!(
                "unknown storage_mode '{other}'; defaulting to sparse"
            ));
            DEFAULT_STORAGE_MODE
        }
    }
}

fn canonical_disk_mode(
    raw: Option<&str>,
    field: &str,
    warnings: &mut Vec<String>,
) -> Option<DiskIoMode> {
    let text = raw?;
    match text.trim().to_ascii_lowercase().as_str() {
        "enable_os_cache" => Some(DiskIoMode::EnableOsCache),
        "disable_os_cache" => Some(DiskIoMode::DisableOsCache),
        "write_through" => Some(DiskIoMode::WriteThrough),
        "" => None,
        other => {
            warnings.push(format!("unknown {field} '{other}'; ignoring"));
            None
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

fn sanitize_tracker(value: &TrackerConfig, warnings: &mut Vec<String>) -> TrackerConfig {
    let mut config = value.clone();
    config.default = sanitize_tracker_list(&config.default, "tracker.default", warnings);
    config.extra = sanitize_tracker_list(&config.extra, "tracker.extra", warnings);
    config.user_agent = sanitize_optional_string(
        config.user_agent.take(),
        "tracker.user_agent",
        warnings,
        255,
    );
    config.announce_ip = sanitize_optional_string(
        config.announce_ip.take(),
        "tracker.announce_ip",
        warnings,
        255,
    );
    config.listen_interface = sanitize_optional_string(
        config.listen_interface.take(),
        "tracker.listen_interface",
        warnings,
        255,
    );
    config.request_timeout_ms = sanitize_timeout(
        config.request_timeout_ms,
        "tracker.request_timeout_ms",
        warnings,
    );
    config.ssl_cert =
        sanitize_optional_string(config.ssl_cert.take(), "tracker.ssl_cert", warnings, 512);
    config.ssl_private_key = sanitize_optional_string(
        config.ssl_private_key.take(),
        "tracker.ssl_private_key",
        warnings,
        512,
    );
    config.ssl_ca_cert = sanitize_optional_string(
        config.ssl_ca_cert.take(),
        "tracker.ssl_ca_cert",
        warnings,
        512,
    );

    config.proxy = sanitize_tracker_proxy(config.proxy.take(), warnings);
    config.auth = sanitize_tracker_auth(config.auth.take(), warnings);

    config
}

fn sanitize_tracker_list(
    values: &[String],
    field: &str,
    warnings: &mut Vec<String>,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut trackers = Vec::new();
    for entry in values {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.len() > 512 {
            warnings.push(format!("{field} entry exceeds 512 characters; skipping"));
            continue;
        }
        let key = trimmed.to_ascii_lowercase();
        if seen.insert(key) {
            trackers.push(trimmed.to_string());
        }
    }
    trackers
}

fn sanitize_optional_string(
    value: Option<String>,
    field: &str,
    warnings: &mut Vec<String>,
    max_len: usize,
) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim().to_string();
        if trimmed.is_empty() {
            None
        } else if trimmed.len() > max_len {
            warnings.push(format!("{field} exceeds {max_len} characters; ignoring"));
            None
        } else {
            Some(trimmed)
        }
    })
}

fn sanitize_timeout(value: Option<i64>, field: &str, warnings: &mut Vec<String>) -> Option<i64> {
    value.and_then(|timeout| {
        if (0..=900_000).contains(&timeout) {
            Some(timeout)
        } else {
            warnings.push(format!("{field} must be between 0 and 900000 milliseconds"));
            None
        }
    })
}

fn sanitize_tracker_proxy(
    proxy: Option<TrackerProxyConfig>,
    warnings: &mut Vec<String>,
) -> Option<TrackerProxyConfig> {
    let proxy = proxy?;
    if proxy.host.trim().is_empty() {
        warnings.push("tracker.proxy.host is required; disabling proxy".to_string());
        return None;
    }
    if !(1..=65_535).contains(&proxy.port) {
        warnings
            .push("tracker.proxy.port must be between 1 and 65535; disabling proxy".to_string());
        return None;
    }
    Some(proxy)
}

fn sanitize_tracker_auth(
    auth: Option<TrackerAuthConfig>,
    warnings: &mut Vec<String>,
) -> Option<TrackerAuthConfig> {
    let auth = auth?;
    let has_secret = auth
        .username_secret
        .as_ref()
        .is_some_and(|value| !value.trim().is_empty())
        || auth
            .password_secret
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
        || auth
            .cookie_secret
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty());
    if !has_secret {
        warnings.push("tracker.auth requires at least one secret; disabling auth".to_string());
        return None;
    }
    Some(auth)
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

fn sanitize_endpoints(values: &[String], field: &str, warnings: &mut Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut endpoints = Vec::new();

    for entry in values {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.len() > 255 {
            warnings.push(format!("{field} entry exceeds 255 characters; skipping"));
            continue;
        }
        let key = trimmed.to_ascii_lowercase();
        if seen.insert(key) {
            endpoints.push(trimmed.to_string());
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

fn canonicalize_listen_interface(entry: &str, field: &str) -> Result<String, ConfigError> {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
        return Err(engine_invalid_field(
            field,
            Some(entry.to_string()),
            "entries cannot be empty",
        ));
    }
    if trimmed.contains(char::is_whitespace) {
        return Err(engine_invalid_field(
            field,
            Some(trimmed.to_string()),
            "entries cannot contain whitespace",
        ));
    }

    if let Some(stripped) = trimmed.strip_prefix('[') {
        let Some(closing) = stripped.find(']') else {
            return Err(engine_invalid_field(
                field,
                Some(trimmed.to_string()),
                "IPv6 entries must be bracketed like [::1]:6881",
            ));
        };
        let host = stripped[..closing].trim();
        let remainder = stripped.get(closing + 1..).unwrap_or("").trim();
        if host.is_empty() || !remainder.starts_with(':') {
            return Err(engine_invalid_field(
                field,
                Some(trimmed.to_string()),
                "IPv6 entries must be formatted as [addr]:port",
            ));
        }
        let port_text = remainder.trim_start_matches(':').trim();
        let port = port_text.parse::<i32>().map_err(|_| {
            engine_invalid_field(
                field,
                Some(port_text.to_string()),
                "port must be an integer between 1 and 65535",
            )
        })?;
        if !(1..=65_535).contains(&port) {
            return Err(engine_invalid_field(
                field,
                Some(port_text.to_string()),
                "port must be between 1 and 65535",
            ));
        }
        return Ok(format!("[{host}]:{port}"));
    }

    let Some((host, port_text)) = trimmed.rsplit_once(':') else {
        return Err(engine_invalid_field(
            field,
            Some(trimmed.to_string()),
            "entries must be host:port or [ipv6]:port",
        ));
    };
    let host = host.trim();
    if host.is_empty() {
        return Err(engine_invalid_field(
            field,
            Some(trimmed.to_string()),
            "host component cannot be empty",
        ));
    }
    let port = port_text.parse::<i32>().map_err(|_| {
        engine_invalid_field(
            field,
            Some(port_text.to_string()),
            "port must be an integer between 1 and 65535",
        )
    })?;
    if !(1..=65_535).contains(&port) {
        return Err(engine_invalid_field(
            field,
            Some(port_text.to_string()),
            "port must be between 1 and 65535",
        ));
    }
    Ok(format!("{host}:{port}"))
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

fn canonicalize_ipv6_mode(raw: &str, warnings: &mut Vec<String>) -> EngineIpv6Mode {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "disabled" | "disable" | "off" => EngineIpv6Mode::Disabled,
        "enabled" | "enable" | "on" | "v6" | "ipv6" => EngineIpv6Mode::Enabled,
        "prefer_v6" | "prefer-v6" | "prefer6" | "prefer" => EngineIpv6Mode::PreferV6,
        other => {
            warnings.push(format!(
                "unknown ipv6_mode '{other}'; defaulting to 'disabled'"
            ));
            EngineIpv6Mode::Disabled
        }
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

fn sanitize_ip_filter(value: &IpFilterConfig, warnings: &mut Vec<String>) -> IpFilterConfig {
    let mut config = value.clone();
    config.cidrs = sanitize_ip_filter_cidrs(&config.cidrs, warnings);
    if let Some(url) = &config.blocklist_url
        && url.trim().is_empty()
    {
        warnings.push("ip_filter.blocklist_url empty; clearing".to_string());
        config.blocklist_url = None;
    }
    config
}

fn sanitize_ip_filter_cidrs(values: &[String], warnings: &mut Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut cidrs = Vec::new();
    for entry in values {
        match canonicalize_ip_filter_entry(entry, "ip_filter.cidrs") {
            Ok((canonical, _)) => {
                let key = canonical.to_ascii_lowercase();
                if seen.insert(key) {
                    cidrs.push(canonical);
                }
            }
            Err(err) => warnings.push(err.to_string()),
        }
    }
    cidrs
}

fn sanitize_peer_classes(
    value: &PeerClassesConfig,
    warnings: &mut Vec<String>,
) -> PeerClassesConfig {
    let mut classes = Vec::new();
    for entry in &value.classes {
        if let Some(class) = sanitize_peer_class_entry(entry, warnings) {
            classes.push(class);
        }
    }
    let mut seen = HashSet::new();
    classes.retain(|class| seen.insert(class.id));
    classes.sort_by_key(|class| class.id);

    let mut defaults = Vec::new();
    for entry in &value.default {
        if classes.iter().any(|class| class.id == *entry) {
            defaults.push(*entry);
        } else {
            warnings.push(format!(
                "peer_classes.default contains undefined id {entry}; skipping"
            ));
        }
    }
    defaults.sort_unstable();
    defaults.dedup();

    PeerClassesConfig {
        classes,
        default: defaults,
    }
}

fn sanitize_peer_class_entry(
    entry: &PeerClassConfig,
    warnings: &mut Vec<String>,
) -> Option<PeerClassConfig> {
    if entry.id > 31 {
        warnings.push(format!(
            "peer_classes id {} out of range 0..31; skipping entry",
            entry.id
        ));
        return None;
    }

    let label = if entry.label.trim().is_empty() {
        format!("class_{}", entry.id)
    } else {
        entry.label.trim().to_string()
    };

    let download_priority = clamp_priority(entry.download_priority, "download_priority", warnings);
    let upload_priority = clamp_priority(entry.upload_priority, "upload_priority", warnings);
    let connection_limit_factor =
        sanitize_connection_limit_factor(entry.connection_limit_factor, warnings);

    Some(PeerClassConfig {
        id: entry.id,
        label,
        download_priority,
        upload_priority,
        connection_limit_factor,
        ignore_unchoke_slots: entry.ignore_unchoke_slots,
    })
}

fn sanitize_connection_limit_factor(value: u16, warnings: &mut Vec<String>) -> u16 {
    if value == 0 {
        warnings.push(
            "peer_classes.connection_limit_factor must be > 0; defaulting to 100".to_string(),
        );
        return 100;
    }
    if value > i16::MAX as u16 {
        warnings.push(format!(
            "peer_classes.connection_limit_factor {value} exceeds storage cap; clamping to {}",
            i16::MAX
        ));
        return i16::MAX as u16;
    }
    value
}

fn clamp_priority(value: u8, field: &str, warnings: &mut Vec<String>) -> u8 {
    if (1..=255).contains(&value) {
        value
    } else {
        warnings.push(format!(
            "peer_classes.{field} {value} out of range 1..255; clamping to 1"
        ));
        1
    }
}

fn clamp_rate_limit(field: &str, value: Option<i64>, warnings: &mut Vec<String>) -> Option<i64> {
    match value {
        Some(limit) if limit <= 0 => {
            warnings.push(format!(
                "{field} {limit} is non-positive; disabling override"
            ));
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

/// Canonicalize a single IP filter entry into a normalized CIDR and rule.
///
/// # Errors
/// Returns `ConfigError` when the entry cannot be parsed or the rule is invalid.
pub fn canonicalize_ip_filter_entry(
    entry: &str,
    field: &str,
) -> Result<(String, IpFilterRule), ConfigError> {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
        return Err(engine_invalid_field(
            field,
            Some(entry.to_string()),
            "CIDR entries cannot be empty",
        ));
    }

    let (network, prefix) = trimmed.split_once('/').ok_or_else(|| {
        engine_invalid_field(
            field,
            Some(trimmed.to_string()),
            "CIDR entries must include a /prefix",
        )
    })?;
    let network = network.trim();
    let prefix = prefix.trim();
    if network.is_empty() || prefix.is_empty() {
        return Err(engine_invalid_field(
            field,
            Some(trimmed.to_string()),
            "CIDR entries must include a network and prefix",
        ));
    }

    let parsed_ip: IpAddr = network.parse().map_err(|_| {
        engine_invalid_field(
            field,
            Some(network.to_string()),
            "CIDR entries must include a valid IP address",
        )
    })?;
    let prefix_len: u8 = prefix.parse().map_err(|_| {
        engine_invalid_field(
            field,
            Some(prefix.to_string()),
            "CIDR prefix must be numeric",
        )
    })?;

    let (canonical, rule) = match parsed_ip {
        IpAddr::V4(addr) => {
            if prefix_len > 32 {
                return Err(engine_invalid_field(
                    field,
                    Some(prefix_len.to_string()),
                    "IPv4 prefix must be <= 32",
                ));
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
                IpFilterRule { start, end },
            )
        }
        IpAddr::V6(addr) => {
            if prefix_len > 128 {
                return Err(engine_invalid_field(
                    field,
                    Some(prefix_len.to_string()),
                    "IPv6 prefix must be <= 128",
                ));
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
                IpFilterRule { start, end },
            )
        }
    };

    Ok((canonical, rule))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_ipv6_modes() {
        let mut warnings = Vec::new();
        assert_eq!(
            canonicalize_ipv6_mode("prefer_v6", &mut warnings),
            EngineIpv6Mode::PreferV6
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn rejects_invalid_cidr_prefix() {
        assert!(matches!(
            canonicalize_ip_filter_entry("10.0.0.0/40", "ip_filter.cidrs"),
            Err(ConfigError::InvalidField { .. })
        ));
    }

    #[test]
    fn canonicalize_ip_filter_entry_accepts_ipv4_and_ipv6() -> Result<(), ConfigError> {
        let (canonical, rule) = canonicalize_ip_filter_entry("10.0.0.0/24", "ip_filter.cidrs")?;
        assert_eq!(canonical, "10.0.0.0/24");
        assert_eq!(rule.start, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 0)));
        assert_eq!(rule.end, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 255)));

        let (canonical_v6, rule_v6) =
            canonicalize_ip_filter_entry("2001:db8::/64", "ip_filter.cidrs")?;
        assert_eq!(canonical_v6, "2001:db8::/64");
        assert_eq!(
            rule_v6.start,
            IpAddr::V6("2001:db8::".parse().expect("valid ipv6"))
        );
        assert!(matches!(rule_v6.end, IpAddr::V6(_)));
        Ok(())
    }

    #[test]
    fn listen_interfaces_normalize_and_warn() {
        let mut warnings = Vec::new();
        let values = vec![
            " ".to_string(),
            "host:6881".to_string(),
            "HOST:6881".to_string(),
            "host:70000".to_string(),
            "bad host:6881".to_string(),
            "missingport".to_string(),
            "[::1]:6881".to_string(),
        ];
        let normalized = sanitize_listen_interfaces(&values, "listen_interfaces", &mut warnings);
        assert_eq!(
            normalized,
            vec!["host:6881".to_string(), "[::1]:6881".to_string()]
        );
        assert!(!warnings.is_empty());
        let ipv6 = canonicalize_ipv6_mode("unknown", &mut warnings);
        assert_eq!(ipv6, EngineIpv6Mode::Disabled);
    }

    #[test]
    fn outgoing_ports_and_peer_dscp_clamp() {
        let mut warnings = Vec::new();
        assert!(sanitize_outgoing_ports(Some(5000), None, &mut warnings).is_none());
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("outgoing_port_min"))
        );
        assert!(sanitize_outgoing_ports(Some(70000), Some(1), &mut warnings).is_none());
        let range = sanitize_outgoing_ports(Some(1000), Some(2000), &mut warnings)
            .expect("expected valid outgoing range");
        assert_eq!(range.start, 1000);
        assert_eq!(range.end, 2000);
        assert!(sanitize_peer_dscp(Some(100), &mut warnings).is_none());
        assert_eq!(sanitize_peer_dscp(Some(42), &mut warnings), Some(42));
    }

    #[test]
    fn alt_speed_schedule_validates_days_and_times() {
        let mut warnings = Vec::new();
        let empty = AltSpeedSchedule {
            days: Vec::new(),
            start_minutes: 0,
            end_minutes: 0,
        };
        assert!(sanitize_alt_speed_schedule(Some(empty), &mut warnings).is_none());
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("days empty"))
        );

        warnings.clear();
        let schedule = AltSpeedSchedule {
            days: vec![Weekday::Mon],
            start_minutes: 2000,
            end_minutes: 2000,
        };
        assert!(sanitize_alt_speed_schedule(Some(schedule), &mut warnings).is_none());
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("out of range"))
        );
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("start equals end"))
        );
    }

    #[test]
    fn alt_speed_requires_caps_and_schedule() {
        let mut warnings = Vec::new();
        let caps_no_schedule = AltSpeedConfig {
            download_bps: Some(100),
            upload_bps: None,
            schedule: None,
        };
        let sanitized = sanitize_alt_speed(&caps_no_schedule, &mut warnings);
        assert_eq!(sanitized, AltSpeedConfig::default());
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("schedule missing"))
        );

        warnings.clear();
        let schedule_no_caps = AltSpeedConfig {
            download_bps: None,
            upload_bps: None,
            schedule: Some(AltSpeedSchedule {
                days: vec![Weekday::Tue],
                start_minutes: 10,
                end_minutes: 20,
            }),
        };
        let sanitized = sanitize_alt_speed(&schedule_no_caps, &mut warnings);
        assert_eq!(sanitized, AltSpeedConfig::default());
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("requires download_bps"))
        );
    }

    #[test]
    fn sanitize_endpoints_dedupes_and_skips_invalid() {
        let mut warnings = Vec::new();
        let entries = vec![
            String::new(),
            "tracker.example".to_string(),
            "TRACKER.EXAMPLE".to_string(),
            "x".repeat(300),
        ];
        let normalized = sanitize_endpoints(&entries, "tracker.endpoints", &mut warnings);
        assert_eq!(normalized, vec!["tracker.example".to_string()]);
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("exceeds 255 characters"))
        );
    }

    #[test]
    fn limit_sanitizers_clamp_and_warn() {
        let mut warnings = Vec::new();
        assert_eq!(
            sanitize_positive_limit(Some(5), "max_active", &mut warnings),
            Some(5)
        );
        assert!(sanitize_positive_limit(Some(0), "max_active", &mut warnings).is_none());
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("non-positive"))
        );

        warnings.clear();
        assert_eq!(
            sanitize_stats_interval(
                Some(i64::from(i32::MAX) + 5),
                "stats_interval_ms",
                &mut warnings
            ),
            Some(i32::MAX)
        );
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("exceeds max"))
        );
        warnings.clear();
        assert!(sanitize_stats_interval(Some(-1), "stats_interval_ms", &mut warnings).is_none());
        assert!(warnings.iter().any(|warning| warning.contains("negative")));

        warnings.clear();
        assert!(sanitize_seed_ratio_limit(Some(-1.0), &mut warnings).is_none());
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("seed_ratio_limit"))
        );
        warnings.clear();
        assert!(sanitize_seed_time_limit(Some(-5), &mut warnings).is_none());
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("seed_time_limit"))
        );
        warnings.clear();
        assert!(sanitize_max_queued_disk_bytes(Some(-1), &mut warnings).is_none());
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("max_queued_disk_bytes"))
        );
        warnings.clear();
        assert!(sanitize_optimistic_unchoke_slots(Some(0), &mut warnings).is_none());
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("optimistic_unchoke_slots"))
        );
    }

    #[test]
    fn canonical_algorithms_default_with_warning() {
        let mut warnings = Vec::new();
        assert_eq!(
            canonical_choking_algorithm("unknown", &mut warnings),
            ChokingAlgorithm::FixedSlots
        );
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("unknown choking_algorithm"))
        );
        warnings.clear();
        assert_eq!(
            canonical_seed_choking_algorithm("unknown", &mut warnings),
            SeedChokingAlgorithm::RoundRobin
        );
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("unknown seed_choking_algorithm"))
        );
    }
}
