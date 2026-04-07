use super::super::*;
use chrono::Utc;
use revaer_data::error::DataError;
use revaer_data::indexers::backup::{
    BackupIndexerInstanceRow, BackupRateLimitPolicyRow, BackupRoutingPolicyRow, BackupTagRow,
};
use revaer_data::indexers::conflicts::SourceMetadataConflictRow;
use revaer_data::indexers::definitions::{ImportedIndexerDefinitionRow, IndexerDefinitionRow};
use revaer_data::indexers::import_jobs::{ImportJobResultRow, ImportJobStatusRow};
use revaer_data::indexers::notifications::IndexerHealthNotificationHookRow;
use revaer_data::indexers::policies::PolicySetRuleListRow;
use revaer_data::indexers::search_pages::{
    SearchPageFetchRow, SearchPageSummaryRow, SearchRequestExplainabilityRow,
};
use revaer_data::indexers::search_profiles::SearchProfileListRow;
use revaer_data::indexers::torznab::TorznabInstanceListRow;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

fn job_failed(detail: &str) -> DataError {
    DataError::JobFailed {
        operation: "test-op",
        job_key: "test-job",
        error_code: Some("P0001".to_string()),
        error_detail: Some(detail.to_string()),
    }
}

fn assert_cardigann_prepared_fields(prepared: &PreparedCardigannDefinitionImport) {
    let base_url = prepared
        .fields
        .iter()
        .find(|field| field.field_name == "base_url")
        .expect("base_url field should be present");
    assert_eq!(base_url.label, "Base URL");
    assert_eq!(base_url.field_type, "string");
    assert!(base_url.is_required);
    assert_eq!(
        base_url.default_value_plain.as_deref(),
        Some("https://indexer.example")
    );

    let api_key = prepared
        .fields
        .iter()
        .find(|field| field.field_name == "api_key")
        .expect("api_key field should be present");
    assert_eq!(api_key.label, "api_key");
    assert_eq!(api_key.field_type, "api_key");
    assert!(api_key.is_required);

    let ratio = prepared
        .fields
        .iter()
        .find(|field| field.field_name == "ratio")
        .expect("ratio field should be present");
    assert_eq!(ratio.field_type, "number_decimal");
    assert_eq!(ratio.default_value_decimal.as_deref(), Some("1.25"));

    let retries = prepared
        .fields
        .iter()
        .find(|field| field.field_name == "retries")
        .expect("retries field should be present");
    assert_eq!(retries.field_type, "number_int");
    assert_eq!(retries.default_value_int, Some(3));

    let enabled = prepared
        .fields
        .iter()
        .find(|field| field.field_name == "enabled")
        .expect("enabled field should be present");
    assert_eq!(enabled.field_type, "bool");
    assert_eq!(enabled.default_value_bool, Some(true));

    let mode = prepared
        .fields
        .iter()
        .find(|field| field.field_name == "mode")
        .expect("mode field should be present");
    assert_eq!(mode.field_type, "select_single");
    assert_eq!(mode.option_values, vec!["standard", "proxy"]);
    assert_eq!(mode.option_labels, vec!["Standard", "Proxy"]);

    let data_import = mode.to_data_import();
    assert_eq!(data_import.field_name, "mode");
    assert_eq!(data_import.field_type, "select_single");
    assert_eq!(data_import.option_values, vec!["standard", "proxy"]);
}

fn assert_cardigann_canonical_json(prepared: &PreparedCardigannDefinitionImport) {
    let canonical: JsonValue = serde_json::from_str(&prepared.canonical_definition_text)
        .expect("canonical json should be valid");
    assert_eq!(canonical["id"], "demo-slug");
    assert_eq!(canonical["name"], "Demo Indexer");
    assert_eq!(
        canonical["settings"][0]["default_value"],
        "https://indexer.example"
    );
    assert_eq!(canonical["settings"][2]["default_value"], "1.25");
    assert_eq!(canonical["settings"][3]["default_value"], "3");
    assert_eq!(canonical["settings"][4]["default_value"], "true");
    assert_eq!(canonical["settings"][5]["options"][1]["label"], "Proxy");
}

fn assert_definition_import_and_hook_rows(now: chrono::DateTime<Utc>) {
    let definition = map_indexer_definition_row(IndexerDefinitionRow {
        upstream_source: "cardigann".to_string(),
        upstream_slug: "demo".to_string(),
        display_name: "Demo".to_string(),
        protocol: "torrent".to_string(),
        engine: "torznab".to_string(),
        schema_version: 2,
        definition_hash: "f".repeat(64),
        is_deprecated: false,
        created_at: now,
        updated_at: now,
    });
    assert_eq!(definition.upstream_slug, "demo");

    let imported = map_imported_indexer_definition_row(ImportedIndexerDefinitionRow {
        upstream_source: "cardigann".to_string(),
        upstream_slug: "demo".to_string(),
        display_name: "Demo".to_string(),
        protocol: "torrent".to_string(),
        engine: "torznab".to_string(),
        schema_version: 2,
        definition_hash: "a".repeat(64),
        is_deprecated: true,
        created_at: now,
        updated_at: now,
        field_count: 3,
        option_count: 2,
    });
    assert_eq!(imported.field_count, 3);
    assert_eq!(imported.option_count, 2);
    assert!(imported.definition.is_deprecated);

    let hook = map_health_notification_hook_row(IndexerHealthNotificationHookRow {
        indexer_health_notification_hook_public_id: Uuid::new_v4(),
        channel: "webhook".to_string(),
        display_name: "Pager".to_string(),
        status_threshold: "degraded".to_string(),
        webhook_url: Some("https://hooks.example".to_string()),
        email: None,
        is_enabled: true,
        updated_at: now,
    });
    assert_eq!(hook.channel, "webhook");
    assert_eq!(hook.webhook_url.as_deref(), Some("https://hooks.example"));
}

