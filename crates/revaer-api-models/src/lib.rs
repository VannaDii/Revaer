#![forbid(unsafe_code)]
#![deny(
    warnings,
    dead_code,
    unused,
    unused_imports,
    unused_must_use,
    unreachable_pub,
    clippy::all,
    clippy::pedantic,
    clippy::cargo,
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]
#![allow(clippy::multiple_crate_versions)]
//! Shared HTTP DTOs for the Revaer public API.
//!
//! These types are re-used by the CLI for request/response encoding to keep the
//! contract deterministic. The conversions live close to the server so the
//! mapping from domain objects (`TorrentStatus`, `FileSelectionUpdate`, etc.)
//! remains a single source of truth.
use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use revaer_config::AppAuthMode;
use revaer_config::ConfigSnapshot;
use revaer_events::TorrentState;
use revaer_torrent_core::{
    AddTorrentOptions, FileSelectionRules, FileSelectionUpdate, PeerChoke, PeerInterest,
    PeerSnapshot, StorageMode, TorrentSource, TorrentStatus,
    model::{
        TorrentAuthorRequest as CoreTorrentAuthorRequest,
        TorrentAuthorResult as CoreTorrentAuthorResult, TorrentOptionsUpdate,
        TorrentTrackersUpdate, TorrentWebSeedsUpdate,
    },
};
pub use revaer_torrent_core::{
    FilePriority, FilePriorityOverride, TorrentCleanupPolicy, TorrentLabelPolicy, TorrentRateLimit,
};

/// RFC9457-compatible problem document surfaced on validation/runtime errors.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    /// URI reference identifying the problem type.
    pub kind: String,
    /// Short, human-readable summary of the issue.
    pub title: String,
    /// HTTP status code associated with the error.
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Detailed diagnostic message when available.
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Parameters that failed validation, if applicable.
    pub invalid_params: Option<Vec<ProblemInvalidParam>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Structured context fields associated with the error.
    pub context: Option<Vec<ProblemContextField>>,
}

/// Structured context field attached to a [`ProblemDetails`] payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProblemContextField {
    /// Context field name.
    pub name: String,
    /// Context field value.
    pub value: String,
}

/// Invalid parameter pointer surfaced alongside a [`ProblemDetails`] payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProblemInvalidParam {
    /// JSON Pointer to the offending field.
    pub pointer: String,
    /// Human-readable description of the validation failure.
    pub message: String,
}

/// Health component status used by the `/health` endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthComponentResponse {
    /// Component health status ("ok", "degraded").
    pub status: String,
    /// Optional schema revision associated with the component.
    pub revision: Option<i64>,
}

/// Basic health response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    /// Overall health status.
    pub status: String,
    /// Application mode ("setup" or "active").
    pub mode: String,
    /// Database component health details.
    pub database: HealthComponentResponse,
}

/// Detailed health metrics snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthMetricsResponse {
    /// Config watch latency in milliseconds.
    pub config_watch_latency_ms: i64,
    /// Config apply latency in milliseconds.
    pub config_apply_latency_ms: i64,
    /// Total count of config update failures.
    pub config_update_failures_total: u64,
    /// Total count of slow config watches.
    pub config_watch_slow_total: u64,
    /// Total count of guardrail violations.
    pub guardrail_violations_total: u64,
    /// Total count of rate-limit throttles.
    pub rate_limit_throttled_total: u64,
}

/// Torrent-specific health snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorrentHealthResponse {
    /// Count of active torrents.
    pub active: i64,
    /// Queue depth snapshot.
    pub queue_depth: i64,
}

/// Full health response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FullHealthResponse {
    /// Overall health status.
    pub status: String,
    /// Application mode ("setup" or "active").
    pub mode: String,
    /// Schema revision identifier.
    pub revision: i64,
    /// Build identifier.
    pub build: String,
    /// Degraded component list.
    pub degraded: Vec<String>,
    /// Metrics snapshot for config and guardrails.
    pub metrics: HealthMetricsResponse,
    /// Torrent health snapshot for queue sizing.
    pub torrent: TorrentHealthResponse,
}

/// Dashboard response payload (overview counts).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DashboardResponse {
    /// Aggregate download throughput in bytes per second.
    pub download_bps: u64,
    /// Aggregate upload throughput in bytes per second.
    pub upload_bps: u64,
    /// Count of active torrents.
    pub active: u32,
    /// Count of paused torrents.
    pub paused: u32,
    /// Count of completed torrents.
    pub completed: u32,
    /// Total disk capacity (GB).
    pub disk_total_gb: u32,
    /// Used disk capacity (GB).
    pub disk_used_gb: u32,
}

/// Setup start response payload returned by `/admin/setup/start`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetupStartResponse {
    /// Plaintext setup token.
    pub token: String,
    /// Token expiry timestamp as an RFC3339 string.
    pub expires_at: String,
}

/// Setup start request payload accepted by `/admin/setup/start`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SetupStartRequest {
    /// Optional identifier for the actor requesting the token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issued_by: Option<String>,
    /// Optional TTL in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
}

/// Setup complete response payload returned by `/admin/setup/complete`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupCompleteResponse {
    /// Updated configuration snapshot.
    pub snapshot: ConfigSnapshot,
    /// Bootstrap API key (`key_id:secret`) when auth is enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// API key expiry timestamp as an RFC3339 string when auth is enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_expires_at: Option<String>,
}

/// API key refresh response payload returned by `/v1/auth/refresh`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiKeyRefreshResponse {
    /// API key expiry timestamp as an RFC3339 string.
    pub api_key_expires_at: String,
}

/// Factory reset request payload accepted by `/admin/factory-reset`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FactoryResetRequest {
    /// Confirmation phrase that must match `factory reset`.
    pub confirm: String,
}

/// Tag creation request payload for indexer tags.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TagCreateRequest {
    /// Unique lowercase tag key.
    pub tag_key: String,
    /// Human-readable tag name.
    pub display_name: String,
}

/// Tag update request payload for indexer tags.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TagUpdateRequest {
    /// Tag public identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_public_id: Option<Uuid>,
    /// Tag key (lowercase).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_key: Option<String>,
    /// Updated display name for the tag.
    pub display_name: String,
}

/// Tag delete request payload for indexer tags.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TagDeleteRequest {
    /// Tag public identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_public_id: Option<Uuid>,
    /// Tag key (lowercase).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_key: Option<String>,
}

/// Tag response payload returned by indexer tag endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TagResponse {
    /// Tag public identifier.
    pub tag_public_id: Uuid,
    /// Tag key (lowercase) when provided by the caller.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_key: Option<String>,
    /// Display name associated with the tag.
    pub display_name: String,
}

/// Operator-visible tag summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TagListItemResponse {
    /// Tag public identifier.
    pub tag_public_id: Uuid,
    /// Stable tag key.
    pub tag_key: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Tag list response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TagListResponse {
    /// Collection of active tags.
    pub tags: Vec<TagListItemResponse>,
}

/// Health notification hook creation request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerHealthNotificationHookCreateRequest {
    /// Channel type (`email` or `webhook`).
    pub channel: String,
    /// Operator-facing label for the hook.
    pub display_name: String,
    /// Lowest connectivity status that should trigger the hook.
    pub status_threshold: String,
    /// Webhook URL when `channel=webhook`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
    /// Email address when `channel=email`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// Health notification hook update request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerHealthNotificationHookUpdateRequest {
    /// Public identifier of the hook to update.
    pub indexer_health_notification_hook_public_id: Uuid,
    /// Updated display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Updated status threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_threshold: Option<String>,
    /// Updated webhook URL for webhook hooks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
    /// Updated email address for email hooks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Updated enabled state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_enabled: Option<bool>,
}

/// Health notification hook delete request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerHealthNotificationHookDeleteRequest {
    /// Public identifier of the hook to delete.
    pub indexer_health_notification_hook_public_id: Uuid,
}

/// Health notification hook response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerHealthNotificationHookResponse {
    /// Public identifier of the hook.
    pub indexer_health_notification_hook_public_id: Uuid,
    /// Channel type (`email` or `webhook`).
    pub channel: String,
    /// Operator-facing label.
    pub display_name: String,
    /// Lowest status that should trigger delivery.
    pub status_threshold: String,
    /// Webhook URL when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
    /// Email address when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Whether the hook is active.
    pub is_enabled: bool,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Health notification hook list response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerHealthNotificationHookListResponse {
    /// Configured hooks in display order.
    pub hooks: Vec<IndexerHealthNotificationHookResponse>,
}

/// Rate limit policy creation request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateLimitPolicyCreateRequest {
    /// Human-readable policy name.
    pub display_name: String,
    /// Allowed requests per minute.
    pub rpm: i32,
    /// Allowed burst tokens.
    pub burst: i32,
    /// Maximum concurrent requests.
    pub concurrent: i32,
}

/// Rate limit policy update request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateLimitPolicyUpdateRequest {
    /// Updated display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Updated requests per minute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rpm: Option<i32>,
    /// Updated burst size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub burst: Option<i32>,
    /// Updated concurrent request limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concurrent: Option<i32>,
}

/// Rate limit policy response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateLimitPolicyResponse {
    /// Rate limit policy public identifier.
    pub rate_limit_policy_public_id: Uuid,
}

/// Rate-limit policy inventory item for operator read/list surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateLimitPolicyListItemResponse {
    /// Rate-limit policy public identifier.
    pub rate_limit_policy_public_id: Uuid,
    /// Operator-facing display name.
    pub display_name: String,
    /// Requests-per-minute budget.
    pub requests_per_minute: i32,
    /// Burst budget.
    pub burst: i32,
    /// Concurrent request budget.
    pub concurrent_requests: i32,
    /// Whether the policy is system-seeded.
    pub is_system: bool,
}

/// Rate-limit policy list response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateLimitPolicyListResponse {
    /// Collection of rate-limit policies.
    pub rate_limit_policies: Vec<RateLimitPolicyListItemResponse>,
}

/// Rate limit policy assignment request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateLimitPolicyAssignmentRequest {
    /// Rate limit policy public identifier to assign, or `null` to clear.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_policy_public_id: Option<Uuid>,
}

/// Tracker category mapping upsert request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackerCategoryMappingUpsertRequest {
    /// Optional Torznab instance public identifier for app-scoped overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub torznab_instance_public_id: Option<Uuid>,
    /// Optional indexer definition upstream slug for definition-specific overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexer_definition_upstream_slug: Option<String>,
    /// Optional indexer instance public identifier for instance-specific overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexer_instance_public_id: Option<Uuid>,
    /// Tracker category id.
    pub tracker_category: i32,
    /// Tracker subcategory id (defaults to 0 when omitted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracker_subcategory: Option<i32>,
    /// Torznab category id.
    pub torznab_cat_id: i32,
    /// Optional media domain key to constrain the mapping.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_domain_key: Option<String>,
}

/// Tracker category mapping delete request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackerCategoryMappingDeleteRequest {
    /// Optional Torznab instance public identifier for app-scoped overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub torznab_instance_public_id: Option<Uuid>,
    /// Optional indexer definition upstream slug for definition-specific overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexer_definition_upstream_slug: Option<String>,
    /// Optional indexer instance public identifier for instance-specific overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexer_instance_public_id: Option<Uuid>,
    /// Tracker category id.
    pub tracker_category: i32,
    /// Tracker subcategory id (defaults to 0 when omitted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracker_subcategory: Option<i32>,
}

