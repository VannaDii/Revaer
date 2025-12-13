//! Guard rails for translating runtime config into native engine options.
//!
//! # Design
//! - Clamps or disables invalid values to keep the native session stable.
//! - Collects warnings so callers can surface guard-rail applications.
//! - Keeps the runtime→FFI mapping central to avoid drift as fields grow.

use crate::ffi::ffi;
use crate::types::{
    EngineRuntimeConfig, IpFilterRule as RuntimeIpFilterRule, IpFilterRuntimeConfig,
    TrackerProxyRuntime, TrackerProxyType, TrackerRuntimeConfig,
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

        let (listen_port, set_listen_port) = match config.listen_port {
            Some(port) if (1..=65_535).contains(&port) => (port, true),
            Some(port) => {
                warnings.push(format!(
                    "listen_port {port} is out of range; skipping listen override"
                ));
                (0, false)
            }
            None => (0, false),
        };

        let max_active = match config.max_active {
            Some(limit) if limit > 0 => limit,
            Some(limit) => {
                warnings.push(format!(
                    "max_active {limit} is non-positive; leaving unlimited"
                ));
                -1
            }
            None => -1,
        };

        let download_rate_limit = clamp_rate_limit(
            "download_rate_limit",
            config.download_rate_limit,
            &mut warnings,
        );
        let upload_rate_limit =
            clamp_rate_limit("upload_rate_limit", config.upload_rate_limit, &mut warnings);

        if config.download_root.trim().is_empty() {
            warnings.push(
                "download_root is empty; native session will fall back to its defaults".to_string(),
            );
        }
        if config.resume_dir.trim().is_empty() {
            warnings.push(
                "resume_dir is empty; native session will fall back to its defaults".to_string(),
            );
        }

        let (ip_filter_rules, has_ip_filter) = map_ip_filter(config.ip_filter.as_ref());

        let options = ffi::EngineOptions {
            network: ffi::EngineNetworkOptions {
                listen_port,
                set_listen_port,
                enable_dht: config.enable_dht,
                enable_lsd: bool::from(config.enable_lsd),
                enable_upnp: bool::from(config.enable_upnp),
                enable_natpmp: bool::from(config.enable_natpmp),
                enable_pex: bool::from(config.enable_pex),
                dht_bootstrap_nodes: config.dht_bootstrap_nodes.clone(),
                dht_router_nodes: config.dht_router_nodes.clone(),
                encryption_policy: config.encryption.as_u8(),
                ip_filter_rules,
                has_ip_filter,
            },
            limits: ffi::EngineLimitOptions {
                max_active,
                download_rate_limit,
                upload_rate_limit,
            },
            storage: ffi::EngineStorageOptions {
                download_root: config.download_root.clone(),
                resume_dir: config.resume_dir.clone(),
            },
            behavior: ffi::EngineBehaviorOptions {
                sequential_default: config.sequential_default,
            },
            tracker: map_tracker_options(&config.tracker),
        };

        Self { options, warnings }
    }
}

fn map_tracker_options(config: &TrackerRuntimeConfig) -> ffi::EngineTrackerOptions {
    let proxy = map_proxy(config.proxy.as_ref());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        EncryptionPolicy, EngineRuntimeConfig, IpFilterRule as RuntimeIpFilterRule,
        IpFilterRuntimeConfig, TrackerProxyRuntime, TrackerProxyType, TrackerRuntimeConfig,
    };

    #[test]
    fn plan_clamps_invalid_values() {
        let config = EngineRuntimeConfig {
            download_root: "   ".into(),
            resume_dir: String::new(),
            enable_dht: true,
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            sequential_default: false,
            listen_port: Some(70_000),
            max_active: Some(0),
            download_rate_limit: Some(-1),
            upload_rate_limit: None,
            encryption: EncryptionPolicy::Disable,
            tracker: TrackerRuntimeConfig::default(),
            ip_filter: None,
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
            enable_dht: false,
            enable_lsd: true.into(),
            enable_upnp: true.into(),
            enable_natpmp: true.into(),
            enable_pex: true.into(),
            dht_bootstrap_nodes: vec!["router.bittorrent.com:6881".into()],
            dht_router_nodes: vec!["dht.transmissionbt.com:6881".into()],
            sequential_default: true,
            listen_port: Some(6_881),
            max_active: Some(16),
            download_rate_limit: Some(256_000),
            upload_rate_limit: Some(128_000),
            encryption: EncryptionPolicy::Require,
            tracker: TrackerRuntimeConfig::default(),
            ip_filter: None,
        };

        let plan = EngineOptionsPlan::from_runtime_config(&config);
        assert!(plan.warnings.is_empty());
        assert!(plan.options.network.set_listen_port);
        assert_eq!(plan.options.network.listen_port, 6_881);
        assert_eq!(plan.options.limits.max_active, 16);
        assert_eq!(plan.options.limits.download_rate_limit, 256_000);
        assert_eq!(plan.options.limits.upload_rate_limit, 128_000);
        assert!(plan.options.behavior.sequential_default);
        assert_eq!(plan.options.network.encryption_policy, 0);
        assert!(plan.options.network.enable_lsd);
        assert!(plan.options.network.enable_upnp);
        assert!(plan.options.network.enable_natpmp);
        assert!(plan.options.network.enable_pex);
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
    fn tracker_options_are_mapped() {
        let config = EngineRuntimeConfig {
            download_root: "/data".into(),
            resume_dir: "/state".into(),
            enable_dht: false,
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            sequential_default: false,
            listen_port: None,
            max_active: None,
            download_rate_limit: None,
            upload_rate_limit: None,
            encryption: EncryptionPolicy::Prefer,
            tracker: TrackerRuntimeConfig {
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
            },
            ip_filter: None,
        };

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
    }

    #[test]
    fn ip_filter_rules_are_forwarded() {
        let config = EngineRuntimeConfig {
            download_root: "/data".into(),
            resume_dir: "/state".into(),
            enable_dht: false,
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            sequential_default: false,
            listen_port: None,
            max_active: None,
            download_rate_limit: None,
            upload_rate_limit: None,
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
        };

        let plan = EngineOptionsPlan::from_runtime_config(&config);
        assert!(plan.options.network.has_ip_filter);
        assert_eq!(plan.options.network.ip_filter_rules.len(), 1);
        assert_eq!(plan.options.network.ip_filter_rules[0].start, "10.0.0.1");
        assert_eq!(plan.options.network.ip_filter_rules[0].end, "10.0.0.1");
    }
}
