//! Guard rails for translating runtime config into native engine options.
//!
//! # Design
//! - Clamps or disables invalid values to keep the native session stable.
//! - Collects warnings so callers can surface guard-rail applications.
//! - Keeps the runtime→FFI mapping central to avoid drift as fields grow.

use crate::ffi::ffi;
use crate::types::{
    EngineRuntimeConfig, IpFilterRule as RuntimeIpFilterRule, IpFilterRuntimeConfig,
    OutgoingPortRange, TrackerAuthRuntime, TrackerProxyRuntime, TrackerProxyType,
    TrackerRuntimeConfig,
};

/// Planned native engine options plus guard-rail warnings.
#[derive(Debug)]
pub(super) struct EngineOptionsPlan {
    /// Options passed to the native libtorrent session.
    pub options: ffi::EngineOptions,
    /// Warnings describing clamp/disable decisions applied to the request.
    pub warnings: Vec<String>,
}

impl EngineOptionsPlan {
    /// Clamp/normalise the runtime config into FFI-ready engine options.
    #[must_use]
    pub(super) fn from_runtime_config(config: &EngineRuntimeConfig) -> Self {
        let mut warnings = Vec::new();

        let options = ffi::EngineOptions {
            network: build_network_options(config, &mut warnings),
            limits: build_limit_options(config, &mut warnings),
            storage: build_storage_options(config, &mut warnings),
            behavior: ffi::EngineBehaviorOptions {
                sequential_default: config.sequential_default,
                auto_managed: bool::from(config.auto_managed),
                auto_manage_prefer_seeds: bool::from(config.auto_manage_prefer_seeds),
                dont_count_slow_torrents: bool::from(config.dont_count_slow_torrents),
                super_seeding: bool::from(config.super_seeding),
            },
            tracker: map_tracker_options(&config.tracker),
        };

        Self { options, warnings }
    }
}

fn build_network_options(
    config: &EngineRuntimeConfig,
    warnings: &mut Vec<String>,
) -> ffi::EngineNetworkOptions {
    let listen_interfaces = config.listen_interfaces.clone();
    let has_listen_interfaces = !listen_interfaces.is_empty();

    let (listen_port, set_listen_port) = if has_listen_interfaces {
        (0, false)
    } else {
        match config.listen_port {
            Some(port) if (1..=65_535).contains(&port) => (port, true),
            Some(port) => {
                warnings.push(format!(
                    "listen_port {port} is out of range; skipping listen override"
                ));
                (0, false)
            }
            None => (0, false),
        }
    };

    let (ip_filter_rules, has_ip_filter) = map_ip_filter(config.ip_filter.as_ref());
    let (outgoing_port_min, outgoing_port_max, has_outgoing_port_range) =
        map_outgoing_ports(config.outgoing_ports);
    let (peer_dscp, has_peer_dscp) = map_peer_dscp(config.peer_dscp);

    ffi::EngineNetworkOptions {
        listen_port,
        set_listen_port,
        listen_interfaces,
        has_listen_interfaces,
        enable_dht: config.enable_dht,
        enable_lsd: bool::from(config.enable_lsd),
        enable_upnp: bool::from(config.enable_upnp),
        enable_natpmp: bool::from(config.enable_natpmp),
        enable_pex: bool::from(config.enable_pex),
        outgoing_port_min,
        outgoing_port_max,
        has_outgoing_port_range,
        peer_dscp,
        has_peer_dscp,
        anonymous_mode: bool::from(config.anonymous_mode),
        force_proxy: bool::from(config.force_proxy),
        prefer_rc4: bool::from(config.prefer_rc4),
        allow_multiple_connections_per_ip: bool::from(config.allow_multiple_connections_per_ip),
        enable_outgoing_utp: bool::from(config.enable_outgoing_utp),
        enable_incoming_utp: bool::from(config.enable_incoming_utp),
        dht_bootstrap_nodes: config.dht_bootstrap_nodes.clone(),
        dht_router_nodes: config.dht_router_nodes.clone(),
        encryption_policy: config.encryption.as_u8(),
        ip_filter_rules,
        has_ip_filter,
    }
}