/// Media domain to Torznab category mapping upsert request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaDomainMappingUpsertRequest {
    /// Media domain key.
    pub media_domain_key: String,
    /// Torznab category id.
    pub torznab_cat_id: i32,
    /// Whether this mapping is the primary category for the domain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_primary: Option<bool>,
}

/// Media domain to Torznab category mapping delete request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaDomainMappingDeleteRequest {
    /// Media domain key.
    pub media_domain_key: String,
    /// Torznab category id.
    pub torznab_cat_id: i32,
}

/// Torznab instance creation request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorznabInstanceCreateRequest {
    /// Search profile public identifier to bind.
    pub search_profile_public_id: Uuid,
    /// Display name for the instance.
    pub display_name: String,
}

/// Torznab instance enable/disable request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorznabInstanceStateRequest {
    /// Whether the instance is enabled.
    pub is_enabled: bool,
}

/// Torznab instance response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorznabInstanceResponse {
    /// Torznab instance public identifier.
    pub torznab_instance_public_id: Uuid,
    /// Plaintext API key for the instance.
    pub api_key_plaintext: String,
}

/// Torznab-instance inventory item for operator read/list surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorznabInstanceListItemResponse {
    /// Torznab-instance public identifier.
    pub torznab_instance_public_id: Uuid,
    /// Operator-facing display name.
    pub display_name: String,
    /// Whether the Torznab endpoint is enabled.
    pub is_enabled: bool,
    /// Linked search-profile public identifier.
    pub search_profile_public_id: Uuid,
    /// Linked search-profile display name.
    pub search_profile_display_name: String,
}

/// Torznab-instance inventory response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorznabInstanceListResponse {
    /// Operator-visible Torznab instances.
    pub torznab_instances: Vec<TorznabInstanceListItemResponse>,
}

/// Search profile creation request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchProfileCreateRequest {
    /// Display name for the search profile.
    pub display_name: String,
    /// Whether this profile should become the default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
    /// Optional page size override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i32>,
    /// Optional default media domain key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_media_domain_key: Option<String>,
    /// Optional user public identifier to scope this profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_public_id: Option<Uuid>,
}

/// Search profile update request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchProfileUpdateRequest {
    /// Updated display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Updated page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i32>,
}

/// Search profile set default request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchProfileDefaultRequest {
    /// Optional page size override for the default profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i32>,
}

/// Search profile default media domain request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchProfileDefaultDomainRequest {
    /// Optional default media domain key (omit to clear).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_media_domain_key: Option<String>,
}

/// Search profile domain allowlist request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchProfileDomainAllowlistRequest {
    /// Ordered list of media domain keys to allow.
    pub media_domain_keys: Vec<String>,
}

/// Search profile policy set request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchProfilePolicySetRequest {
    /// Policy set public identifier.
    pub policy_set_public_id: Uuid,
}

/// Search profile indexer allow/block request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchProfileIndexerSetRequest {
    /// Indexer instance public identifiers.
    pub indexer_instance_public_ids: Vec<Uuid>,
}

/// Search profile tag allow/block/prefer request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchProfileTagSetRequest {
    /// Optional tag public identifiers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_public_ids: Option<Vec<Uuid>>,
    /// Optional tag keys.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_keys: Option<Vec<String>>,
}

/// Search profile response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchProfileResponse {
    /// Search profile public identifier.
    pub search_profile_public_id: Uuid,
}

/// Search-profile inventory item for operator read/list surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchProfileListItemResponse {
    /// Search-profile public identifier.
    pub search_profile_public_id: Uuid,
    /// Operator-facing display name.
    pub display_name: String,
    /// Whether the profile is marked as default.
    pub is_default: bool,
    /// Optional page-size override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i32>,
    /// Optional default media-domain key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_media_domain_key: Option<String>,
    /// Allowed media-domain keys.
    pub media_domain_keys: Vec<String>,
    /// Attached policy-set public identifiers.
    pub policy_set_public_ids: Vec<Uuid>,
    /// Attached policy-set display names.
    pub policy_set_display_names: Vec<String>,
    /// Explicitly allowed indexer-instance public identifiers.
    pub allow_indexer_public_ids: Vec<Uuid>,
    /// Explicitly blocked indexer-instance public identifiers.
    pub block_indexer_public_ids: Vec<Uuid>,
    /// Allowed tag keys.
    pub allow_tag_keys: Vec<String>,
    /// Blocked tag keys.
    pub block_tag_keys: Vec<String>,
    /// Preferred tag keys.
    pub prefer_tag_keys: Vec<String>,
}

/// Search-profile inventory response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchProfileListResponse {
    /// Operator-visible search profiles.
    pub search_profiles: Vec<SearchProfileListItemResponse>,
}

/// Search request creation request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchRequestCreateRequest {
    /// Raw query text (may be empty for identifier-only searches).
    pub query_text: String,
    /// Query type key (`free_text`, `imdb`, `tmdb`, `tvdb`, `season_episode`).
    pub query_type: String,
    /// Optional Torznab mode (`generic`, `tv`, `movie`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub torznab_mode: Option<String>,
    /// Optional requested media domain key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_media_domain_key: Option<String>,
    /// Optional page size override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i32>,
    /// Optional search profile public identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_profile_public_id: Option<Uuid>,
    /// Optional policy set public identifier for this request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_policy_set_public_id: Option<Uuid>,
    /// Optional season number for TV queries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season_number: Option<i32>,
    /// Optional episode number for TV queries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode_number: Option<i32>,
    /// Optional identifier types (`imdb`, `tmdb`, `tvdb`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier_types: Option<Vec<String>>,
    /// Optional identifier values matching `identifier_types`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier_values: Option<Vec<String>>,
    /// Optional Torznab category ids.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub torznab_cat_ids: Option<Vec<i32>>,
}

/// Search request creation response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchRequestCreateResponse {
    /// Search request public identifier.
    pub search_request_public_id: Uuid,
    /// Policy set public identifier applied to this request.
    pub request_policy_set_public_id: Uuid,
}

/// Search page summary payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchPageSummaryResponse {
    /// Page number for the search request.
    pub page_number: i32,
    /// Timestamp when the page was sealed, if sealed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sealed_at: Option<DateTime<Utc>>,
    /// Item count for the page.
    pub item_count: i32,
}

/// Search page list response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchPageListResponse {
    /// Pages available for the search request.
    pub pages: Vec<SearchPageSummaryResponse>,
    /// Explainability details when no or few results are returned.
    pub explainability: SearchRequestExplainabilityResponse,
}

/// Explainability details for a search request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchRequestExplainabilityResponse {
    /// Whether the request had zero runnable indexers.
    pub zero_runnable_indexers: bool,
    /// Number of indexer runs skipped by cancellation.
    pub skipped_canceled_indexers: i32,
    /// Number of indexer runs skipped by terminal failures.
    pub skipped_failed_indexers: i32,
    /// Number of blocked decision rows recorded for this request.
    pub blocked_results: i32,
    /// Policy rules that blocked results.
    pub blocked_rule_public_ids: Vec<Uuid>,
    /// Number of indexer runs currently rate-limited.
    pub rate_limited_indexers: i32,
    /// Number of indexer runs currently retrying.
    pub retrying_indexers: i32,
}

/// Search page item payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchPageItemResponse {
    /// Position within the page.
    pub position: i32,
    /// Canonical torrent public identifier.
    pub canonical_torrent_public_id: Uuid,
    /// Canonical torrent display title.
    pub title_display: String,
    /// Optional canonical size in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<i64>,
    /// Optional infohash v1 value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub infohash_v1: Option<String>,
    /// Optional infohash v2 value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub infohash_v2: Option<String>,
    /// Optional magnet hash value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magnet_hash: Option<String>,
    /// Optional canonical torrent source public identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_torrent_source_public_id: Option<Uuid>,
    /// Optional indexer instance public identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexer_instance_public_id: Option<Uuid>,
    /// Optional indexer instance display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexer_display_name: Option<String>,
    /// Optional last seen seeders count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seeders: Option<i32>,
    /// Optional last seen leechers count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leechers: Option<i32>,
    /// Optional last seen published timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<DateTime<Utc>>,
    /// Optional last seen download URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    /// Optional last seen magnet URI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magnet_uri: Option<String>,
    /// Optional last seen details URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details_url: Option<String>,
    /// Optional tracker name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracker_name: Option<String>,
    /// Optional tracker category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracker_category: Option<i32>,
    /// Optional tracker subcategory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracker_subcategory: Option<i32>,
}

/// Search page response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchPageResponse {
    /// Page number for the search request.
    pub page_number: i32,
    /// Timestamp when the page was sealed, if sealed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sealed_at: Option<DateTime<Utc>>,
    /// Item count for the page.
    pub item_count: i32,
    /// Page items in stable order.
    pub items: Vec<SearchPageItemResponse>,
}

/// Policy set creation request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicySetCreateRequest {
    /// Display name for the policy set.
    pub display_name: String,
    /// Policy scope key (`global`, `user`, `profile`, `request`).
    pub scope: String,
    /// Optional enablement flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Policy set update request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicySetUpdateRequest {
    /// Updated display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

/// Policy set reorder request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicySetReorderRequest {
    /// Ordered list of policy set public identifiers.
    pub ordered_policy_set_public_ids: Vec<Uuid>,
}

/// Policy set response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicySetResponse {
    /// Policy set public identifier.
    pub policy_set_public_id: Uuid,
}

/// Policy-rule inventory item nested under a policy set.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyRuleListItemResponse {
    /// Policy-rule public identifier.
    pub policy_rule_public_id: Uuid,
    /// Rule type key.
    pub rule_type: String,
    /// Match field key.
    pub match_field: String,
    /// Match operator key.
    pub match_operator: String,
    /// Sort order for evaluation.
    pub sort_order: i32,
    /// Optional text match value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_value_text: Option<String>,
    /// Optional integer match value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_value_int: Option<i32>,
    /// Optional UUID match value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_value_uuid: Option<Uuid>,
    /// Action key.
    pub action: String,
    /// Severity key.
    pub severity: String,
    /// Whether matching is case-insensitive.
    pub is_case_insensitive: bool,
    /// Optional rationale text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    /// Optional expiry timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    /// Whether the rule is disabled.
    pub is_disabled: bool,
}

/// Policy-set inventory item for operator read/list surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicySetListItemResponse {
    /// Policy-set public identifier.
    pub policy_set_public_id: Uuid,
    /// Operator-facing display name.
    pub display_name: String,
    /// Scope key.
    pub scope: String,
    /// Whether the set is enabled.
    pub is_enabled: bool,
    /// Optional user public identifier for user-scoped sets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_public_id: Option<Uuid>,
    /// Ordered policy rules attached to the set.
    pub rules: Vec<PolicyRuleListItemResponse>,
}

/// Policy-set inventory response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicySetListResponse {
    /// Operator-visible policy sets.
    pub policy_sets: Vec<PolicySetListItemResponse>,
}

/// Value-set item for policy rule creation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyRuleValueItemRequest {
    /// Optional text value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_text: Option<String>,
    /// Optional integer value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_int: Option<i32>,
    /// Optional bigint value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_bigint: Option<i64>,
    /// Optional UUID value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_uuid: Option<Uuid>,
}