fn assert_profile_policy_and_torznab_inventory() {
    let policy_public_id = Uuid::new_v4();
    let first_rule_public_id = Uuid::new_v4();
    let second_rule_public_id = Uuid::new_v4();
    let indexer_instance_public_id = Uuid::new_v4();
    let search_profile_public_id = Uuid::new_v4();
    let torznab_instance_public_id = Uuid::new_v4();

    let search_profile = build_search_profile_inventory_item(SearchProfileListRow {
        search_profile_public_id,
        display_name: "Movies".to_string(),
        is_default: true,
        page_size: Some(75),
        default_media_domain_key: Some("movies".to_string()),
        media_domain_keys: vec!["movies".to_string(), "tv".to_string()],
        policy_set_public_ids: vec![policy_public_id],
        policy_set_display_names: vec!["Quality".to_string()],
        allow_indexer_public_ids: vec![indexer_instance_public_id],
        block_indexer_public_ids: vec![Uuid::new_v4()],
        allow_tag_keys: vec!["trusted".to_string()],
        block_tag_keys: vec!["cam".to_string()],
        prefer_tag_keys: vec!["freeleech".to_string()],
    });
    assert!(search_profile.is_default);
    assert_eq!(search_profile.policy_set_display_names, vec!["Quality"]);

    let policy_inventory = build_policy_set_inventory(&[
        PolicySetRuleListRow {
            policy_set_public_id: policy_public_id,
            policy_set_display_name: "Quality".to_string(),
            scope: "profile".to_string(),
            is_enabled: true,
            user_public_id: None,
            policy_rule_public_id: Some(second_rule_public_id),
            rule_type: Some("block_title_regex".to_string()),
            match_field: Some("title".to_string()),
            match_operator: Some("regex".to_string()),
            sort_order: Some(20),
            match_value_text: Some("cam".to_string()),
            match_value_int: None,
            match_value_uuid: None,
            action: Some("drop_canonical".to_string()),
            severity: Some("hard".to_string()),
            is_case_insensitive: Some(true),
            rationale: Some("cam rip".to_string()),
            expires_at: None,
            is_rule_disabled: Some(true),
        },
        PolicySetRuleListRow {
            policy_set_public_id: policy_public_id,
            policy_set_display_name: "Quality".to_string(),
            scope: "profile".to_string(),
            is_enabled: true,
            user_public_id: None,
            policy_rule_public_id: Some(first_rule_public_id),
            rule_type: Some("block_title_regex".to_string()),
            match_field: Some("title".to_string()),
            match_operator: Some("regex".to_string()),
            sort_order: Some(10),
            match_value_text: Some("ts".to_string()),
            match_value_int: None,
            match_value_uuid: None,
            action: Some("drop_canonical".to_string()),
            severity: Some("hard".to_string()),
            is_case_insensitive: Some(false),
            rationale: Some("ts".to_string()),
            expires_at: None,
            is_rule_disabled: Some(false),
        },
    ]);
    assert_eq!(policy_inventory.len(), 1);
    assert_eq!(
        policy_inventory[0].rules[0].policy_rule_public_id,
        first_rule_public_id
    );
    assert_eq!(
        policy_inventory[0].rules[1].policy_rule_public_id,
        second_rule_public_id
    );
    assert!(policy_inventory[0].rules[1].is_disabled);

    let torznab = build_torznab_instance_inventory_item(TorznabInstanceListRow {
        torznab_instance_public_id,
        display_name: "Torznab".to_string(),
        is_enabled: true,
        search_profile_public_id,
        search_profile_display_name: "Movies".to_string(),
    });
    assert_eq!(torznab.search_profile_display_name, "Movies");
}

fn sample_backup_routing_rows(
    routing_policy_public_id: Uuid,
    rate_limit_policy_public_id: Uuid,
    secret_public_id: Uuid,
) -> Vec<BackupRoutingPolicyRow> {
    vec![
        BackupRoutingPolicyRow {
            routing_policy_public_id,
            display_name: "Route A".to_string(),
            mode: "http_proxy".to_string(),
            rate_limit_policy_public_id: Some(rate_limit_policy_public_id),
            rate_limit_display_name: Some("Burst".to_string()),
            param_key: Some("proxy_host".to_string()),
            value_plain: Some("proxy.internal".to_string()),
            value_int: None,
            value_bool: None,
            secret_public_id: None,
            secret_type: None,
        },
        BackupRoutingPolicyRow {
            routing_policy_public_id,
            display_name: "Route A".to_string(),
            mode: "http_proxy".to_string(),
            rate_limit_policy_public_id: Some(rate_limit_policy_public_id),
            rate_limit_display_name: Some("Burst".to_string()),
            param_key: Some("http_proxy_auth".to_string()),
            value_plain: None,
            value_int: None,
            value_bool: None,
            secret_public_id: Some(secret_public_id),
            secret_type: Some("password".to_string()),
        },
    ]
}

fn assert_routing_inventory_summary(routing_rows: &[BackupRoutingPolicyRow]) {
    let routing_inventory = build_routing_policy_inventory(routing_rows);
    assert_eq!(routing_inventory.len(), 1);
    assert_eq!(routing_inventory[0].parameter_count, 2);
    assert_eq!(routing_inventory[0].secret_binding_count, 1);
}

fn sample_backup_indexer_instance_rows(
    indexer_instance_public_id: Uuid,
    routing_policy_public_id: Uuid,
    rate_limit_policy_public_id: Uuid,
    secret_public_id: Uuid,
) -> Vec<BackupIndexerInstanceRow> {
    vec![
        BackupIndexerInstanceRow {
            indexer_instance_public_id,
            upstream_slug: "demo".to_string(),
            display_name: "Indexer A".to_string(),
            instance_status: "enabled".to_string(),
            rss_status: "enabled".to_string(),
            automatic_search_status: "enabled".to_string(),
            interactive_search_status: "enabled".to_string(),
            priority: 50,
            trust_tier_key: Some("public".to_string()),
            routing_policy_public_id: Some(routing_policy_public_id),
            routing_policy_display_name: Some("Route A".to_string()),
            connect_timeout_ms: 5_000,
            read_timeout_ms: 15_000,
            max_parallel_requests: 3,
            rate_limit_policy_public_id: Some(rate_limit_policy_public_id),
            rate_limit_display_name: Some("Burst".to_string()),
            rss_subscription_enabled: Some(true),
            rss_interval_seconds: Some(1800),
            media_domain_key: Some("tv".to_string()),
            tag_key: Some("trusted".to_string()),
            field_name: Some("base_url".to_string()),
            field_type: Some("text".to_string()),
            value_plain: Some("https://indexer.example".to_string()),
            value_int: None,
            value_decimal: None,
            value_bool: None,
            secret_public_id: None,
            secret_type: None,
        },
        BackupIndexerInstanceRow {
            indexer_instance_public_id,
            upstream_slug: "demo".to_string(),
            display_name: "Indexer A".to_string(),
            instance_status: "enabled".to_string(),
            rss_status: "enabled".to_string(),
            automatic_search_status: "enabled".to_string(),
            interactive_search_status: "enabled".to_string(),
            priority: 50,
            trust_tier_key: Some("public".to_string()),
            routing_policy_public_id: Some(routing_policy_public_id),
            routing_policy_display_name: Some("Route A".to_string()),
            connect_timeout_ms: 5_000,
            read_timeout_ms: 15_000,
            max_parallel_requests: 3,
            rate_limit_policy_public_id: Some(rate_limit_policy_public_id),
            rate_limit_display_name: Some("Burst".to_string()),
            rss_subscription_enabled: Some(true),
            rss_interval_seconds: Some(1800),
            media_domain_key: Some("movies".to_string()),
            tag_key: Some("freeleech".to_string()),
            field_name: Some("api_key".to_string()),
            field_type: Some("api_key".to_string()),
            value_plain: None,
            value_int: None,
            value_decimal: None,
            value_bool: None,
            secret_public_id: Some(secret_public_id),
            secret_type: Some("password".to_string()),
        },
    ]
}