fn build_limit_options(
    config: &EngineRuntimeConfig,
    warnings: &mut Vec<String>,
) -> ffi::EngineLimitOptions {
    let max_active = map_positive_limit("max_active", config.max_active, warnings);
    let connections_limit =
        map_positive_limit("connections_limit", config.connections_limit, warnings);
    let connections_limit_per_torrent = map_positive_limit(
        "connections_limit_per_torrent",
        config.connections_limit_per_torrent,
        warnings,
    );
    let unchoke_slots = map_positive_limit("unchoke_slots", config.unchoke_slots, warnings);
    let half_open_limit = map_positive_limit("half_open_limit", config.half_open_limit, warnings);

    let download_rate_limit =
        clamp_rate_limit("download_rate_limit", config.download_rate_limit, warnings);
    let upload_rate_limit =
        clamp_rate_limit("upload_rate_limit", config.upload_rate_limit, warnings);
    let (seed_ratio_limit, has_seed_ratio_limit) =
        map_seed_ratio_limit(config.seed_ratio_limit, warnings);
    let (seed_time_limit, has_seed_time_limit) =
        map_seed_time_limit(config.seed_time_limit, warnings);
    let optimistic_unchoke_slots = config.optimistic_unchoke_slots.unwrap_or_default();
    let max_queued_disk_bytes = config
        .max_queued_disk_bytes
        .map(|value| value.min(i64::from(i32::MAX)))
        .unwrap_or_default();
    let (stats_interval_ms, has_stats_interval) = match config.stats_interval_ms {
        Some(value) if value > 0 => (value, true),
        _ => (0, false),
    };

    ffi::EngineLimitOptions {
        max_active,
        download_rate_limit,
        upload_rate_limit,
        seed_ratio_limit,
        has_seed_ratio_limit,
        seed_time_limit,
        has_seed_time_limit,
        connections_limit,
        connections_limit_per_torrent,
        unchoke_slots,
        half_open_limit,
        choking_algorithm: config.choking_algorithm.as_i32(),
        seed_choking_algorithm: config.seed_choking_algorithm.as_i32(),
        strict_super_seeding: bool::from(config.strict_super_seeding),
        optimistic_unchoke_slots,
        has_optimistic_unchoke_slots: config.optimistic_unchoke_slots.is_some(),
        max_queued_disk_bytes: i32::try_from(max_queued_disk_bytes).unwrap_or(i32::MAX),
        has_max_queued_disk_bytes: config.max_queued_disk_bytes.is_some(),
        stats_interval_ms,
        has_stats_interval,
    }
}

fn build_storage_options(
    config: &EngineRuntimeConfig,
    warnings: &mut Vec<String>,
) -> ffi::EngineStorageOptions {
    if config.download_root.trim().is_empty() {
        warnings.push(
            "download_root is empty; native session will fall back to its defaults".to_string(),
        );
    }
    if config.resume_dir.trim().is_empty() {
        warnings
            .push("resume_dir is empty; native session will fall back to its defaults".to_string());
    }

    ffi::EngineStorageOptions {
        download_root: config.download_root.clone(),
        resume_dir: config.resume_dir.clone(),
    }
}

