//! Configuration schema migrations and helpers shared across crates.

use crate::error::{DataError, Result};
use chrono::{DateTime, Utc};
use sqlx::postgres::PgRow;
use sqlx::{Executor, FromRow, PgPool, Postgres, Row};
use uuid::Uuid;

/// LISTEN/NOTIFY channel for configuration revision broadcasts.
pub const SETTINGS_CHANNEL: &str = "revaer_settings_changed";

fn map_query_err(operation: &'static str) -> impl FnOnce(sqlx::Error) -> DataError {
    move |source| DataError::QueryFailed { operation, source }
}

/// Apply all configuration-related migrations (shared with runtime).
///
/// # Errors
///
/// Returns an error when migration execution fails.
pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    // Migrations cover configuration, tracker normalization, and peer class state.
    let mut migrator = sqlx::migrate!("./migrations");
    migrator.set_ignore_missing(true);
    migrator
        .run(pool)
        .await
        .map_err(|source| DataError::MigrationFailed { source })?;
    Ok(())
}

/// Raw projection of the `app_profile` table.
#[derive(Debug, Clone, FromRow)]
pub struct AppProfileRow {
    /// Primary key for the application profile row.
    pub id: Uuid,
    /// Friendly instance identifier.
    pub instance_name: String,
    /// Operational mode string (`setup` or `active`).
    pub mode: String,
    /// Monotonic revision number.
    pub version: i64,
    /// API HTTP port.
    pub http_port: i32,
    /// Bind address stored in the database.
    pub bind_addr: String,
    /// Telemetry log level override.
    pub telemetry_level: Option<String>,
    /// Telemetry log format override.
    pub telemetry_format: Option<String>,
    /// OpenTelemetry enablement flag.
    pub telemetry_otel_enabled: Option<bool>,
    /// OpenTelemetry service name override.
    pub telemetry_otel_service_name: Option<String>,
    /// OpenTelemetry endpoint override.
    pub telemetry_otel_endpoint: Option<String>,
    /// Immutable configuration keys.
    pub immutable_keys: Vec<String>,
}

/// Raw projection of label policy entries.
#[derive(Debug, Clone, FromRow)]
pub struct LabelPolicyRow {
    /// Label kind (`category` or `tag`).
    pub kind: String,
    /// Label name.
    pub name: String,
    /// Optional download directory override.
    pub download_dir: Option<String>,
    /// Optional download rate limit in bytes per second.
    pub rate_limit_download_bps: Option<i64>,
    /// Optional upload rate limit in bytes per second.
    pub rate_limit_upload_bps: Option<i64>,
    /// Optional queue position override.
    pub queue_position: Option<i32>,
    /// Optional auto-managed flag.
    pub auto_managed: Option<bool>,
    /// Optional seed ratio limit.
    pub seed_ratio_limit: Option<f64>,
    /// Optional seed time limit in seconds.
    pub seed_time_limit: Option<i64>,
    /// Optional cleanup seed ratio limit.
    pub cleanup_seed_ratio_limit: Option<f64>,
    /// Optional cleanup seed time limit.
    pub cleanup_seed_time_limit: Option<i64>,
    /// Optional cleanup remove data flag.
    pub cleanup_remove_data: Option<bool>,
}

/// NAT traversal and peer exchange toggles stored for an engine profile.
#[derive(Debug, Clone, Copy, Default)]
pub struct NatToggleSet {
    bits: u8,
}

impl NatToggleSet {
    const LSD: u8 = 0b0001;
    const UPNP: u8 = 0b0010;
    const NATPMP: u8 = 0b0100;
    const PEX: u8 = 0b1000;

    /// Construct a new toggle set from `[lsd, upnp, natpmp, pex]` flags.
    #[must_use]
    pub const fn from_flags(flags: [bool; 4]) -> Self {
        let mut bits = 0;
        if flags[0] {
            bits |= Self::LSD;
        }
        if flags[1] {
            bits |= Self::UPNP;
        }
        if flags[2] {
            bits |= Self::NATPMP;
        }
        if flags[3] {
            bits |= Self::PEX;
        }
        Self { bits }
    }

    /// Whether local service discovery (LSD) is enabled.
    #[must_use]
    pub const fn lsd(self) -> bool {
        self.bits & Self::LSD != 0
    }

    /// Whether `UPnP` port mapping is enabled.
    #[must_use]
    pub const fn upnp(self) -> bool {
        self.bits & Self::UPNP != 0
    }

    /// Whether NAT-PMP port mapping is enabled.
    #[must_use]
    pub const fn natpmp(self) -> bool {
        self.bits & Self::NATPMP != 0
    }

    /// Whether peer exchange (PEX) is enabled.
    #[must_use]
    pub const fn pex(self) -> bool {
        self.bits & Self::PEX != 0
    }
}

/// Privacy and transport toggles stored for an engine profile.
#[derive(Debug, Clone, Copy, Default)]
pub struct PrivacyToggleSet {
    bits: u8,
}

impl PrivacyToggleSet {
    const ANONYMOUS: u8 = 0b00_0001;
    const FORCE_PROXY: u8 = 0b00_0010;
    const PREFER_RC4: u8 = 0b00_0100;
    const MULTIPLE_CONNECTIONS_PER_IP: u8 = 0b00_1000;
    const OUTGOING_UTP: u8 = 0b01_0000;
    const INCOMING_UTP: u8 = 0b10_0000;

    /// Construct a new toggle set from `[anonymous, force_proxy, prefer_rc4, multiple_per_ip, outgoing_utp, incoming_utp]`.
    #[must_use]
    pub const fn from_flags(flags: [bool; 6]) -> Self {
        let mut bits = 0;
        if flags[0] {
            bits |= Self::ANONYMOUS;
        }
        if flags[1] {
            bits |= Self::FORCE_PROXY;
        }
        if flags[2] {
            bits |= Self::PREFER_RC4;
        }
        if flags[3] {
            bits |= Self::MULTIPLE_CONNECTIONS_PER_IP;
        }
        if flags[4] {
            bits |= Self::OUTGOING_UTP;
        }
        if flags[5] {
            bits |= Self::INCOMING_UTP;
        }
        Self { bits }
    }

    /// Whether anonymous mode is enabled.
    #[must_use]
    pub const fn anonymous_mode(self) -> bool {
        self.bits & Self::ANONYMOUS != 0
    }

    /// Whether peers must use a proxy.
    #[must_use]
    pub const fn force_proxy(self) -> bool {
        self.bits & Self::FORCE_PROXY != 0
    }

    /// Whether RC4 encryption is preferred.
    #[must_use]
    pub const fn prefer_rc4(self) -> bool {
        self.bits & Self::PREFER_RC4 != 0
    }

    /// Whether multiple connections per IP are allowed.
    #[must_use]
    pub const fn allow_multiple_connections_per_ip(self) -> bool {
        self.bits & Self::MULTIPLE_CONNECTIONS_PER_IP != 0
    }

    /// Whether outgoing uTP is enabled.
    #[must_use]
    pub const fn enable_outgoing_utp(self) -> bool {
        self.bits & Self::OUTGOING_UTP != 0
    }

    /// Whether incoming uTP is enabled.
    #[must_use]
    pub const fn enable_incoming_utp(self) -> bool {
        self.bits & Self::INCOMING_UTP != 0
    }
}

/// Queue policy toggles stored for an engine profile.
#[derive(Debug, Clone, Copy, Default)]
pub struct QueuePolicySet {
    bits: u8,
}

impl QueuePolicySet {
    const AUTO_MANAGED: u8 = 0b001;
    const PREFER_SEEDS: u8 = 0b010;
    const DONT_COUNT_SLOW: u8 = 0b100;

    /// Construct a new set from `[auto_managed, prefer_seeds, dont_count_slow]` flags.
    #[must_use]
    pub const fn from_flags(flags: [bool; 3]) -> Self {
        let mut bits = 0;
        if flags[0] {
            bits |= Self::AUTO_MANAGED;
        }
        if flags[1] {
            bits |= Self::PREFER_SEEDS;
        }
        if flags[2] {
            bits |= Self::DONT_COUNT_SLOW;
        }
        Self { bits }
    }

    /// Whether torrents are auto-managed by default.
    #[must_use]
    pub const fn auto_managed(self) -> bool {
        self.bits & Self::AUTO_MANAGED != 0
    }

