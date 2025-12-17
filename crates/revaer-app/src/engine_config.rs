//! Engine profile normalisation and runtime mapping helpers.
//!
//! # Design
//! - Derives a runtime configuration from the stored engine profile while applying guard rails.
//! - Carries the effective profile (including warnings) for observability and API responses.
//! - Keeps encryption mapping centralised to avoid drift between API/config/runtime layers.

use revaer_config::engine_profile::{
    AltSpeedConfig, ChokingAlgorithm, SeedChokingAlgorithm, StorageMode as ConfigStorageMode,
};
use revaer_config::{
    EngineEncryptionPolicy, EngineIpv6Mode, EngineNetworkConfig, EngineProfile,
    EngineProfileEffective, IpFilterConfig, IpFilterRule, TrackerAuthConfig, TrackerConfig,
    TrackerProxyConfig, TrackerProxyType, normalize_engine_profile,
};
use revaer_torrent_core::TorrentRateLimit;
use revaer_torrent_libt::{
    EncryptionPolicy, EngineRuntimeConfig, IpFilterRule as RuntimeIpFilterRule,
    IpFilterRuntimeConfig, Ipv6Mode as RuntimeIpv6Mode, TrackerAuthRuntime, TrackerProxyRuntime,
    TrackerProxyType as RuntimeProxyType, TrackerRuntimeConfig,
    types::{
        AltSpeedRuntimeConfig as RuntimeAltSpeedConfig,
        AltSpeedSchedule as RuntimeAltSpeedSchedule, ChokingAlgorithm as RuntimeChokingAlgorithm,
        OutgoingPortRange as RuntimeOutgoingPortRange,
        SeedChokingAlgorithm as RuntimeSeedChokingAlgorithm, StorageMode as RuntimeStorageMode,
    },
};

/// Runtime plan derived from the persisted engine profile, including effective values and
/// guard-rail warnings.
#[derive(Debug, Clone)]
pub(crate) struct EngineRuntimePlan {
    /// Effective, clamped profile used for observability and API responses.
    pub effective: EngineProfileEffective,
    /// Runtime configuration to apply to the engine.
    pub runtime: EngineRuntimeConfig,
}

impl EngineRuntimePlan {
    /// Derive a runtime configuration and effective profile from the persisted engine profile.
    #[must_use]
    pub(crate) fn from_profile(profile: &EngineProfile) -> Self {
        let effective = normalize_engine_profile(profile);
        let tracker = map_tracker_config(
            serde_json::from_value::<TrackerConfig>(effective.tracker.clone()).unwrap_or_default(),
        );
        let runtime_ipv6_mode = map_ipv6_mode(effective.network.ipv6_mode);
        let (listen_interfaces, listen_port) = derive_listen_config(&effective.network);
        let ip_filter = map_ip_filter_config(&effective.network.ip_filter);
        let outgoing_ports =
            effective
                .network
                .outgoing_ports
                .map(|range| RuntimeOutgoingPortRange {
                    start: range.start,
                    end: range.end,
                });
        let alt_speed = map_alt_speed(&effective.alt_speed);
        let runtime = EngineRuntimeConfig {
            download_root: effective.storage.download_root.clone(),
            resume_dir: effective.storage.resume_dir.clone(),
            storage_mode: map_storage_mode(effective.storage.storage_mode),
            use_partfile: bool::from(effective.storage.use_partfile).into(),
            cache_size: effective.storage.cache_size,
            cache_expiry: effective.storage.cache_expiry,
            coalesce_reads: bool::from(effective.storage.coalesce_reads).into(),
            coalesce_writes: bool::from(effective.storage.coalesce_writes).into(),
            use_disk_cache_pool: bool::from(effective.storage.use_disk_cache_pool).into(),
            enable_dht: effective.network.enable_dht,
            dht_bootstrap_nodes: effective.network.dht_bootstrap_nodes.clone(),
            dht_router_nodes: effective.network.dht_router_nodes.clone(),
            enable_lsd: bool::from(effective.network.enable_lsd).into(),
            enable_upnp: bool::from(effective.network.enable_upnp).into(),
            enable_natpmp: bool::from(effective.network.enable_natpmp).into(),
            enable_pex: bool::from(effective.network.enable_pex).into(),
            outgoing_ports,
            peer_dscp: effective.network.peer_dscp,
            anonymous_mode: bool::from(effective.network.anonymous_mode).into(),
            force_proxy: bool::from(effective.network.force_proxy).into(),
            prefer_rc4: bool::from(effective.network.prefer_rc4).into(),
            allow_multiple_connections_per_ip: bool::from(
                effective.network.allow_multiple_connections_per_ip,
            )
            .into(),
            enable_outgoing_utp: bool::from(effective.network.enable_outgoing_utp).into(),
            enable_incoming_utp: bool::from(effective.network.enable_incoming_utp).into(),
            sequential_default: effective.behavior.sequential_default,
            auto_managed: bool::from(effective.behavior.auto_managed).into(),
            auto_manage_prefer_seeds: bool::from(effective.behavior.auto_manage_prefer_seeds)
                .into(),
            dont_count_slow_torrents: bool::from(effective.behavior.dont_count_slow_torrents)
                .into(),
            listen_interfaces,
            ipv6_mode: runtime_ipv6_mode,
            listen_port,
            max_active: effective.limits.max_active,
            download_rate_limit: effective.limits.download_rate_limit,
            upload_rate_limit: effective.limits.upload_rate_limit,
            seed_ratio_limit: effective.limits.seed_ratio_limit,
            seed_time_limit: effective.limits.seed_time_limit,
            alt_speed,
            stats_interval_ms: effective.limits.stats_interval_ms,
            connections_limit: effective.limits.connections_limit,
            connections_limit_per_torrent: effective.limits.connections_limit_per_torrent,
            unchoke_slots: effective.limits.unchoke_slots,
            half_open_limit: effective.limits.half_open_limit,
            choking_algorithm: map_choking_algorithm(effective.limits.choking_algorithm),
            seed_choking_algorithm: map_seed_choking_algorithm(
                effective.limits.seed_choking_algorithm,
            ),
            strict_super_seeding: bool::from(effective.limits.strict_super_seeding).into(),
            optimistic_unchoke_slots: effective.limits.optimistic_unchoke_slots,
            max_queued_disk_bytes: effective.limits.max_queued_disk_bytes,
            encryption: map_encryption_policy(effective.network.encryption),
            tracker,
            ip_filter,
            super_seeding: bool::from(effective.behavior.super_seeding).into(),
        };

        Self { effective, runtime }
    }