fn assert_backup_indexer_inventory(instance_rows: &[BackupIndexerInstanceRow]) {
    let instance_inventory = build_indexer_instance_inventory(instance_rows);
    assert_eq!(instance_inventory.len(), 1);
    assert_eq!(
        instance_inventory[0].media_domain_keys,
        vec!["movies".to_string(), "tv".to_string()]
    );
    assert_eq!(
        instance_inventory[0].tag_keys,
        vec!["freeleech".to_string(), "trusted".to_string()]
    );
    assert_eq!(instance_inventory[0].fields.len(), 2);
}

fn assert_backup_snapshot_summary(
    tag_public_id: Uuid,
    rate_limit_policy_public_id: Uuid,
    secret_public_id: Uuid,
    routing_rows: &[BackupRoutingPolicyRow],
    instance_rows: &[BackupIndexerInstanceRow],
) {
    let snapshot = build_backup_snapshot(
        vec![BackupTagRow {
            tag_public_id,
            tag_key: "trusted".to_string(),
            display_name: "Trusted".to_string(),
        }],
        vec![BackupRateLimitPolicyRow {
            rate_limit_policy_public_id,
            display_name: "Burst".to_string(),
            requests_per_minute: 60,
            burst: 20,
            concurrent_requests: 2,
            is_system: false,
        }],
        routing_rows,
        instance_rows,
    );
    assert_eq!(snapshot.version, "revaer.indexers.backup.v1");
    assert_eq!(snapshot.tags.len(), 1);
    assert_eq!(snapshot.rate_limit_policies.len(), 1);
    assert_eq!(snapshot.routing_policies.len(), 1);
    assert_eq!(snapshot.routing_policies[0].parameters.len(), 2);
    assert_eq!(snapshot.indexer_instances.len(), 1);
    assert_eq!(
        snapshot.indexer_instances[0].media_domain_keys,
        vec!["movies".to_string(), "tv".to_string()]
    );
    assert_eq!(snapshot.secrets.len(), 1);
    assert_eq!(snapshot.secrets[0].secret_public_id, secret_public_id);
}

fn assert_routing_instance_and_backup_snapshot() {
    let routing_policy_public_id = Uuid::new_v4();
    let rate_limit_policy_public_id = Uuid::new_v4();
    let indexer_instance_public_id = Uuid::new_v4();
    let secret_public_id = Uuid::new_v4();
    let tag_public_id = Uuid::new_v4();

    let routing_rows = sample_backup_routing_rows(
        routing_policy_public_id,
        rate_limit_policy_public_id,
        secret_public_id,
    );
    let instance_rows = sample_backup_indexer_instance_rows(
        indexer_instance_public_id,
        routing_policy_public_id,
        rate_limit_policy_public_id,
        secret_public_id,
    );

    assert_routing_inventory_summary(&routing_rows);
    assert_backup_indexer_inventory(&instance_rows);
    assert_backup_snapshot_summary(
        tag_public_id,
        rate_limit_policy_public_id,
        secret_public_id,
        &routing_rows,
        &instance_rows,
    );
}

fn assert_search_page_and_explainability_rows(now: chrono::DateTime<Utc>) {
    let blocked_rule_public_id = Uuid::new_v4();
    let canonical_torrent_public_id = Uuid::new_v4();
    let canonical_torrent_source_public_id = Uuid::new_v4();
    let indexer_instance_public_id = Uuid::new_v4();

    let summary = map_search_page_summary(&SearchPageSummaryRow {
        page_number: 2,
        sealed_at: Some(now),
        item_count: 4,
    });
    assert_eq!(summary.page_number, 2);
    assert_eq!(summary.item_count, 4);

    let item = map_search_page_item(&SearchPageFetchRow {
        page_number: 2,
        sealed_at: Some(now),
        item_count: 4,
        item_position: Some(1),
        canonical_torrent_public_id: Some(canonical_torrent_public_id),
        title_display: Some("Ubuntu".to_string()),
        size_bytes: Some(1_024),
        infohash_v1: Some("a".repeat(40)),
        infohash_v2: None,
        magnet_hash: None,
        canonical_torrent_source_public_id: Some(canonical_torrent_source_public_id),
        indexer_instance_public_id: Some(indexer_instance_public_id),
        indexer_display_name: Some("Indexer".to_string()),
        seeders: Some(8),
        leechers: Some(2),
        published_at: Some(now),
        download_url: Some("https://example.test/download".to_string()),
        magnet_uri: Some("magnet:?xt=urn:btih:abc".to_string()),
        details_url: Some("https://example.test/details".to_string()),
        tracker_name: Some("Tracker".to_string()),
        tracker_category: Some(2000),
        tracker_subcategory: Some(2040),
    })
    .expect("complete row should map into a page item");
    assert_eq!(item.position, 1);
    assert_eq!(item.tracker_name.as_deref(), Some("Tracker"));

    let missing_item = map_search_page_item(&SearchPageFetchRow {
        page_number: 1,
        sealed_at: None,
        item_count: 0,
        item_position: None,
        canonical_torrent_public_id: None,
        title_display: None,
        size_bytes: None,
        infohash_v1: None,
        infohash_v2: None,
        magnet_hash: None,
        canonical_torrent_source_public_id: None,
        indexer_instance_public_id: None,
        indexer_display_name: None,
        seeders: None,
        leechers: None,
        published_at: None,
        download_url: None,
        magnet_uri: None,
        details_url: None,
        tracker_name: None,
        tracker_category: None,
        tracker_subcategory: None,
    });
    assert!(missing_item.is_none());

    let explainability = map_search_request_explainability(&SearchRequestExplainabilityRow {
        zero_runnable_indexers: true,
        skipped_canceled_indexers: 1,
        skipped_failed_indexers: 2,
        blocked_results: 3,
        blocked_rule_public_ids: vec![blocked_rule_public_id],
        rate_limited_indexers: 4,
        retrying_indexers: 5,
    });
    assert!(explainability.zero_runnable_indexers);
    assert_eq!(
        explainability.blocked_rule_public_ids,
        vec![blocked_rule_public_id]
    );
}