/// Policy rule creation request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyRuleCreateRequest {
    /// Rule type key.
    pub rule_type: String,
    /// Match field key.
    pub match_field: String,
    /// Match operator key.
    pub match_operator: String,
    /// Sort order for rule evaluation.
    pub sort_order: i32,
    /// Optional match text value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_value_text: Option<String>,
    /// Optional match integer value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_value_int: Option<i32>,
    /// Optional match UUID value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_value_uuid: Option<Uuid>,
    /// Optional value-set items for `in_set` matching.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_set_items: Option<Vec<PolicyRuleValueItemRequest>>,
    /// Policy action key.
    pub action: String,
    /// Policy severity key.
    pub severity: String,
    /// Optional case-insensitive match flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_case_insensitive: Option<bool>,
    /// Optional rule rationale.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    /// Optional expiry timestamp (RFC3339).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// Policy rule reorder request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyRuleReorderRequest {
    /// Ordered list of policy rule public identifiers.
    pub ordered_policy_rule_public_ids: Vec<Uuid>,
}

/// Policy rule response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyRuleResponse {
    /// Policy rule public identifier.
    pub policy_rule_public_id: Uuid,
}

/// Import job creation request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportJobCreateRequest {
    /// Import source key (`prowlarr_api`, `prowlarr_backup`).
    pub source: String,
    /// Optional dry run flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_dry_run: Option<bool>,
    /// Optional target search profile public identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_search_profile_public_id: Option<Uuid>,
    /// Optional target Torznab instance public identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_torznab_instance_public_id: Option<Uuid>,
}

/// Import job response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportJobResponse {
    /// Import job public identifier.
    pub import_job_public_id: Uuid,
}

/// Import job run request payload for Prowlarr API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportJobRunProwlarrApiRequest {
    /// Prowlarr base URL.
    pub prowlarr_url: String,
    /// Secret public identifier for the Prowlarr API key.
    pub prowlarr_api_key_secret_public_id: Uuid,
}

/// Import job run request payload for Prowlarr backup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportJobRunProwlarrBackupRequest {
    /// Reference to the backup blob.
    pub backup_blob_ref: String,
}

/// Import job status response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportJobStatusResponse {
    /// Job status label.
    pub status: String,
    /// Total result count.
    pub result_total: i32,
    /// Imported ready count.
    pub result_imported_ready: i32,
    /// Imported needs secret count.
    pub result_imported_needs_secret: i32,
    /// Imported test failed count.
    pub result_imported_test_failed: i32,
    /// Unmapped definition count.
    pub result_unmapped_definition: i32,
    /// Skipped duplicate count.
    pub result_skipped_duplicate: i32,
}

/// Import job result response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportJobResultResponse {
    /// Prowlarr identifier string.
    pub prowlarr_identifier: String,
    /// Upstream slug for the indexer definition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_slug: Option<String>,
    /// Public identifier for the created indexer instance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexer_instance_public_id: Option<Uuid>,
    /// Result status label.
    pub status: String,
    /// Optional detail message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Preserved enabled state from the imported source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_is_enabled: Option<bool>,
    /// Preserved priority from the imported source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_priority: Option<i32>,
    /// Count of missing required secret fields detected during import.
    pub missing_secret_fields: i32,
    /// Preserved media domain keys derived from imported categories.
    pub media_domain_keys: Vec<String>,
    /// Preserved tag keys derived from imported source tags.
    pub tag_keys: Vec<String>,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
}

/// Import job results response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportJobResultsResponse {
    /// Import job results.
    pub results: Vec<ImportJobResultResponse>,
}

/// Source metadata conflict row returned for operator review.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerSourceMetadataConflictResponse {
    /// Numeric conflict identifier defined by the ERD resolve/reopen proc contract.
    pub conflict_id: i64,
    /// Conflict type label.
    pub conflict_type: String,
    /// Existing durable value.
    pub existing_value: String,
    /// Incoming conflicting value.
    pub incoming_value: String,
    /// Timestamp when the conflict was observed.
    pub observed_at: DateTime<Utc>,
    /// Timestamp when the conflict was resolved, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<DateTime<Utc>>,
    /// Resolution label, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    /// Optional resolution note.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_note: Option<String>,
}

/// Source metadata conflict list response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerSourceMetadataConflictListResponse {
    /// Ordered conflict rows for operator review.
    pub conflicts: Vec<IndexerSourceMetadataConflictResponse>,
}

/// Request payload to resolve a source metadata conflict.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerSourceMetadataConflictResolveRequest {
    /// Numeric conflict identifier to resolve.
    pub conflict_id: i64,
    /// Resolution key (`accepted_incoming`, `kept_existing`, `merged`, `ignored`).
    pub resolution: String,
    /// Optional operator note persisted to the audit log.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_note: Option<String>,
}

/// Request payload to reopen a resolved source metadata conflict.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerSourceMetadataConflictReopenRequest {
    /// Numeric conflict identifier to reopen.
    pub conflict_id: i64,
    /// Optional operator note recorded during reopen.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_note: Option<String>,
}

/// Secret reference preserved in a backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerBackupSecretRef {
    /// Secret public identifier referenced by exported bindings.
    pub secret_public_id: Uuid,
    /// Secret type label.
    pub secret_type: String,
}

/// Tag item included in an indexer backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerBackupTagItem {
    /// Tag key.
    pub tag_key: String,
    /// Human-readable display name.
    pub display_name: String,
}

/// Rate-limit policy item included in an indexer backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerBackupRateLimitPolicyItem {
    /// Display name used to re-create or resolve the policy.
    pub display_name: String,
    /// Requests-per-minute budget.
    pub requests_per_minute: i32,
    /// Burst budget.
    pub burst: i32,
    /// Concurrent request budget.
    pub concurrent_requests: i32,
    /// Whether the policy is system-seeded.
    pub is_system: bool,
}

/// Routing-policy parameter item included in an indexer backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerBackupRoutingParameterItem {
    /// Parameter key.
    pub param_key: String,
    /// Optional plain-text parameter value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_plain: Option<String>,
    /// Optional integer parameter value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_int: Option<i32>,
    /// Optional boolean parameter value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_bool: Option<bool>,
    /// Optional bound secret public identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_public_id: Option<Uuid>,
}

/// Routing policy item included in an indexer backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerBackupRoutingPolicyItem {
    /// Display name used to re-create the policy.
    pub display_name: String,
    /// Routing mode key.
    pub mode: String,
    /// Optional assigned rate-limit policy display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_display_name: Option<String>,
    /// Exported parameters and secret references.
    pub parameters: Vec<IndexerBackupRoutingParameterItem>,
}

/// Field item included in an indexer-instance backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerBackupFieldItem {
    /// Field name.
    pub field_name: String,
    /// Field type label.
    pub field_type: String,
    /// Optional plain-text field value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_plain: Option<String>,
    /// Optional integer field value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_int: Option<i32>,
    /// Optional decimal field value represented as text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_decimal: Option<String>,
    /// Optional boolean field value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_bool: Option<bool>,
    /// Optional bound secret public identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_public_id: Option<Uuid>,
}

/// Indexer instance item included in an indexer backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerBackupIndexerInstanceItem {
    /// Definition upstream slug to instantiate.
    pub upstream_slug: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Instance status (`enabled` or `disabled`).
    pub instance_status: String,
    /// RSS setting status (`enabled` or `disabled`).
    pub rss_status: String,
    /// Automatic search status (`enabled` or `disabled`).
    pub automatic_search_status: String,
    /// Interactive search status (`enabled` or `disabled`).
    pub interactive_search_status: String,
    /// Priority override.
    pub priority: i32,
    /// Optional trust tier key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_tier_key: Option<String>,
    /// Optional routing policy display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_policy_display_name: Option<String>,
    /// Connection timeout in milliseconds.
    pub connect_timeout_ms: i32,
    /// Read timeout in milliseconds.
    pub read_timeout_ms: i32,
    /// Maximum parallel requests.
    pub max_parallel_requests: i32,
    /// Optional directly assigned rate-limit policy display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_display_name: Option<String>,
    /// Optional RSS subscription enabled state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rss_subscription_enabled: Option<bool>,
    /// Optional RSS subscription interval.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rss_interval_seconds: Option<i32>,
    /// Assigned media-domain keys.
    pub media_domain_keys: Vec<String>,
    /// Assigned tag keys.
    pub tag_keys: Vec<String>,
    /// Exported field values and secret references.
    pub fields: Vec<IndexerBackupFieldItem>,
}

/// Full indexer backup snapshot payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerBackupSnapshot {
    /// Backup format version.
    pub version: String,
    /// When the snapshot was exported.
    pub exported_at: DateTime<Utc>,
    /// Exported tag definitions.
    pub tags: Vec<IndexerBackupTagItem>,
    /// Exported rate-limit policies.
    pub rate_limit_policies: Vec<IndexerBackupRateLimitPolicyItem>,
    /// Exported routing policies.
    pub routing_policies: Vec<IndexerBackupRoutingPolicyItem>,
    /// Exported indexer instances.
    pub indexer_instances: Vec<IndexerBackupIndexerInstanceItem>,
    /// Distinct referenced secrets.
    pub secrets: Vec<IndexerBackupSecretRef>,
}

/// Response payload returned when exporting an indexer backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerBackupExportResponse {
    /// Exported snapshot document.
    pub snapshot: IndexerBackupSnapshot,
}

/// Request payload for restoring an indexer backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerBackupRestoreRequest {
    /// Snapshot to restore.
    pub snapshot: IndexerBackupSnapshot,
}

/// Unresolved secret binding surfaced during restore.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerBackupUnresolvedSecretBinding {
    /// Entity type (`routing_policy` or `indexer_instance`).
    pub entity_type: String,
    /// Entity display name.
    pub entity_display_name: String,
    /// Field or parameter key requiring a secret.
    pub binding_key: String,
    /// Missing secret public identifier.
    pub secret_public_id: Uuid,
}

/// Restore summary payload for indexer backup import.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerBackupRestoreResponse {
    /// Number of tags created.
    pub created_tag_count: i32,
    /// Number of rate-limit policies created.
    pub created_rate_limit_policy_count: i32,
    /// Number of routing policies created.
    pub created_routing_policy_count: i32,
    /// Number of indexer instances created.
    pub created_indexer_instance_count: i32,
    /// Secret bindings skipped because the referenced secret was unavailable.
    pub unresolved_secret_bindings: Vec<IndexerBackupUnresolvedSecretBinding>,
}

/// Secret creation request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretCreateRequest {
    /// Secret type label (`api_key`, `password`, `cookie`, `token`, `header_value`).
    pub secret_type: String,
    /// Plaintext secret value.
    pub secret_value: String,
}

/// Secret rotation request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretRotateRequest {
    /// Secret public identifier to rotate.
    pub secret_public_id: Uuid,
    /// New plaintext secret value.
    pub secret_value: String,
}

/// Secret revocation request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretRevokeRequest {
    /// Secret public identifier to revoke.
    pub secret_public_id: Uuid,
}

/// Secret response payload returned by secret endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretResponse {
    /// Secret public identifier.
    pub secret_public_id: Uuid,
}