    /// Convert global rate limits into the torrent-core representation used by the worker.
    #[must_use]
    pub(crate) fn global_rate_limit(&self) -> TorrentRateLimit {
        TorrentRateLimit {
            download_bps: self
                .runtime
                .download_rate_limit
                .and_then(|value| u64::try_from(value).ok()),
            upload_bps: self
                .runtime
                .upload_rate_limit
                .and_then(|value| u64::try_from(value).ok()),
        }
    }
}

const fn map_encryption_policy(policy: EngineEncryptionPolicy) -> EncryptionPolicy {
    match policy {
        EngineEncryptionPolicy::Require => EncryptionPolicy::Require,
        EngineEncryptionPolicy::Prefer => EncryptionPolicy::Prefer,
        EngineEncryptionPolicy::Disable => EncryptionPolicy::Disable,
    }
}

const fn map_choking_algorithm(algorithm: ChokingAlgorithm) -> RuntimeChokingAlgorithm {
    match algorithm {
        ChokingAlgorithm::FixedSlots => RuntimeChokingAlgorithm::FixedSlots,
        ChokingAlgorithm::RateBased => RuntimeChokingAlgorithm::RateBased,
    }
}

const fn map_seed_choking_algorithm(
    algorithm: SeedChokingAlgorithm,
) -> RuntimeSeedChokingAlgorithm {
    match algorithm {
        SeedChokingAlgorithm::RoundRobin => RuntimeSeedChokingAlgorithm::RoundRobin,
        SeedChokingAlgorithm::FastestUpload => RuntimeSeedChokingAlgorithm::FastestUpload,
        SeedChokingAlgorithm::AntiLeech => RuntimeSeedChokingAlgorithm::AntiLeech,
    }
}

const fn map_storage_mode(mode: ConfigStorageMode) -> RuntimeStorageMode {
    match mode {
        ConfigStorageMode::Sparse => RuntimeStorageMode::Sparse,
        ConfigStorageMode::Allocate => RuntimeStorageMode::Allocate,
    }
}

fn map_alt_speed(config: &AltSpeedConfig) -> Option<RuntimeAltSpeedConfig> {
    let schedule = config.schedule.as_ref()?;
    if config.download_bps.is_none() && config.upload_bps.is_none() {
        return None;
    }

    Some(RuntimeAltSpeedConfig {
        download_bps: config.download_bps,
        upload_bps: config.upload_bps,
        schedule: RuntimeAltSpeedSchedule {
            days: schedule.days.clone(),
            start_minutes: schedule.start_minutes,
            end_minutes: schedule.end_minutes,
        },
    })
}