fn map_tracker_options(config: &TrackerRuntimeConfig) -> ffi::EngineTrackerOptions {
    let proxy = map_proxy(config.proxy.as_ref());
    let auth = map_auth(config.auth.as_ref());
    ffi::EngineTrackerOptions {
        default_trackers: config.default.clone(),
        extra_trackers: config.extra.clone(),
        replace_trackers: config.replace,
        user_agent: config.user_agent.clone().unwrap_or_default(),
        has_user_agent: config.user_agent.is_some(),
        announce_ip: config.announce_ip.clone().unwrap_or_default(),
        has_announce_ip: config.announce_ip.is_some(),
        listen_interface: config.listen_interface.clone().unwrap_or_default(),
        has_listen_interface: config.listen_interface.is_some(),
        request_timeout_ms: config.request_timeout_ms.unwrap_or_default(),
        has_request_timeout: config.request_timeout_ms.is_some(),
        announce_to_all: config.announce_to_all,
        proxy,
        auth,
    }
}

fn map_proxy(proxy: Option<&TrackerProxyRuntime>) -> ffi::TrackerProxyOptions {
    proxy.map_or_else(
        || ffi::TrackerProxyOptions {
            host: String::new(),
            username_secret: String::new(),
            password_secret: String::new(),
            port: 0,
            kind: 0,
            proxy_peers: false,
            has_proxy: false,
        },
        |proxy| ffi::TrackerProxyOptions {
            host: proxy.host.clone(),
            username_secret: proxy.username_secret.clone().unwrap_or_default(),
            password_secret: proxy.password_secret.clone().unwrap_or_default(),
            port: proxy.port,
            kind: map_proxy_kind(proxy.kind),
            proxy_peers: proxy.proxy_peers,
            has_proxy: true,
        },
    )
}

fn map_auth(auth: Option<&TrackerAuthRuntime>) -> ffi::TrackerAuthOptions {
    auth.map_or_else(
        || ffi::TrackerAuthOptions {
            username: String::new(),
            password: String::new(),
            cookie: String::new(),
            username_secret: String::new(),
            password_secret: String::new(),
            cookie_secret: String::new(),
            has_username: false,
            has_password: false,
            has_cookie: false,
        },
        |auth| ffi::TrackerAuthOptions {
            username: auth.username.clone().unwrap_or_default(),
            password: auth.password.clone().unwrap_or_default(),
            cookie: auth.cookie.clone().unwrap_or_default(),
            username_secret: auth.username_secret.clone().unwrap_or_default(),
            password_secret: auth.password_secret.clone().unwrap_or_default(),
            cookie_secret: auth.cookie_secret.clone().unwrap_or_default(),
            has_username: auth.username.is_some(),
            has_password: auth.password.is_some(),
            has_cookie: auth.cookie.is_some(),
        },
    )
}

const fn map_proxy_kind(kind: TrackerProxyType) -> u8 {
    match kind {
        TrackerProxyType::Http => 0,
        TrackerProxyType::Https => 1,
        TrackerProxyType::Socks5 => 2,
    }
}

fn map_ip_filter(ip_filter: Option<&IpFilterRuntimeConfig>) -> (Vec<ffi::IpFilterRule>, bool) {
    ip_filter.map_or((Vec::new(), false), |filter| {
        let rules = filter
            .rules
            .iter()
            .map(map_ip_filter_rule)
            .collect::<Vec<_>>();
        (rules, true)
    })
}

fn map_ip_filter_rule(rule: &RuntimeIpFilterRule) -> ffi::IpFilterRule {
    ffi::IpFilterRule {
        start: rule.start.clone(),
        end: rule.end.clone(),
    }
}

fn map_outgoing_ports(range: Option<OutgoingPortRange>) -> (i32, i32, bool) {
    range.map_or((0, 0, false), |ports| {
        (i32::from(ports.start), i32::from(ports.end), true)
    })
}

fn map_peer_dscp(value: Option<u8>) -> (i32, bool) {
    // DSCP occupies the upper 6 bits of the TOS byte; libtorrent expects the full byte value.
    value.map_or((0, false), |mark| (i32::from(mark) << 2, true))
}