/// Operator-visible secret metadata response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretMetadataResponse {
    /// Secret public identifier.
    pub secret_public_id: Uuid,
    /// Secret type label.
    pub secret_type: String,
    /// Whether the secret has been revoked.
    pub is_revoked: bool,
    /// When the secret was created.
    pub created_at: DateTime<Utc>,
    /// When the secret was last rotated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotated_at: Option<DateTime<Utc>>,
    /// Count of current bindings.
    pub binding_count: i64,
}

/// Secret metadata list response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretMetadataListResponse {
    /// Collection of secret metadata rows.
    pub secrets: Vec<SecretMetadataResponse>,
}

/// Indexer definition summary payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerDefinitionResponse {
    /// Upstream catalog source identifier.
    pub upstream_source: String,
    /// Upstream slug for the definition.
    pub upstream_slug: String,
    /// Human-readable name of the definition.
    pub display_name: String,
    /// Protocol label (`torrent`, `usenet`).
    pub protocol: String,
    /// Engine label (`torznab`, `cardigann`).
    pub engine: String,
    /// Schema version for the definition metadata.
    pub schema_version: i32,
    /// Canonical definition hash (sha256 hex).
    pub definition_hash: String,
    /// Whether the definition is deprecated.
    pub is_deprecated: bool,
    /// Timestamp when the definition was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp when the definition was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Indexer definition list response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerDefinitionListResponse {
    /// Collection of indexer definitions.
    pub definitions: Vec<IndexerDefinitionResponse>,
}

/// Cardigann definition import request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CardigannDefinitionImportRequest {
    /// Raw Cardigann YAML payload to normalize into the definition catalog.
    pub yaml_payload: String,
    /// Optional deprecated flag override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_deprecated: Option<bool>,
}

/// Cardigann definition import response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CardigannDefinitionImportResponse {
    /// Imported or updated definition summary.
    pub definition: IndexerDefinitionResponse,
    /// Imported field count.
    pub field_count: i32,
    /// Imported option count.
    pub option_count: i32,
}

/// Routing policy creation request payload for indexer routing policies.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutingPolicyCreateRequest {
    /// Human-readable routing policy name.
    pub display_name: String,
    /// Routing policy mode (`direct`, `http_proxy`, `socks_proxy`, `flaresolverr`).
    pub mode: String,
}

/// Routing policy response payload returned by routing policy endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutingPolicyResponse {
    /// Routing policy public identifier.
    pub routing_policy_public_id: Uuid,
    /// Routing policy display name.
    pub display_name: String,
    /// Routing policy mode.
    pub mode: String,
}

/// Routing policy inventory item for operator read/list surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutingPolicyListItemResponse {
    /// Routing policy public identifier.
    pub routing_policy_public_id: Uuid,
    /// Routing policy display name.
    pub display_name: String,
    /// Routing policy mode.
    pub mode: String,
    /// Optional assigned rate-limit policy public identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_policy_public_id: Option<Uuid>,
    /// Optional assigned rate-limit policy display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_display_name: Option<String>,
    /// Count of visible parameters on the policy.
    pub parameter_count: usize,
    /// Count of secret-backed parameters on the policy.
    pub secret_binding_count: usize,
}

/// Routing policy list response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutingPolicyListResponse {
    /// Collection of routing policies.
    pub routing_policies: Vec<RoutingPolicyListItemResponse>,
}

/// Routing policy parameter detail returned by the operator read endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutingPolicyParameterResponse {
    /// Routing policy parameter key.
    pub param_key: String,
    /// Optional plain-text parameter value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_plain: Option<String>,
    /// Optional integer parameter value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_int: Option<i32>,
    /// Optional boolean parameter value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_bool: Option<bool>,
    /// Optional bound secret public identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_public_id: Option<Uuid>,
    /// Optional operator-facing binding name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_binding_name: Option<String>,
}

/// Routing policy detail payload returned by the operator read endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutingPolicyDetailResponse {
    /// Routing policy public identifier.
    pub routing_policy_public_id: Uuid,
    /// Routing policy display name.
    pub display_name: String,
    /// Routing policy mode.
    pub mode: String,
    /// Optional assigned rate-limit policy public identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_policy_public_id: Option<Uuid>,
    /// Optional assigned rate-limit policy display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_display_name: Option<String>,
    /// Optional requests-per-minute budget.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_requests_per_minute: Option<i32>,
    /// Optional burst budget.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_burst: Option<i32>,
    /// Optional concurrent request budget.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_concurrent_requests: Option<i32>,
    /// Parameter rows associated with the policy.
    pub parameters: Vec<RoutingPolicyParameterResponse>,
}

/// Routing policy parameter set request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutingPolicyParamSetRequest {
    /// Routing policy parameter key.
    pub param_key: String,
    /// Optional text value for the parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_plain: Option<String>,
    /// Optional integer value for the parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_int: Option<i32>,
    /// Optional boolean value for the parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_bool: Option<bool>,
}

/// Routing policy secret binding request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutingPolicySecretBindRequest {
    /// Routing policy parameter key.
    pub param_key: String,
    /// Secret public identifier to bind.
    pub secret_public_id: Uuid,
}

/// Indexer instance creation request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerInstanceCreateRequest {
    /// Upstream slug key for the indexer definition to instantiate.
    pub indexer_definition_upstream_slug: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Optional priority (0-100, default 50).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    /// Optional trust tier key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_tier_key: Option<String>,
    /// Optional routing policy public identifier to bind.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_policy_public_id: Option<Uuid>,
}

/// Indexer instance update request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct IndexerInstanceUpdateRequest {
    /// Updated display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Updated priority.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    /// Updated trust tier key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_tier_key: Option<String>,
    /// Updated routing policy public identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_policy_public_id: Option<Uuid>,
    /// Enable or disable the instance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_enabled: Option<bool>,
    /// Enable or disable RSS polling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_rss: Option<bool>,
    /// Enable automatic search for the instance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_automatic_search: Option<bool>,
    /// Enable interactive search for the instance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_interactive_search: Option<bool>,
}

/// Indexer instance response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerInstanceResponse {
    /// Indexer instance public identifier.
    pub indexer_instance_public_id: Uuid,
}

/// Indexer field inventory item surfaced on operator read/list views.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerInstanceFieldInventoryResponse {
    /// Field name.
    pub field_name: String,
    /// Field type label.
    pub field_type: String,
    /// Optional plain-text field value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_plain: Option<String>,
    /// Optional integer field value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_int: Option<i32>,
    /// Optional decimal field value represented as text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_decimal: Option<String>,
    /// Optional boolean field value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_bool: Option<bool>,
    /// Optional bound secret public identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_public_id: Option<Uuid>,
}

/// Indexer instance inventory item for operator read/list surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerInstanceListItemResponse {
    /// Indexer instance public identifier.
    pub indexer_instance_public_id: Uuid,
    /// Definition upstream slug.
    pub upstream_slug: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Instance status (`enabled` or `disabled`).
    pub instance_status: String,
    /// RSS setting status (`enabled` or `disabled`).
    pub rss_status: String,
    /// Automatic search status (`enabled` or `disabled`).
    pub automatic_search_status: String,
    /// Interactive search status (`enabled` or `disabled`).
    pub interactive_search_status: String,
    /// Priority override.
    pub priority: i32,
    /// Optional trust tier key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_tier_key: Option<String>,
    /// Optional routing policy public identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_policy_public_id: Option<Uuid>,
    /// Optional routing policy display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_policy_display_name: Option<String>,
    /// Connection timeout in milliseconds.
    pub connect_timeout_ms: i32,
    /// Read timeout in milliseconds.
    pub read_timeout_ms: i32,
    /// Maximum parallel requests.
    pub max_parallel_requests: i32,
    /// Optional directly assigned rate-limit policy public identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_policy_public_id: Option<Uuid>,
    /// Optional directly assigned rate-limit policy display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_display_name: Option<String>,
    /// Optional RSS subscription enabled state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rss_subscription_enabled: Option<bool>,
    /// Optional RSS subscription interval in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rss_interval_seconds: Option<i32>,
    /// Assigned media-domain keys.
    pub media_domain_keys: Vec<String>,
    /// Assigned tag keys.
    pub tag_keys: Vec<String>,
    /// Configured field values and bound secret references.
    pub fields: Vec<IndexerInstanceFieldInventoryResponse>,
}

/// Indexer instance list response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerInstanceListResponse {
    /// Collection of indexer instances.
    pub indexer_instances: Vec<IndexerInstanceListItemResponse>,
}

/// RSS subscription update request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct IndexerRssSubscriptionUpdateRequest {
    /// Enable or disable the subscription row.
    pub is_enabled: bool,
    /// Optional interval override in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_seconds: Option<i32>,
}

/// RSS subscription snapshot payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerRssSubscriptionResponse {
    /// Indexer instance public identifier.
    pub indexer_instance_public_id: Uuid,
    /// Instance enablement status: `enabled` or `disabled`.
    pub instance_status: String,
    /// Instance RSS setting status: `enabled` or `disabled`.
    pub rss_setting_status: String,
    /// Subscription status: `missing`, `enabled`, or `disabled`.
    pub subscription_status: String,
    /// Poll interval in seconds.
    pub interval_seconds: i32,
    /// Last successful poll timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_polled_at: Option<DateTime<Utc>>,
    /// Next scheduled poll timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_poll_at: Option<DateTime<Utc>>,
    /// Current backoff in seconds after retryable failures.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backoff_seconds: Option<i32>,
    /// Last RSS failure class, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_class: Option<String>,
}

/// RSS seen-item snapshot payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerRssSeenItemResponse {
    /// Normalized feed GUID or stable item identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_guid: Option<String>,
    /// Infohash v1 identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub infohash_v1: Option<String>,
    /// Infohash v2 identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub infohash_v2: Option<String>,
    /// Magnet hash identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magnet_hash: Option<String>,
    /// When the item was first recorded for this indexer.
    pub first_seen_at: DateTime<Utc>,
}

/// RSS seen-item list payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerRssSeenItemsResponse {
    /// Ordered recent seen items for the target indexer.
    pub items: Vec<IndexerRssSeenItemResponse>,
}

/// Manual RSS seen-item mark request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct IndexerRssSeenMarkRequest {
    /// Optional feed GUID or stable item identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_guid: Option<String>,
    /// Optional v1 infohash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub infohash_v1: Option<String>,
    /// Optional v2 infohash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub infohash_v2: Option<String>,
    /// Optional magnet hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magnet_hash: Option<String>,
}

/// Manual RSS seen-item mark response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerRssSeenMarkResponse {
    /// Marked or matched item snapshot.
    pub item: IndexerRssSeenItemResponse,
    /// Whether a new row was inserted.
    pub inserted: bool,
}