fn map_tracker_config(config: TrackerConfig) -> TrackerRuntimeConfig {
    TrackerRuntimeConfig {
        default: config.default,
        extra: config.extra,
        replace: config.replace,
        user_agent: config.user_agent,
        announce_ip: config.announce_ip,
        listen_interface: config.listen_interface,
        request_timeout_ms: config.request_timeout_ms,
        announce_to_all: config.announce_to_all,
        proxy: config.proxy.map(map_proxy_config),
        auth: config.auth.map(map_tracker_auth),
    }
}

fn map_proxy_config(config: TrackerProxyConfig) -> TrackerProxyRuntime {
    TrackerProxyRuntime {
        host: config.host,
        port: config.port,
        username_secret: config.username_secret,
        password_secret: config.password_secret,
        kind: map_proxy_kind(config.kind),
        proxy_peers: config.proxy_peers,
    }
}

const fn map_proxy_kind(kind: TrackerProxyType) -> RuntimeProxyType {
    match kind {
        TrackerProxyType::Http => RuntimeProxyType::Http,
        TrackerProxyType::Https => RuntimeProxyType::Https,
        TrackerProxyType::Socks5 => RuntimeProxyType::Socks5,
    }
}

fn map_tracker_auth(config: TrackerAuthConfig) -> TrackerAuthRuntime {
    TrackerAuthRuntime {
        username_secret: config.username_secret,
        password_secret: config.password_secret,
        cookie_secret: config.cookie_secret,
        ..TrackerAuthRuntime::default()
    }
}

const fn map_ipv6_mode(mode: EngineIpv6Mode) -> RuntimeIpv6Mode {
    match mode {
        EngineIpv6Mode::Disabled => RuntimeIpv6Mode::Disabled,
        EngineIpv6Mode::Enabled => RuntimeIpv6Mode::Enabled,
        EngineIpv6Mode::PreferV6 => RuntimeIpv6Mode::PreferV6,
    }
}

fn derive_listen_config(network: &EngineNetworkConfig) -> (Vec<String>, Option<i32>) {
    if !network.listen_interfaces.is_empty() {
        return (network.listen_interfaces.clone(), None);
    }
    network.listen_port.map_or_else(
        || (Vec::new(), None),
        |port| match network.ipv6_mode {
            EngineIpv6Mode::Disabled => (Vec::new(), Some(port)),
            EngineIpv6Mode::Enabled => (
                vec![format!("0.0.0.0:{port}"), format!("[::]:{port}")],
                None,
            ),
            EngineIpv6Mode::PreferV6 => (
                vec![format!("[::]:{port}"), format!("0.0.0.0:{port}")],
                None,
            ),
        },
    )
}

fn map_ip_filter_config(config: &IpFilterConfig) -> Option<IpFilterRuntimeConfig> {
    if config.cidrs.is_empty() && config.blocklist_url.is_none() {
        return None;
    }

    let rules = config.rules().unwrap_or_default();
    let runtime_rules = rules.iter().map(map_ip_filter_rule).collect::<Vec<_>>();

    Some(IpFilterRuntimeConfig {
        rules: runtime_rules,
        blocklist_url: config.blocklist_url.clone(),
        etag: config.etag.clone(),
        last_updated_at: config
            .last_updated_at
            .map(|timestamp| timestamp.to_rfc3339()),
    })
}

