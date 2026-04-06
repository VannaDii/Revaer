//! Indexer-related HTTP handlers.
//!
//! # Design
//! - Keep each handler module focused on one indexer feature area.
//! - Translate service errors into stable API error shapes.
//! - Use the seeded system actor ID for system-initiated indexer operations.

use crate::http::errors::ApiError;
use uuid::Uuid;

mod allocation;
pub mod backup;
pub mod category_mappings;
pub mod conflicts;
pub mod connectivity;
pub mod definitions;
pub mod import_jobs;
pub mod instances;
mod normalization;
pub mod notifications;
pub mod policies;
pub mod rate_limits;
pub mod routing_policies;
pub mod rss;
pub mod search_pages;
pub mod search_profiles;
pub mod search_requests;
pub mod secrets;
pub mod tags;
#[cfg(test)]
pub(crate) mod test_support;
pub mod torznab_instances;

/// System actor identifier for indexer operations.
///
/// This intentionally uses `Uuid::nil()` as the reserved system actor public ID
/// seeded in `app_user` (`user_id` = 0). Stored procedures expect this UUID to
/// resolve to the system user; do not use it for user-supplied identities or
/// as a generic "missing" sentinel.
pub(crate) const SYSTEM_ACTOR_PUBLIC_ID: Uuid = Uuid::nil();

pub(crate) fn checked_string_capacity(capacity: usize) -> Result<String, ApiError> {
    allocation::checked_string_capacity(capacity)
}

pub(crate) use backup::{export_indexer_backup, restore_indexer_backup};
pub(crate) use category_mappings::{
    delete_media_domain_mapping, delete_tracker_category_mapping, upsert_media_domain_mapping,
    upsert_tracker_category_mapping,
};
pub(crate) use conflicts::{
    list_source_metadata_conflicts, reopen_source_metadata_conflict,
    resolve_source_metadata_conflict,
};
pub(crate) use connectivity::{
    get_indexer_connectivity_profile, get_indexer_health_events, get_indexer_source_reputation,
};
pub(crate) use definitions::{import_cardigann_definition, list_indexer_definitions};
pub(crate) use import_jobs::{
    create_import_job, get_import_job_status, list_import_job_results, run_import_job_prowlarr_api,
    run_import_job_prowlarr_backup,
};
pub(crate) use instances::{
    bind_indexer_instance_field_secret, create_indexer_instance, finalize_indexer_instance_test,
    get_indexer_instance_cf_state, list_indexer_instances, prepare_indexer_instance_test,
    reset_indexer_instance_cf_state, set_indexer_instance_field_value,
    set_indexer_instance_media_domains, set_indexer_instance_tags, update_indexer_instance,
};
pub(crate) use notifications::{
    create_health_notification_hook, delete_health_notification_hook,
    list_health_notification_hooks, update_health_notification_hook,
};
pub(crate) use policies::{
    create_policy_rule, create_policy_set, disable_policy_rule, disable_policy_set,
    enable_policy_rule, enable_policy_set, list_policy_sets, reorder_policy_rules,
    reorder_policy_sets, update_policy_set,
};
pub(crate) use rate_limits::{
    create_rate_limit_policy, delete_rate_limit_policy, list_rate_limit_policies,
    set_indexer_instance_rate_limit, set_routing_policy_rate_limit, update_rate_limit_policy,
};
pub(crate) use routing_policies::{
    bind_routing_policy_secret, create_routing_policy, get_routing_policy, list_routing_policies,
    set_routing_policy_param,
};
pub(crate) use rss::{
    get_indexer_rss_items, get_indexer_rss_subscription, mark_indexer_rss_item_seen,
    put_indexer_rss_subscription,
};
pub(crate) use search_pages::{get_search_page, list_search_pages};
pub(crate) use search_profiles::{
    add_search_profile_policy_set, create_search_profile, list_search_profiles,
    remove_search_profile_policy_set, set_search_profile_default,
    set_search_profile_default_domain, set_search_profile_domain_allowlist,
    set_search_profile_indexer_allow, set_search_profile_indexer_block,
    set_search_profile_tag_allow, set_search_profile_tag_block, set_search_profile_tag_prefer,
    update_search_profile,
};
pub(crate) use search_requests::{cancel_search_request, create_search_request};
pub(crate) use secrets::{create_secret, list_secret_metadata, revoke_secret, rotate_secret};
pub(crate) use tags::{create_tag, delete_tag, delete_tag_by_key, list_tags, update_tag};
pub(crate) use torznab_instances::{
    create_torznab_instance, delete_torznab_instance, list_torznab_instances,
    rotate_torznab_instance_key, set_torznab_instance_state,
};