/// Indexer instance test prepare response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerInstanceTestPrepareResponse {
    /// Whether executor can run the test.
    pub can_execute: bool,
    /// Error classification (when preparation fails).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_class: Option<String>,
    /// Error code (when preparation fails).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// Detail string for UI display.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Indexer engine label.
    pub engine: String,
    /// Routing policy public identifier, if configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_policy_public_id: Option<Uuid>,
    /// Connection timeout in milliseconds.
    pub connect_timeout_ms: i32,
    /// Read timeout in milliseconds.
    pub read_timeout_ms: i32,
    /// Field names aligned with config arrays.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_names: Option<Vec<String>>,
    /// Field types aligned with config arrays.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_types: Option<Vec<String>>,
    /// Plain string values aligned with config arrays.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_plain: Option<Vec<Option<String>>>,
    /// Integer values aligned with config arrays.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_int: Option<Vec<Option<i32>>>,
    /// Decimal values aligned with config arrays.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_decimal: Option<Vec<Option<String>>>,
    /// Boolean values aligned with config arrays.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_bool: Option<Vec<Option<bool>>>,
    /// Secret public ids aligned with config arrays.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_public_ids: Option<Vec<Option<Uuid>>>,
}

/// Indexer instance test finalize request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerInstanceTestFinalizeRequest {
    /// Whether the test succeeded.
    pub ok: bool,
    /// Error class label when test failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_class: Option<String>,
    /// Error code when test failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// Detail string for UI display.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Optional result count for diagnostics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_count: Option<i32>,
}

/// Indexer instance test finalize response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerInstanceTestFinalizeResponse {
    /// Whether the test succeeded.
    pub ok: bool,
    /// Error class label when test failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_class: Option<String>,
    /// Error code when test failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// Detail string for UI display.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Optional result count for diagnostics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_count: Option<i32>,
}

/// Media domain assignment request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct IndexerInstanceMediaDomainsRequest {
    /// Media domain keys to assign.
    pub media_domain_keys: Vec<String>,
}

/// Tag assignment request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct IndexerInstanceTagsRequest {
    /// Tag public identifiers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_public_ids: Option<Vec<Uuid>>,
    /// Tag keys.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_keys: Option<Vec<String>>,
}

/// Indexer instance field value set request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct IndexerInstanceFieldValueRequest {
    /// Field name to update.
    pub field_name: String,
    /// Optional plain text value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_plain: Option<String>,
    /// Optional integer value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_int: Option<i32>,
    /// Optional decimal value represented as text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_decimal: Option<String>,
    /// Optional boolean value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_bool: Option<bool>,
}

/// Indexer instance field secret bind request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerInstanceFieldSecretBindRequest {
    /// Field name to bind.
    pub field_name: String,
    /// Secret public identifier to attach.
    pub secret_public_id: Uuid,
}

/// Cloudflare state reset request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerCfStateResetRequest {
    /// Reason for the manual reset (logged and audited).
    pub reason: String,
}

/// Cloudflare state response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerCfStateResponse {
    /// Current state (clear, challenged, solved, banned, cooldown).
    pub state: String,
    /// Timestamp of last state change.
    pub last_changed_at: chrono::DateTime<chrono::Utc>,
    /// Optional CF session expiration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cf_session_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Optional cooldown end.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooldown_until: Option<chrono::DateTime<chrono::Utc>>,
    /// Optional backoff seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backoff_seconds: Option<i32>,
    /// Consecutive failures counter.
    pub consecutive_failures: i32,
    /// Last error class if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_class: Option<String>,
}

/// Connectivity profile response for an indexer instance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexerConnectivityProfileResponse {
    /// Whether a derived profile snapshot exists yet.
    pub profile_exists: bool,
    /// Connectivity status when a snapshot exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Dominant error class when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_class: Option<String>,
    /// p50 latency in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_p50_ms: Option<i32>,
    /// p95 latency in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_p95_ms: Option<i32>,
    /// One-hour success rate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_rate_1h: Option<f64>,
    /// Twenty-four-hour success rate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_rate_24h: Option<f64>,
    /// Last profile refresh timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_checked_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Source reputation snapshot for one window bucket.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexerSourceReputationResponse {
    /// Window identifier (`1h`, `24h`, `7d`).
    pub window_key: String,
    /// Window start timestamp.
    pub window_start: chrono::DateTime<chrono::Utc>,
    /// Request success rate.
    pub request_success_rate: f64,
    /// Acquisition success rate.
    pub acquisition_success_rate: f64,
    /// Fake-result rate.
    pub fake_rate: f64,
    /// DMCA/removal rate.
    pub dmca_rate: f64,
    /// Total requests observed.
    pub request_count: i32,
    /// Successful requests observed.
    pub request_success_count: i32,
    /// Total acquisitions observed.
    pub acquisition_count: i32,
    /// Successful acquisitions observed.
    pub acquisition_success_count: i32,
    /// Minimum sample threshold used for the rollup.
    pub min_samples: i32,
    /// Timestamp when this row was computed.
    pub computed_at: chrono::DateTime<chrono::Utc>,
}

/// Reputation list payload for an indexer instance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexerSourceReputationListResponse {
    /// Recent reputation rows for the selected window.
    pub items: Vec<IndexerSourceReputationResponse>,
}

/// Health-event row for an indexer instance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerHealthEventResponse {
    /// When the event occurred.
    pub occurred_at: chrono::DateTime<chrono::Utc>,
    /// Event type key.
    pub event_type: String,
    /// Request latency in milliseconds when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<i32>,
    /// HTTP status code when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<i32>,
    /// Error class when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_class: Option<String>,
    /// Optional diagnostic detail text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Health-event list payload for an indexer instance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexerHealthEventListResponse {
    /// Recent health events for the selected indexer instance.
    pub items: Vec<IndexerHealthEventResponse>,
}

/// Directory entry returned by the filesystem browser.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsEntry {
    /// Base name of the entry.
    pub name: String,
    /// Full path for the entry.
    pub path: String,
    /// Classification for the entry.
    pub kind: FsEntryKind,
}

/// Filesystem entry kinds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FsEntryKind {
    /// Directory entry.
    Directory,
    /// Regular file entry.
    File,
    /// Symbolic link entry.
    Symlink,
    /// Other or unknown entry.
    Other,
}

/// Filesystem browser payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsBrowseResponse {
    /// Resolved path for the request.
    pub path: String,
    /// Parent path if available.
    pub parent: Option<String>,
    /// Directory entries sorted by kind and name.
    pub entries: Vec<FsEntry>,
}

/// Enumerates the coarse torrent lifecycle states surfaced via the API.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TorrentStateKind {
    /// Awaiting initial processing by the engine.
    Queued,
    /// Downloading metadata (e.g., contacting trackers / DHT).
    FetchingMetadata,
    /// Actively fetching pieces from the swarm.
    Downloading,
    /// Seeding to peers.
    Seeding,
    /// Completed and ready for post-processing.
    Completed,
    /// Encountered an unrecoverable failure.
    Failed,
    /// Paused or otherwise stopped without error.
    Stopped,
}

impl From<TorrentState> for TorrentStateKind {
    fn from(value: TorrentState) -> Self {
        match value {
            TorrentState::Queued => Self::Queued,
            TorrentState::FetchingMetadata => Self::FetchingMetadata,
            TorrentState::Downloading => Self::Downloading,
            TorrentState::Seeding => Self::Seeding,
            TorrentState::Completed => Self::Completed,
            TorrentState::Failed { .. } => Self::Failed,
            TorrentState::Stopped => Self::Stopped,
        }
    }
}

/// Describes the state + optional failure message for a torrent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorrentStateView {
    /// Normalised lifecycle state label.
    pub kind: TorrentStateKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional failure context if the torrent stopped unexpectedly.
    pub failure_message: Option<String>,
}

impl From<TorrentState> for TorrentStateView {
    fn from(value: TorrentState) -> Self {
        let kind = TorrentStateKind::from(value.clone());
        let failure_message = match value {
            TorrentState::Failed { message } => Some(message),
            _ => None,
        };
        Self {
            kind,
            failure_message,
        }
    }
}

/// Aggregated progress metrics for a torrent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TorrentProgressView {
    /// Bytes downloaded so far.
    pub bytes_downloaded: u64,
    /// Total bytes expected for the torrent.
    pub bytes_total: u64,
    /// Percentage (0.0–100.0) of completion.
    pub percent_complete: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Estimated time to completion in seconds, when calculable.
    pub eta_seconds: Option<u64>,
}

impl From<&TorrentStatus> for TorrentProgressView {
    fn from(status: &TorrentStatus) -> Self {
        Self {
            bytes_downloaded: status.progress.bytes_downloaded,
            bytes_total: status.progress.bytes_total,
            percent_complete: status.progress.percent_complete(),
            eta_seconds: status.progress.eta_seconds,
        }
    }
}

/// Transfer rates surfaced with a torrent snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TorrentRatesView {
    /// Current download throughput in bytes per second.
    pub download_bps: u64,
    /// Current upload throughput in bytes per second.
    pub upload_bps: u64,
    /// Share ratio calculated as uploaded/downloaded.
    pub ratio: f64,
}

impl From<&TorrentStatus> for TorrentRatesView {
    fn from(status: &TorrentStatus) -> Self {
        Self {
            download_bps: status.rates.download_bps,
            upload_bps: status.rates.upload_bps,
            ratio: status.rates.ratio,
        }
    }
}

/// File metadata returned when the client requests detailed torrent views.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorrentFileView {
    /// Zero-based index assigned by the engine.
    pub index: u32,
    /// Normalised relative path of the file inside the torrent.
    pub path: String,
    /// Total size of the file in bytes.
    pub size_bytes: u64,
    /// Number of bytes downloaded so far.
    pub bytes_completed: u64,
    /// Requested priority level for the file.
    pub priority: FilePriority,
    /// Indicates whether the file is currently selected for download.
    pub selected: bool,
}

impl From<revaer_torrent_core::TorrentFile> for TorrentFileView {
    fn from(file: revaer_torrent_core::TorrentFile) -> Self {
        Self {
            index: file.index,
            path: file.path,
            size_bytes: file.size_bytes,
            bytes_completed: file.bytes_completed,
            priority: file.priority,
            selected: file.selected,
        }
    }
}

/// Current selection rules applied to a torrent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TorrentSelectionView {
    #[serde(default)]
    /// Glob-style patterns that force inclusion.
    pub include: Vec<String>,
    #[serde(default)]
    /// Glob-style patterns that force exclusion.
    pub exclude: Vec<String>,
    /// Indicates whether fluff filtering is enabled.
    pub skip_fluff: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Explicit per-file priority overrides.
    pub priorities: Vec<FilePriorityOverride>,
}

impl From<&FileSelectionUpdate> for TorrentSelectionView {
    fn from(selection: &FileSelectionUpdate) -> Self {
        Self {
            include: selection.include.clone(),
            exclude: selection.exclude.clone(),
            skip_fluff: selection.skip_fluff,
            priorities: selection.priorities.clone(),
        }
    }
}

