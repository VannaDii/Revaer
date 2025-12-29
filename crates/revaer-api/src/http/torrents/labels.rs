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
    pub(in crate::http) fn from_label_policies(
        policies: &[LabelPolicy],
    ) -> Result<Self, ApiError> {
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

fn label_policy_to_torrent(policy: &LabelPolicy) -> TorrentLabelPolicy {
    let rate_limit = if policy.rate_limit_download_bps.is_some()
        || policy.rate_limit_upload_bps.is_some()
    {
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

    let cleanup_seed_ratio_limit = policy.cleanup.as_ref().and_then(|cleanup| cleanup.seed_ratio_limit);
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