fn assert_import_job_and_conflict_rows(now: chrono::DateTime<Utc>) {
    let indexer_instance_public_id = Uuid::new_v4();

    let status = map_import_job_status(ImportJobStatusRow {
        status: "pending".to_string(),
        result_total: 4,
        result_imported_ready: 1,
        result_imported_needs_secret: 2,
        result_imported_test_failed: 0,
        result_unmapped_definition: 1,
        result_skipped_duplicate: 0,
    });
    assert_eq!(status.result_total, 4);
    assert_eq!(status.result_imported_needs_secret, 2);

    let result = map_import_job_result(ImportJobResultRow {
        prowlarr_identifier: "prowlarr-demo".to_string(),
        upstream_slug: Some("demo".to_string()),
        indexer_instance_public_id: Some(indexer_instance_public_id),
        status: "imported_ready".to_string(),
        detail: Some("ok".to_string()),
        resolved_is_enabled: Some(true),
        resolved_priority: Some(70),
        missing_secret_fields: 0,
        media_domain_keys: vec!["tv".to_string()],
        tag_keys: vec!["trusted".to_string()],
        created_at: now,
    });
    assert_eq!(result.prowlarr_identifier, "prowlarr-demo");
    assert_eq!(result.media_domain_keys, vec!["tv"]);

    let conflict = map_source_metadata_conflict(SourceMetadataConflictRow {
        conflict_id: 7,
        conflict_type: "hash".to_string(),
        existing_value: "a".repeat(40),
        incoming_value: "b".repeat(40),
        observed_at: now,
        resolved_at: Some(now),
        resolution: Some("kept_existing".to_string()),
        resolution_note: Some("reviewed".to_string()),
    });
    assert_eq!(conflict.conflict_id, 7);
    assert_eq!(conflict.resolution.as_deref(), Some("kept_existing"));
}

#[test]
fn cardigann_definition_import_normalizes_defaults_and_options() {
    let prepared = parse_cardigann_definition_import(
        r#"
id: "  Demo-Slug  "
name: "  Demo Indexer  "
description: "Indexer description"
caps:
  tv-search: true
settings:
  - name: " base_url "
    label: " Base URL "
    type: text
    required: true
    default: " https://indexer.example "
  - name: " api_key "
    type: apikey
    required: true
  - name: " ratio "
    type: float
    default: 1.25
  - name: " retries "
    type: number
    default: 3
  - name: " enabled "
    type: checkbox
    default: true
  - name: " mode "
    type: select
    options:
      - value: standard
        label: Standard
      - value: proxy
        label: " Proxy "
"#,
    )
    .expect("valid cardigann yaml should parse");

    assert_eq!(prepared.upstream_slug, "demo-slug");
    assert_eq!(prepared.display_name, "Demo Indexer");
    assert_eq!(prepared.fields.len(), 6);
    assert_cardigann_prepared_fields(&prepared);
    assert_cardigann_canonical_json(&prepared);
}

#[test]
fn cardigann_definition_import_rejects_invalid_payload_shapes() {
    let empty =
        parse_cardigann_definition_import("   ").expect_err("blank payload should be rejected");
    assert_eq!(empty.kind(), IndexerDefinitionServiceErrorKind::Invalid);
    assert_eq!(empty.code(), Some("cardigann_yaml_payload_missing"));

    let invalid_yaml = parse_cardigann_definition_import("id: [oops")
        .expect_err("invalid yaml should be rejected");
    assert_eq!(invalid_yaml.code(), Some("cardigann_yaml_invalid"));

    let missing_slug = parse_cardigann_definition_import(
        r#"
id: "   "
name: Demo
settings: []
"#,
    )
    .expect_err("blank slug should be rejected");
    assert_eq!(
        missing_slug.code(),
        Some("cardigann_definition_slug_missing")
    );

    let missing_name = parse_cardigann_definition_import(
        r#"
id: demo
name: "   "
settings: []
"#,
    )
    .expect_err("blank display name should be rejected");
    assert_eq!(
        missing_name.code(),
        Some("cardigann_definition_name_missing")
    );

    let missing_field_name = parse_cardigann_definition_import(
        r#"
id: demo
name: Demo
settings:
  - name: "   "
    type: text
"#,
    )
    .expect_err("blank setting name should be rejected");
    assert_eq!(
        missing_field_name.code(),
        Some("cardigann_setting_name_missing")
    );

    let unsupported_type = parse_cardigann_definition_import(
        r"
id: demo
name: Demo
settings:
  - name: api_key
    type: unsupported
",
    )
    .expect_err("unsupported type should be rejected");
    assert_eq!(
        unsupported_type.code(),
        Some("cardigann_setting_type_unsupported")
    );

    let invalid_default = parse_cardigann_definition_import(
        r#"
id: demo
name: Demo
settings:
  - name: enabled
    type: checkbox
    default: "yes"
"#,
    )
    .expect_err("mismatched default type should be rejected");
    assert_eq!(
        invalid_default.code(),
        Some("cardigann_setting_default_invalid")
    );

    let invalid_option = parse_cardigann_definition_import(
        r#"
id: demo
name: Demo
settings:
  - name: mode
    type: select
    options:
      - "   "
"#,
    )
    .expect_err("blank option should be rejected");
    assert_eq!(
        invalid_option.code(),
        Some("cardigann_setting_option_invalid")
    );
}

