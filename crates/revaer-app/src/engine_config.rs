//! Engine profile normalisation and runtime mapping helpers.
//!
//! # Design
//! - Derives a runtime configuration from the stored engine profile while applying guard rails.
//! - Carries the effective profile (including warnings) for observability and API responses.
//! - Keeps encryption mapping centralised to avoid drift between API/config/runtime layers.

use revaer_config::{
    EngineEncryptionPolicy, EngineProfile, EngineProfileEffective, normalize_engine_profile,
};
use revaer_torrent_core::TorrentRateLimit;
use revaer_torrent_libt::{EncryptionPolicy, EngineRuntimeConfig};

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
        let runtime = EngineRuntimeConfig {
            download_root: effective.storage.download_root.clone(),
            resume_dir: effective.storage.resume_dir.clone(),
            enable_dht: effective.network.enable_dht,
            sequential_default: effective.behavior.sequential_default,
            listen_port: effective.network.listen_port,
            max_active: effective.limits.max_active,
            download_rate_limit: effective.limits.download_rate_limit,
            upload_rate_limit: effective.limits.upload_rate_limit,
            encryption: map_encryption_policy(effective.network.encryption),
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
            dht: true,
            encryption: "unknown".into(),
            max_active: Some(0),
            max_download_bps: Some(MAX_RATE_LIMIT_BPS + 10),
            max_upload_bps: Some(0),
            sequential_default: true,
            resume_dir: String::new(),
            download_root: "   ".into(),
            tracker: json!("not-an-object"),
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
    }

    #[test]
    fn encryption_policy_maps_variants() {
        let base = EngineProfile {
            id: Uuid::new_v4(),
            implementation: "libtorrent".into(),
            listen_port: None,
            dht: false,
            encryption: "require".into(),
            max_active: None,
            max_download_bps: None,
            max_upload_bps: None,
            sequential_default: false,
            resume_dir: "/var/resume".into(),
            download_root: "/data".into(),
            tracker: json!({}),
        };

        let require = EngineRuntimePlan::from_profile(&base);
        assert_eq!(require.runtime.encryption, EncryptionPolicy::Require);

        let mut prefer_profile = base.clone();
        prefer_profile.encryption = "prefer".into();
        let prefer = EngineRuntimePlan::from_profile(&prefer_profile);
        assert_eq!(prefer.runtime.encryption, EncryptionPolicy::Prefer);

        let mut disable_profile = base;
        disable_profile.encryption = "disable".into();
        let disable = EngineRuntimePlan::from_profile(&disable_profile);
        assert_eq!(disable.runtime.encryption, EncryptionPolicy::Disable);
    }
}