fn map_seed_ratio_limit(value: Option<f64>, warnings: &mut Vec<String>) -> (f64, bool) {
    match value {
        Some(ratio) if ratio.is_finite() && ratio >= 0.0 => (ratio, true),
        Some(ratio) => {
            warnings.push(format!(
                "seed_ratio_limit {ratio} is invalid; disabling ratio stop"
            ));
            (-1.0, false)
        }
        None => (-1.0, false),
    }
}

fn map_seed_time_limit(value: Option<i64>, warnings: &mut Vec<String>) -> (i64, bool) {
    match value {
        Some(limit) if limit >= 0 => {
            if limit > i64::from(i32::MAX) {
                warnings.push(format!(
                    "seed_time_limit {limit} exceeds native bounds; clamping to {}",
                    i32::MAX
                ));
                (i64::from(i32::MAX), true)
            } else {
                (limit, true)
            }
        }
        Some(limit) => {
            warnings.push(format!(
                "seed_time_limit {limit} is negative; disabling seeding timeout"
            ));
            (-1, false)
        }
        None => (-1, false),
    }
}

fn clamp_rate_limit(field: &str, value: Option<i64>, warnings: &mut Vec<String>) -> i64 {
    match value {
        Some(limit) if limit > 0 => limit,
        Some(limit) => {
            warnings.push(format!(
                "{field} {limit} is non-positive; disabling the limit"
            ));
            -1
        }
        None => -1,
    }
}