#[test]
fn row_and_inventory_mappers_preserve_operator_facing_shapes() {
    let now = Utc::now();
    assert_definition_import_and_hook_rows(now);
    assert_profile_policy_and_torznab_inventory();
    assert_routing_instance_and_backup_snapshot();
}

#[test]
fn search_and_import_row_mappers_preserve_optional_values() {
    let now = Utc::now();
    assert_search_page_and_explainability_rows(now);
    assert_import_job_and_conflict_rows(now);
}

#[test]
fn service_error_wrappers_preserve_kind_code_and_sqlstate() {
    let indexer_definition =
        map_indexer_definition_error("definition", &job_failed("actor_not_found"));
    assert_eq!(
        indexer_definition.kind(),
        IndexerDefinitionServiceErrorKind::Unauthorized
    );
    assert_eq!(indexer_definition.code(), Some("actor_not_found"));
    assert_eq!(indexer_definition.sqlstate(), Some("P0001"));

    let tag = map_tag_error("tag", &job_failed("tag_deleted"));
    assert_eq!(tag.kind(), TagServiceErrorKind::Conflict);

    let notification = map_health_notification_error("hook", &job_failed("email_invalid"));
    assert_eq!(
        notification.kind(),
        HealthNotificationServiceErrorKind::Invalid
    );

    let routing = map_routing_policy_error("routing", &job_failed("routing_policy_not_found"));
    assert_eq!(routing.kind(), RoutingPolicyServiceErrorKind::NotFound);

    let rate_limit = map_rate_limit_policy_error("rate-limit", &job_failed("policy_in_use"));
    assert_eq!(rate_limit.kind(), RateLimitPolicyServiceErrorKind::Conflict);

    let search_profile = map_search_profile_error("profile", &job_failed("policy_set_not_found"));
    assert_eq!(
        search_profile.kind(),
        SearchProfileServiceErrorKind::NotFound
    );

    let search_request = map_search_request_error("request", &job_failed("invalid_query"));
    assert_eq!(
        search_request.kind(),
        SearchRequestServiceErrorKind::Invalid
    );

    let import_job = map_import_job_error("import", &job_failed("import_source_mismatch"));
    assert_eq!(import_job.kind(), ImportJobServiceErrorKind::Conflict);

    let conflict = map_source_metadata_conflict_error(&job_failed("conflict_not_found"));
    assert_eq!(
        conflict.kind(),
        SourceMetadataConflictServiceErrorKind::NotFound
    );

    let backup = map_indexer_backup_error("backup", &job_failed("tag_key_already_exists"));
    assert_eq!(backup.kind(), IndexerBackupServiceErrorKind::Conflict);

    let policy = map_policy_error("policy", &job_failed("policy_set_not_found"));
    assert_eq!(policy.kind(), PolicyServiceErrorKind::NotFound);

    let category = map_category_mapping_error("category", &job_failed("media_domain_key_invalid"));
    assert_eq!(category.kind(), CategoryMappingServiceErrorKind::Invalid);

    let torznab_instance = map_torznab_instance_error(
        "torznab-instance",
        &job_failed("display_name_already_exists"),
    );
    assert_eq!(
        torznab_instance.kind(),
        TorznabInstanceServiceErrorKind::Conflict
    );

    let torznab_access = map_torznab_access_error("torznab-access", &job_failed("api_key_invalid"));
    assert_eq!(torznab_access.kind(), TorznabAccessErrorKind::Unauthorized);

    let secret = map_secret_error("secret", &job_failed("secret_not_found"));
    assert_eq!(secret.kind(), SecretServiceErrorKind::NotFound);

    let indexer = map_indexer_instance_error("indexer", &job_failed("definition_not_found"));
    assert_eq!(indexer.kind(), IndexerInstanceServiceErrorKind::NotFound);

    let field = map_indexer_instance_field_error("field", &job_failed("field_not_secret"));
    assert_eq!(field.kind(), IndexerInstanceFieldErrorKind::Conflict);
}

fn assert_definition_tag_and_notification_detail_kinds() {
    for detail in [
        "definition_upstream_slug_missing",
        "definition_display_name_missing",
        "definition_canonical_text_missing",
        "definition_field_name_missing",
        "definition_field_label_missing",
        "definition_option_length_mismatch",
    ] {
        assert_eq!(
            indexer_definition_error_kind(Some(detail)),
            IndexerDefinitionServiceErrorKind::Invalid
        );
    }
    assert_eq!(
        indexer_definition_error_kind(Some("actor_unauthorized")),
        IndexerDefinitionServiceErrorKind::Unauthorized
    );
    assert_eq!(
        indexer_definition_error_kind(Some("unknown")),
        IndexerDefinitionServiceErrorKind::Storage
    );

    for detail in ["tag_not_found", "unknown_key"] {
        assert_eq!(tag_error_kind(Some(detail)), TagServiceErrorKind::NotFound);
    }
    for detail in ["tag_key_already_exists", "tag_deleted"] {
        assert_eq!(tag_error_kind(Some(detail)), TagServiceErrorKind::Conflict);
    }
    for detail in [
        "tag_reference_missing",
        "tag_key_missing",
        "tag_key_empty",
        "tag_key_not_lowercase",
        "tag_key_too_long",
        "display_name_missing",
        "display_name_empty",
        "display_name_too_long",
        "invalid_tag_reference",
    ] {
        assert_eq!(tag_error_kind(Some(detail)), TagServiceErrorKind::Invalid);
    }

    assert_eq!(
        health_notification_error_kind(Some("hook_not_found")),
        HealthNotificationServiceErrorKind::NotFound
    );
    assert_eq!(
        health_notification_error_kind(Some("actor_unauthorized")),
        HealthNotificationServiceErrorKind::Unauthorized
    );
    for detail in [
        "channel_invalid",
        "display_name_missing",
        "status_threshold_missing",
        "webhook_url_missing",
        "webhook_url_invalid",
        "email_missing",
        "email_invalid",
        "hook_missing",
        "channel_payload_mismatch",
    ] {
        assert_eq!(
            health_notification_error_kind(Some(detail)),
            HealthNotificationServiceErrorKind::Invalid
        );
    }
}