    /// Whether queue management prefers seeds.
    #[must_use]
    pub const fn prefer_seeds(self) -> bool {
        self.bits & Self::PREFER_SEEDS != 0
    }

    /// Whether slow torrents are excluded from active slot accounting.
    #[must_use]
    pub const fn dont_count_slow(self) -> bool {
        self.bits & Self::DONT_COUNT_SLOW != 0
    }
}

/// Seeding-related toggles stored for an engine profile.
#[derive(Debug, Clone, Copy, Default)]
pub struct SeedingToggleSet {
    bits: u8,
}

impl SeedingToggleSet {
    const SEQUENTIAL: u8 = 0b001;
    const SUPER_SEEDING: u8 = 0b010;
    const STRICT_SUPER_SEEDING: u8 = 0b100;

    /// Construct a new set from `[sequential_default, super_seeding, strict_super_seeding]` flags.
    #[must_use]
    pub const fn from_flags(flags: [bool; 3]) -> Self {
        let mut bits = 0;
        if flags[0] {
            bits |= Self::SEQUENTIAL;
        }
        if flags[1] {
            bits |= Self::SUPER_SEEDING;
        }
        if flags[2] {
            bits |= Self::STRICT_SUPER_SEEDING;
        }
        Self { bits }
    }

    /// Whether sequential mode is the default.
    #[must_use]
    pub const fn sequential_default(self) -> bool {
        self.bits & Self::SEQUENTIAL != 0
    }

    /// Whether torrents default to super-seeding.
    #[must_use]
    pub const fn super_seeding(self) -> bool {
        self.bits & Self::SUPER_SEEDING != 0
    }

    /// Whether strict super-seeding is enabled.
    #[must_use]
    pub const fn strict_super_seeding(self) -> bool {
        self.bits & Self::STRICT_SUPER_SEEDING != 0
    }
}

/// Storage toggles stored for an engine profile.
#[derive(Debug, Clone, Copy, Default)]
pub struct StorageToggleSet {
    bits: u8,
}

impl StorageToggleSet {
    const USE_PARTFILE: u8 = 0b0001;
    const COALESCE_READS: u8 = 0b0010;
    const COALESCE_WRITES: u8 = 0b0100;
    const USE_DISK_CACHE_POOL: u8 = 0b1000;

    /// Construct a new set from `[use_partfile, coalesce_reads, coalesce_writes, use_disk_cache_pool]` flags.
    #[must_use]
    pub const fn from_flags(flags: [bool; 4]) -> Self {
        let mut bits = 0;
        if flags[0] {
            bits |= Self::USE_PARTFILE;
        }
        if flags[1] {
            bits |= Self::COALESCE_READS;
        }
        if flags[2] {
            bits |= Self::COALESCE_WRITES;
        }
        if flags[3] {
            bits |= Self::USE_DISK_CACHE_POOL;
        }
        Self { bits }
    }

    /// Whether partfiles should be used for incomplete pieces.
    #[must_use]
    pub const fn use_partfile(self) -> bool {
        self.bits & Self::USE_PARTFILE != 0
    }

    /// Whether disk reads should be coalesced.
    #[must_use]
    pub const fn coalesce_reads(self) -> bool {
        self.bits & Self::COALESCE_READS != 0
    }

    /// Whether disk writes should be coalesced.
    #[must_use]
    pub const fn coalesce_writes(self) -> bool {
        self.bits & Self::COALESCE_WRITES != 0
    }

    /// Whether the shared disk cache pool should be used.
    #[must_use]
    pub const fn use_disk_cache_pool(self) -> bool {
        self.bits & Self::USE_DISK_CACHE_POOL != 0
    }
}

/// Raw projection of the `engine_profile` table.
#[derive(Debug, Clone)]
pub struct EngineProfileRow {
    /// Primary key for the engine profile.
    pub id: Uuid,
    /// Engine implementation identifier.
    pub implementation: String,
    /// Optional listening port.
    pub listen_port: Option<i32>,
    /// DHT enablement flag.
    pub dht: bool,
    /// Encryption policy string.
    pub encryption: String,
    /// Optional active torrent limit.
    pub max_active: Option<i32>,
    /// Optional global download cap.
    pub max_download_bps: Option<i64>,
    /// Optional global upload cap.
    pub max_upload_bps: Option<i64>,
    /// Optional share ratio stop threshold.
    pub seed_ratio_limit: Option<f64>,
    /// Optional seeding time limit in seconds.
    pub seed_time_limit: Option<i64>,
    /// Queue policy toggles.
    pub queue: QueuePolicySet,
    /// Seeding behaviour toggles.
    pub seeding: SeedingToggleSet,
    /// Choking algorithm selection.
    pub choking_algorithm: String,
    /// Seed choking algorithm selection.
    pub seed_choking_algorithm: String,
    /// Optional optimistic unchoke slot override.
    pub optimistic_unchoke_slots: Option<i32>,
    /// Optional disk queue limit override.
    pub max_queued_disk_bytes: Option<i64>,
    /// Resume data directory.
    pub resume_dir: String,
    /// Download root directory.
    pub download_root: String,
    /// Storage allocation mode.
    pub storage_mode: String,
    /// Storage-related toggles.
    pub storage: StorageToggleSet,
    /// Optional disk read mode.
    pub disk_read_mode: Option<String>,
    /// Optional disk write mode.
    pub disk_write_mode: Option<String>,
    /// Whether piece hash verification is enabled.
    pub verify_piece_hashes: bool,
    /// Optional cache size in MiB.
    pub cache_size: Option<i32>,
    /// Optional cache expiry in seconds.
    pub cache_expiry: Option<i32>,
    /// Tracker user agent override.
    pub tracker_user_agent: Option<String>,
    /// Tracker announce IP override.
    pub tracker_announce_ip: Option<String>,
    /// Tracker listen interface override.
    pub tracker_listen_interface: Option<String>,
    /// Tracker request timeout in milliseconds.
    pub tracker_request_timeout_ms: Option<i32>,
    /// Whether to announce to all tiers.
    pub tracker_announce_to_all: Option<bool>,
    /// Whether to replace trackers on add.
    pub tracker_replace_trackers: Option<bool>,
    /// Tracker proxy host.
    pub tracker_proxy_host: Option<String>,
    /// Tracker proxy port.
    pub tracker_proxy_port: Option<i32>,
    /// Tracker proxy kind.
    pub tracker_proxy_kind: Option<String>,
    /// Secret name for tracker proxy username.
    pub tracker_proxy_username_secret: Option<String>,
    /// Secret name for tracker proxy password.
    pub tracker_proxy_password_secret: Option<String>,
    /// Secret name for tracker auth username.
    pub tracker_auth_username_secret: Option<String>,
    /// Secret name for tracker auth password.
    pub tracker_auth_password_secret: Option<String>,
    /// Secret name for tracker auth cookie.
    pub tracker_auth_cookie_secret: Option<String>,
    /// Optional client certificate path for tracker TLS.
    pub tracker_ssl_cert: Option<String>,
    /// Optional client private key path for tracker TLS.
    pub tracker_ssl_private_key: Option<String>,
    /// Optional CA certificate bundle path for tracker TLS.
    pub tracker_ssl_ca_cert: Option<String>,
    /// Whether to verify tracker TLS certificates.
    pub tracker_ssl_verify: Option<bool>,
    /// Whether to proxy peer connections.
    pub tracker_proxy_peers: Option<bool>,
    /// Default tracker endpoints.
    pub tracker_default_urls: Vec<String>,
    /// Extra tracker endpoints.
    pub tracker_extra_urls: Vec<String>,
    /// NAT traversal and PEX toggles.
    pub nat: NatToggleSet,
    /// DHT bootstrap nodes.
    pub dht_bootstrap_nodes: Vec<String>,
    /// DHT router nodes.
    pub dht_router_nodes: Vec<String>,
    /// IP filter blocklist URL.
    pub ip_filter_blocklist_url: Option<String>,
    /// IP filter blocklist `ETag`.
    pub ip_filter_etag: Option<String>,
    /// Timestamp of last successful blocklist refresh.
    pub ip_filter_last_updated_at: Option<DateTime<Utc>>,
    /// Last blocklist error.
    pub ip_filter_last_error: Option<String>,
    /// IP filter CIDR entries.
    pub ip_filter_cidrs: Vec<String>,
    /// Peer class IDs configured for the profile.
    pub peer_class_ids: Vec<i16>,
    /// Peer class labels.
    pub peer_class_labels: Vec<String>,
    /// Peer class download priorities.
    pub peer_class_download_priorities: Vec<i16>,
    /// Peer class upload priorities.
    pub peer_class_upload_priorities: Vec<i16>,
    /// Peer class connection limit factors.
    pub peer_class_connection_limit_factors: Vec<i16>,
    /// Peer class ignore-unchoke slot flags.
    pub peer_class_ignore_unchoke_slots: Vec<bool>,
    /// Peer class defaults list.
    pub peer_class_default_ids: Vec<i16>,
    /// Listen interface overrides.
    pub listen_interfaces: Vec<String>,
    /// IPv6 policy flag.
    pub ipv6_mode: String,
    /// Privacy and transport toggles.
    pub privacy: PrivacyToggleSet,
    /// Optional starting port for outgoing connections.
    pub outgoing_port_min: Option<i32>,
    /// Optional ending port for outgoing connections.
    pub outgoing_port_max: Option<i32>,
    /// Optional DSCP/TOS value applied to peer sockets.
    pub peer_dscp: Option<i32>,
    /// Optional global peer connection limit.
    pub connections_limit: Option<i32>,
    /// Optional per-torrent peer connection limit.
    pub connections_limit_per_torrent: Option<i32>,
    /// Optional unchoke slot limit.
    pub unchoke_slots: Option<i32>,
    /// Optional half-open connection limit.
    pub half_open_limit: Option<i32>,
    /// Alternate download cap in bytes per second.
    pub alt_speed_download_bps: Option<i64>,
    /// Alternate upload cap in bytes per second.
    pub alt_speed_upload_bps: Option<i64>,
    /// Alternate speed schedule start (minutes since midnight).
    pub alt_speed_schedule_start_minutes: Option<i32>,
    /// Alternate speed schedule end (minutes since midnight).
    pub alt_speed_schedule_end_minutes: Option<i32>,
    /// Alternate speed schedule days.
    pub alt_speed_days: Vec<String>,
    /// Optional stats interval in milliseconds for session alerts.
    pub stats_interval_ms: Option<i32>,
}

