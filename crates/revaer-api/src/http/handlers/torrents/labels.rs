//! Category/tag policy parsing and application helpers.
//!
//! # Design
//! - Normalize label names to trimmed, non-empty values and validate policy bounds up front.
//! - Apply policies as defaults only, so explicit request overrides always win.
//! - Fail fast on malformed config or invalid policies and surface consistent API errors.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use tracing::error;

use crate::app::state::ApiState;
use crate::http::auth::map_config_error;
use crate::http::errors::ApiError;
use revaer_config::{LabelKind, LabelPolicy, SettingsChangeset};
use revaer_events::Event as CoreEvent;
use revaer_torrent_core::{
    AddTorrentOptions, TorrentCleanupPolicy, TorrentLabelPolicy, TorrentRateLimit,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(in crate::http) struct TorrentLabelCatalog {
    #[serde(default, rename = "torrent_categories")]
    pub(crate) categories: HashMap<String, TorrentLabelPolicy>,
    #[serde(default, rename = "torrent_tags")]
    pub(crate) tags: HashMap<String, TorrentLabelPolicy>,
}

impl TorrentLabelCatalog {
    pub(in crate::http) fn from_label_policies(policies: &[LabelPolicy]) -> Result<Self, ApiError> {
        let mut catalog = Self::default();
        for policy in policies {
            let name = normalize_label_name(policy.kind.as_str(), &policy.name)?;
            let torrent_policy = label_policy_to_torrent(policy);
            match policy.kind {
                LabelKind::Category => {
                    catalog.categories.insert(name, torrent_policy);
                }
                LabelKind::Tag => {
                    catalog.tags.insert(name, torrent_policy);
                }
            }
        }
        Ok(catalog)
    }

    pub(in crate::http) fn to_label_policies(&self) -> Vec<LabelPolicy> {
        let mut policies = Vec::new();
        for (name, policy) in &self.categories {
            policies.push(torrent_policy_to_label(LabelKind::Category, name, policy));
        }
        for (name, policy) in &self.tags {
            policies.push(torrent_policy_to_label(LabelKind::Tag, name, policy));
        }
        policies
    }

    pub(in crate::http) fn upsert_category(
        &mut self,
        name: &str,
        policy: TorrentLabelPolicy,
    ) -> Result<(), ApiError> {
        let name = normalize_label_name("category", name)?;
        validate_label_policy(&policy)?;
        self.categories.insert(name, policy);
        Ok(())
    }

    pub(in crate::http) fn upsert_tag(
        &mut self,
        name: &str,
        policy: TorrentLabelPolicy,
    ) -> Result<(), ApiError> {
        let name = normalize_label_name("tag", name)?;
        validate_label_policy(&policy)?;
        self.tags.insert(name, policy);
        Ok(())
    }
}

pub(in crate::http) fn apply_label_policies(
    catalog: &TorrentLabelCatalog,
    options: &mut AddTorrentOptions,
) {
    if let Some(category) = options.category.as_deref() {
        let trimmed = category.trim();
        if let Some(policy) = catalog.categories.get(trimmed) {
            apply_label_policy(options, policy);
        }
    }

    if options.tags.is_empty() {
        return;
    }

    let tags: BTreeSet<String> = options
        .tags
        .iter()
        .map(|tag| tag.trim())
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect();
    for tag in tags {
        if let Some(policy) = catalog.tags.get(tag.as_str()) {
            apply_label_policy(options, policy);
        }
    }
}

pub(in crate::http) async fn load_label_catalog(
    state: &ApiState,
) -> Result<TorrentLabelCatalog, ApiError> {
    let profile = state.config.get_app_profile().await.map_err(|err| {
        error!(error = %err, "failed to load app profile for labels");
        ApiError::internal("failed to load app profile")
    })?;
    TorrentLabelCatalog::from_label_policies(&profile.label_policies)
}

pub(in crate::http) async fn update_label_catalog<F>(
    state: &ApiState,
    actor: &str,
    reason: &str,
    mutator: F,
) -> Result<TorrentLabelCatalog, ApiError>
where
    F: FnOnce(&mut TorrentLabelCatalog) -> Result<(), ApiError>,
{
    let profile = state.config.get_app_profile().await.map_err(|err| {
        error!(error = %err, "failed to load app profile for label update");
        ApiError::internal("failed to load app profile")
    })?;
    let mut catalog = TorrentLabelCatalog::from_label_policies(&profile.label_policies)?;
    mutator(&mut catalog)?;
    let mut updated_profile = profile.clone();
    updated_profile.label_policies = catalog.to_label_policies();
    let changeset = SettingsChangeset {
        app_profile: Some(updated_profile),
        ..SettingsChangeset::default()
    };
    state
        .config
        .apply_changeset(actor, reason, changeset)
        .await
        .map_err(|err| map_config_error(&err, "failed to update torrent labels"))?;
    state.publish_event(CoreEvent::SettingsChanged {
        description: format!("torrent labels updated by {actor}"),
    });
    Ok(catalog)
}

fn apply_label_policy(options: &mut AddTorrentOptions, policy: &TorrentLabelPolicy) {
    if options.download_dir.is_none() {
        options.download_dir.clone_from(&policy.download_dir);
    }
    if let Some(rate_limit) = policy.rate_limit.as_ref() {
        if options.rate_limit.download_bps.is_none() {
            options.rate_limit.download_bps = rate_limit.download_bps;
        }
        if options.rate_limit.upload_bps.is_none() {
            options.rate_limit.upload_bps = rate_limit.upload_bps;
        }
    }
    if options.queue_position.is_none() {
        options.queue_position = policy.queue_position;
    }
    if options.auto_managed.is_none() {
        options.auto_managed = policy.auto_managed;
    }
    if options.seed_ratio_limit.is_none() {
        options.seed_ratio_limit = policy.seed_ratio_limit;
    }
    if options.seed_time_limit.is_none() {
        options.seed_time_limit = policy.seed_time_limit;
    }
    if options.cleanup.is_none() {
        options.cleanup.clone_from(&policy.cleanup);
    }
}

fn label_policy_to_torrent(policy: &LabelPolicy) -> TorrentLabelPolicy {
    let rate_limit =
        if policy.rate_limit_download_bps.is_some() || policy.rate_limit_upload_bps.is_some() {
            Some(TorrentRateLimit {
                download_bps: policy
                    .rate_limit_download_bps
                    .and_then(|value| u64::try_from(value).ok()),
                upload_bps: policy
                    .rate_limit_upload_bps
                    .and_then(|value| u64::try_from(value).ok()),
            })
        } else {
            None
        };

    let remove_data = policy.cleanup_remove_data.unwrap_or(false);
    let cleanup = if policy.cleanup_seed_ratio_limit.is_some()
        || policy.cleanup_seed_time_limit.is_some()
        || remove_data
    {
        Some(TorrentCleanupPolicy {
            seed_ratio_limit: policy.cleanup_seed_ratio_limit,
            seed_time_limit: policy
                .cleanup_seed_time_limit
                .and_then(|value| u64::try_from(value).ok()),
            remove_data,
        })
    } else {
        None
    };

    TorrentLabelPolicy {
        download_dir: policy.download_dir.clone(),
        rate_limit,
        queue_position: policy.queue_position,
        auto_managed: policy.auto_managed,
        seed_ratio_limit: policy.seed_ratio_limit,
        seed_time_limit: policy
            .seed_time_limit
            .and_then(|value| u64::try_from(value).ok()),
        cleanup,
    }
}

fn torrent_policy_to_label(
    kind: LabelKind,
    name: &str,
    policy: &TorrentLabelPolicy,
) -> LabelPolicy {
    let rate_limit_download_bps = policy
        .rate_limit
        .as_ref()
        .and_then(|limit| limit.download_bps)
        .and_then(|value| i64::try_from(value).ok());
    let rate_limit_upload_bps = policy
        .rate_limit
        .as_ref()
        .and_then(|limit| limit.upload_bps)
        .and_then(|value| i64::try_from(value).ok());

    let cleanup_seed_ratio_limit = policy
        .cleanup
        .as_ref()
        .and_then(|cleanup| cleanup.seed_ratio_limit);
    let cleanup_seed_time_limit = policy
        .cleanup
        .as_ref()
        .and_then(|cleanup| cleanup.seed_time_limit)
        .and_then(|value| i64::try_from(value).ok());
    let cleanup_remove_data = policy.cleanup.as_ref().map(|cleanup| cleanup.remove_data);

    LabelPolicy {
        kind,
        name: name.to_string(),
        download_dir: policy.download_dir.clone(),
        rate_limit_download_bps,
        rate_limit_upload_bps,
        queue_position: policy.queue_position,
        auto_managed: policy.auto_managed,
        seed_ratio_limit: policy.seed_ratio_limit,
        seed_time_limit: policy
            .seed_time_limit
            .and_then(|value| i64::try_from(value).ok()),
        cleanup_seed_ratio_limit,
        cleanup_seed_time_limit,
        cleanup_remove_data,
    }
}

pub(in crate::http) fn normalize_label_name(kind: &str, raw: &str) -> Result<String, ApiError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request("label name must not be empty")
            .with_context_field("label_kind", kind));
    }
    Ok(trimmed.to_string())
}

