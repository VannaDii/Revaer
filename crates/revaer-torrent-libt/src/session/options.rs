//! Guard rails for translating runtime config into native engine options.
//!
//! # Design
//! - Clamps or disables invalid values to keep the native session stable.
//! - Collects warnings so callers can surface guard-rail applications.
//! - Keeps the runtime→FFI mapping central to avoid drift as fields grow.

use crate::ffi::ffi;
use crate::types::EngineRuntimeConfig;

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

        let options = ffi::EngineOptions {
            network: ffi::EngineNetworkOptions {
                listen_port,
                set_listen_port,
                enable_dht: config.enable_dht,
                encryption_policy: config.encryption.as_u8(),
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
        };

        Self { options, warnings }
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
    use crate::types::{EncryptionPolicy, EngineRuntimeConfig};

    #[test]
    fn plan_clamps_invalid_values() {
        let config = EngineRuntimeConfig {
            download_root: "   ".into(),
            resume_dir: String::new(),
            enable_dht: true,
            sequential_default: false,
            listen_port: Some(70_000),
            max_active: Some(0),
            download_rate_limit: Some(-1),
            upload_rate_limit: None,
            encryption: EncryptionPolicy::Disable,
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
            sequential_default: true,
            listen_port: Some(6_881),
            max_active: Some(16),
            download_rate_limit: Some(256_000),
            upload_rate_limit: Some(128_000),
            encryption: EncryptionPolicy::Require,
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
    }
}