#[derive(Debug, Clone, Copy)]
struct EngineProfileToggleFlags {
    queue: QueuePolicySet,
    seeding: SeedingToggleSet,
    storage: StorageToggleSet,
    nat: NatToggleSet,
    privacy: PrivacyToggleSet,
}

impl EngineProfileToggleFlags {
    fn from_row(row: &PgRow) -> std::result::Result<Self, sqlx::Error> {
        Ok(Self {
            queue: QueuePolicySet::from_flags([
                row.try_get("auto_managed")?,
                row.try_get("auto_manage_prefer_seeds")?,
                row.try_get("dont_count_slow_torrents")?,
            ]),
            seeding: SeedingToggleSet::from_flags([
                row.try_get("sequential_default")?,
                row.try_get("super_seeding")?,
                row.try_get("strict_super_seeding")?,
            ]),
            storage: StorageToggleSet::from_flags([
                row.try_get("use_partfile")?,
                row.try_get("coalesce_reads")?,
                row.try_get("coalesce_writes")?,
                row.try_get("use_disk_cache_pool")?,
            ]),
            nat: NatToggleSet::from_flags([
                row.try_get("enable_lsd")?,
                row.try_get("enable_upnp")?,
                row.try_get("enable_natpmp")?,
                row.try_get("enable_pex")?,
            ]),
            privacy: PrivacyToggleSet::from_flags([
                row.try_get("anonymous_mode")?,
                row.try_get("force_proxy")?,
                row.try_get("prefer_rc4")?,
                row.try_get("allow_multiple_connections_per_ip")?,
                row.try_get("enable_outgoing_utp")?,
                row.try_get("enable_incoming_utp")?,
            ]),
        })
    }
}

macro_rules! engine_profile_row_from_row {
    ($row:ident, $toggles:ident) => {
        EngineProfileRow {
            id: $row.try_get("id")?,
            implementation: $row.try_get("implementation")?,
            listen_port: $row.try_get("listen_port")?,
            dht: $row.try_get("dht")?,
            encryption: $row.try_get("encryption")?,
            max_active: $row.try_get("max_active")?,
            max_download_bps: $row.try_get("max_download_bps")?,
            max_upload_bps: $row.try_get("max_upload_bps")?,
            seed_ratio_limit: $row.try_get("seed_ratio_limit")?,
            seed_time_limit: $row.try_get("seed_time_limit")?,
            queue: $toggles.queue,
            seeding: $toggles.seeding,
            choking_algorithm: $row.try_get("choking_algorithm")?,
            seed_choking_algorithm: $row.try_get("seed_choking_algorithm")?,
            optimistic_unchoke_slots: $row.try_get("optimistic_unchoke_slots")?,
            max_queued_disk_bytes: $row.try_get("max_queued_disk_bytes")?,
            resume_dir: $row.try_get("resume_dir")?,
            download_root: $row.try_get("download_root")?,
            storage_mode: $row.try_get("storage_mode")?,
            storage: $toggles.storage,
            disk_read_mode: $row.try_get("disk_read_mode")?,
            disk_write_mode: $row.try_get("disk_write_mode")?,
            verify_piece_hashes: $row.try_get("verify_piece_hashes")?,
            cache_size: $row.try_get("cache_size")?,
            cache_expiry: $row.try_get("cache_expiry")?,
            tracker_user_agent: $row.try_get("tracker_user_agent")?,
            tracker_announce_ip: $row.try_get("tracker_announce_ip")?,
            tracker_listen_interface: $row.try_get("tracker_listen_interface")?,
            tracker_request_timeout_ms: $row.try_get("tracker_request_timeout_ms")?,
            tracker_announce_to_all: $row.try_get("tracker_announce_to_all")?,
            tracker_replace_trackers: $row.try_get("tracker_replace_trackers")?,
            tracker_proxy_host: $row.try_get("tracker_proxy_host")?,
            tracker_proxy_port: $row.try_get("tracker_proxy_port")?,
            tracker_proxy_kind: $row.try_get("tracker_proxy_kind")?,
            tracker_proxy_username_secret: $row.try_get("tracker_proxy_username_secret")?,
            tracker_proxy_password_secret: $row.try_get("tracker_proxy_password_secret")?,
            tracker_auth_username_secret: $row.try_get("tracker_auth_username_secret")?,
            tracker_auth_password_secret: $row.try_get("tracker_auth_password_secret")?,
            tracker_auth_cookie_secret: $row.try_get("tracker_auth_cookie_secret")?,
            tracker_ssl_cert: $row.try_get("tracker_ssl_cert")?,
            tracker_ssl_private_key: $row.try_get("tracker_ssl_private_key")?,
            tracker_ssl_ca_cert: $row.try_get("tracker_ssl_ca_cert")?,
            tracker_ssl_verify: $row.try_get("tracker_ssl_verify")?,
            tracker_proxy_peers: $row.try_get("tracker_proxy_peers")?,
            tracker_default_urls: $row.try_get("tracker_default_urls")?,
            tracker_extra_urls: $row.try_get("tracker_extra_urls")?,
            nat: $toggles.nat,
            dht_bootstrap_nodes: $row.try_get("dht_bootstrap_nodes")?,
            dht_router_nodes: $row.try_get("dht_router_nodes")?,
            ip_filter_blocklist_url: $row.try_get("ip_filter_blocklist_url")?,
            ip_filter_etag: $row.try_get("ip_filter_etag")?,
            ip_filter_last_updated_at: $row.try_get("ip_filter_last_updated_at")?,
            ip_filter_last_error: $row.try_get("ip_filter_last_error")?,
            ip_filter_cidrs: $row.try_get("ip_filter_cidrs")?,
            peer_class_ids: $row.try_get("peer_class_ids")?,
            peer_class_labels: $row.try_get("peer_class_labels")?,
            peer_class_download_priorities: $row.try_get("peer_class_download_priorities")?,
            peer_class_upload_priorities: $row.try_get("peer_class_upload_priorities")?,
            peer_class_connection_limit_factors: $row
                .try_get("peer_class_connection_limit_factors")?,
            peer_class_ignore_unchoke_slots: $row.try_get("peer_class_ignore_unchoke_slots")?,
            peer_class_default_ids: $row.try_get("peer_class_default_ids")?,
            listen_interfaces: $row.try_get("listen_interfaces")?,
            ipv6_mode: $row.try_get("ipv6_mode")?,
            privacy: $toggles.privacy,
            outgoing_port_min: $row.try_get("outgoing_port_min")?,
            outgoing_port_max: $row.try_get("outgoing_port_max")?,
            peer_dscp: $row.try_get("peer_dscp")?,
            connections_limit: $row.try_get("connections_limit")?,
            connections_limit_per_torrent: $row.try_get("connections_limit_per_torrent")?,
            unchoke_slots: $row.try_get("unchoke_slots")?,
            half_open_limit: $row.try_get("half_open_limit")?,
            alt_speed_download_bps: $row.try_get("alt_speed_download_bps")?,
            alt_speed_upload_bps: $row.try_get("alt_speed_upload_bps")?,
            alt_speed_schedule_start_minutes: $row.try_get("alt_speed_schedule_start_minutes")?,
            alt_speed_schedule_end_minutes: $row.try_get("alt_speed_schedule_end_minutes")?,
            alt_speed_days: $row.try_get("alt_speed_days")?,
            stats_interval_ms: $row.try_get("stats_interval_ms")?,
        }
    };
}