fn map_ip_filter_rule(rule: &IpFilterRule) -> RuntimeIpFilterRule {
    RuntimeIpFilterRule {
        start: rule.start.to_string(),
        end: rule.end.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revaer_config::MAX_RATE_LIMIT_BPS;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn runtime_plan_clamps_and_warns() {
        let profile = EngineProfile {
            id: Uuid::new_v4(),
            implementation: "libtorrent".into(),
            listen_port: Some(70_000),
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
            encryption: "unknown".into(),
            max_active: Some(0),
            max_download_bps: Some(MAX_RATE_LIMIT_BPS + 10),
            max_upload_bps: Some(0),
            seed_ratio_limit: None,
            seed_time_limit: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            stats_interval_ms: None,
            alt_speed: json!({}),
            sequential_default: true,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: false.into(),
            dont_count_slow_torrents: true.into(),
            super_seeding: false.into(),
            choking_algorithm: EngineProfile::default_choking_algorithm(),
            seed_choking_algorithm: EngineProfile::default_seed_choking_algorithm(),
            strict_super_seeding: false.into(),
            optimistic_unchoke_slots: None,
            max_queued_disk_bytes: None,
            resume_dir: String::new(),
            download_root: "   ".into(),
            storage_mode: EngineProfile::default_storage_mode(),
            use_partfile: EngineProfile::default_use_partfile(),
            cache_size: None,
            cache_expiry: None,
            coalesce_reads: EngineProfile::default_coalesce_reads(),
            coalesce_writes: EngineProfile::default_coalesce_writes(),
            use_disk_cache_pool: EngineProfile::default_use_disk_cache_pool(),
            tracker: json!("not-an-object"),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            ip_filter: json!({}),
        };
        let plan = EngineRuntimePlan::from_profile(&profile);

        assert!(plan.runtime.listen_port.is_none());
        assert!(plan.runtime.max_active.is_none());
        assert_eq!(plan.runtime.download_rate_limit, Some(MAX_RATE_LIMIT_BPS));
        assert!(plan.runtime.upload_rate_limit.is_none());
        assert_eq!(plan.runtime.encryption, EncryptionPolicy::Prefer);
        assert_eq!(plan.runtime.download_root, "/data/staging");
        assert_eq!(plan.runtime.resume_dir, "/var/lib/revaer/state");
        assert_eq!(plan.effective.tracker, json!({}));
        assert!(plan.runtime.tracker.default.is_empty());
        assert!(
            !plan.effective.warnings.is_empty(),
            "warnings should be surfaced when clamping"
        );
        assert!(
            plan.effective
                .warnings
                .iter()
                .any(|msg| msg.contains("guard rail")),
            "guard rail clamp should emit a warning"
        );
        assert!(!plan.runtime.enable_lsd.is_enabled());
        assert!(!plan.runtime.enable_upnp.is_enabled());
        assert!(!plan.runtime.enable_natpmp.is_enabled());
        assert!(!plan.runtime.enable_pex.is_enabled());
        assert!(plan.runtime.dht_bootstrap_nodes.is_empty());
        assert!(plan.runtime.dht_router_nodes.is_empty());
    }

    #[test]
    fn encryption_policy_maps_variants() {
        let base = EngineProfile {
            id: Uuid::new_v4(),
            implementation: "libtorrent".into(),
            listen_port: None,
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
            dht: false,
            encryption: "require".into(),
            max_active: None,
            max_download_bps: None,
            max_upload_bps: None,
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
            resume_dir: "/var/resume".into(),
            download_root: "/data".into(),
            storage_mode: EngineProfile::default_storage_mode(),
            use_partfile: EngineProfile::default_use_partfile(),
            cache_size: None,
            cache_expiry: None,
            coalesce_reads: EngineProfile::default_coalesce_reads(),
            coalesce_writes: EngineProfile::default_coalesce_writes(),
            use_disk_cache_pool: EngineProfile::default_use_disk_cache_pool(),
            tracker: json!({}),
            enable_lsd: true.into(),
            enable_upnp: true.into(),
            enable_natpmp: true.into(),
            enable_pex: true.into(),
            dht_bootstrap_nodes: vec!["router.bittorrent.com:6881".into()],
            dht_router_nodes: vec!["dht.transmissionbt.com:6881".into()],
            ip_filter: json!({}),
        };

        let require = EngineRuntimePlan::from_profile(&base);
        assert_eq!(require.runtime.encryption, EncryptionPolicy::Require);
        assert!(require.runtime.enable_lsd.is_enabled());
        assert!(require.runtime.enable_upnp.is_enabled());
        assert!(require.runtime.enable_natpmp.is_enabled());
        assert!(require.runtime.enable_pex.is_enabled());
        assert_eq!(
            require.runtime.dht_bootstrap_nodes,
            vec!["router.bittorrent.com:6881".to_string()]
        );
        assert_eq!(
            require.runtime.dht_router_nodes,
            vec!["dht.transmissionbt.com:6881".to_string()]
        );

        let mut prefer_profile = base.clone();
        prefer_profile.encryption = "prefer".into();
        let prefer = EngineRuntimePlan::from_profile(&prefer_profile);
        assert_eq!(prefer.runtime.encryption, EncryptionPolicy::Prefer);

        let mut disable_profile = base;
        disable_profile.encryption = "disable".into();
        let disable = EngineRuntimePlan::from_profile(&disable_profile);
        assert_eq!(disable.runtime.encryption, EncryptionPolicy::Disable);
    }

    #[test]
    fn ip_filter_is_threaded_into_runtime() {
        let profile = EngineProfile {
            id: Uuid::new_v4(),
            implementation: "libtorrent".into(),
            listen_port: None,
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
            dht: false,
            encryption: "prefer".into(),
            max_active: None,
            max_download_bps: None,
            max_upload_bps: None,
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
            resume_dir: "/var/resume".into(),
            download_root: "/data".into(),
            storage_mode: EngineProfile::default_storage_mode(),
            use_partfile: EngineProfile::default_use_partfile(),
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
            ip_filter: json!({
                "cidrs": ["10.0.0.0/8"],
                "blocklist_url": "https://example.com/blocklist",
                "etag": "etag-1",
                "last_updated_at": "2024-01-01T00:00:00Z"
            }),
        };

        let plan = EngineRuntimePlan::from_profile(&profile);
        let filter = plan.runtime.ip_filter.expect("ip filter present");
        assert_eq!(filter.rules.len(), 1);
        assert_eq!(filter.rules[0].start, "10.0.0.0");
        assert_eq!(filter.rules[0].end, "10.255.255.255");
        assert_eq!(
            filter.blocklist_url.as_deref(),
            Some("https://example.com/blocklist")
        );
        assert_eq!(filter.etag.as_deref(), Some("etag-1"));
        assert_eq!(
            filter.last_updated_at.as_deref(),
            Some("2024-01-01T00:00:00+00:00")
        );
    }

    #[test]
    fn anonymous_mode_enforces_proxy_when_available() {
        let profile = EngineProfile {
            id: Uuid::new_v4(),
            implementation: "libtorrent".into(),
            listen_port: None,
            listen_interfaces: Vec::new(),
            ipv6_mode: "disabled".into(),
            anonymous_mode: true.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            outgoing_port_min: None,
            outgoing_port_max: None,
            peer_dscp: None,
            dht: false,
            encryption: "prefer".into(),
            max_active: None,
            max_download_bps: None,
            max_upload_bps: None,
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
            resume_dir: "/var/resume".into(),
            download_root: "/data".into(),
            storage_mode: EngineProfile::default_storage_mode(),
            use_partfile: EngineProfile::default_use_partfile(),
            cache_size: None,
            cache_expiry: None,
            coalesce_reads: EngineProfile::default_coalesce_reads(),
            coalesce_writes: EngineProfile::default_coalesce_writes(),
            use_disk_cache_pool: EngineProfile::default_use_disk_cache_pool(),
            tracker: json!({
                "proxy": {
                    "host": "proxy.example",
                    "port": 8080,
                    "proxy_peers": true
                }
            }),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            ip_filter: json!({}),
        };

        let plan = EngineRuntimePlan::from_profile(&profile);
        assert!(bool::from(plan.effective.network.force_proxy));
        assert!(bool::from(plan.runtime.force_proxy));
        assert!(
            plan.effective
                .warnings
                .iter()
                .any(|msg| msg.contains("anonymous_mode")),
            "warning should note proxy enforcement"
        );
    }

    #[test]
    fn prefer_v6_enables_dual_stack_listeners() {
        let profile = EngineProfile {
            id: Uuid::new_v4(),
            implementation: "libtorrent".into(),
            listen_port: Some(6_881),
            listen_interfaces: Vec::new(),
            ipv6_mode: "prefer_v6".into(),
            anonymous_mode: false.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            outgoing_port_min: None,
            outgoing_port_max: None,
            peer_dscp: None,
            dht: false,
            encryption: "prefer".into(),
            max_active: None,
            max_download_bps: None,
            max_upload_bps: None,
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
            resume_dir: "/var/resume".into(),
            download_root: "/data".into(),
            storage_mode: EngineProfile::default_storage_mode(),
            use_partfile: EngineProfile::default_use_partfile(),
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
        };

        let plan = EngineRuntimePlan::from_profile(&profile);
        assert!(
            plan.runtime.listen_port.is_none(),
            "port is encoded into interfaces"
        );
        assert_eq!(
            plan.runtime.listen_interfaces,
            vec!["[::]:6881".to_string(), "0.0.0.0:6881".to_string()]
        );
        assert_eq!(plan.runtime.ipv6_mode, RuntimeIpv6Mode::PreferV6);
    }
}
