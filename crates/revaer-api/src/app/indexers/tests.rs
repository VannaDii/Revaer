use super::*;
use crate::models::IndexerBackupSnapshot;
use chrono::{TimeZone, Utc};

macro_rules! assert_error_contract {
    ($error_ty:ty, $kind:expr, $display:literal) => {{
        let error = <$error_ty>::new($kind)
            .with_code("duplicate_key")
            .with_sqlstate("23505");
        assert_eq!(error.kind(), $kind);
        assert_eq!(error.code(), Some("duplicate_key"));
        assert_eq!(error.sqlstate(), Some("23505"));
        assert_eq!(error.to_string(), $display);
        assert!(std::error::Error::source(&error).is_none());
    }};
}

macro_rules! assert_storage_kind {
    ($expr:expr, $kind:expr) => {{
        let error = $expr.expect_err("storage error expected");
        assert_eq!(error.kind(), $kind);
    }};
}

fn backup_snapshot() -> IndexerBackupSnapshot {
    IndexerBackupSnapshot {
        version: "1".to_string(),
        exported_at: Utc
            .with_ymd_and_hms(2025, 1, 2, 3, 4, 5)
            .single()
            .expect("valid timestamp"),
        tags: Vec::new(),
        rate_limit_policies: Vec::new(),
        routing_policies: Vec::new(),
        indexer_instances: Vec::new(),
        secrets: Vec::new(),
    }
}

#[test]
fn service_error_types_preserve_kind_code_sqlstate_and_display() {
    assert_error_contract!(
        IndexerDefinitionServiceError,
        IndexerDefinitionServiceErrorKind::Storage,
        "indexer definition service error"
    );
    assert_error_contract!(
        IndexerBackupServiceError,
        IndexerBackupServiceErrorKind::Storage,
        "indexer backup service error"
    );
    assert_error_contract!(
        TagServiceError,
        TagServiceErrorKind::Storage,
        "tag service error"
    );
    assert_error_contract!(
        HealthNotificationServiceError,
        HealthNotificationServiceErrorKind::Storage,
        "health notification hook service error"
    );
    assert_error_contract!(
        RoutingPolicyServiceError,
        RoutingPolicyServiceErrorKind::Storage,
        "routing policy service error"
    );
    assert_error_contract!(
        RateLimitPolicyServiceError,
        RateLimitPolicyServiceErrorKind::Storage,
        "rate limit policy service error"
    );
    assert_error_contract!(
        SearchProfileServiceError,
        SearchProfileServiceErrorKind::Storage,
        "search profile service error"
    );
    assert_error_contract!(
        SearchRequestServiceError,
        SearchRequestServiceErrorKind::Storage,
        "search request service error"
    );
    assert_error_contract!(
        ImportJobServiceError,
        ImportJobServiceErrorKind::Storage,
        "import job service error"
    );
    assert_error_contract!(
        SourceMetadataConflictServiceError,
        SourceMetadataConflictServiceErrorKind::Storage,
        "source metadata conflict service error"
    );
    assert_error_contract!(
        PolicyServiceError,
        PolicyServiceErrorKind::Storage,
        "policy service error"
    );
    assert_error_contract!(
        CategoryMappingServiceError,
        CategoryMappingServiceErrorKind::Storage,
        "category mapping service error"
    );
    assert_error_contract!(
        TorznabInstanceServiceError,
        TorznabInstanceServiceErrorKind::Storage,
        "torznab instance service error"
    );
    assert_error_contract!(
        TorznabAccessError,
        TorznabAccessErrorKind::Storage,
        "torznab access error"
    );
    assert_error_contract!(
        IndexerInstanceServiceError,
        IndexerInstanceServiceErrorKind::Storage,
        "indexer instance service error"
    );
    assert_error_contract!(
        IndexerInstanceFieldError,
        IndexerInstanceFieldErrorKind::Storage,
        "indexer instance field service error"
    );
    assert_error_contract!(
        SecretServiceError,
        SecretServiceErrorKind::Storage,
        "secret service error"
    );
}