impl<'r> FromRow<'r, PgRow> for EngineProfileRow {
    fn from_row(row: &'r PgRow) -> std::result::Result<Self, sqlx::Error> {
        let toggles = EngineProfileToggleFlags::from_row(row)?;
        Ok(engine_profile_row_from_row!(row, toggles))
    }
}

/// Raw projection of the `fs_policy` table.
#[derive(Debug, Clone, FromRow)]
pub struct FsPolicyRow {
    /// Primary key for the filesystem policy.
    pub id: Uuid,
    /// Root path for completed artifacts.
    pub library_root: String,
    /// Whether archives should be extracted.
    pub extract: bool,
    /// PAR2 verification policy.
    pub par2: String,
    /// Whether nested directory structures should be flattened.
    pub flatten: bool,
    /// Move mode string (`copy`, `move`, `hardlink`).
    pub move_mode: String,
    /// Paths to keep during cleanup.
    pub cleanup_keep: Vec<String>,
    /// Paths to drop during cleanup.
    pub cleanup_drop: Vec<String>,
    /// Optional chmod value for files.
    pub chmod_file: Option<String>,
    /// Optional chmod value for directories.
    pub chmod_dir: Option<String>,
    /// Optional owner override.
    pub owner: Option<String>,
    /// Optional group override.
    pub group: Option<String>,
    /// Optional umask override.
    pub umask: Option<String>,
    /// Allowed path prefixes.
    pub allow_paths: Vec<String>,
}

/// Raw projection of an active setup token.
#[derive(Debug, Clone, FromRow)]
pub struct ActiveTokenRow {
    /// Unique identifier for the token.
    pub id: Uuid,
    /// Hashed token string.
    pub token_hash: String,
    /// Expiration timestamp.
    pub expires_at: DateTime<Utc>,
}

/// Raw projection of an API key entry.
#[derive(Debug, Clone, FromRow)]
pub struct ApiKeyRow {
    /// Public key identifier.
    pub key_id: String,
    /// Optional human-readable label.
    pub label: Option<String>,
    /// Whether the key is currently enabled.
    pub enabled: bool,
    /// Optional expiration timestamp for the key.
    pub expires_at: Option<DateTime<Utc>>,
    /// Optional API key rate limit burst.
    pub rate_limit_burst: Option<i32>,
    /// Optional API key rate limit period in seconds.
    pub rate_limit_per_seconds: Option<i64>,
}

/// Raw projection used for API key auth.
#[derive(Debug, Clone, FromRow)]
pub struct ApiKeyAuthRow {
    /// Stored hash of the API key.
    pub hash: String,
    /// Whether the key is currently enabled.
    pub enabled: bool,
    /// Optional human-readable label.
    pub label: Option<String>,
    /// Optional expiration timestamp for the key.
    pub expires_at: Option<DateTime<Utc>>,
    /// Optional API key rate limit burst.
    pub rate_limit_burst: Option<i32>,
    /// Optional API key rate limit period in seconds.
    pub rate_limit_per_seconds: Option<i64>,
}

/// Raw projection of a stored secret.
#[derive(Debug, Clone, FromRow)]
pub struct SecretRow {
    /// Secret name.
    pub name: String,
    /// Encrypted secret bytes.
    pub ciphertext: Vec<u8>,
}

/// Input payload for inserting a setup token.
#[derive(Debug, Clone)]
pub struct NewSetupToken<'a> {
    /// Pre-hashed secret token.
    pub token_hash: &'a str,
    /// Expiration timestamp.
    pub expires_at: DateTime<Utc>,
    /// Issuer identity.
    pub issued_by: &'a str,
}

/// Input payload for inserting an API key.
#[derive(Debug, Clone)]
pub struct NewApiKey<'a> {
    /// Key identifier.
    pub key_id: &'a str,
    /// Hashed secret value.
    pub hash: &'a str,
    /// Optional human-readable label.
    pub label: Option<&'a str>,
    /// Whether the key is currently enabled.
    pub enabled: bool,
    /// Optional API key rate limit burst.
    pub burst: Option<i32>,
    /// Optional API key rate limit period in seconds.
    pub per_seconds: Option<i64>,
    /// Optional expiration timestamp for the key.
    pub expires_at: Option<DateTime<Utc>>,
}

/// Manually bump the configuration revision for a table that lacks triggers.
///
/// # Errors
///
/// Returns an error when the revision update or notification fails.
pub async fn bump_revision<'e, E>(executor: E, source_table: &str) -> Result<i64>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar("SELECT revaer_config.bump_revision(_source_table => $1)")
        .bind(source_table)
        .fetch_one(executor)
        .await
        .map_err(map_query_err("bump settings revision"))
}

/// Remove expired setup tokens that have not been consumed.
///
/// # Errors
///
/// Returns an error if the delete statement fails.
pub async fn cleanup_expired_setup_tokens<'e, E>(executor: E) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.cleanup_expired_setup_tokens()")
        .execute(executor)
        .await
        .map_err(map_query_err("cleanup expired setup tokens"))?;
    Ok(())
}

/// Mark all active setup tokens as consumed.
///
/// # Errors
///
/// Returns an error if the update statement fails.
pub async fn invalidate_active_setup_tokens<'e, E>(executor: E) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.invalidate_active_setup_tokens()")
        .execute(executor)
        .await
        .map_err(map_query_err("invalidate active setup tokens"))?;
    Ok(())
}

/// Insert a freshly issued setup token.
///
/// # Errors
///
/// Returns an error if the insert fails.
pub async fn insert_setup_token<'e, E>(executor: E, token: &NewSetupToken<'_>) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.insert_setup_token(_token_hash => $1, _expires_at => $2, _issued_by => $3)",
    )
        .bind(token.token_hash)
        .bind(token.expires_at)
        .bind(token.issued_by)
    .execute(executor)
    .await
    .map_err(map_query_err("insert setup token"))?;
    Ok(())
}

/// Mark a setup token as consumed (either expired or actively used).
///
/// # Errors
///
/// Returns an error if the update fails.
pub async fn mark_setup_token_consumed<'e, E>(executor: E, token_id: Uuid) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.consume_setup_token(_token_id => $1)")
        .bind(token_id)
        .execute(executor)
        .await
        .map_err(map_query_err("consume setup token"))?;
    Ok(())
}

/// Perform a factory reset of configuration + runtime tables.
///
/// # Errors
///
/// Returns an error if the reset procedure fails to execute.
pub async fn factory_reset<'e, E>(executor: E) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.factory_reset()")
        .execute(executor)
        .await
        .map_err(map_query_err("execute factory reset"))?;
    Ok(())
}

/// Load a secret row by name.
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn fetch_secret_by_name<'e, E>(executor: E, name: &str) -> Result<Option<SecretRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, SecretRow>("SELECT * FROM revaer_config.fetch_secret_by_name(_name => $1)")
        .bind(name)
        .fetch_optional(executor)
        .await
        .map_err(map_query_err("fetch secret by name"))
}

/// String columns on `fs_policy` that store textual values.
#[derive(Debug, Clone, Copy)]
pub enum FsStringField {
    /// Destination root.
    LibraryRoot,
    /// PAR2 policy.
    Par2,
    /// Move mode.
    MoveMode,
}