fn validate_label_policy(policy: &TorrentLabelPolicy) -> Result<(), ApiError> {
    if let Some(download_dir) = policy.download_dir.as_ref()
        && download_dir.trim().is_empty()
    {
        return Err(ApiError::bad_request("download_dir must not be empty"));
    }
    if let Some(queue_position) = policy.queue_position
        && queue_position < 0
    {
        return Err(ApiError::bad_request(
            "queue_position must be zero or a positive integer",
        ));
    }
    if let Some(seed_ratio_limit) = policy.seed_ratio_limit {
        ensure_ratio_limit(seed_ratio_limit, "seed_ratio_limit")?;
    }
    if let Some(cleanup) = policy.cleanup.as_ref() {
        validate_cleanup_policy(cleanup)?;
    }
    Ok(())
}

fn validate_cleanup_policy(cleanup: &TorrentCleanupPolicy) -> Result<(), ApiError> {
    if cleanup.seed_ratio_limit.is_none() && cleanup.seed_time_limit.is_none() {
        return Err(ApiError::bad_request(
            "cleanup policy requires seed_ratio_limit or seed_time_limit",
        ));
    }
    if let Some(seed_ratio_limit) = cleanup.seed_ratio_limit {
        ensure_ratio_limit(seed_ratio_limit, "cleanup.seed_ratio_limit")?;
    }
    Ok(())
}