async fn assert_noop_read_storage_defaults_for_notifications_and_search(
    indexers: &NoopIndexers,
    actor: Uuid,
    resource: Uuid,
    related: Uuid,
    identifier_types: &[String],
    identifier_values: &[String],
    torznab_cat_ids: &[i32],
) {
    assert_storage_kind!(
        indexers
            .indexer_definition_import_cardigann(actor, "yaml", Some(false))
            .await,
        IndexerDefinitionServiceErrorKind::Storage
    );
    assert_storage_kind!(indexers.tag_list(actor).await, TagServiceErrorKind::Storage);
    assert_storage_kind!(
        indexers.secret_metadata_list(actor).await,
        SecretServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.indexer_health_notification_hook_list(actor).await,
        HealthNotificationServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_health_notification_hook_get(actor, resource)
            .await,
        HealthNotificationServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_health_notification_hook_create(
                actor,
                "webhook",
                "Ops",
                "warning",
                Some("https://example.invalid/hook"),
                Some("ops@example.invalid"),
            )
            .await,
        HealthNotificationServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_health_notification_hook_update(HealthNotificationHookUpdateParams {
                actor_user_public_id: actor,
                hook_public_id: resource,
                display_name: Some("Ops"),
                status_threshold: Some("warning"),
                webhook_url: Some("https://example.invalid/hook"),
                email: Some("ops@example.invalid"),
                is_enabled: Some(true),
            })
            .await,
        HealthNotificationServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_health_notification_hook_delete(actor, resource)
            .await,
        HealthNotificationServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .search_request_create(SearchRequestCreateParams {
                actor_user_public_id: Some(actor),
                query_text: "ubuntu",
                query_type: "interactive",
                torznab_mode: Some("search"),
                requested_media_domain_key: Some("tv"),
                page_size: Some(50),
                search_profile_public_id: Some(resource),
                request_policy_set_public_id: Some(related),
                season_number: Some(1),
                episode_number: Some(2),
                identifier_types: Some(identifier_types),
                identifier_values: Some(identifier_values),
                torznab_cat_ids: Some(torznab_cat_ids),
            })
            .await,
        SearchRequestServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.search_request_cancel(actor, resource).await,
        SearchRequestServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.search_page_list(actor, resource).await,
        SearchRequestServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.search_page_fetch(actor, resource, 2).await,
        SearchRequestServiceErrorKind::Storage
    );
}

async fn assert_noop_read_storage_defaults_for_routing_import_and_conflicts(
    indexers: &NoopIndexers,
    actor: Uuid,
    resource: Uuid,
    related: Uuid,
) {
    assert_storage_kind!(
        indexers.routing_policy_list(actor).await,
        RoutingPolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.routing_policy_get(actor, resource).await,
        RoutingPolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.rate_limit_policy_list(actor).await,
        RateLimitPolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.search_profile_list(actor).await,
        SearchProfileServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .import_job_create(
                actor,
                "prowlarr",
                Some(false),
                Some(resource),
                Some(related)
            )
            .await,
        ImportJobServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .import_job_run_prowlarr_api(resource, "https://example.invalid", related)
            .await,
        ImportJobServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .import_job_run_prowlarr_backup(resource, "blob://backup")
            .await,
        ImportJobServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.import_job_get_status(resource).await,
        ImportJobServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.import_job_list_results(resource).await,
        ImportJobServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .source_metadata_conflict_list(actor, Some(true), Some(25))
            .await,
        SourceMetadataConflictServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .source_metadata_conflict_resolve(actor, 42, "accepted_incoming", Some("note"))
            .await,
        SourceMetadataConflictServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .source_metadata_conflict_reopen(actor, 42, Some("note"))
            .await,
        SourceMetadataConflictServiceErrorKind::Storage
    );
}