impl FsStringField {
    const fn column(self) -> &'static str {
        match self {
            Self::LibraryRoot => "library_root",
            Self::Par2 => "par2",
            Self::MoveMode => "move_mode",
        }
    }
}

/// Boolean columns on `fs_policy`.
#[derive(Debug, Clone, Copy)]
pub enum FsBooleanField {
    /// Extract flag.
    Extract,
    /// Flatten flag.
    Flatten,
}

impl FsBooleanField {
    const fn column(self) -> &'static str {
        match self {
            Self::Extract => "extract",
            Self::Flatten => "flatten",
        }
    }
}

/// Array/JSON columns on `fs_policy`.
#[derive(Debug, Clone, Copy)]
pub enum FsArrayField {
    /// Cleanup keep patterns.
    CleanupKeep,
    /// Cleanup drop patterns.
    CleanupDrop,
    /// Allowed paths array.
    AllowPaths,
}

impl FsArrayField {
    const fn column(self) -> &'static str {
        match self {
            Self::CleanupKeep => "cleanup_keep",
            Self::CleanupDrop => "cleanup_drop",
            Self::AllowPaths => "allow_paths",
        }
    }
}

/// Optional text columns on `fs_policy`.
#[derive(Debug, Clone, Copy)]
pub enum FsOptionalStringField {
    /// File chmod field.
    ChmodFile,
    /// Directory chmod field.
    ChmodDir,
    /// Owner column.
    Owner,
    /// Group column.
    Group,
    /// Umask column.
    Umask,
}

impl FsOptionalStringField {
    const fn column(self) -> &'static str {
        match self {
            Self::ChmodFile => "chmod_file",
            Self::ChmodDir => "chmod_dir",
            Self::Owner => "owner",
            Self::Group => "group",
            Self::Umask => "umask",
        }
    }
}

/// Load the application profile row for the provided identifier.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_app_profile_row<'e, E>(executor: E, id: Uuid) -> Result<AppProfileRow>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, AppProfileRow>(
        "SELECT * FROM revaer_config.fetch_app_profile_row(_id => $1)",
    )
    .bind(id)
    .fetch_one(executor)
    .await
    .map_err(map_query_err("fetch app profile row"))
}

/// Load label policies for the application profile.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_app_label_policies<'e, E>(
    executor: E,
    profile_id: Uuid,
) -> Result<Vec<LabelPolicyRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, LabelPolicyRow>(
        "SELECT * FROM revaer_config.list_app_label_policies(_profile_id => $1)",
    )
    .bind(profile_id)
    .fetch_all(executor)
    .await
    .map_err(map_query_err("list app label policies"))
}

/// Load the engine profile row for the provided identifier.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_engine_profile_row<'e, E>(executor: E, id: Uuid) -> Result<EngineProfileRow>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, EngineProfileRow>(
        "SELECT * FROM revaer_config.fetch_engine_profile_row(_id => $1)",
    )
    .bind(id)
    .fetch_one(executor)
    .await
    .map_err(map_query_err("fetch engine profile row"))
}

/// Load the filesystem policy row for the provided identifier.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_fs_policy_row<'e, E>(executor: E, id: Uuid) -> Result<FsPolicyRow>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, FsPolicyRow>("SELECT * FROM revaer_config.fetch_fs_policy_row(_id => $1)")
        .bind(id)
        .fetch_one(executor)
        .await
        .map_err(map_query_err("fetch fs policy row"))
}

/// Fetch the monotonic configuration revision.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_revision<'e, E>(executor: E) -> Result<i64>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar("SELECT revaer_config.fetch_revision()")
        .fetch_one(executor)
        .await
        .map_err(map_query_err("fetch settings revision"))
}

/// Fetch API keys for configuration reads.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_api_keys<'e, E>(executor: E) -> Result<Vec<ApiKeyRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, ApiKeyRow>("SELECT * FROM revaer_config.fetch_api_keys()")
        .fetch_all(executor)
        .await
        .map_err(map_query_err("fetch api keys"))
}

/// Fetch a single active setup token row (if any) for the caller.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_active_setup_token<'e, E>(executor: E) -> Result<Option<ActiveTokenRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, ActiveTokenRow>("SELECT * FROM revaer_config.fetch_active_setup_token()")
        .fetch_optional(executor)
        .await
        .map_err(map_query_err("fetch active setup token"))
}

/// Fetch the API key authentication material for a given key identifier.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_api_key_auth<'e, E>(executor: E, key_id: &str) -> Result<Option<ApiKeyAuthRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, ApiKeyAuthRow>(
        "SELECT * FROM revaer_config.fetch_api_key_auth(_key_id => $1)",
    )
    .bind(key_id)
    .fetch_optional(executor)
    .await
    .map_err(map_query_err("fetch api key auth"))
}

/// Fetch the hashed secret for a given API key.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_api_key_hash<'e, E>(executor: E, key_id: &str) -> Result<Option<String>>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT revaer_config.fetch_api_key_hash(_key_id => $1)",
    )
    .bind(key_id)
    .fetch_one(executor)
    .await
    .map_err(map_query_err("fetch api key hash"))
}

/// Delete an API key.
///
/// # Errors
///
/// Returns an error when the deletion fails.
pub async fn delete_api_key<'e, E>(executor: E, key_id: &str) -> Result<u64>
where
    E: Executor<'e, Database = Postgres>,
{
    let removed =
        sqlx::query_scalar::<_, i64>("SELECT revaer_config.delete_api_key(_key_id => $1)")
            .bind(key_id)
            .fetch_one(executor)
            .await
            .map_err(map_query_err("delete api key"))?;
    Ok(u64::try_from(removed).unwrap_or_default())
}

/// Update the hashed secret for an API key.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_api_key_hash<'e, E>(executor: E, key_id: &str, hash: &str) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_api_key_hash(_key_id => $1, _hash => $2)")
        .bind(key_id)
        .bind(hash)
        .execute(executor)
        .await
        .map_err(map_query_err("update api key hash"))?;
    Ok(())
}

/// Update the label for an API key.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_api_key_label<'e, E>(
    executor: E,
    key_id: &str,
    label: Option<&str>,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_api_key_label(_key_id => $1, _label => $2)")
        .bind(key_id)
        .bind(label)
        .execute(executor)
        .await
        .map_err(map_query_err("update api key label"))?;
    Ok(())
}

/// Update the enabled flag for an API key.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_api_key_enabled<'e, E>(executor: E, key_id: &str, enabled: bool) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_api_key_enabled(_key_id => $1, _enabled => $2)")
        .bind(key_id)
        .bind(enabled)
        .execute(executor)
        .await
        .map_err(map_query_err("update api key enabled"))?;
    Ok(())
}

/// Update the `rate_limit` column for an API key.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_api_key_rate_limit<'e, E>(
    executor: E,
    key_id: &str,
    burst: Option<i32>,
    per_seconds: Option<i64>,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.update_api_key_rate_limit(_key_id => $1, _burst => $2, _per_seconds => $3)",
    )
        .bind(key_id)
        .bind(burst)
        .bind(per_seconds)
        .execute(executor)
        .await
        .map_err(map_query_err("update api key rate limit"))?;
    Ok(())
}

/// Insert a new API key.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn insert_api_key<'e, E>(executor: E, new_key: &NewApiKey<'_>) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.insert_api_key(_key_id => $1, _hash => $2, _label => $3, _enabled => $4, _burst => $5, _per_seconds => $6, _expires_at => $7)",
    )
    .bind(new_key.key_id)
    .bind(new_key.hash)
    .bind(new_key.label)
    .bind(new_key.enabled)
    .bind(new_key.burst)
    .bind(new_key.per_seconds)
    .bind(new_key.expires_at)
    .execute(executor)
    .await
    .map_err(map_query_err("insert api key"))?;
    Ok(())
}

/// Update API key expiration.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_api_key_expires_at<'e, E>(
    executor: E,
    key_id: &str,
    expires_at: Option<DateTime<Utc>>,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_api_key_expires_at(_key_id => $1, _expires_at => $2)")
        .bind(key_id)
        .bind(expires_at)
        .execute(executor)
        .await
        .map_err(map_query_err("update api key expires at"))?;
    Ok(())
}