fn assert_routing_rate_limit_and_profile_detail_kinds() {
    assert_routing_policy_detail_kinds();
    assert_rate_limit_policy_detail_kinds();
    assert_search_profile_detail_kinds();
}

fn assert_routing_policy_detail_kinds() {
    assert_eq!(
        routing_policy_error_kind(Some("routing_policy_not_found")),
        RoutingPolicyServiceErrorKind::NotFound
    );
    assert_eq!(
        routing_policy_error_kind(Some("secret_not_found")),
        RoutingPolicyServiceErrorKind::NotFound
    );
    assert_eq!(
        routing_policy_error_kind(Some("display_name_already_exists")),
        RoutingPolicyServiceErrorKind::Conflict
    );
    assert_eq!(
        routing_policy_error_kind(Some("routing_policy_deleted")),
        RoutingPolicyServiceErrorKind::Conflict
    );
    for detail in [
        "display_name_missing",
        "display_name_empty",
        "display_name_too_long",
        "mode_missing",
        "unsupported_routing_mode",
        "routing_policy_missing",
        "param_key_missing",
        "param_not_allowed",
        "param_requires_secret",
        "param_value_invalid",
        "param_value_out_of_range",
        "param_value_too_long",
        "secret_missing",
    ] {
        assert_eq!(
            routing_policy_error_kind(Some(detail)),
            RoutingPolicyServiceErrorKind::Invalid
        );
    }
}

fn assert_rate_limit_policy_detail_kinds() {
    for detail in [
        "policy_not_found",
        "indexer_not_found",
        "routing_policy_not_found",
    ] {
        assert_eq!(
            rate_limit_policy_error_kind(Some(detail)),
            RateLimitPolicyServiceErrorKind::NotFound
        );
    }
    for detail in [
        "display_name_already_exists",
        "policy_is_system",
        "policy_in_use",
        "policy_deleted",
        "indexer_deleted",
        "routing_policy_deleted",
    ] {
        assert_eq!(
            rate_limit_policy_error_kind(Some(detail)),
            RateLimitPolicyServiceErrorKind::Conflict
        );
    }
    for detail in [
        "display_name_missing",
        "display_name_empty",
        "display_name_too_long",
        "limit_missing",
        "rpm_out_of_range",
        "burst_out_of_range",
        "concurrent_out_of_range",
        "policy_missing",
        "indexer_missing",
        "routing_policy_missing",
        "scope_missing",
        "scope_id_missing",
        "capacity_invalid",
        "tokens_invalid",
    ] {
        assert_eq!(
            rate_limit_policy_error_kind(Some(detail)),
            RateLimitPolicyServiceErrorKind::Invalid
        );
    }
}

fn assert_search_profile_detail_kinds() {
    for detail in [
        "search_profile_not_found",
        "media_domain_not_found",
        "policy_set_not_found",
        "indexer_not_found",
        "tag_not_found",
        "user_not_found",
    ] {
        assert_eq!(
            search_profile_error_kind(Some(detail)),
            SearchProfileServiceErrorKind::NotFound
        );
    }
    for detail in [
        "search_profile_deleted",
        "policy_set_deleted",
        "indexer_block_conflict",
        "indexer_allow_conflict",
        "tag_block_conflict",
        "tag_allow_conflict",
    ] {
        assert_eq!(
            search_profile_error_kind(Some(detail)),
            SearchProfileServiceErrorKind::Conflict
        );
    }
    for detail in [
        "display_name_missing",
        "display_name_empty",
        "display_name_too_long",
        "search_profile_missing",
        "policy_set_missing",
        "policy_set_invalid_scope",
        "media_domain_key_invalid",
        "indexer_id_invalid",
        "tag_key_invalid",
        "invalid_tag_reference",
        "default_not_in_allowlist",
        "unknown_key",
    ] {
        assert_eq!(
            search_profile_error_kind(Some(detail)),
            SearchProfileServiceErrorKind::Invalid
        );
    }
}

fn assert_search_request_import_and_conflict_detail_kinds() {
    for detail in [
        "search_request_not_found",
        "search_profile_not_found",
        "media_domain_not_found",
        "search_page_not_found",
    ] {
        assert_eq!(
            search_request_error_kind(Some(detail)),
            SearchRequestServiceErrorKind::NotFound
        );
    }
    for detail in [
        "query_text_missing",
        "query_text_too_long",
        "identifier_input_invalid",
        "invalid_identifier_combo",
        "invalid_query",
        "invalid_season_episode_combo",
        "invalid_torznab_mode",
        "invalid_identifier_mismatch",
        "query_type_missing",
        "media_domain_key_invalid",
        "invalid_request_policy_set",
        "invalid_category_filter",
        "search_request_missing",
        "page_number_missing",
        "page_number_invalid",
    ] {
        assert_eq!(
            search_request_error_kind(Some(detail)),
            SearchRequestServiceErrorKind::Invalid
        );
    }

    for detail in [
        "import_job_not_found",
        "search_profile_not_found",
        "torznab_instance_not_found",
        "secret_not_found",
    ] {
        assert_eq!(
            import_job_error_kind(Some(detail)),
            ImportJobServiceErrorKind::NotFound
        );
    }
    for detail in ["import_job_not_startable", "import_source_mismatch"] {
        assert_eq!(
            import_job_error_kind(Some(detail)),
            ImportJobServiceErrorKind::Conflict
        );
    }
    for detail in [
        "import_job_missing",
        "source_missing",
        "prowlarr_url_missing",
        "prowlarr_url_too_long",
        "secret_missing",
        "backup_blob_missing",
        "backup_blob_too_long",
        "config_too_long",
    ] {
        assert_eq!(
            import_job_error_kind(Some(detail)),
            ImportJobServiceErrorKind::Invalid
        );
    }

    assert_eq!(
        map_source_metadata_conflict_error(&job_failed("conflict_not_found")).kind(),
        SourceMetadataConflictServiceErrorKind::NotFound
    );
    for detail in [
        "conflict_already_resolved",
        "conflict_not_resolved",
        "source_guid_conflict",
    ] {
        assert_eq!(
            map_source_metadata_conflict_error(&job_failed(detail)).kind(),
            SourceMetadataConflictServiceErrorKind::Conflict
        );
    }
    for detail in [
        "conflict_missing",
        "resolution_missing",
        "resolution_note_too_long",
        "incoming_value_invalid",
        "limit_invalid",
    ] {
        assert_eq!(
            map_source_metadata_conflict_error(&job_failed(detail)).kind(),
            SourceMetadataConflictServiceErrorKind::Invalid
        );
    }
}