async fn assert_noop_read_storage_defaults_for_backup_and_torznab(
    indexers: &NoopIndexers,
    actor: Uuid,
    resource: Uuid,
    related: Uuid,
) {
    assert_storage_kind!(
        indexers.indexer_backup_export(actor).await,
        IndexerBackupServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_backup_restore(actor, &backup_snapshot())
            .await,
        IndexerBackupServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.policy_set_list(actor).await,
        PolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.torznab_instance_list(actor).await,
        TorznabInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .torznab_instance_authenticate(resource, "secret")
            .await,
        TorznabAccessErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.torznab_download_prepare(resource, related).await,
        TorznabAccessErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.torznab_category_list().await,
        TorznabAccessErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .torznab_feed_category_ids(resource, related, Some(5000), Some(42))
            .await,
        TorznabAccessErrorKind::Storage
    );
}

async fn assert_noop_read_storage_defaults_for_indexer_instances(
    indexers: &NoopIndexers,
    actor: Uuid,
    resource: Uuid,
) {
    assert_storage_kind!(
        indexers.indexer_instance_list(actor).await,
        IndexerInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_connectivity_profile_get(actor, resource)
            .await,
        IndexerInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_source_reputation_list(IndexerSourceReputationListParams {
                actor_user_public_id: actor,
                indexer_instance_public_id: resource,
                window_key: Some("24h"),
                limit: Some(10),
            })
            .await,
        IndexerInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_health_event_list(IndexerHealthEventListParams {
                actor_user_public_id: actor,
                indexer_instance_public_id: resource,
                limit: Some(10),
            })
            .await,
        IndexerInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.indexer_rss_subscription_get(actor, resource).await,
        IndexerInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_rss_subscription_set(IndexerRssSubscriptionParams {
                actor_user_public_id: actor,
                indexer_instance_public_id: resource,
                is_enabled: true,
                interval_seconds: Some(900),
            })
            .await,
        IndexerInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_rss_seen_list(IndexerRssSeenListParams {
                actor_user_public_id: actor,
                indexer_instance_public_id: resource,
                limit: Some(20),
            })
            .await,
        IndexerInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_rss_seen_mark(IndexerRssSeenMarkParams {
                actor_user_public_id: actor,
                indexer_instance_public_id: resource,
                item_guid: Some("guid"),
                infohash_v1: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                infohash_v2: None,
                magnet_hash: None,
            })
            .await,
        IndexerInstanceServiceErrorKind::Storage
    );
}

async fn assert_noop_mutating_storage_defaults_for_tags_and_routing(
    indexers: &NoopIndexers,
    actor: Uuid,
    resource: Uuid,
    related: Uuid,
) {
    assert_storage_kind!(
        indexers.indexer_definition_list(actor).await,
        IndexerDefinitionServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.tag_create(actor, "hd", "HD").await,
        TagServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .tag_update(actor, Some(resource), Some("hd"), "Ultra HD")
            .await,
        TagServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.tag_delete(actor, Some(resource), Some("hd")).await,
        TagServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .routing_policy_create(actor, "Primary", "strict")
            .await,
        RoutingPolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .routing_policy_set_param(
                actor,
                resource,
                "timeout_seconds",
                Some("30"),
                Some(30),
                Some(true),
            )
            .await,
        RoutingPolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .routing_policy_bind_secret(actor, resource, "api_key", related)
            .await,
        RoutingPolicyServiceErrorKind::Storage
    );
}

async fn assert_noop_mutating_storage_defaults_for_rate_limits(
    indexers: &NoopIndexers,
    actor: Uuid,
    resource: Uuid,
    related: Uuid,
) {
    assert_storage_kind!(
        indexers
            .rate_limit_policy_create(actor, "Default", 60, 10, 4)
            .await,
        RateLimitPolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .rate_limit_policy_update(actor, resource, Some("Burst"), Some(120), Some(20), Some(8))
            .await,
        RateLimitPolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .rate_limit_policy_soft_delete(actor, resource)
            .await,
        RateLimitPolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_instance_set_rate_limit_policy(actor, resource, Some(related))
            .await,
        RateLimitPolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .routing_policy_set_rate_limit_policy(actor, resource, Some(related))
            .await,
        RateLimitPolicyServiceErrorKind::Storage
    );
}