/// Upsert a value in `settings_secret`.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn upsert_secret<'e, E>(
    executor: E,
    name: &str,
    ciphertext: &[u8],
    actor: &str,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.upsert_secret(_name => $1, _ciphertext => $2, _actor => $3)")
        .bind(name)
        .bind(ciphertext)
        .bind(actor)
        .execute(executor)
        .await
        .map_err(map_query_err("upsert secret"))?;
    Ok(())
}

/// Delete a secret entry.
///
/// # Errors
///
/// Returns an error when the delete fails.
pub async fn delete_secret<'e, E>(executor: E, name: &str) -> Result<u64>
where
    E: Executor<'e, Database = Postgres>,
{
    let removed = sqlx::query_scalar::<_, i64>("SELECT revaer_config.delete_secret(_name => $1)")
        .bind(name)
        .fetch_one(executor)
        .await
        .map_err(map_query_err("delete secret"))?;
    Ok(u64::try_from(removed).unwrap_or_default())
}

/// Update the application `instance_name` field.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_app_instance_name<'e, E>(
    executor: E,
    id: Uuid,
    instance_name: &str,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_app_instance_name(_id => $1, _instance_name => $2)")
        .bind(id)
        .bind(instance_name)
        .execute(executor)
        .await
        .map_err(map_query_err("update app instance name"))?;
    Ok(())
}

/// Update the application mode field.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_app_mode<'e, E>(executor: E, id: Uuid, mode: &str) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_app_mode(_id => $1, _mode => $2)")
        .bind(id)
        .bind(mode)
        .execute(executor)
        .await
        .map_err(map_query_err("update app mode"))?;
    Ok(())
}

/// Update the application HTTP port field.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_app_http_port<'e, E>(executor: E, id: Uuid, http_port: i32) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_app_http_port(_id => $1, _port => $2)")
        .bind(id)
        .bind(http_port)
        .execute(executor)
        .await
        .map_err(map_query_err("update app http port"))?;
    Ok(())
}

/// Update the application bind address field.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_app_bind_addr<'e, E>(executor: E, id: Uuid, bind_addr: &str) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_app_bind_addr(_id => $1, _bind_addr => $2)")
        .bind(id)
        .bind(bind_addr)
        .execute(executor)
        .await
        .map_err(map_query_err("update app bind addr"))?;
    Ok(())
}

/// Update the application telemetry fields.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_app_telemetry<'e, E>(
    executor: E,
    id: Uuid,
    level: Option<&str>,
    format: Option<&str>,
    otel_enabled: Option<bool>,
    otel_service_name: Option<&str>,
    otel_endpoint: Option<&str>,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.update_app_telemetry(_id => $1, _level => $2, _format => $3, _otel_enabled => $4, _otel_service_name => $5, _otel_endpoint => $6)",
    )
        .bind(id)
        .bind(level)
        .bind(format)
        .bind(otel_enabled)
        .bind(otel_service_name)
        .bind(otel_endpoint)
        .execute(executor)
        .await
        .map_err(map_query_err("update app telemetry"))?;
    Ok(())
}

/// Input payload for replacing application label policies.
#[derive(Debug, Clone)]
pub struct AppLabelPoliciesUpdate<'a> {
    /// Label kinds.
    pub kinds: &'a [String],
    /// Label names.
    pub names: &'a [String],
    /// Optional download directories.
    pub download_dirs: &'a [Option<String>],
    /// Optional download rate limits in bytes per second.
    pub rate_limit_download_bps: &'a [Option<i64>],
    /// Optional upload rate limits in bytes per second.
    pub rate_limit_upload_bps: &'a [Option<i64>],
    /// Optional queue positions.
    pub queue_positions: &'a [Option<i32>],
    /// Optional auto-managed flags.
    pub auto_managed: &'a [Option<bool>],
    /// Optional seed ratio limits.
    pub seed_ratio_limits: &'a [Option<f64>],
    /// Optional seed time limits in seconds.
    pub seed_time_limits: &'a [Option<i64>],
    /// Optional cleanup seed ratio limits.
    pub cleanup_seed_ratio_limits: &'a [Option<f64>],
    /// Optional cleanup seed time limits in seconds.
    pub cleanup_seed_time_limits: &'a [Option<i64>],
    /// Optional cleanup remove data flags.
    pub cleanup_remove_data: &'a [Option<bool>],
}

/// Replace the application label policies.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn replace_app_label_policies<'e, E>(
    executor: E,
    id: Uuid,
    update: &AppLabelPoliciesUpdate<'_>,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.replace_app_label_policies(_profile_id => $1, _kinds => $2, _names => $3, _download_dirs => $4, _rate_limit_download_bps => $5, _rate_limit_upload_bps => $6, _queue_positions => $7, _auto_managed => $8, _seed_ratio_limits => $9, _seed_time_limits => $10, _cleanup_seed_ratio_limits => $11, _cleanup_seed_time_limits => $12, _cleanup_remove_data => $13)",
    )
        .bind(id)
        .bind(update.kinds)
        .bind(update.names)
        .bind(update.download_dirs)
        .bind(update.rate_limit_download_bps)
        .bind(update.rate_limit_upload_bps)
        .bind(update.queue_positions)
        .bind(update.auto_managed)
        .bind(update.seed_ratio_limits)
        .bind(update.seed_time_limits)
        .bind(update.cleanup_seed_ratio_limits)
        .bind(update.cleanup_seed_time_limits)
        .bind(update.cleanup_remove_data)
        .execute(executor)
        .await
        .map_err(map_query_err("replace app label policies"))?;
    Ok(())
}

/// Update the application immutable keys list.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_app_immutable_keys<'e, E>(
    executor: E,
    id: Uuid,
    immutable_keys: &[String],
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_app_immutable_keys(_id => $1, _keys => $2)")
        .bind(id)
        .bind(immutable_keys)
        .execute(executor)
        .await
        .map_err(map_query_err("update app immutable keys"))?;
    Ok(())
}

/// Increment the application profile version for optimistic locking.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn bump_app_profile_version<'e, E>(executor: E, id: Uuid) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.bump_app_profile_version(_id => $1)")
        .bind(id)
        .execute(executor)
        .await
        .map_err(map_query_err("bump app profile version"))?;
    Ok(())
}

/// Tracker announce policy update payload.
#[derive(Debug, Clone)]
pub struct TrackerAnnouncePolicy {
    /// Whether to announce to all tiers.
    pub announce_to_all: bool,
    /// Whether to replace trackers on add.
    pub replace_trackers: bool,
}

/// Tracker proxy policy update payload.
#[derive(Debug, Clone)]
pub struct TrackerProxyPolicy {
    /// Whether to proxy peer connections.
    pub proxy_peers: bool,
}

/// Tracker TLS policy update payload.
#[derive(Debug, Clone)]
pub struct TrackerTlsPolicy {
    /// Whether to verify tracker TLS certificates.
    pub verify: bool,
}

/// Tracker configuration update payload used for persistence.
#[derive(Debug, Clone)]
pub struct TrackerConfigUpdate<'a> {
    /// Tracker user agent override.
    pub user_agent: Option<&'a str>,
    /// Tracker announce IP override.
    pub announce_ip: Option<&'a str>,
    /// Tracker listen interface override.
    pub listen_interface: Option<&'a str>,
    /// Tracker request timeout in milliseconds.
    pub request_timeout_ms: Option<i32>,
    /// Tracker announce policy.
    pub announce: TrackerAnnouncePolicy,
    /// Tracker proxy host.
    pub proxy_host: Option<&'a str>,
    /// Tracker proxy port.
    pub proxy_port: Option<i32>,
    /// Tracker proxy kind.
    pub proxy_kind: Option<&'a str>,
    /// Secret name for tracker proxy username.
    pub proxy_username_secret: Option<&'a str>,
    /// Secret name for tracker proxy password.
    pub proxy_password_secret: Option<&'a str>,
    /// Tracker proxy policy.
    pub proxy: TrackerProxyPolicy,
    /// Optional client certificate path for tracker TLS.
    pub ssl_cert: Option<&'a str>,
    /// Optional client private key path for tracker TLS.
    pub ssl_private_key: Option<&'a str>,
    /// Optional CA certificate bundle path for tracker TLS.
    pub ssl_ca_cert: Option<&'a str>,
    /// Tracker TLS policy.
    pub tls: TrackerTlsPolicy,
    /// Secret name for tracker auth username.
    pub auth_username_secret: Option<&'a str>,
    /// Secret name for tracker auth password.
    pub auth_password_secret: Option<&'a str>,
    /// Secret name for tracker auth cookie.
    pub auth_cookie_secret: Option<&'a str>,
    /// Default tracker endpoints.
    pub default_urls: &'a [String],
    /// Extra tracker endpoints.
    pub extra_urls: &'a [String],
}