fn map_positive_limit(field: &str, value: Option<i32>, warnings: &mut Vec<String>) -> i32 {
    match value {
        Some(limit) if limit > 0 => limit,
        Some(limit) => {
            warnings.push(format!(
                "{field} {limit} is non-positive; leaving unlimited"
            ));
            -1
        }
        None => -1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        ChokingAlgorithm, EncryptionPolicy, EngineRuntimeConfig,
        IpFilterRule as RuntimeIpFilterRule, IpFilterRuntimeConfig, Ipv6Mode, SeedChokingAlgorithm,
        TrackerProxyRuntime, TrackerProxyType, TrackerRuntimeConfig,
    };

    #[test]
    fn plan_clamps_invalid_values() {
        let config = EngineRuntimeConfig {
            download_root: "   ".into(),
            resume_dir: String::new(),
            listen_interfaces: Vec::new(),
            ipv6_mode: Ipv6Mode::Disabled,
            enable_dht: true,
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            outgoing_ports: None,
            peer_dscp: None,
            anonymous_mode: false.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            sequential_default: false,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: false.into(),
            dont_count_slow_torrents: true.into(),
            listen_port: Some(70_000),
            max_active: Some(0),
            download_rate_limit: Some(-1),
            upload_rate_limit: None,
            seed_ratio_limit: None,
            seed_time_limit: None,
            alt_speed: None,
            stats_interval_ms: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            choking_algorithm: ChokingAlgorithm::FixedSlots,
            seed_choking_algorithm: SeedChokingAlgorithm::RoundRobin,
            strict_super_seeding: false.into(),
            optimistic_unchoke_slots: None,
            max_queued_disk_bytes: None,
            encryption: EncryptionPolicy::Disable,
            tracker: TrackerRuntimeConfig::default(),
            ip_filter: None,
            super_seeding: false.into(),
        };

        let plan = EngineOptionsPlan::from_runtime_config(&config);
        assert!(
            plan.warnings.iter().any(|msg| msg.contains("listen_port")),
            "invalid listen ports should produce warnings"
        );
        assert!(
            plan.warnings
                .iter()
                .any(|msg| msg.contains("download_root")),
            "empty download roots should be warned about"
        );
        assert_eq!(plan.options.network.listen_port, 0);
        assert!(!plan.options.network.set_listen_port);
        assert_eq!(plan.options.limits.max_active, -1);
        assert_eq!(plan.options.limits.download_rate_limit, -1);
        assert_eq!(plan.options.limits.upload_rate_limit, -1);
        assert_eq!(plan.options.network.encryption_policy, 2);
    }

    #[test]
    fn plan_preserves_valid_values() {
        let config = EngineRuntimeConfig {
            download_root: "/data".into(),
            resume_dir: "/state".into(),
            listen_interfaces: Vec::new(),
            ipv6_mode: Ipv6Mode::Disabled,
            enable_dht: false,
            enable_lsd: true.into(),
            enable_upnp: true.into(),
            enable_natpmp: true.into(),
            enable_pex: true.into(),
            outgoing_ports: Some(OutgoingPortRange {
                start: 6_000,
                end: 6_100,
            }),
            peer_dscp: Some(8),
            anonymous_mode: false.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            dht_bootstrap_nodes: vec!["router.bittorrent.com:6881".into()],
            dht_router_nodes: vec!["dht.transmissionbt.com:6881".into()],
            sequential_default: true,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: true.into(),
            dont_count_slow_torrents: true.into(),
            listen_port: Some(6_881),
            max_active: Some(16),
            download_rate_limit: Some(256_000),
            upload_rate_limit: Some(128_000),
            seed_ratio_limit: None,
            seed_time_limit: None,
            alt_speed: None,
            stats_interval_ms: Some(1_500),
            connections_limit: Some(200),
            connections_limit_per_torrent: Some(80),
            unchoke_slots: Some(40),
            half_open_limit: Some(10),
            choking_algorithm: ChokingAlgorithm::RateBased,
            seed_choking_algorithm: SeedChokingAlgorithm::FastestUpload,
            strict_super_seeding: true.into(),
            optimistic_unchoke_slots: Some(3),
            max_queued_disk_bytes: Some(5_000_000),
            encryption: EncryptionPolicy::Require,
            tracker: TrackerRuntimeConfig::default(),
            ip_filter: None,
            super_seeding: true.into(),
        };

        let plan = EngineOptionsPlan::from_runtime_config(&config);
        assert!(plan.warnings.is_empty());
        assert!(plan.options.network.set_listen_port);
        assert_eq!(plan.options.network.listen_port, 6_881);
        assert_eq!(plan.options.limits.max_active, 16);
        assert_eq!(plan.options.limits.download_rate_limit, 256_000);
        assert_eq!(plan.options.limits.upload_rate_limit, 128_000);
        assert_eq!(plan.options.limits.connections_limit, 200);
        assert_eq!(plan.options.limits.connections_limit_per_torrent, 80);
        assert_eq!(plan.options.limits.unchoke_slots, 40);
        assert_eq!(plan.options.limits.half_open_limit, 10);
        assert_eq!(plan.options.limits.choking_algorithm, 2);
        assert_eq!(plan.options.limits.seed_choking_algorithm, 1);
        assert!(plan.options.limits.strict_super_seeding);
        assert!(plan.options.limits.has_optimistic_unchoke_slots);
        assert_eq!(plan.options.limits.optimistic_unchoke_slots, 3);
        assert!(plan.options.limits.has_max_queued_disk_bytes);
        assert_eq!(plan.options.limits.max_queued_disk_bytes, 5_000_000);
        assert!(plan.options.limits.has_stats_interval);
        assert_eq!(plan.options.limits.stats_interval_ms, 1_500);
        assert!(plan.options.behavior.sequential_default);
        assert!(plan.options.behavior.super_seeding);
        assert_eq!(plan.options.network.encryption_policy, 0);
        assert!(plan.options.network.enable_lsd);
        assert!(plan.options.network.enable_upnp);
        assert!(plan.options.network.enable_natpmp);
        assert!(plan.options.network.enable_pex);
        assert!(plan.options.network.has_outgoing_port_range);
        assert_eq!(plan.options.network.outgoing_port_min, 6_000);
        assert_eq!(plan.options.network.outgoing_port_max, 6_100);
        assert!(plan.options.network.has_peer_dscp);
        assert_eq!(plan.options.network.peer_dscp, 32);
        assert_eq!(
            plan.options.network.dht_bootstrap_nodes,
            vec!["router.bittorrent.com:6881".to_string()]
        );
        assert_eq!(
            plan.options.network.dht_router_nodes,
            vec!["dht.transmissionbt.com:6881".to_string()]
        );
    }

    #[test]
    fn seed_limits_are_forwarded_and_clamped() {
        let config = EngineRuntimeConfig {
            download_root: "/data".into(),
            resume_dir: "/state".into(),
            listen_interfaces: Vec::new(),
            ipv6_mode: Ipv6Mode::Disabled,
            enable_dht: false,
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            outgoing_ports: None,
            peer_dscp: None,
            anonymous_mode: false.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            sequential_default: false,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: false.into(),
            dont_count_slow_torrents: true.into(),
            listen_port: None,
            max_active: None,
            download_rate_limit: None,
            upload_rate_limit: None,
            seed_ratio_limit: Some(1.5),
            seed_time_limit: Some(i64::from(i32::MAX) + 10),
            alt_speed: None,
            stats_interval_ms: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            choking_algorithm: ChokingAlgorithm::FixedSlots,
            seed_choking_algorithm: SeedChokingAlgorithm::RoundRobin,
            strict_super_seeding: false.into(),
            optimistic_unchoke_slots: None,
            max_queued_disk_bytes: None,
            encryption: EncryptionPolicy::Prefer,
            tracker: TrackerRuntimeConfig::default(),
            ip_filter: None,
            super_seeding: false.into(),
        };

        let plan = EngineOptionsPlan::from_runtime_config(&config);
        assert!(plan.options.limits.has_seed_ratio_limit);
        assert!(
            (plan.options.limits.seed_ratio_limit - 1.5).abs() < f64::EPSILON,
            "unexpected clamped ratio: {}",
            plan.options.limits.seed_ratio_limit
        );
        assert!(plan.options.limits.has_seed_time_limit);
        assert_eq!(plan.options.limits.seed_time_limit, i64::from(i32::MAX));
        assert!(
            plan.warnings
                .iter()
                .any(|msg| msg.contains("seed_time_limit") && msg.contains("clamping"))
        );
    }

    #[test]
    fn privacy_toggles_are_forwarded() {
        let config = EngineRuntimeConfig {
            download_root: "/data".into(),
            resume_dir: "/state".into(),
            listen_interfaces: Vec::new(),
            ipv6_mode: Ipv6Mode::Disabled,
            enable_dht: false,
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            outgoing_ports: None,
            peer_dscp: None,
            anonymous_mode: true.into(),
            force_proxy: true.into(),
            prefer_rc4: true.into(),
            allow_multiple_connections_per_ip: true.into(),
            enable_outgoing_utp: true.into(),
            enable_incoming_utp: true.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            sequential_default: false,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: false.into(),
            dont_count_slow_torrents: true.into(),
            listen_port: None,
            max_active: None,
            download_rate_limit: None,
            upload_rate_limit: None,
            seed_ratio_limit: None,
            seed_time_limit: None,
            alt_speed: None,
            stats_interval_ms: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            choking_algorithm: ChokingAlgorithm::FixedSlots,
            seed_choking_algorithm: SeedChokingAlgorithm::RoundRobin,
            strict_super_seeding: false.into(),
            optimistic_unchoke_slots: None,
            max_queued_disk_bytes: None,
            encryption: EncryptionPolicy::Prefer,
            tracker: TrackerRuntimeConfig::default(),
            ip_filter: None,
            super_seeding: false.into(),
        };

        let plan = EngineOptionsPlan::from_runtime_config(&config);
        let network = plan.options.network;
        assert!(network.anonymous_mode);
        assert!(network.force_proxy);
        assert!(network.prefer_rc4);
        assert!(network.allow_multiple_connections_per_ip);
        assert!(network.enable_outgoing_utp);
        assert!(network.enable_incoming_utp);
    }

    #[test]
    fn listen_interfaces_override_port_binding() {
        let config = EngineRuntimeConfig {
            download_root: "/data".into(),
            resume_dir: "/state".into(),
            listen_interfaces: vec!["eth0:7000".into(), "[::]:7000".into()],
            ipv6_mode: Ipv6Mode::Enabled,
            enable_dht: false,
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            outgoing_ports: None,
            peer_dscp: None,
            anonymous_mode: false.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            sequential_default: false,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: false.into(),
            dont_count_slow_torrents: true.into(),
            listen_port: Some(6_881),
            max_active: None,
            download_rate_limit: None,
            upload_rate_limit: None,
            seed_ratio_limit: None,
            seed_time_limit: None,
            alt_speed: None,
            stats_interval_ms: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            choking_algorithm: ChokingAlgorithm::FixedSlots,
            seed_choking_algorithm: SeedChokingAlgorithm::RoundRobin,
            strict_super_seeding: false.into(),
            optimistic_unchoke_slots: None,
            max_queued_disk_bytes: None,
            encryption: EncryptionPolicy::Prefer,
            tracker: TrackerRuntimeConfig::default(),
            ip_filter: None,
            super_seeding: false.into(),
        };

        let plan = EngineOptionsPlan::from_runtime_config(&config);
        assert!(plan.options.network.has_listen_interfaces);
        assert_eq!(plan.options.network.listen_interfaces.len(), 2);
        assert!(!plan.options.network.set_listen_port);
        assert_eq!(plan.options.network.listen_port, 0);
    }

    fn runtime_config_with_tracker(tracker: TrackerRuntimeConfig) -> EngineRuntimeConfig {
        EngineRuntimeConfig {
            download_root: "/data".into(),
            resume_dir: "/state".into(),
            listen_interfaces: Vec::new(),
            ipv6_mode: Ipv6Mode::Disabled,
            enable_dht: false,
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            outgoing_ports: None,
            peer_dscp: None,
            anonymous_mode: false.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            sequential_default: false,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: false.into(),
            dont_count_slow_torrents: true.into(),
            listen_port: None,
            max_active: None,
            download_rate_limit: None,
            upload_rate_limit: None,
            seed_ratio_limit: None,
            seed_time_limit: None,
            alt_speed: None,
            stats_interval_ms: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            choking_algorithm: ChokingAlgorithm::FixedSlots,
            seed_choking_algorithm: SeedChokingAlgorithm::RoundRobin,
            strict_super_seeding: false.into(),
            optimistic_unchoke_slots: None,
            max_queued_disk_bytes: None,
            encryption: EncryptionPolicy::Prefer,
            tracker,
            ip_filter: None,
            super_seeding: false.into(),
        }
    }

    #[test]
    fn tracker_options_are_mapped() {
        let tracker_config = TrackerRuntimeConfig {
            default: vec!["https://tracker.example/announce".into()],
            extra: vec!["udp://tracker.backup/announce".into()],
            replace: true,
            user_agent: Some("revaer/1.0".into()),
            announce_ip: Some("192.168.1.100".into()),
            listen_interface: Some("0.0.0.0:9000".into()),
            request_timeout_ms: Some(5_000),
            announce_to_all: true,
            proxy: Some(TrackerProxyRuntime {
                host: "proxy.example".into(),
                port: 8080,
                username_secret: Some("user".into()),
                password_secret: Some("pass".into()),
                kind: TrackerProxyType::Socks5,
                proxy_peers: true,
            }),
            auth: Some(TrackerAuthRuntime {
                username: Some("resolved-user".into()),
                password: Some("resolved-pass".into()),
                cookie: Some("tracker-cookie".into()),
                username_secret: Some("user-secret".into()),
                password_secret: Some("pass-secret".into()),
                cookie_secret: Some("cookie-secret".into()),
            }),
        };
        let config = runtime_config_with_tracker(tracker_config);

        let plan = EngineOptionsPlan::from_runtime_config(&config);
        let tracker = plan.options.tracker;
        assert_eq!(
            tracker.default_trackers,
            vec!["https://tracker.example/announce"]
        );
        assert_eq!(
            tracker.extra_trackers,
            vec!["udp://tracker.backup/announce".to_string()]
        );
        assert!(tracker.replace_trackers);
        assert_eq!(tracker.user_agent, "revaer/1.0");
        assert!(tracker.has_user_agent);
        assert_eq!(tracker.announce_ip, "192.168.1.100");
        assert!(tracker.has_announce_ip);
        assert_eq!(tracker.listen_interface, "0.0.0.0:9000");
        assert!(tracker.has_listen_interface);
        assert_eq!(tracker.request_timeout_ms, 5_000);
        assert!(tracker.has_request_timeout);
        assert!(tracker.announce_to_all);
        assert!(tracker.proxy.has_proxy);
        assert_eq!(tracker.proxy.host, "proxy.example");
        assert_eq!(tracker.proxy.port, 8080);
        assert_eq!(tracker.proxy.username_secret, "user");
        assert_eq!(tracker.proxy.password_secret, "pass");
        assert!(tracker.proxy.proxy_peers);
        assert_eq!(tracker.proxy.kind, 2);
        assert!(tracker.auth.has_username);
        assert!(tracker.auth.has_password);
        assert!(tracker.auth.has_cookie);
        assert_eq!(tracker.auth.username, "resolved-user");
        assert_eq!(tracker.auth.password, "resolved-pass");
        assert_eq!(tracker.auth.cookie, "tracker-cookie");
        assert_eq!(tracker.auth.username_secret, "user-secret");
        assert_eq!(tracker.auth.password_secret, "pass-secret");
        assert_eq!(tracker.auth.cookie_secret, "cookie-secret");
    }

    #[test]
    fn ip_filter_rules_are_forwarded() {
        let config = EngineRuntimeConfig {
            download_root: "/data".into(),
            resume_dir: "/state".into(),
            listen_interfaces: Vec::new(),
            ipv6_mode: Ipv6Mode::Disabled,
            enable_dht: false,
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            outgoing_ports: None,
            peer_dscp: None,
            anonymous_mode: false.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            sequential_default: false,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: false.into(),
            dont_count_slow_torrents: true.into(),
            listen_port: None,
            max_active: None,
            download_rate_limit: None,
            upload_rate_limit: None,
            seed_ratio_limit: None,
            seed_time_limit: None,
            alt_speed: None,
            stats_interval_ms: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            choking_algorithm: ChokingAlgorithm::FixedSlots,
            seed_choking_algorithm: SeedChokingAlgorithm::RoundRobin,
            strict_super_seeding: false.into(),
            optimistic_unchoke_slots: None,
            max_queued_disk_bytes: None,
            encryption: EncryptionPolicy::Prefer,
            tracker: TrackerRuntimeConfig::default(),
            ip_filter: Some(IpFilterRuntimeConfig {
                rules: vec![RuntimeIpFilterRule {
                    start: "10.0.0.1".into(),
                    end: "10.0.0.1".into(),
                }],
                blocklist_url: Some("https://example.com/list".into()),
                etag: Some("v1".into()),
                last_updated_at: Some("2024-01-01T00:00:00Z".into()),
            }),
            super_seeding: false.into(),
        };

        let plan = EngineOptionsPlan::from_runtime_config(&config);
        assert!(plan.options.network.has_ip_filter);
        assert_eq!(plan.options.network.ip_filter_rules.len(), 1);
        assert_eq!(plan.options.network.ip_filter_rules[0].start, "10.0.0.1");
        assert_eq!(plan.options.network.ip_filter_rules[0].end, "10.0.0.1");
    }
}