async fn assert_noop_mutating_storage_defaults_for_search_profiles(
    indexers: &NoopIndexers,
    actor: Uuid,
    resource: Uuid,
    related: Uuid,
    media_domain_keys: &[String],
    tag_keys: &[String],
    ordered_ids: &[Uuid],
) {
    assert_storage_kind!(
        indexers
            .search_profile_create(
                actor,
                "Primary",
                Some(true),
                Some(50),
                Some("tv"),
                Some(related)
            )
            .await,
        SearchProfileServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .search_profile_update(actor, resource, Some("Updated"), Some(25))
            .await,
        SearchProfileServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .search_profile_set_default(actor, resource, Some(25))
            .await,
        SearchProfileServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .search_profile_set_default_domain(actor, resource, Some("tv"))
            .await,
        SearchProfileServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .search_profile_set_domain_allowlist(actor, resource, media_domain_keys)
            .await,
        SearchProfileServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .search_profile_add_policy_set(actor, resource, related)
            .await,
        SearchProfileServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .search_profile_remove_policy_set(actor, resource, related)
            .await,
        SearchProfileServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .search_profile_indexer_allow(actor, resource, ordered_ids)
            .await,
        SearchProfileServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .search_profile_indexer_block(actor, resource, ordered_ids)
            .await,
        SearchProfileServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .search_profile_tag_allow(actor, resource, Some(ordered_ids), Some(tag_keys))
            .await,
        SearchProfileServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .search_profile_tag_block(actor, resource, Some(ordered_ids), Some(tag_keys))
            .await,
        SearchProfileServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .search_profile_tag_prefer(actor, resource, Some(ordered_ids), Some(tag_keys))
            .await,
        SearchProfileServiceErrorKind::Storage
    );
}

async fn assert_noop_mutating_storage_defaults_for_policies(
    indexers: &NoopIndexers,
    actor: Uuid,
    resource: Uuid,
    ordered_ids: &[Uuid],
) {
    assert_storage_kind!(
        indexers
            .policy_set_create(actor, "Minimum Seed", "request", Some(true))
            .await,
        PolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .policy_set_update(actor, resource, Some("Updated"))
            .await,
        PolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.policy_set_enable(actor, resource).await,
        PolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.policy_set_disable(actor, resource).await,
        PolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.policy_set_reorder(actor, ordered_ids).await,
        PolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .policy_rule_create(PolicyRuleCreateParams {
                actor_user_public_id: actor,
                policy_set_public_id: resource,
                rule_type: "seed_ratio".to_string(),
                match_field: "tracker".to_string(),
                match_operator: "equals".to_string(),
                sort_order: 10,
                match_value_text: Some("tracker-a".to_string()),
                match_value_int: None,
                match_value_uuid: None,
                value_set_items: Some(vec![PolicyRuleValueItem {
                    value_text: Some("tracker-a".to_string()),
                    value_int: None,
                    value_bigint: None,
                    value_uuid: None,
                }]),
                action: "block".to_string(),
                severity: "warning".to_string(),
                is_case_insensitive: Some(true),
                rationale: Some("keep ratios healthy".to_string()),
                expires_at: Some(
                    Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0)
                        .single()
                        .expect("valid timestamp"),
                ),
            })
            .await,
        PolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.policy_rule_enable(actor, resource).await,
        PolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.policy_rule_disable(actor, resource).await,
        PolicyServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .policy_rule_reorder(actor, resource, ordered_ids)
            .await,
        PolicyServiceErrorKind::Storage
    );
}