/// IP filter configuration update payload used for persistence.
#[derive(Debug, Clone)]
pub struct IpFilterUpdate<'a> {
    /// IP filter blocklist URL.
    pub blocklist_url: Option<&'a str>,
    /// IP filter blocklist `ETag`.
    pub etag: Option<&'a str>,
    /// Timestamp of last successful blocklist refresh.
    pub last_updated_at: Option<DateTime<Utc>>,
    /// Last blocklist error.
    pub last_error: Option<&'a str>,
    /// IP filter CIDR entries.
    pub cidrs: &'a [String],
}

/// Alternate speed configuration update payload used for persistence.
#[derive(Debug, Clone)]
pub struct AltSpeedUpdate<'a> {
    /// Alternate download cap in bytes per second.
    pub download_bps: Option<i64>,
    /// Alternate upload cap in bytes per second.
    pub upload_bps: Option<i64>,
    /// Alternate speed schedule start (minutes since midnight).
    pub schedule_start_minutes: Option<i32>,
    /// Alternate speed schedule end (minutes since midnight).
    pub schedule_end_minutes: Option<i32>,
    /// Alternate speed schedule days.
    pub days: &'a [String],
}

/// Peer class configuration update payload used for persistence.
#[derive(Debug, Clone)]
pub struct PeerClassesUpdate<'a> {
    /// Peer class identifiers.
    pub class_ids: &'a [i16],
    /// Peer class labels.
    pub labels: &'a [String],
    /// Peer class download priorities.
    pub download_priorities: &'a [i16],
    /// Peer class upload priorities.
    pub upload_priorities: &'a [i16],
    /// Peer class connection limit factors.
    pub connection_limit_factors: &'a [i16],
    /// Peer class ignore-unchoke slot flags.
    pub ignore_unchoke_slots: &'a [bool],
    /// Default peer class identifiers.
    pub default_class_ids: &'a [i16],
}

/// Aggregated engine profile payload used for the unified update path.
#[derive(Debug, Clone)]
pub struct EngineProfileUpdate<'a> {
    /// Primary key for the engine profile row.
    pub id: Uuid,
    /// Engine implementation identifier.
    pub implementation: &'a str,
    /// Optional listen port override.
    pub listen_port: Option<i32>,
    /// DHT enablement flag.
    pub dht: bool,
    /// Encryption policy string.
    pub encryption: &'a str,
    /// Optional maximum active torrent count.
    pub max_active: Option<i32>,
    /// Optional global download cap in bytes per second.
    pub max_download_bps: Option<i64>,
    /// Optional global upload cap in bytes per second.
    pub max_upload_bps: Option<i64>,
    /// Optional share ratio stop threshold.
    pub seed_ratio_limit: Option<f64>,
    /// Optional seeding time limit in seconds.
    pub seed_time_limit: Option<i64>,
    /// Queue policy toggles.
    pub queue: QueuePolicySet,
    /// Seeding behaviour toggles.
    pub seeding: SeedingToggleSet,
    /// Choking algorithm selection.
    pub choking_algorithm: &'a str,
    /// Seed choking algorithm selection.
    pub seed_choking_algorithm: &'a str,
    /// Optional optimistic unchoke slot override.
    pub optimistic_unchoke_slots: Option<i32>,
    /// Optional disk queue limit override.
    pub max_queued_disk_bytes: Option<i64>,
    /// Directory for fast-resume payloads.
    pub resume_dir: &'a str,
    /// Root directory for active downloads.
    pub download_root: &'a str,
    /// Storage allocation mode.
    pub storage_mode: &'a str,
    /// Storage-related toggles.
    pub storage: StorageToggleSet,
    /// Optional disk read mode override.
    pub disk_read_mode: Option<&'a str>,
    /// Optional disk write mode override.
    pub disk_write_mode: Option<&'a str>,
    /// Whether piece hash verification is enabled.
    pub verify_piece_hashes: bool,
    /// Optional cache size in MiB.
    pub cache_size: Option<i32>,
    /// Optional cache expiry in seconds.
    pub cache_expiry: Option<i32>,
    /// NAT traversal and PEX toggles.
    pub nat: NatToggleSet,
    /// IPv6 policy flag.
    pub ipv6_mode: &'a str,
    /// Privacy and transport toggles.
    pub privacy: PrivacyToggleSet,
    /// Optional starting port for outgoing connections.
    pub outgoing_port_min: Option<i32>,
    /// Optional ending port for outgoing connections.
    pub outgoing_port_max: Option<i32>,
    /// Optional DSCP/TOS value applied to peer sockets.
    pub peer_dscp: Option<i32>,
    /// Optional global peer connection limit.
    pub connections_limit: Option<i32>,
    /// Optional per-torrent peer connection limit.
    pub connections_limit_per_torrent: Option<i32>,
    /// Optional unchoke slot limit.
    pub unchoke_slots: Option<i32>,
    /// Optional half-open connection limit.
    pub half_open_limit: Option<i32>,
    /// Optional stats interval in milliseconds.
    pub stats_interval_ms: Option<i32>,
}

/// Update the engine profile in a single stored procedure call.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_engine_profile<'e, E>(
    executor: E,
    profile: &EngineProfileUpdate<'_>,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.update_engine_profile(_id => $1, _implementation => $2, _listen_port => $3, _dht => $4, _encryption => $5, _max_active => $6, _max_download_bps => $7, _max_upload_bps => $8, _seed_ratio_limit => $9, _seed_time_limit => $10, _sequential_default => $11, _auto_managed => $12, _auto_manage_prefer_seeds => $13, _dont_count_slow_torrents => $14, _super_seeding => $15, _choking_algorithm => $16, _seed_choking_algorithm => $17, _strict_super_seeding => $18, _optimistic_unchoke_slots => $19, _max_queued_disk_bytes => $20, _resume_dir => $21, _download_root => $22, _storage_mode => $23, _use_partfile => $24, _cache_size => $25, _cache_expiry => $26, _coalesce_reads => $27, _coalesce_writes => $28, _use_disk_cache_pool => $29, _disk_read_mode => $30, _disk_write_mode => $31, _verify_piece_hashes => $32, _lsd => $33, _upnp => $34, _natpmp => $35, _pex => $36, _ipv6_mode => $37, _anonymous_mode => $38, _force_proxy => $39, _prefer_rc4 => $40, _allow_multiple_connections_per_ip => $41, _enable_outgoing_utp => $42, _enable_incoming_utp => $43, _outgoing_port_min => $44, _outgoing_port_max => $45, _peer_dscp => $46, _connections_limit => $47, _connections_limit_per_torrent => $48, _unchoke_slots => $49, _half_open_limit => $50, _stats_interval_ms => $51)",
    )
    .bind(profile.id)
    .bind(profile.implementation)
    .bind(profile.listen_port)
    .bind(profile.dht)
    .bind(profile.encryption)
    .bind(profile.max_active)
    .bind(profile.max_download_bps)
    .bind(profile.max_upload_bps)
    .bind(profile.seed_ratio_limit)
    .bind(profile.seed_time_limit)
    .bind(profile.seeding.sequential_default())
    .bind(profile.queue.auto_managed())
    .bind(profile.queue.prefer_seeds())
    .bind(profile.queue.dont_count_slow())
    .bind(profile.seeding.super_seeding())
    .bind(profile.choking_algorithm)
    .bind(profile.seed_choking_algorithm)
    .bind(profile.seeding.strict_super_seeding())
    .bind(profile.optimistic_unchoke_slots)
    .bind(profile.max_queued_disk_bytes)
    .bind(profile.resume_dir)
    .bind(profile.download_root)
    .bind(profile.storage_mode)
    .bind(profile.storage.use_partfile())
    .bind(profile.cache_size)
    .bind(profile.cache_expiry)
    .bind(profile.storage.coalesce_reads())
    .bind(profile.storage.coalesce_writes())
    .bind(profile.storage.use_disk_cache_pool())
    .bind(profile.disk_read_mode)
    .bind(profile.disk_write_mode)
    .bind(profile.verify_piece_hashes)
    .bind(profile.nat.lsd())
    .bind(profile.nat.upnp())
    .bind(profile.nat.natpmp())
    .bind(profile.nat.pex())
    .bind(profile.ipv6_mode)
    .bind(profile.privacy.anonymous_mode())
    .bind(profile.privacy.force_proxy())
    .bind(profile.privacy.prefer_rc4())
    .bind(profile.privacy.allow_multiple_connections_per_ip())
    .bind(profile.privacy.enable_outgoing_utp())
    .bind(profile.privacy.enable_incoming_utp())
    .bind(profile.outgoing_port_min)
    .bind(profile.outgoing_port_max)
    .bind(profile.peer_dscp)
    .bind(profile.connections_limit)
    .bind(profile.connections_limit_per_torrent)
    .bind(profile.unchoke_slots)
    .bind(profile.half_open_limit)
    .bind(profile.stats_interval_ms)
    .execute(executor)
    .await
    .map_err(map_query_err("update engine profile"))?;
    Ok(())
}