fn ensure_ratio_limit(value: f64, field: &str) -> Result<(), ApiError> {
    if value < 0.0 || !value.is_finite() {
        return Err(
            ApiError::bad_request("ratio limit must be a non-negative number")
                .with_context_field("field", field),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::indexers::test_indexers;
    use crate::app::state::ApiState;
    use crate::config::ConfigFacade;
    use anyhow::{Result, anyhow};
    use async_trait::async_trait;
    use revaer_config::{
        ApiKeyAuth, AppAuthMode, AppMode, AppProfile, AppliedChanges, ConfigError, ConfigResult,
        ConfigSnapshot, SetupToken, TelemetryConfig,
    };
    use revaer_events::EventBus;
    use revaer_telemetry::Metrics;
    use serde_json::json;
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex as AsyncMutex;
    use tokio::time::timeout;
    use tokio_stream::StreamExt;
    use uuid::Uuid;

    #[derive(Clone, Default)]
    struct LabelConfig {
        label_policies: Arc<AsyncMutex<Vec<LabelPolicy>>>,
        fail_profile: bool,
        fail_apply: bool,
    }

    #[async_trait]
    impl ConfigFacade for LabelConfig {
        async fn get_app_profile(&self) -> ConfigResult<AppProfile> {
            if self.fail_profile {
                return Err(ConfigError::Io {
                    operation: "config.get_app_profile",
                    source: std::io::Error::other("profile failure"),
                });
            }
            Ok(AppProfile {
                id: Uuid::new_v4(),
                instance_name: "labels".to_string(),
                mode: AppMode::Active,
                auth_mode: AppAuthMode::ApiKey,
                version: 1,
                http_port: 8080,
                bind_addr: IpAddr::V4(Ipv4Addr::LOCALHOST),
                local_networks: vec!["127.0.0.0/8".to_string()],
                telemetry: TelemetryConfig::default(),
                label_policies: self.label_policies.lock().await.clone(),
                immutable_keys: Vec::new(),
            })
        }

        async fn issue_setup_token(&self, _: Duration, _: &str) -> ConfigResult<SetupToken> {
            Err(ConfigError::InvalidField {
                section: "labels".to_string(),
                field: "setup_token".to_string(),
                value: None,
                reason: "not implemented",
            })
        }

        async fn validate_setup_token(&self, _: &str) -> ConfigResult<()> {
            Err(ConfigError::SetupTokenInvalid)
        }

        async fn consume_setup_token(&self, _: &str) -> ConfigResult<()> {
            Err(ConfigError::SetupTokenInvalid)
        }

        async fn apply_changeset(
            &self,
            _: &str,
            _: &str,
            changeset: SettingsChangeset,
        ) -> ConfigResult<AppliedChanges> {
            if self.fail_apply {
                return Err(ConfigError::InvalidField {
                    section: "labels".to_string(),
                    field: "changeset".to_string(),
                    value: None,
                    reason: "apply failed",
                });
            }
            if let Some(app_profile) = changeset.app_profile.as_ref() {
                *self.label_policies.lock().await = app_profile.label_policies.clone();
            }
            Ok(AppliedChanges {
                revision: 1,
                app_profile: Some(self.get_app_profile().await?),
                engine_profile: None,
                fs_policy: None,
            })
        }

        async fn snapshot(&self) -> ConfigResult<ConfigSnapshot> {
            Err(ConfigError::InvalidField {
                section: "labels".to_string(),
                field: "snapshot".to_string(),
                value: None,
                reason: "not implemented",
            })
        }

        async fn authenticate_api_key(&self, _: &str, _: &str) -> ConfigResult<Option<ApiKeyAuth>> {
            Ok(None)
        }

        async fn has_api_keys(&self) -> ConfigResult<bool> {
            Ok(true)
        }

        async fn factory_reset(&self) -> ConfigResult<()> {
            Err(ConfigError::InvalidField {
                section: "labels".to_string(),
                field: "factory_reset".to_string(),
                value: None,
                reason: "not implemented",
            })
        }
    }

    fn api_state(config: Arc<dyn ConfigFacade>) -> Result<Arc<ApiState>> {
        Ok(Arc::new(ApiState::new(
            config,
            test_indexers(),
            Metrics::new().map_err(|err| anyhow!(err))?,
            Arc::new(json!({})),
            EventBus::with_capacity(8),
            None,
        )))
    }

    fn sample_policy() -> TorrentLabelPolicy {
        TorrentLabelPolicy {
            download_dir: Some(".server_root/downloads/movies".to_string()),
            rate_limit: Some(TorrentRateLimit {
                download_bps: Some(1_000),
                upload_bps: Some(2_000),
            }),
            queue_position: Some(3),
            auto_managed: Some(false),
            seed_ratio_limit: Some(1.5),
            seed_time_limit: Some(7_200),
            cleanup: Some(TorrentCleanupPolicy {
                seed_ratio_limit: Some(2.0),
                seed_time_limit: Some(3_600),
                remove_data: true,
            }),
        }
    }

    #[test]
    fn catalog_round_trips_label_policies() -> Result<()> {
        let policies = vec![
            LabelPolicy {
                kind: LabelKind::Category,
                name: " movies ".to_string(),
                download_dir: Some(".server_root/downloads/movies".to_string()),
                rate_limit_download_bps: Some(1_000),
                rate_limit_upload_bps: Some(2_000),
                queue_position: Some(3),
                auto_managed: Some(false),
                seed_ratio_limit: Some(1.5),
                seed_time_limit: Some(7_200),
                cleanup_seed_ratio_limit: Some(2.0),
                cleanup_seed_time_limit: Some(3_600),
                cleanup_remove_data: Some(true),
            },
            LabelPolicy {
                kind: LabelKind::Tag,
                name: " featured ".to_string(),
                download_dir: None,
                rate_limit_download_bps: Some(500),
                rate_limit_upload_bps: None,
                queue_position: None,
                auto_managed: Some(true),
                seed_ratio_limit: None,
                seed_time_limit: None,
                cleanup_seed_ratio_limit: None,
                cleanup_seed_time_limit: None,
                cleanup_remove_data: None,
            },
        ];

        let catalog = TorrentLabelCatalog::from_label_policies(&policies)?;
        assert!(catalog.categories.contains_key("movies"));
        assert!(catalog.tags.contains_key("featured"));

        let round_trip = catalog.to_label_policies();
        assert_eq!(round_trip.len(), 2);

        let category = round_trip
            .iter()
            .find(|policy| policy.kind == LabelKind::Category)
            .ok_or_else(|| anyhow!("missing category policy"))?;
        assert_eq!(category.name, "movies");
        assert_eq!(category.rate_limit_download_bps, Some(1_000));
        assert_eq!(category.cleanup_seed_time_limit, Some(3_600));
        assert_eq!(category.cleanup_remove_data, Some(true));

        let tag = round_trip
            .iter()
            .find(|policy| policy.kind == LabelKind::Tag)
            .ok_or_else(|| anyhow!("missing tag policy"))?;
        assert_eq!(tag.name, "featured");
        assert_eq!(tag.rate_limit_download_bps, Some(500));
        assert_eq!(tag.auto_managed, Some(true));
        Ok(())
    }

    #[test]
    fn normalize_and_validate_label_policy_rejects_invalid_values() {
        let blank = normalize_label_name("category", "   ").expect_err("blank names must fail");
        assert_eq!(blank.status(), axum::http::StatusCode::BAD_REQUEST);
        assert_eq!(blank.detail(), Some("label name must not be empty"));

        let empty_dir = TorrentLabelPolicy {
            download_dir: Some("   ".to_string()),
            ..TorrentLabelPolicy::default()
        };
        assert_eq!(
            validate_label_policy(&empty_dir)
                .expect_err("blank download dir must fail")
                .detail(),
            Some("download_dir must not be empty")
        );

        let bad_queue = TorrentLabelPolicy {
            queue_position: Some(-1),
            ..TorrentLabelPolicy::default()
        };
        assert_eq!(
            validate_label_policy(&bad_queue)
                .expect_err("negative queue must fail")
                .detail(),
            Some("queue_position must be zero or a positive integer")
        );

        let bad_ratio = TorrentLabelPolicy {
            seed_ratio_limit: Some(f64::NAN),
            ..TorrentLabelPolicy::default()
        };
        assert_eq!(
            validate_label_policy(&bad_ratio)
                .expect_err("nan ratio must fail")
                .detail(),
            Some("ratio limit must be a non-negative number")
        );

        let cleanup_missing_threshold = TorrentLabelPolicy {
            cleanup: Some(TorrentCleanupPolicy {
                seed_ratio_limit: None,
                seed_time_limit: None,
                remove_data: true,
            }),
            ..TorrentLabelPolicy::default()
        };
        assert_eq!(
            validate_label_policy(&cleanup_missing_threshold)
                .expect_err("cleanup without thresholds must fail")
                .detail(),
            Some("cleanup policy requires seed_ratio_limit or seed_time_limit")
        );
    }

    #[test]
    fn apply_label_policies_only_fills_missing_values() {
        let mut catalog = TorrentLabelCatalog::default();
        catalog.categories.insert(
            "movies".to_string(),
            TorrentLabelPolicy {
                queue_position: Some(7),
                auto_managed: Some(false),
                download_dir: Some(".server_root/downloads/category".to_string()),
                ..TorrentLabelPolicy::default()
            },
        );
        catalog.tags.insert(
            "featured".to_string(),
            TorrentLabelPolicy {
                rate_limit: Some(TorrentRateLimit {
                    download_bps: Some(100),
                    upload_bps: Some(200),
                }),
                cleanup: Some(TorrentCleanupPolicy {
                    seed_ratio_limit: Some(1.2),
                    seed_time_limit: Some(3_600),
                    remove_data: false,
                }),
                ..TorrentLabelPolicy::default()
            },
        );

        let mut options = AddTorrentOptions {
            category: Some(" movies ".to_string()),
            tags: vec![
                "featured".to_string(),
                " featured ".to_string(),
                String::new(),
            ],
            download_dir: Some(".server_root/downloads/explicit".to_string()),
            rate_limit: TorrentRateLimit {
                download_bps: None,
                upload_bps: Some(999),
            },
            ..AddTorrentOptions::default()
        };

        apply_label_policies(&catalog, &mut options);

        assert_eq!(
            options.download_dir.as_deref(),
            Some(".server_root/downloads/explicit")
        );
        assert_eq!(options.queue_position, Some(7));
        assert_eq!(options.auto_managed, Some(false));
        assert_eq!(options.rate_limit.download_bps, Some(100));
        assert_eq!(options.rate_limit.upload_bps, Some(999));
        assert_eq!(
            options.cleanup,
            Some(TorrentCleanupPolicy {
                seed_ratio_limit: Some(1.2),
                seed_time_limit: Some(3_600),
                remove_data: false,
            })
        );
    }

    #[tokio::test]
    async fn load_label_catalog_returns_internal_error_when_profile_fails() -> Result<()> {
        let state = api_state(Arc::new(LabelConfig {
            fail_profile: true,
            ..LabelConfig::default()
        }))?;

        let err = load_label_catalog(&state)
            .await
            .expect_err("profile failure should surface");
        assert_eq!(err.status(), axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(err.detail(), Some("failed to load app profile"));
        Ok(())
    }

    #[tokio::test]
    async fn update_label_catalog_persists_changes_and_emits_event() -> Result<()> {
        let config = Arc::new(LabelConfig::default());
        let state = api_state(config.clone())?;
        let mut stream = state.events.subscribe(None);

        let catalog = update_label_catalog(&state, "alice", "update labels", |catalog| {
            catalog.upsert_category(" movies ", sample_policy())
        })
        .await?;

        assert!(catalog.categories.contains_key("movies"));

        let stored = config.label_policies.lock().await.clone();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].kind, LabelKind::Category);
        assert_eq!(stored[0].name, "movies");
        assert_eq!(stored[0].rate_limit_download_bps, Some(1_000));

        let envelope = timeout(Duration::from_secs(1), stream.next())
            .await?
            .ok_or_else(|| anyhow!("expected event payload"))?
            .map_err(|err| anyhow!(err))?;
        assert!(matches!(
            envelope.event,
            CoreEvent::SettingsChanged { ref description }
                if description == "torrent labels updated by alice"
        ));
        Ok(())
    }

    #[tokio::test]
    async fn update_label_catalog_maps_apply_errors() -> Result<()> {
        let state = api_state(Arc::new(LabelConfig {
            fail_apply: true,
            ..LabelConfig::default()
        }))?;

        let err = update_label_catalog(&state, "alice", "update labels", |catalog| {
            catalog.upsert_tag("featured", TorrentLabelPolicy::default())
        })
        .await
        .expect_err("apply failure should be mapped");
        assert_eq!(err.status(), axum::http::StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err.kind(), crate::http::constants::PROBLEM_CONFIG_INVALID);
        Ok(())
    }
}