async fn assert_noop_mutating_storage_defaults_for_category_mappings(
    indexers: &NoopIndexers,
    actor: Uuid,
    resource: Uuid,
    related: Uuid,
) {
    assert_storage_kind!(
        indexers
            .tracker_category_mapping_upsert(TrackerCategoryMappingUpsertParams {
                actor_user_public_id: actor,
                torznab_instance_public_id: Some(resource),
                indexer_definition_upstream_slug: Some("prowlarr"),
                indexer_instance_public_id: Some(related),
                tracker_category: 5000,
                tracker_subcategory: Some(42),
                torznab_cat_id: 5030,
                media_domain_key: Some("tv"),
            })
            .await,
        CategoryMappingServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .tracker_category_mapping_delete(TrackerCategoryMappingDeleteParams {
                actor_user_public_id: actor,
                torznab_instance_public_id: Some(resource),
                indexer_definition_upstream_slug: Some("prowlarr"),
                indexer_instance_public_id: Some(related),
                tracker_category: 5000,
                tracker_subcategory: Some(42),
            })
            .await,
        CategoryMappingServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .media_domain_mapping_upsert(actor, "tv", 5030, Some(true))
            .await,
        CategoryMappingServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .media_domain_mapping_delete(actor, "tv", 5030)
            .await,
        CategoryMappingServiceErrorKind::Storage
    );
}

async fn assert_noop_mutating_storage_defaults_for_torznab(
    indexers: &NoopIndexers,
    actor: Uuid,
    resource: Uuid,
) {
    assert_storage_kind!(
        indexers
            .torznab_instance_create(actor, resource, "Primary")
            .await,
        TorznabInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.torznab_instance_rotate_key(actor, resource).await,
        TorznabInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .torznab_instance_enable_disable(actor, resource, true)
            .await,
        TorznabInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.torznab_instance_soft_delete(actor, resource).await,
        TorznabInstanceServiceErrorKind::Storage
    );
}

async fn assert_noop_mutating_storage_defaults_for_instances(
    indexers: &NoopIndexers,
    actor: Uuid,
    resource: Uuid,
    related: Uuid,
    media_domain_keys: &[String],
    tag_keys: &[String],
    ordered_ids: &[Uuid],
) {
    assert_storage_kind!(
        indexers
            .indexer_instance_create(
                actor,
                "prowlarr",
                "Primary",
                Some(10),
                Some("trusted"),
                Some(related),
            )
            .await,
        IndexerInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_instance_update(IndexerInstanceUpdateParams {
                actor_user_public_id: actor,
                indexer_instance_public_id: resource,
                display_name: Some("Updated"),
                priority: Some(20),
                trust_tier_key: Some("trusted"),
                routing_policy_public_id: Some(related),
                is_enabled: Some(true),
                enable_rss: Some(true),
                enable_automatic_search: Some(true),
                enable_interactive_search: Some(false),
            })
            .await,
        IndexerInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_instance_set_media_domains(actor, resource, media_domain_keys)
            .await,
        IndexerInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_instance_set_tags(actor, resource, Some(ordered_ids), Some(tag_keys))
            .await,
        IndexerInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_instance_field_set_value(IndexerInstanceFieldValueParams {
                actor_user_public_id: actor,
                indexer_instance_public_id: resource,
                field_name: "api_key",
                value_plain: Some("value"),
                value_int: Some(5),
                value_decimal: Some("1.5"),
                value_bool: Some(true),
            })
            .await,
        IndexerInstanceFieldErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_instance_field_bind_secret(actor, resource, "api_key", related)
            .await,
        IndexerInstanceFieldErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_cf_state_reset(IndexerCfStateResetParams {
                actor_user_public_id: actor,
                indexer_instance_public_id: resource,
                reason: "manual reset",
            })
            .await,
        IndexerInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.indexer_cf_state_get(actor, resource).await,
        IndexerInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_instance_test_prepare(actor, resource)
            .await,
        IndexerInstanceServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers
            .indexer_instance_test_finalize(IndexerInstanceTestFinalizeParams {
                actor_user_public_id: actor,
                indexer_instance_public_id: resource,
                ok: false,
                error_class: Some("network"),
                error_code: Some("timeout"),
                detail: Some("request timed out"),
                result_count: Some(0),
            })
            .await,
        IndexerInstanceServiceErrorKind::Storage
    );
}