/// Snapshot of the configurable settings applied to a torrent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TorrentSettingsView {
    #[serde(default)]
    /// Tags associated with the torrent.
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional category assigned to the torrent.
    pub category: Option<String>,
    #[serde(default)]
    /// Trackers recorded for the torrent.
    pub trackers: Vec<String>,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    /// Tracker messages/errors keyed by URL.
    pub tracker_messages: std::collections::HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Per-torrent bandwidth limits when present.
    pub rate_limit: Option<TorrentRateLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Per-torrent peer connection cap when configured.
    pub connections_limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Download directory applied at admission time.
    pub download_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Comment captured from the torrent metainfo.
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Source label captured from the torrent metainfo.
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Private flag captured from the torrent metainfo.
    pub private: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Storage allocation mode applied to the torrent.
    pub storage_mode: Option<StorageMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether partfiles are enabled for this torrent.
    pub use_partfile: Option<bool>,
    /// Whether sequential mode is currently active.
    pub sequential: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// File selection rules most recently requested.
    pub selection: Option<TorrentSelectionView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether super-seeding is enabled for the torrent.
    pub super_seeding: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether the torrent was admitted in seed mode.
    pub seed_mode: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional share ratio stop threshold.
    pub seed_ratio_limit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional seeding time stop threshold in seconds.
    pub seed_time_limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional cleanup policy applied after seeding thresholds are met.
    pub cleanup: Option<TorrentCleanupPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether the torrent is auto-managed by the queue.
    pub auto_managed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional queue position when auto-managed is disabled.
    pub queue_position: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether peer exchange is enabled for the torrent.
    pub pex_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    /// Web seeds attached to the torrent.
    pub web_seeds: Vec<String>,
}

/// High-level view returned when listing torrents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TorrentSummary {
    /// Stable identifier for the torrent.
    pub id: Uuid,
    /// Human-friendly name if present.
    pub name: Option<String>,
    /// Current lifecycle state of the torrent.
    pub state: TorrentStateView,
    /// Transfer progress statistics.
    pub progress: TorrentProgressView,
    /// Observed bandwidth figures.
    pub rates: TorrentRatesView,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Absolute path to the library artifact once finalised.
    pub library_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Active download root path.
    pub download_dir: Option<String>,
    /// Whether sequential mode is enabled.
    pub sequential: bool,
    #[serde(default)]
    /// Tags associated with the torrent.
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional category assigned to the torrent.
    pub category: Option<String>,
    #[serde(default)]
    /// Tracker URLs recorded for the torrent.
    pub trackers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Per-torrent rate cap overrides applied on admission.
    pub rate_limit: Option<revaer_torrent_core::TorrentRateLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Per-torrent peer connection cap applied on admission.
    pub connections_limit: Option<i32>,
    /// Timestamp when the torrent was registered with the engine.
    pub added_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Time the torrent completed, if known.
    pub completed_at: Option<DateTime<Utc>>,
    /// Timestamp of the latest status update.
    pub last_updated: DateTime<Utc>,
}

/// Policy entry describing a category or tag label.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TorrentLabelEntry {
    /// Label name.
    pub name: String,
    #[serde(default)]
    /// Policy defaults applied when the label is used.
    pub policy: TorrentLabelPolicy,
}

impl From<TorrentStatus> for TorrentSummary {
    fn from(status: TorrentStatus) -> Self {
        Self {
            id: status.id,
            name: status.name.clone(),
            state: TorrentStateView::from(status.state.clone()),
            progress: TorrentProgressView::from(&status),
            rates: TorrentRatesView::from(&status),
            library_path: status.library_path.clone(),
            download_dir: status.download_dir.clone(),
            sequential: status.sequential,
            tags: Vec::new(),
            category: None,
            trackers: Vec::new(),
            rate_limit: None,
            connections_limit: None,
            added_at: status.added_at,
            completed_at: status.completed_at,
            last_updated: status.last_updated,
        }
    }
}

impl TorrentSummary {
    /// Attach API-layer metadata (tags/trackers) captured alongside the torrent.
    #[must_use]
    pub fn with_metadata(
        mut self,
        tags: Vec<String>,
        category: Option<String>,
        trackers: Vec<String>,
        rate_limit: Option<revaer_torrent_core::TorrentRateLimit>,
        connections_limit: Option<i32>,
    ) -> Self {
        self.tags = tags;
        self.category = category;
        self.trackers = trackers;
        self.rate_limit = rate_limit;
        self.connections_limit = connections_limit;
        self
    }
}

/// Full detail view returned when querying a specific torrent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TorrentDetail {
    #[serde(flatten)]
    /// Summary information for the torrent.
    pub summary: TorrentSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Current configurable settings applied to the torrent.
    pub settings: Option<TorrentSettingsView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Detailed file breakdown if requested.
    pub files: Option<Vec<TorrentFileView>>,
}

impl From<TorrentStatus> for TorrentDetail {
    fn from(status: TorrentStatus) -> Self {
        let summary = TorrentSummary::from(status.clone());
        let files = status
            .files
            .map(|items| items.into_iter().map(TorrentFileView::from).collect());
        let settings = TorrentSettingsView {
            tags: Vec::new(),
            category: None,
            trackers: Vec::new(),
            tracker_messages: std::collections::HashMap::new(),
            rate_limit: None,
            connections_limit: None,
            download_dir: status.download_dir.clone(),
            comment: status.comment.clone(),
            source: status.source.clone(),
            private: status.private,
            storage_mode: None,
            use_partfile: None,
            sequential: status.sequential,
            selection: None,
            super_seeding: None,
            seed_mode: None,
            seed_ratio_limit: None,
            seed_time_limit: None,
            cleanup: None,
            auto_managed: None,
            queue_position: None,
            pex_enabled: None,
            web_seeds: Vec::new(),
        };
        Self {
            summary,
            settings: Some(settings),
            files,
        }
    }
}

/// Paginated list response for the torrent collection endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TorrentListResponse {
    /// Page of torrent summaries.
    pub torrents: Vec<TorrentSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Cursor for retrieving the next page, when available.
    pub next: Option<String>,
}

/// JSON body accepted by `POST /v1/torrents` when carrying a magnet URI or
/// base64-encoded `.torrent` metainfo payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TorrentCreateRequest {
    /// Client-provided identifier for idempotent operations.
    pub id: Uuid,
    #[serde(default)]
    /// Magnet URI used to describe the torrent.
    pub magnet: Option<String>,
    #[serde(default)]
    /// Base64-encoded `.torrent` payload.
    pub metainfo: Option<String>,
    #[serde(default)]
    /// Friendly display name override.
    pub name: Option<String>,
    #[serde(default)]
    /// Optional comment override for torrent metadata.
    pub comment: Option<String>,
    #[serde(default)]
    /// Optional source override for torrent metadata.
    pub source: Option<String>,
    #[serde(default)]
    /// Optional private flag override for torrent metadata.
    pub private: Option<bool>,
    #[serde(default)]
    /// Optional download directory to stage content.
    pub download_dir: Option<String>,
    #[serde(default)]
    /// Optional storage allocation mode override.
    pub storage_mode: Option<StorageMode>,
    #[serde(default)]
    /// Enables sequential download mode on creation when set.
    pub sequential: Option<bool>,
    #[serde(default)]
    /// Adds the torrent in a paused/queued state when true.
    pub start_paused: Option<bool>,
    #[serde(default)]
    /// Adds the torrent in seed mode (assumes data is complete).
    pub seed_mode: Option<bool>,
    #[serde(default)]
    /// Percentage of pieces to hash-check before honoring seed mode.
    pub hash_check_sample_pct: Option<u8>,
    #[serde(default)]
    /// Enables super-seeding on admission when set.
    pub super_seeding: Option<bool>,
    #[serde(default)]
    /// Tags to associate with the torrent immediately.
    pub tags: Vec<String>,
    #[serde(default)]
    /// Optional category assigned to the torrent.
    pub category: Option<String>,
    #[serde(default)]
    /// Additional tracker URLs to register.
    pub trackers: Vec<String>,
    #[serde(default)]
    /// Whether the supplied trackers should replace profile defaults.
    pub replace_trackers: bool,
    #[serde(default)]
    /// Glob patterns that should be selected during the initial download.
    pub include: Vec<String>,
    #[serde(default)]
    /// Glob patterns that must be excluded from the download set.
    pub exclude: Vec<String>,
    #[serde(default)]
    /// Indicates whether the built-in fluff filtering preset should be applied.
    pub skip_fluff: bool,
    #[serde(default)]
    /// Optional download bandwidth cap in bytes per second.
    pub max_download_bps: Option<u64>,
    #[serde(default)]
    /// Optional upload bandwidth cap in bytes per second.
    pub max_upload_bps: Option<u64>,
    #[serde(default)]
    /// Optional per-torrent peer connection limit.
    pub max_connections: Option<i32>,
    #[serde(default)]
    /// Optional share ratio threshold before stopping seeding.
    pub seed_ratio_limit: Option<f64>,
    #[serde(default)]
    /// Optional seeding time limit in seconds.
    pub seed_time_limit: Option<u64>,
    #[serde(default)]
    /// Optional override for auto-managed queueing.
    pub auto_managed: Option<bool>,
    #[serde(default)]
    /// Optional queue position when auto-managed is disabled.
    pub queue_position: Option<i32>,
    #[serde(default)]
    /// Optional override for peer exchange behaviour.
    pub pex_enabled: Option<bool>,
    #[serde(default)]
    /// Optional list of web seeds to attach on admission.
    pub web_seeds: Vec<String>,
    #[serde(default)]
    /// Whether supplied web seeds should replace existing seeds.
    pub replace_web_seeds: bool,
}

impl TorrentCreateRequest {
    /// Translate the client payload into the engine-facing [`AddTorrentOptions`].
    #[must_use]
    pub fn to_options(&self) -> AddTorrentOptions {
        let tags = self
            .tags
            .iter()
            .map(|tag| tag.trim())
            .filter(|tag| !tag.is_empty())
            .map(ToString::to_string)
            .collect();
        let category = self
            .category
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        AddTorrentOptions {
            name_hint: self.name.clone(),
            comment: self.comment.clone(),
            source: self.source.clone(),
            private: self.private,
            category,
            download_dir: self.download_dir.clone(),
            storage_mode: self.storage_mode,
            sequential: self.sequential,
            start_paused: self.start_paused,
            seed_mode: self.seed_mode,
            hash_check_sample_pct: self
                .hash_check_sample_pct
                .and_then(|value| if value > 0 { Some(value) } else { None }),
            super_seeding: self.super_seeding,
            file_rules: FileSelectionRules {
                include: self.include.clone(),
                exclude: self.exclude.clone(),
                skip_fluff: self.skip_fluff,
            },
            rate_limit: TorrentRateLimit {
                download_bps: self.max_download_bps,
                upload_bps: self.max_upload_bps,
            },
            connections_limit: self
                .max_connections
                .and_then(|value| if value > 0 { Some(value) } else { None }),
            seed_ratio_limit: self.seed_ratio_limit,
            seed_time_limit: self.seed_time_limit,
            auto_managed: self.auto_managed,
            queue_position: self.queue_position,
            pex_enabled: self.pex_enabled,
            web_seeds: self.web_seeds.clone(),
            replace_web_seeds: self.replace_web_seeds,
            tracker_auth: None,
            tags,
            cleanup: None,
            trackers: Vec::new(),
            replace_trackers: self.replace_trackers,
        }
    }

    /// Establish the [`TorrentSource`] from the payload.
    ///
    /// Returns `None` if neither a magnet URI nor metainfo payload is provided.
    #[must_use]
    pub fn to_source(&self) -> Option<TorrentSource> {
        self.magnet
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map_or_else(
                || {
                    self.metainfo.as_ref().and_then(|encoded| {
                        general_purpose::STANDARD
                            .decode(encoded)
                            .map(TorrentSource::metainfo)
                            .ok()
                    })
                },
                |magnet| Some(TorrentSource::magnet(magnet.to_string())),
            )
    }
}

