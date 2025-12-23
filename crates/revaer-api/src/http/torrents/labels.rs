//! Category/tag policy parsing and application helpers.
//!
//! # Design
//! - Normalize label names to trimmed, non-empty values and validate policy bounds up front.
//! - Apply policies as defaults only, so explicit request overrides always win.
//! - Fail fast on malformed config or invalid policies and surface consistent API errors.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tracing::{error, warn};

use crate::app::state::ApiState;
use crate::http::auth::map_config_error;
use crate::http::errors::ApiError;
use revaer_config::SettingsChangeset;
use revaer_events::Event as CoreEvent;
use revaer_torrent_core::{AddTorrentOptions, TorrentCleanupPolicy, TorrentLabelPolicy};

const FEATURE_TORRENT_CATEGORIES: &str = "torrent_categories";
const FEATURE_TORRENT_TAGS: &str = "torrent_tags";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(in crate::http) struct TorrentLabelCatalog {
    #[serde(default, rename = "torrent_categories")]
    pub(crate) categories: HashMap<String, TorrentLabelPolicy>,
    #[serde(default, rename = "torrent_tags")]
    pub(crate) tags: HashMap<String, TorrentLabelPolicy>,
}

impl TorrentLabelCatalog {
    pub(in crate::http) fn from_features(features: &Value) -> Result<Self, ApiError> {
        if features.is_null() {
            return Ok(Self::default());
        }
        if !features.is_object() {
            return Err(ApiError::internal(
                "app_profile.features must be an object to manage torrent labels",
            ));
        }
        let catalog: Self = serde_json::from_value(features.clone()).map_err(|err| {
            warn!(error = %err, "failed to decode torrent label catalog");
            ApiError::internal("failed to decode torrent label catalog")
        })?;
        Ok(catalog)
    }

    pub(in crate::http) fn merge_into_features(&self, base: &Value) -> Result<Value, ApiError> {
        let mut map = match base.as_object() {
            Some(map) => map.clone(),
            None if base.is_null() => Map::new(),
            None => {
                return Err(ApiError::internal(
                    "app_profile.features must be an object to update torrent labels",
                ));
            }
        };
        map.insert(
            FEATURE_TORRENT_CATEGORIES.to_string(),
            serde_json::to_value(&self.categories).map_err(|err| {
                warn!(error = %err, "failed to encode torrent categories");
                ApiError::internal("failed to encode torrent categories")
            })?,
        );
        map.insert(
            FEATURE_TORRENT_TAGS.to_string(),
            serde_json::to_value(&self.tags).map_err(|err| {
                warn!(error = %err, "failed to encode torrent tags");
                ApiError::internal("failed to encode torrent tags")
            })?,
        );
        Ok(Value::Object(map))
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
    TorrentLabelCatalog::from_features(&profile.features)
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
    let mut catalog = TorrentLabelCatalog::from_features(&profile.features)?;
    mutator(&mut catalog)?;
    let features = catalog.merge_into_features(&profile.features)?;
    let changeset = SettingsChangeset {
        app_profile: Some(json!({ "features": features })),
        ..SettingsChangeset::default()
    };
    state
        .config
        .apply_changeset(actor, reason, changeset)
        .await
        .map_err(|err| map_config_error(err, "failed to update torrent labels"))?;
    let _ = state.events.publish(CoreEvent::SettingsChanged {
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

pub(in crate::http) fn normalize_label_name(kind: &str, raw: &str) -> Result<String, ApiError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(format!(
            "{kind} name must not be empty"
        )));
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
        return Err(ApiError::bad_request(format!(
            "{field} must be a non-negative number"
        )));
    }
    Ok(())
}