/// Replace list-based engine profile values for a given kind.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn set_engine_list_values<'e, E>(
    executor: E,
    profile_id: Uuid,
    kind: &str,
    values: &[String],
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.set_engine_list_values(_profile_id => $1, _kind => $2, _values => $3)",
    )
    .bind(profile_id)
    .bind(kind)
    .bind(values)
    .execute(executor)
    .await
    .map_err(map_query_err("set engine list values"))?;
    Ok(())
}

/// Replace the engine IP filter configuration.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn set_engine_ip_filter<'e, E>(
    executor: E,
    profile_id: Uuid,
    update: &IpFilterUpdate<'_>,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.set_engine_ip_filter(_profile_id => $1, _blocklist_url => $2, _etag => $3, _last_updated_at => $4, _last_error => $5, _cidrs => $6)",
    )
    .bind(profile_id)
    .bind(update.blocklist_url)
    .bind(update.etag)
    .bind(update.last_updated_at)
    .bind(update.last_error)
    .bind(update.cidrs)
    .execute(executor)
    .await
    .map_err(map_query_err("set engine ip filter"))?;
    Ok(())
}

/// Replace the engine alternate speed configuration.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn set_engine_alt_speed<'e, E>(
    executor: E,
    profile_id: Uuid,
    update: &AltSpeedUpdate<'_>,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.set_engine_alt_speed(_profile_id => $1, _download_bps => $2, _upload_bps => $3, _schedule_start_minutes => $4, _schedule_end_minutes => $5, _days => $6)",
    )
    .bind(profile_id)
    .bind(update.download_bps)
    .bind(update.upload_bps)
    .bind(update.schedule_start_minutes)
    .bind(update.schedule_end_minutes)
    .bind(update.days)
    .execute(executor)
    .await
    .map_err(map_query_err("set engine alt speed"))?;
    Ok(())
}

/// Replace the tracker configuration for the engine profile.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn set_tracker_config<'e, E>(
    executor: E,
    profile_id: Uuid,
    update: &TrackerConfigUpdate<'_>,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.set_tracker_config(_profile_id => $1, _user_agent => $2, _announce_ip => $3, _listen_interface => $4, _request_timeout_ms => $5, _announce_to_all => $6, _replace_trackers => $7, _proxy_host => $8, _proxy_port => $9, _proxy_kind => $10, _proxy_username_secret => $11, _proxy_password_secret => $12, _proxy_peers => $13, _ssl_cert => $14, _ssl_private_key => $15, _ssl_ca_cert => $16, _ssl_tracker_verify => $17, _auth_username_secret => $18, _auth_password_secret => $19, _auth_cookie_secret => $20, _default_urls => $21, _extra_urls => $22)",
    )
    .bind(profile_id)
    .bind(update.user_agent)
    .bind(update.announce_ip)
    .bind(update.listen_interface)
    .bind(update.request_timeout_ms)
    .bind(update.announce.announce_to_all)
    .bind(update.announce.replace_trackers)
    .bind(update.proxy_host)
    .bind(update.proxy_port)
    .bind(update.proxy_kind)
    .bind(update.proxy_username_secret)
    .bind(update.proxy_password_secret)
    .bind(update.proxy.proxy_peers)
    .bind(update.ssl_cert)
    .bind(update.ssl_private_key)
    .bind(update.ssl_ca_cert)
    .bind(update.tls.verify)
    .bind(update.auth_username_secret)
    .bind(update.auth_password_secret)
    .bind(update.auth_cookie_secret)
    .bind(update.default_urls)
    .bind(update.extra_urls)
    .execute(executor)
    .await
    .map_err(map_query_err("set tracker configuration"))?;
    Ok(())
}

/// Replace the peer class configuration for the engine profile.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn set_peer_classes<'e, E>(
    executor: E,
    profile_id: Uuid,
    update: &PeerClassesUpdate<'_>,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.set_peer_classes(_profile_id => $1, _class_ids => $2, _labels => $3, _download_priorities => $4, _upload_priorities => $5, _connection_limit_factors => $6, _ignore_unchoke_slots => $7, _default_class_ids => $8)",
    )
    .bind(profile_id)
    .bind(update.class_ids)
    .bind(update.labels)
    .bind(update.download_priorities)
    .bind(update.upload_priorities)
    .bind(update.connection_limit_factors)
    .bind(update.ignore_unchoke_slots)
    .bind(update.default_class_ids)
    .execute(executor)
    .await
    .map_err(map_query_err("set peer classes"))?;
    Ok(())
}

/// Update a string column on `fs_policy`.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_fs_string_field<'e, E>(
    executor: E,
    id: Uuid,
    field: FsStringField,
    value: &str,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.update_fs_string_field(_id => $1, _column => $2, _value => $3)",
    )
    .bind(id)
    .bind(field.column())
    .bind(value)
    .execute(executor)
    .await
    .map_err(map_query_err("update fs string field"))?;
    Ok(())
}

/// Update a boolean column on `fs_policy`.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_fs_boolean_field<'e, E>(
    executor: E,
    id: Uuid,
    field: FsBooleanField,
    value: bool,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.update_fs_boolean_field(_id => $1, _column => $2, _value => $3)",
    )
    .bind(id)
    .bind(field.column())
    .bind(value)
    .execute(executor)
    .await
    .map_err(map_query_err("update fs boolean field"))?;
    Ok(())
}

/// Update a list column on `fs_policy`.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_fs_array_field<'e, E>(
    executor: E,
    id: Uuid,
    field: FsArrayField,
    values: &[String],
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.update_fs_array_field(_id => $1, _column => $2, _values => $3)",
    )
    .bind(id)
    .bind(field.column())
    .bind(values)
    .execute(executor)
    .await
    .map_err(map_query_err("update fs array field"))?;
    Ok(())
}

/// Update an optional string column on `fs_policy`.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_fs_optional_string_field<'e, E>(
    executor: E,
    id: Uuid,
    field: FsOptionalStringField,
    value: Option<&str>,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.update_fs_optional_string_field(_id => $1, _column => $2, _value => $3)",
    )
    .bind(id)
    .bind(field.column())
    .bind(value)
    .execute(executor)
    .await
    .map_err(map_query_err("update fs optional string field"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fs_column_mappings_are_stable() {
        assert_eq!(FsStringField::LibraryRoot.column(), "library_root");
        assert_eq!(FsStringField::Par2.column(), "par2");
        assert_eq!(FsStringField::MoveMode.column(), "move_mode");
        assert_eq!(FsBooleanField::Extract.column(), "extract");
        assert_eq!(FsBooleanField::Flatten.column(), "flatten");
        assert_eq!(FsArrayField::CleanupKeep.column(), "cleanup_keep");
        assert_eq!(FsArrayField::CleanupDrop.column(), "cleanup_drop");
        assert_eq!(FsArrayField::AllowPaths.column(), "allow_paths");
        assert_eq!(FsOptionalStringField::ChmodFile.column(), "chmod_file");
        assert_eq!(FsOptionalStringField::ChmodDir.column(), "chmod_dir");
        assert_eq!(FsOptionalStringField::Owner.column(), "owner");
        assert_eq!(FsOptionalStringField::Group.column(), "group");
        assert_eq!(FsOptionalStringField::Umask.column(), "umask");
    }
}