/// JSON body accepted by `POST /v1/torrents/create` to author a new torrent file.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TorrentAuthorRequest {
    /// Local filesystem path to a file or directory to hash.
    pub root_path: String,
    #[serde(default)]
    /// Tracker URLs to embed in the metainfo.
    pub trackers: Vec<String>,
    #[serde(default)]
    /// Web seed URLs to embed in the metainfo.
    pub web_seeds: Vec<String>,
    #[serde(default)]
    /// Glob patterns that should be included.
    pub include: Vec<String>,
    #[serde(default)]
    /// Glob patterns that should be excluded.
    pub exclude: Vec<String>,
    #[serde(default)]
    /// Whether the skip-fluff preset should be applied.
    pub skip_fluff: bool,
    #[serde(default)]
    /// Optional piece length override in bytes.
    pub piece_length: Option<u32>,
    #[serde(default)]
    /// Whether to mark the torrent as private.
    pub private: bool,
    #[serde(default)]
    /// Optional comment embedded in the metainfo.
    pub comment: Option<String>,
    #[serde(default)]
    /// Optional source label embedded in the metainfo.
    pub source: Option<String>,
}

impl TorrentAuthorRequest {
    /// Translate the request payload into a core authoring request.
    #[must_use]
    pub fn to_core(&self) -> CoreTorrentAuthorRequest {
        CoreTorrentAuthorRequest {
            root_path: self.root_path.clone(),
            trackers: self.trackers.clone(),
            web_seeds: self.web_seeds.clone(),
            file_rules: FileSelectionRules {
                include: self.include.clone(),
                exclude: self.exclude.clone(),
                skip_fluff: self.skip_fluff,
            },
            piece_length: self.piece_length,
            private: self.private,
            comment: self.comment.clone(),
            source: self.source.clone(),
        }
    }
}

/// File entry returned in a torrent authoring response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorrentAuthorFileView {
    /// Relative file path inside the torrent.
    pub path: String,
    /// File size in bytes.
    pub size_bytes: u64,
}

/// Response returned by `POST /v1/torrents/create`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorrentAuthorResponse {
    /// Base64-encoded metainfo payload.
    pub metainfo: String,
    /// Magnet URI derived from the metainfo.
    pub magnet_uri: String,
    /// Best available info hash string.
    pub info_hash: String,
    /// Effective piece length in bytes.
    pub piece_length: u32,
    /// Total payload size in bytes.
    pub total_size: u64,
    #[serde(default)]
    /// Files included in the torrent.
    pub files: Vec<TorrentAuthorFileView>,
    #[serde(default)]
    /// Warnings generated during authoring.
    pub warnings: Vec<String>,
    #[serde(default)]
    /// Trackers embedded in the metainfo.
    pub trackers: Vec<String>,
    #[serde(default)]
    /// Web seeds embedded in the metainfo.
    pub web_seeds: Vec<String>,
    /// Private flag embedded in the metainfo.
    pub private: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Comment embedded in the metainfo.
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Source label embedded in the metainfo.
    pub source: Option<String>,
}

impl TorrentAuthorResponse {
    #[must_use]
    /// Convert the core authoring result into an API response payload.
    pub fn from_core(result: CoreTorrentAuthorResult) -> Self {
        let files = result
            .files
            .into_iter()
            .map(|file| TorrentAuthorFileView {
                path: file.path,
                size_bytes: file.size_bytes,
            })
            .collect();
        Self {
            metainfo: general_purpose::STANDARD.encode(result.metainfo),
            magnet_uri: result.magnet_uri,
            info_hash: result.info_hash,
            piece_length: result.piece_length,
            total_size: result.total_size,
            files,
            warnings: result.warnings,
            trackers: result.trackers,
            web_seeds: result.web_seeds,
            private: result.private,
            comment: result.comment,
            source: result.source,
        }
    }
}

/// Body accepted by `POST /v1/torrents/{id}/select`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentSelectionRequest {
    #[serde(default)]
    /// Glob patterns that must remain selected.
    pub include: Vec<String>,
    #[serde(default)]
    /// Glob patterns that should be deselected.
    pub exclude: Vec<String>,
    #[serde(default)]
    /// Overrides the skip-fluff preset when present.
    pub skip_fluff: Option<bool>,
    #[serde(default)]
    /// Explicit per-file priority overrides.
    pub priorities: Vec<FilePriorityOverride>,
}

impl From<TorrentSelectionRequest> for FileSelectionUpdate {
    fn from(request: TorrentSelectionRequest) -> Self {
        Self {
            include: request.include,
            exclude: request.exclude,
            skip_fluff: request.skip_fluff.unwrap_or(false),
            priorities: request.priorities,
        }
    }
}

/// Body accepted by `PATCH /v1/torrents/{id}/options`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TorrentOptionsRequest {
    #[serde(default)]
    /// Optional per-torrent peer connection cap.
    pub connections_limit: Option<i32>,
    #[serde(default)]
    /// Optional override for peer exchange behaviour.
    pub pex_enabled: Option<bool>,
    #[serde(default)]
    /// Optional comment update for torrent metadata.
    pub comment: Option<String>,
    #[serde(default)]
    /// Optional source update for torrent metadata.
    pub source: Option<String>,
    #[serde(default)]
    /// Optional private flag update for torrent metadata.
    pub private: Option<bool>,
    #[serde(default)]
    /// Optional toggle to pause or resume the torrent.
    pub paused: Option<bool>,
    #[serde(default)]
    /// Optional toggle for super-seeding.
    pub super_seeding: Option<bool>,
    #[serde(default)]
    /// Optional override for auto-managed queueing.
    pub auto_managed: Option<bool>,
    #[serde(default)]
    /// Optional queue position when auto-managed is disabled.
    pub queue_position: Option<i32>,
    #[serde(default)]
    /// Optional share ratio stop threshold.
    pub seed_ratio_limit: Option<f64>,
    #[serde(default)]
    /// Optional seeding time stop threshold in seconds.
    pub seed_time_limit: Option<u64>,
}

impl TorrentOptionsRequest {
    /// Reject unsupported metadata mutations for post-add updates.
    #[must_use]
    pub const fn unsupported_metadata_message(&self) -> Option<&'static str> {
        if self.comment.is_some() {
            return Some("comment updates are not supported post-add");
        }
        if self.source.is_some() {
            return Some("source updates are not supported post-add");
        }
        if self.private.is_some() {
            return Some("private flag updates are not supported post-add");
        }
        None
    }

    /// Reject unsupported per-torrent seeding overrides.
    #[must_use]
    pub const fn unsupported_seed_limit_message(&self) -> Option<&'static str> {
        if self.seed_ratio_limit.is_some() {
            return Some("seed_ratio_limit overrides are not supported per-torrent");
        }
        if self.seed_time_limit.is_some() {
            return Some("seed_time_limit overrides are not supported per-torrent");
        }
        None
    }

    /// Translate the request payload into a domain update.
    #[must_use]
    pub fn to_update(&self) -> TorrentOptionsUpdate {
        TorrentOptionsUpdate {
            connections_limit: self
                .connections_limit
                .and_then(|value| if value > 0 { Some(value) } else { None }),
            pex_enabled: self.pex_enabled,
            comment: self.comment.clone(),
            source: self.source.clone(),
            private: self.private,
            paused: self.paused,
            super_seeding: self.super_seeding,
            auto_managed: self.auto_managed,
            queue_position: self.queue_position,
            seed_ratio_limit: self.seed_ratio_limit,
            seed_time_limit: self.seed_time_limit,
        }
    }

    /// Returns true when no options were provided.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.connections_limit.is_none()
            && self.pex_enabled.is_none()
            && self.comment.is_none()
            && self.source.is_none()
            && self.private.is_none()
            && self.paused.is_none()
            && self.super_seeding.is_none()
            && self.auto_managed.is_none()
            && self.queue_position.is_none()
            && self.seed_ratio_limit.is_none()
            && self.seed_time_limit.is_none()
    }
}

/// Describes a single tracker associated with a torrent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackerView {
    /// Tracker URL.
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional human-readable status (if available from the engine).
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional last message reported by the tracker.
    pub message: Option<String>,
}

/// Response returned by `GET /v1/torrents/{id}/trackers`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorrentTrackersResponse {
    /// Trackers currently attached to the torrent.
    pub trackers: Vec<TrackerView>,
}

/// Peer snapshot exposed via the torrent endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TorrentPeer {
    /// Endpoint (host:port).
    pub endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional client identifier reported by the peer.
    pub client: Option<String>,
    /// Progress fraction (0.0-1.0).
    pub progress: f64,
    /// Current download rate in bytes per second.
    pub download_bps: u64,
    /// Current upload rate in bytes per second.
    pub upload_bps: u64,
    /// Interest flags for the peer connection.
    pub interest: PeerInterest,
    /// Choke flags for the peer connection.
    pub choke: PeerChoke,
}

impl From<PeerSnapshot> for TorrentPeer {
    fn from(peer: PeerSnapshot) -> Self {
        Self {
            endpoint: peer.endpoint,
            client: peer.client,
            progress: peer.progress,
            download_bps: peer.download_bps,
            upload_bps: peer.upload_bps,
            interest: peer.interest,
            choke: peer.choke,
        }
    }
}

/// Body accepted by `DELETE /v1/torrents/{id}/trackers`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TorrentTrackersRemoveRequest {
    #[serde(default)]
    /// Trackers that should be removed from the torrent.
    pub trackers: Vec<String>,
}

/// Body accepted by `PATCH /v1/torrents/{id}/trackers`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TorrentTrackersRequest {
    #[serde(default)]
    /// Trackers to apply.
    pub trackers: Vec<String>,
    #[serde(default)]
    /// Whether to replace all trackers with the supplied set.
    pub replace: bool,
}

impl TorrentTrackersRequest {
    /// Translate into the domain update.
    #[must_use]
    pub const fn to_update(&self, trackers: Vec<String>) -> TorrentTrackersUpdate {
        TorrentTrackersUpdate {
            trackers,
            replace: self.replace,
        }
    }

    /// Returns true when no tracker changes were supplied.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.trackers.is_empty()
    }
}

/// Body accepted by `PATCH /v1/torrents/{id}/web_seeds`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TorrentWebSeedsRequest {
    #[serde(default)]
    /// Web seeds to apply.
    pub web_seeds: Vec<String>,
    #[serde(default)]
    /// Whether to replace existing web seeds.
    pub replace: bool,
}

impl TorrentWebSeedsRequest {
    /// Translate into the domain update.
    #[must_use]
    pub const fn to_update(&self, web_seeds: Vec<String>) -> TorrentWebSeedsUpdate {
        TorrentWebSeedsUpdate {
            web_seeds,
            replace: self.replace,
        }
    }

    /// Returns true when no web seeds were supplied.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.web_seeds.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose;
    use chrono::{TimeZone, Utc};
    use revaer_events::TorrentState;
    use revaer_torrent_core::{
        FilePriority, FilePriorityOverride, StorageMode, TorrentFile, TorrentProgress,
        TorrentRates, TorrentStatus,
    };
    use std::error::Error;
    use std::io;

    type Result<T> = std::result::Result<T, Box<dyn Error>>;

    fn test_error(message: &'static str) -> Box<dyn Error> {
        Box::new(io::Error::other(message))
    }