fn assert_backup_policy_and_category_mapping_detail_kinds() {
    assert_indexer_backup_detail_kinds();
    assert_policy_detail_kinds();
    assert_category_mapping_detail_kinds();
}

fn assert_indexer_backup_detail_kinds() {
    for detail in [
        "indexer_definition_not_found",
        "routing_policy_not_found",
        "rate_limit_policy_not_found",
        "secret_not_found",
    ] {
        assert_eq!(
            indexer_backup_error_kind(Some(detail)),
            IndexerBackupServiceErrorKind::NotFound
        );
    }
    for detail in [
        "display_name_already_exists",
        "tag_key_already_exists",
        "duplicate_field_name",
        "routing_policy_deleted",
    ] {
        assert_eq!(
            indexer_backup_error_kind(Some(detail)),
            IndexerBackupServiceErrorKind::Conflict
        );
    }
    for detail in [
        "rate_limit_reference_missing",
        "routing_policy_reference_missing",
    ] {
        assert_eq!(
            indexer_backup_error_kind(Some(detail)),
            IndexerBackupServiceErrorKind::Invalid
        );
    }
}

fn assert_policy_detail_kinds() {
    for detail in ["policy_set_not_found", "policy_rule_not_found"] {
        assert_eq!(
            policy_error_kind(Some(detail)),
            PolicyServiceErrorKind::NotFound
        );
    }
    for detail in [
        "global_policy_set_exists",
        "user_policy_set_exists",
        "policy_set_deleted",
    ] {
        assert_eq!(
            policy_error_kind(Some(detail)),
            PolicyServiceErrorKind::Conflict
        );
    }
    for detail in [
        "display_name_missing",
        "display_name_empty",
        "display_name_too_long",
        "scope_missing",
        "policy_set_missing",
        "policy_set_ids_missing",
        "policy_set_ids_empty",
        "profile_policy_set_requires_link",
        "policy_rule_ids_missing",
        "policy_rule_ids_empty",
        "policy_rule_missing",
        "rationale_too_long",
        "match_value_invalid",
        "value_set_missing",
        "value_set_not_allowed",
        "value_set_too_large",
        "value_set_item_invalid",
        "value_set_duplicate",
        "match_operator_invalid",
        "rule_definition_missing",
        "rule_action_missing",
        "match_field_invalid",
        "action_invalid",
    ] {
        assert_eq!(
            policy_error_kind(Some(detail)),
            PolicyServiceErrorKind::Invalid
        );
    }
}

fn assert_category_mapping_detail_kinds() {
    for detail in [
        "mapping_not_found",
        "indexer_definition_not_found",
        "indexer_instance_not_found",
        "indexer_instance_deleted",
        "media_domain_not_found",
        "torznab_category_not_found",
        "torznab_instance_not_found",
        "torznab_instance_deleted",
        "unknown_key",
    ] {
        assert_eq!(
            category_mapping_error_kind(Some(detail)),
            CategoryMappingServiceErrorKind::NotFound
        );
    }
    for detail in [
        "tracker_category_missing",
        "tracker_category_invalid",
        "tracker_subcategory_invalid",
        "torznab_category_missing",
        "media_domain_missing",
        "media_domain_key_invalid",
        "indexer_slug_invalid",
        "indexer_scope_conflict",
    ] {
        assert_eq!(
            category_mapping_error_kind(Some(detail)),
            CategoryMappingServiceErrorKind::Invalid
        );
    }
}

fn assert_torznab_secret_and_indexer_detail_kinds() {
    assert_torznab_instance_detail_kinds();
    assert_torznab_access_detail_kinds();
    assert_secret_detail_kinds();
    assert_indexer_instance_detail_kinds();
    assert_indexer_instance_field_detail_kinds();
}

fn assert_torznab_instance_detail_kinds() {
    for detail in ["torznab_instance_not_found", "search_profile_not_found"] {
        assert_eq!(
            torznab_instance_error_kind(Some(detail)),
            TorznabInstanceServiceErrorKind::NotFound
        );
    }
    for detail in [
        "display_name_already_exists",
        "search_profile_deleted",
        "torznab_instance_deleted",
    ] {
        assert_eq!(
            torznab_instance_error_kind(Some(detail)),
            TorznabInstanceServiceErrorKind::Conflict
        );
    }
    for detail in [
        "actor_missing",
        "actor_not_found",
        "actor_unauthorized",
        "search_profile_missing",
        "display_name_missing",
        "display_name_empty",
        "display_name_too_long",
        "torznab_instance_missing",
    ] {
        assert_eq!(
            torznab_instance_error_kind(Some(detail)),
            TorznabInstanceServiceErrorKind::Invalid
        );
    }
}

fn assert_torznab_access_detail_kinds() {
    assert_eq!(
        torznab_access_error_kind(Some("api_key_missing")),
        TorznabAccessErrorKind::Unauthorized
    );
    assert_eq!(
        torznab_access_error_kind(Some("api_key_invalid")),
        TorznabAccessErrorKind::Unauthorized
    );
    for detail in [
        "torznab_instance_missing",
        "torznab_instance_not_found",
        "torznab_instance_deleted",
        "torznab_instance_disabled",
        "canonical_source_missing",
        "canonical_source_not_found",
        "source_not_in_profile",
        "canonical_not_found",
    ] {
        assert_eq!(
            torznab_access_error_kind(Some(detail)),
            TorznabAccessErrorKind::NotFound
        );
    }
}

fn assert_secret_detail_kinds() {
    assert_eq!(
        secret_error_kind(Some("actor_unauthorized")),
        SecretServiceErrorKind::Unauthorized
    );
    assert_eq!(
        secret_error_kind(Some("secret_not_found")),
        SecretServiceErrorKind::NotFound
    );
    for detail in [
        "secret_type_missing",
        "secret_value_missing",
        "secret_missing",
    ] {
        assert_eq!(
            secret_error_kind(Some(detail)),
            SecretServiceErrorKind::Invalid
        );
    }
}