async fn assert_noop_mutating_storage_defaults_for_secrets(
    indexers: &NoopIndexers,
    actor: Uuid,
    resource: Uuid,
) {
    assert_storage_kind!(
        indexers.secret_create(actor, "api_key", "secret").await,
        SecretServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.secret_rotate(actor, resource, "secret-2").await,
        SecretServiceErrorKind::Storage
    );
    assert_storage_kind!(
        indexers.secret_revoke(actor, resource).await,
        SecretServiceErrorKind::Storage
    );
}

#[tokio::test]
async fn noop_indexers_storage_defaults_cover_read_paths() {
    let indexers = NoopIndexers;
    let actor = Uuid::from_u128(1);
    let resource = Uuid::from_u128(2);
    let related = Uuid::from_u128(3);
    let identifier_types = vec!["imdb".to_string()];
    let identifier_values = vec!["tt1234567".to_string()];
    let torznab_cat_ids = vec![2000];
    assert_noop_read_storage_defaults_for_notifications_and_search(
        &indexers,
        actor,
        resource,
        related,
        &identifier_types,
        &identifier_values,
        &torznab_cat_ids,
    )
    .await;
    assert_noop_read_storage_defaults_for_routing_import_and_conflicts(
        &indexers, actor, resource, related,
    )
    .await;
    assert_noop_read_storage_defaults_for_backup_and_torznab(&indexers, actor, resource, related)
        .await;
    assert_noop_read_storage_defaults_for_indexer_instances(&indexers, actor, resource).await;
}

#[tokio::test]
async fn noop_indexers_storage_defaults_cover_mutating_paths() {
    let indexers = NoopIndexers;
    let actor = Uuid::from_u128(11);
    let resource = Uuid::from_u128(12);
    let related = Uuid::from_u128(13);
    let media_domain_keys = vec!["tv".to_string(), "movies".to_string()];
    let tag_keys = vec!["hd".to_string(), "freeleech".to_string()];
    let ordered_ids = vec![resource, related];
    assert_noop_mutating_storage_defaults_for_tags_and_routing(&indexers, actor, resource, related)
        .await;
    assert_noop_mutating_storage_defaults_for_rate_limits(&indexers, actor, resource, related)
        .await;
    assert_noop_mutating_storage_defaults_for_search_profiles(
        &indexers,
        actor,
        resource,
        related,
        &media_domain_keys,
        &tag_keys,
        &ordered_ids,
    )
    .await;
    assert_noop_mutating_storage_defaults_for_policies(&indexers, actor, resource, &ordered_ids)
        .await;
    assert_noop_mutating_storage_defaults_for_category_mappings(
        &indexers, actor, resource, related,
    )
    .await;
    assert_noop_mutating_storage_defaults_for_torznab(&indexers, actor, resource).await;
    assert_noop_mutating_storage_defaults_for_instances(
        &indexers,
        actor,
        resource,
        related,
        &media_domain_keys,
        &tag_keys,
        &ordered_ids,
    )
    .await;
    assert_noop_mutating_storage_defaults_for_secrets(&indexers, actor, resource).await;
}

#[tokio::test]
async fn test_indexers_returns_arc_wrapped_noop_facade() {
    let indexers = test_indexers();
    assert_storage_kind!(
        indexers.indexer_definition_list(Uuid::from_u128(99)).await,
        IndexerDefinitionServiceErrorKind::Storage
    );
}