    #[test]
    fn torrent_create_request_to_options_maps_fields() {
        let request = TorrentCreateRequest {
            name: Some("Example".to_string()),
            download_dir: Some(".server_root/downloads".to_string()),
            sequential: Some(true),
            include: vec!["**/*.mkv".to_string()],
            exclude: vec!["**/*.tmp".to_string()],
            skip_fluff: true,
            max_download_bps: Some(4_096),
            max_upload_bps: Some(2_048),
            max_connections: Some(50),
            seed_ratio_limit: Some(1.5),
            seed_time_limit: Some(7_200),
            start_paused: Some(true),
            seed_mode: Some(true),
            hash_check_sample_pct: Some(25),
            super_seeding: Some(true),
            tags: vec!["tag-a".to_string(), "tag-b".to_string()],
            auto_managed: Some(false),
            queue_position: Some(2),
            pex_enabled: Some(false),
            web_seeds: vec!["http://seed.example/file".to_string()],
            replace_web_seeds: true,
            storage_mode: Some(StorageMode::Allocate),
            ..TorrentCreateRequest::default()
        };

        let options = request.to_options();
        assert_eq!(options.name_hint.as_deref(), Some("Example"));
        assert_eq!(
            options.download_dir.as_deref(),
            Some(".server_root/downloads")
        );
        assert_eq!(options.sequential, Some(true));
        assert_eq!(options.file_rules.include, vec!["**/*.mkv".to_string()]);
        assert_eq!(options.file_rules.exclude, vec!["**/*.tmp".to_string()]);
        assert!(options.file_rules.skip_fluff);
        assert_eq!(options.rate_limit.download_bps, Some(4_096));
        assert_eq!(options.rate_limit.upload_bps, Some(2_048));
        assert_eq!(options.connections_limit, Some(50));
        assert_eq!(options.seed_ratio_limit, Some(1.5));
        assert_eq!(options.seed_time_limit, Some(7_200));
        assert_eq!(options.start_paused, Some(true));
        assert_eq!(options.seed_mode, Some(true));
        assert_eq!(options.hash_check_sample_pct, Some(25));
        assert_eq!(options.super_seeding, Some(true));
        assert_eq!(options.tags, vec!["tag-a".to_string(), "tag-b".to_string()]);
        assert_eq!(options.auto_managed, Some(false));
        assert_eq!(options.queue_position, Some(2));
        assert_eq!(options.pex_enabled, Some(false));
        assert_eq!(options.storage_mode, Some(StorageMode::Allocate));
        assert_eq!(
            options.web_seeds,
            vec!["http://seed.example/file".to_string()]
        );
        assert!(options.replace_web_seeds);
    }

    #[test]
    fn torrent_create_request_to_source_prefers_magnet() -> Result<()> {
        let request = TorrentCreateRequest {
            magnet: Some("magnet:?xt=urn:btih:example".to_string()),
            metainfo: Some(general_purpose::STANDARD.encode(b"payload")),
            ..TorrentCreateRequest::default()
        };

        match request
            .to_source()
            .ok_or_else(|| test_error("torrent source missing"))?
        {
            TorrentSource::Magnet { uri } => {
                assert!(uri.starts_with("magnet:?xt=urn:btih:example"));
                Ok(())
            }
            TorrentSource::Metainfo { .. } => Err(test_error("unexpected torrent source")),
        }
    }

    #[test]
    fn torrent_create_request_to_source_decodes_metainfo() -> Result<()> {
        let encoded = general_purpose::STANDARD.encode(b"payload-bytes");
        let request = TorrentCreateRequest {
            metainfo: Some(encoded),
            ..TorrentCreateRequest::default()
        };

        match request
            .to_source()
            .ok_or_else(|| test_error("torrent source missing"))?
        {
            TorrentSource::Metainfo { bytes } => {
                assert_eq!(bytes, b"payload-bytes");
                Ok(())
            }
            TorrentSource::Magnet { .. } => Err(test_error("unexpected torrent source")),
        }
    }

    #[test]
    fn torrent_create_request_ignores_non_positive_connection_limit() {
        let request = TorrentCreateRequest {
            max_connections: Some(0),
            ..TorrentCreateRequest::default()
        };

        let options = request.to_options();
        assert!(options.connections_limit.is_none());
    }

    #[test]
    fn torrent_summary_and_detail_from_status_preserves_metadata() -> Result<()> {
        let torrent_id = Uuid::new_v4();
        let added_at = Utc
            .timestamp_millis_opt(0)
            .single()
            .ok_or_else(|| test_error("invalid timestamp"))?;
        let completed_at = Utc
            .timestamp_millis_opt(1_000)
            .single()
            .ok_or_else(|| test_error("invalid timestamp"))?;
        let last_updated = Utc
            .timestamp_millis_opt(2_000)
            .single()
            .ok_or_else(|| test_error("invalid timestamp"))?;
        let status = TorrentStatus {
            id: torrent_id,
            name: Some("Example Torrent".to_string()),
            state: TorrentState::Completed,
            progress: TorrentProgress {
                bytes_downloaded: 75,
                bytes_total: 100,
                eta_seconds: Some(15),
            },
            rates: TorrentRates {
                download_bps: 1_024,
                upload_bps: 512,
                ratio: 0.5,
            },
            files: Some(vec![TorrentFile {
                index: 0,
                path: "movie.mkv".to_string(),
                size_bytes: 100,
                bytes_completed: 75,
                priority: FilePriority::High,
                selected: true,
            }]),
            library_path: Some(".server_root/library/movie".to_string()),
            download_dir: Some(".server_root/downloads/movie".to_string()),
            comment: Some("note".to_string()),
            source: Some("source".to_string()),
            private: Some(true),
            sequential: true,
            added_at,
            completed_at: Some(completed_at),
            last_updated,
        };

        let summary = TorrentSummary::from(status.clone()).with_metadata(
            vec!["tag".to_string()],
            Some("movies".to_string()),
            vec!["tracker".to_string()],
            Some(revaer_torrent_core::TorrentRateLimit {
                download_bps: Some(5_000),
                upload_bps: None,
            }),
            Some(80),
        );
        assert_eq!(summary.id, torrent_id);
        assert_eq!(summary.state.kind, TorrentStateKind::Completed);
        assert_eq!(summary.tags, vec!["tag".to_string()]);
        assert_eq!(summary.category.as_deref(), Some("movies"));
        assert_eq!(summary.trackers, vec!["tracker".to_string()]);
        assert_eq!(
            summary.rate_limit.and_then(|limit| limit.download_bps),
            Some(5_000)
        );
        assert_eq!(summary.connections_limit, Some(80));

        let detail = TorrentDetail::from(status);
        assert_eq!(detail.summary.id, torrent_id);
        let settings = detail
            .settings
            .ok_or_else(|| test_error("settings missing"))?;
        assert_eq!(
            settings.download_dir.as_deref(),
            Some(".server_root/downloads/movie")
        );
        assert!(settings.sequential);
        assert!(settings.selection.is_none());
        let files = detail.files.ok_or_else(|| test_error("files missing"))?;
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "movie.mkv");
        assert!(files[0].selected);
        Ok(())
    }

    #[test]
    fn torrent_selection_request_converts_to_update() {
        let request = TorrentSelectionRequest {
            include: vec!["**/*.mkv".to_string()],
            exclude: vec!["**/*.tmp".to_string()],
            skip_fluff: Some(true),
            priorities: vec![FilePriorityOverride {
                index: 1,
                priority: FilePriority::Low,
            }],
        };

        let update: FileSelectionUpdate = request.clone().into();
        assert_eq!(update.include, request.include);
        assert_eq!(update.exclude, request.exclude);
        assert!(update.skip_fluff);
        assert_eq!(update.priorities.len(), 1);
        assert_eq!(update.priorities[0].index, 1);
        assert_eq!(update.priorities[0].priority, FilePriority::Low);
    }

    #[test]
    fn torrent_options_request_to_update_filters_values() {
        let request = TorrentOptionsRequest {
            connections_limit: Some(0),
            pex_enabled: Some(false),
            comment: None,
            source: None,
            private: None,
            paused: Some(true),
            super_seeding: Some(true),
            auto_managed: Some(false),
            queue_position: Some(3),
            seed_ratio_limit: Some(2.0),
            seed_time_limit: Some(3_600),
        };

        let update = request.to_update();
        assert!(update.connections_limit.is_none());
        assert_eq!(update.pex_enabled, Some(false));
        assert_eq!(update.paused, Some(true));
        assert_eq!(update.super_seeding, Some(true));
        assert_eq!(update.auto_managed, Some(false));
        assert_eq!(update.queue_position, Some(3));
        assert_eq!(update.seed_ratio_limit, Some(2.0));
        assert_eq!(update.seed_time_limit, Some(3_600));
        assert!(!request.is_empty());
    }

    #[test]
    fn torrent_trackers_request_to_update_applies_replace_flag() {
        let request = TorrentTrackersRequest {
            trackers: vec!["https://tracker.example/announce".to_string()],
            replace: true,
        };

        let update = request.to_update(request.trackers.clone());
        assert_eq!(
            update.trackers,
            vec!["https://tracker.example/announce".to_string()]
        );
        assert!(update.replace);
        assert!(!request.is_empty());
    }

    #[test]
    fn torrent_web_seeds_request_to_update_applies_replace_flag() {
        let request = TorrentWebSeedsRequest {
            web_seeds: vec!["http://seed.example/file".to_string()],
            replace: false,
        };

        let update = request.to_update(request.web_seeds.clone());
        assert_eq!(
            update.web_seeds,
            vec!["http://seed.example/file".to_string()]
        );
        assert!(!update.replace);
        assert!(!request.is_empty());
    }
}

/// Envelope describing the action a client wants to perform on a torrent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TorrentAction {
    /// Pause the torrent without removing any data.
    Pause,
    /// Resume a previously paused torrent.
    Resume,
    /// Remove the torrent and optionally delete its data.
    Remove {
        #[serde(default)]
        /// Flag indicating whether to delete downloaded files as well.
        delete_data: bool,
    },
    /// Force a reannounce to trackers.
    Reannounce,
    /// Schedule a full recheck of the torrent contents.
    Recheck,
    /// Toggle sequential download mode.
    Sequential {
        /// Enables sequential reading when `true`.
        enable: bool,
    },
    /// Adjust torrent or global bandwidth limits.
    Rate {
        #[serde(default)]
        /// Download cap in bytes per second.
        download_bps: Option<u64>,
        #[serde(default)]
        /// Upload cap in bytes per second.
        upload_bps: Option<u64>,
    },
    /// Relocate torrent storage to a new download directory.
    Move {
        /// Destination path for in-progress data.
        download_dir: String,
    },
    /// Set or clear a deadline for a specific piece to support streaming.
    PieceDeadline {
        /// Zero-based piece index to target.
        piece: u32,
        #[serde(default)]
        /// Deadline in milliseconds; when omitted the deadline is cleared.
        deadline_ms: Option<u32>,
    },
}

impl TorrentAction {
    /// Translate the action into a [`TorrentRateLimit`] when applicable.
    #[must_use]
    pub const fn to_rate_limit(&self) -> Option<TorrentRateLimit> {
        match self {
            Self::Rate {
                download_bps,
                upload_bps,
            } => Some(TorrentRateLimit {
                download_bps: *download_bps,
                upload_bps: *upload_bps,
            }),
            _ => None,
        }
    }
}