fn assert_indexer_instance_detail_kinds() {
    for detail in [
        "indexer_not_found",
        "definition_not_found",
        "routing_policy_not_found",
        "tag_not_found",
        "unknown_key",
    ] {
        assert_eq!(
            indexer_instance_error_kind(Some(detail)),
            IndexerInstanceServiceErrorKind::NotFound
        );
    }
    for detail in [
        "display_name_already_exists",
        "routing_policy_deleted",
        "definition_deprecated",
        "rss_enable_indexer_disabled",
    ] {
        assert_eq!(
            indexer_instance_error_kind(Some(detail)),
            IndexerInstanceServiceErrorKind::Conflict
        );
    }
    for detail in [
        "actor_missing",
        "actor_not_found",
        "actor_unauthorized",
        "indexer_missing",
        "definition_missing",
        "display_name_missing",
        "display_name_empty",
        "display_name_too_long",
        "priority_out_of_range",
        "unsupported_protocol",
        "media_domain_key_invalid",
        "tag_reference_missing",
        "tag_key_invalid",
        "invalid_tag_reference",
        "indexer_deleted",
        "reason_empty",
        "reason_missing",
        "reason_too_long",
        "limit_out_of_range",
        "rss_item_identifier_missing",
        "item_guid_too_long",
        "infohash_v1_invalid",
        "infohash_v2_invalid",
        "magnet_hash_invalid",
    ] {
        assert_eq!(
            indexer_instance_error_kind(Some(detail)),
            IndexerInstanceServiceErrorKind::Invalid
        );
    }
}

fn assert_indexer_instance_field_detail_kinds() {
    for detail in [
        "indexer_not_found",
        "indexer_deleted",
        "field_not_found",
        "secret_not_found",
    ] {
        assert_eq!(
            indexer_instance_field_error_kind(Some(detail)),
            IndexerInstanceFieldErrorKind::NotFound
        );
    }
    for detail in [
        "field_type_mismatch",
        "field_not_secret",
        "field_requires_secret",
    ] {
        assert_eq!(
            indexer_instance_field_error_kind(Some(detail)),
            IndexerInstanceFieldErrorKind::Conflict
        );
    }
    for detail in [
        "value_type_mismatch",
        "value_count_invalid",
        "value_empty",
        "value_too_long",
        "value_too_short",
        "value_too_small",
        "value_too_large",
        "value_regex_mismatch",
        "value_not_allowed",
        "value_required",
        "field_name_missing",
        "field_name_empty",
        "field_name_too_long",
        "indexer_missing",
        "secret_missing",
    ] {
        assert_eq!(
            indexer_instance_field_error_kind(Some(detail)),
            IndexerInstanceFieldErrorKind::Invalid
        );
    }
}

fn assert_backup_mapping_helpers() {
    let tag_backup = map_indexer_backup_tag_error(
        &TagServiceError::new(TagServiceErrorKind::Conflict)
            .with_code("tag_deleted")
            .with_sqlstate("23505"),
    );
    assert_eq!(tag_backup.kind(), IndexerBackupServiceErrorKind::Conflict);
    assert_eq!(tag_backup.code(), Some("tag_deleted"));
    assert_eq!(tag_backup.sqlstate(), Some("23505"));

    let rate_limit_backup = map_indexer_backup_rate_limit_error(
        &RateLimitPolicyServiceError::new(RateLimitPolicyServiceErrorKind::Invalid)
            .with_code("capacity_invalid")
            .with_sqlstate("22023"),
    );
    assert_eq!(
        rate_limit_backup.kind(),
        IndexerBackupServiceErrorKind::Invalid
    );
    assert_eq!(rate_limit_backup.code(), Some("capacity_invalid"));
    assert_eq!(rate_limit_backup.sqlstate(), Some("22023"));

    let routing_backup = map_indexer_backup_routing_error(
        &RoutingPolicyServiceError::new(RoutingPolicyServiceErrorKind::Unauthorized)
            .with_code("actor_unauthorized")
            .with_sqlstate("42501"),
    );
    assert_eq!(
        routing_backup.kind(),
        IndexerBackupServiceErrorKind::Unauthorized
    );

    let indexer_backup = map_indexer_backup_indexer_error(
        &IndexerInstanceServiceError::new(IndexerInstanceServiceErrorKind::NotFound)
            .with_code("definition_not_found")
            .with_sqlstate("P0001"),
    );
    assert_eq!(
        indexer_backup.kind(),
        IndexerBackupServiceErrorKind::NotFound
    );

    let field_backup = map_indexer_backup_field_error(
        &IndexerInstanceFieldError::new(IndexerInstanceFieldErrorKind::Conflict)
            .with_code("field_requires_secret")
            .with_sqlstate("P0001"),
    );
    assert_eq!(field_backup.kind(), IndexerBackupServiceErrorKind::Conflict);
}

fn assert_backup_secret_lookup() {
    let present = BTreeMap::from([(String::from("Burst"), Uuid::new_v4())]);
    assert_eq!(
        lookup_backup_reference(&present, "Burst", "missing").ok(),
        present.get("Burst").copied()
    );
    let missing = lookup_backup_reference(&present, "Missing", "rate_limit_reference_missing")
        .expect_err("missing reference should error");
    assert_eq!(missing.kind(), IndexerBackupServiceErrorKind::Invalid);
    assert_eq!(missing.code(), Some("rate_limit_reference_missing"));

    assert!(is_missing_secret_error(Some("secret_not_found")));
    assert!(!is_missing_secret_error(Some("other")));
    assert!(!is_missing_secret_error(None));
}

#[test]
fn database_detail_classifiers_cover_domain_contracts() {
    assert_definition_tag_and_notification_detail_kinds();
    assert_routing_rate_limit_and_profile_detail_kinds();
}

#[test]
fn remaining_database_detail_classifiers_cover_domain_contracts() {
    assert_search_request_import_and_conflict_detail_kinds();
    assert_backup_policy_and_category_mapping_detail_kinds();
}

#[test]
fn torznab_secret_indexer_and_backup_helpers_cover_contracts() {
    assert_torznab_secret_and_indexer_detail_kinds();
    assert_backup_mapping_helpers();
    assert_backup_secret_lookup();
}
