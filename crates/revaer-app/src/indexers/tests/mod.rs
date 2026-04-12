use super::*;
use revaer_config::{ConfigService, DbSessionConfig};
use revaer_test_support::postgres::start_postgres;
use std::collections::BTreeMap;
use uuid::Uuid;

const SYSTEM_USER_PUBLIC_ID: Uuid = Uuid::nil();

async fn build_service()
-> anyhow::Result<(IndexerService, revaer_test_support::postgres::TestDatabase)> {
    let database = start_postgres()?;
    let config = ConfigService::new_with_session(
        database.connection_string(),
        Some(DbSessionConfig::new("test-key", "test-secret")),
    )
    .await?;

    Ok((
        IndexerService::new(Arc::new(config), Metrics::new()?),
        database,
    ))
}

fn unique_name(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

#[path = "indexers_pure_tests.rs"]
mod pure_tests;

#[path = "indexers_large_tests.rs"]
mod large_tests;

async fn import_operator_definition_for_tests(
    service: &IndexerService,
    upstream_slug: &str,
    display_name: &str,
) -> anyhow::Result<()> {
    let yaml = format!(
        "id: {upstream_slug}\nname: {display_name}\nsettings:\n  - name: base_url\n    label: Base URL\n    type: text\n    required: true\n  - name: api_key\n    label: API key\n    type: apikey\n    required: true\n"
    );

    let _response = service
        .indexer_definition_import_cardigann(SYSTEM_USER_PUBLIC_ID, &yaml, Some(false))
        .await?;
    Ok(())
}

async fn restore_backup_tag_fixture(service: &IndexerService, tag_key: &str) -> anyhow::Result<()> {
    let created_tag_count = service
        .restore_backup_tags(
            SYSTEM_USER_PUBLIC_ID,
            &[IndexerBackupTagItem {
                tag_key: tag_key.to_string(),
                display_name: "Restore Tag".to_string(),
            }],
        )
        .await?;
    assert_eq!(created_tag_count, 1);
    assert!(
        service
            .tag_list(SYSTEM_USER_PUBLIC_ID)
            .await?
            .iter()
            .any(|tag| tag.tag_key == tag_key)
    );
    Ok(())
}

async fn restore_backup_rate_limit_fixture(
    service: &IndexerService,
    rate_limit_name: &str,
) -> anyhow::Result<(Uuid, BTreeMap<String, Uuid>)> {
    let (created_rate_limit_count, rate_limit_id_by_name) = service
        .restore_backup_rate_limits(
            SYSTEM_USER_PUBLIC_ID,
            &[IndexerBackupRateLimitPolicyItem {
                display_name: rate_limit_name.to_string(),
                requests_per_minute: 90,
                burst: 30,
                concurrent_requests: 3,
                is_system: false,
            }],
        )
        .await?;
    assert_eq!(created_rate_limit_count, 1);
    let rate_limit_public_id = rate_limit_id_by_name
        .get(rate_limit_name)
        .copied()
        .ok_or_else(|| anyhow::anyhow!("restored rate-limit policy should be indexed"))?;
    let rate_limit_policy = service
        .rate_limit_policy_list(SYSTEM_USER_PUBLIC_ID)
        .await?
        .into_iter()
        .find(|policy| policy.rate_limit_policy_public_id == rate_limit_public_id)
        .ok_or_else(|| anyhow::anyhow!("restored rate-limit policy should be listed"))?;
    assert_eq!(rate_limit_policy.display_name, rate_limit_name);
    assert_eq!(rate_limit_policy.requests_per_minute, 90);
    assert_eq!(rate_limit_policy.burst, 30);
    assert_eq!(rate_limit_policy.concurrent_requests, 3);
    Ok((rate_limit_public_id, rate_limit_id_by_name))
}

async fn restore_backup_routing_policy_fixture(
    service: &IndexerService,
    routing_name: &str,
    rate_limit_name: &str,
    rate_limit_public_id: Uuid,
    missing_route_secret: Uuid,
    rate_limit_id_by_name: &BTreeMap<String, Uuid>,
) -> anyhow::Result<(Uuid, BTreeMap<String, Uuid>)> {
    let (created_routing_policy_count, routing_policy_id_by_name, unresolved_routing_bindings) =
        service
            .restore_backup_routing_policies(
                SYSTEM_USER_PUBLIC_ID,
                &[IndexerBackupRoutingPolicyItem {
                    display_name: routing_name.to_string(),
                    mode: "http_proxy".to_string(),
                    rate_limit_display_name: Some(rate_limit_name.to_string()),
                    parameters: vec![
                        IndexerBackupRoutingParameterItem {
                            param_key: "proxy_host".to_string(),
                            value_plain: Some("proxy.internal".to_string()),
                            value_int: None,
                            value_bool: None,
                            secret_public_id: None,
                        },
                        IndexerBackupRoutingParameterItem {
                            param_key: "proxy_port".to_string(),
                            value_plain: None,
                            value_int: Some(8443),
                            value_bool: None,
                            secret_public_id: None,
                        },
                        IndexerBackupRoutingParameterItem {
                            param_key: "http_proxy_auth".to_string(),
                            value_plain: None,
                            value_int: None,
                            value_bool: None,
                            secret_public_id: Some(missing_route_secret),
                        },
                    ],
                }],
                rate_limit_id_by_name,
            )
            .await?;
    assert_eq!(created_routing_policy_count, 1);
    assert_eq!(unresolved_routing_bindings.len(), 1);
    assert_eq!(unresolved_routing_bindings[0].entity_type, "routing_policy");
    assert_eq!(
        unresolved_routing_bindings[0].entity_display_name,
        routing_name
    );
    assert_eq!(
        unresolved_routing_bindings[0].binding_key,
        "http_proxy_auth"
    );
    assert_eq!(
        unresolved_routing_bindings[0].secret_public_id,
        missing_route_secret
    );

    let routing_policy_public_id = routing_policy_id_by_name
        .get(routing_name)
        .copied()
        .ok_or_else(|| anyhow::anyhow!("restored routing policy should be indexed"))?;
    let routing_detail = service
        .routing_policy_get(SYSTEM_USER_PUBLIC_ID, routing_policy_public_id)
        .await?;
    assert_eq!(routing_detail.display_name, routing_name);
    assert_eq!(
        routing_detail.rate_limit_policy_public_id,
        Some(rate_limit_public_id)
    );
    assert!(routing_detail.parameters.iter().any(|parameter| {
        parameter.param_key == "proxy_host"
            && parameter.value_plain.as_deref() == Some("proxy.internal")
    }));
    assert!(routing_detail.parameters.iter().any(|parameter| {
        parameter.param_key == "proxy_port" && parameter.value_int == Some(8443)
    }));
    assert!(!routing_detail.parameters.iter().any(|parameter| {
        parameter.param_key == "http_proxy_auth" && parameter.secret_public_id.is_some()
    }));
    Ok((routing_policy_public_id, routing_policy_id_by_name))
}

struct RestoreBackupIndexerInstanceFixture<'a> {
    definition_slug: &'a str,
    instance_name: &'a str,
    tag_key: &'a str,
    rate_limit_name: &'a str,
    rate_limit_public_id: Uuid,
    rate_limit_id_by_name: &'a BTreeMap<String, Uuid>,
    routing_name: &'a str,
    routing_policy_public_id: Uuid,
    routing_policy_id_by_name: &'a BTreeMap<String, Uuid>,
    missing_field_secret: Uuid,
}

async fn restore_backup_indexer_instance_fixture(
    service: &IndexerService,
    fixture: &RestoreBackupIndexerInstanceFixture<'_>,
) -> anyhow::Result<()> {
    let (created_indexer_instance_count, unresolved_field_bindings) = service
        .restore_backup_indexer_instances(
            SYSTEM_USER_PUBLIC_ID,
            &[IndexerBackupIndexerInstanceItem {
                upstream_slug: fixture.definition_slug.to_string(),
                display_name: fixture.instance_name.to_string(),
                instance_status: "enabled".to_string(),
                rss_status: "enabled".to_string(),
                automatic_search_status: "disabled".to_string(),
                interactive_search_status: "enabled".to_string(),
                priority: 55,
                trust_tier_key: Some("public".to_string()),
                routing_policy_display_name: Some(fixture.routing_name.to_string()),
                connect_timeout_ms: 5_000,
                read_timeout_ms: 15_000,
                max_parallel_requests: 4,
                rate_limit_display_name: Some(fixture.rate_limit_name.to_string()),
                rss_subscription_enabled: Some(true),
                rss_interval_seconds: Some(900),
                media_domain_keys: vec!["tv".to_string()],
                tag_keys: vec![fixture.tag_key.to_string()],
                fields: vec![
                    IndexerBackupFieldItem {
                        field_name: "base_url".to_string(),
                        field_type: "text".to_string(),
                        value_plain: Some("https://indexer.example".to_string()),
                        value_int: None,
                        value_decimal: None,
                        value_bool: None,
                        secret_public_id: None,
                    },
                    IndexerBackupFieldItem {
                        field_name: "api_key".to_string(),
                        field_type: "api_key".to_string(),
                        value_plain: None,
                        value_int: None,
                        value_decimal: None,
                        value_bool: None,
                        secret_public_id: Some(fixture.missing_field_secret),
                    },
                ],
            }],
            fixture.rate_limit_id_by_name,
            fixture.routing_policy_id_by_name,
        )
        .await?;
    assert_eq!(created_indexer_instance_count, 1);
    assert_eq!(unresolved_field_bindings.len(), 1);
    assert_eq!(unresolved_field_bindings[0].entity_type, "indexer_instance");
    assert_eq!(
        unresolved_field_bindings[0].entity_display_name,
        fixture.instance_name
    );
    assert_eq!(unresolved_field_bindings[0].binding_key, "api_key");
    assert_eq!(
        unresolved_field_bindings[0].secret_public_id,
        fixture.missing_field_secret
    );

    let instances = service.indexer_instance_list(SYSTEM_USER_PUBLIC_ID).await?;
    let instance = instances
        .iter()
        .find(|item| item.display_name == fixture.instance_name)
        .ok_or_else(|| anyhow::anyhow!("restored indexer instance should be listed"))?;
    assert_eq!(instance.upstream_slug, fixture.definition_slug);
    assert_eq!(instance.instance_status, "enabled");
    assert_eq!(instance.rss_status, "enabled");
    assert_eq!(instance.automatic_search_status, "disabled");
    assert_eq!(instance.interactive_search_status, "enabled");
    assert_eq!(instance.priority, 55);
    assert_eq!(instance.trust_tier_key.as_deref(), Some("public"));
    assert_eq!(
        instance.routing_policy_public_id,
        Some(fixture.routing_policy_public_id)
    );
    assert_eq!(
        instance.rate_limit_policy_public_id,
        Some(fixture.rate_limit_public_id)
    );
    assert_eq!(instance.media_domain_keys, vec!["tv".to_string()]);
    assert_eq!(instance.tag_keys, vec![fixture.tag_key.to_string()]);
    assert!(instance.fields.iter().any(|field| {
        field.field_name == "base_url"
            && field.value_plain.as_deref() == Some("https://indexer.example")
    }));
    assert!(
        !instance
            .fields
            .iter()
            .any(|field| field.field_name == "api_key" && field.secret_public_id.is_some())
    );
    Ok(())
}

#[tokio::test]
async fn run_operation_records_success_and_error_metrics() -> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let success = service
        .run_operation(
            "indexer.test.success",
            async { Ok::<u32, &'static str>(7) },
            |_| "unexpected".to_string(),
        )
        .await;
    assert_eq!(success, Ok(7));

    let failure = service
        .run_operation(
            "indexer.test.error",
            async { Err::<(), &'static str>("boom") },
            |error| format!("mapped:{error}"),
        )
        .await;
    assert_eq!(failure, Err("mapped:boom".to_string()));

    let rendered = service.telemetry.render()?;
    assert!(rendered.contains("indexer_operations_total"));
    assert!(rendered.contains("operation=\"indexer.test.success\",outcome=\"success\""));
    assert!(rendered.contains("operation=\"indexer.test.error\",outcome=\"error\""));
    assert!(rendered.contains("indexer_operation_latency_ms"));
    Ok(())
}

#[tokio::test]
async fn data_operation_helpers_map_errors_and_record_metrics() -> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let success = service
        .run_data_operation(
            "indexer.test.data.success",
            "policy_set_create",
            async { Ok::<u32, revaer_data::DataError>(11) },
            map_policy_error,
        )
        .await;
    assert!(matches!(success, Ok(11)));

    let failure = service
        .run_data_operation(
            "indexer.test.data.error",
            "policy_set_create",
            async {
                Err::<(), revaer_data::DataError>(revaer_data::DataError::from(
                    sqlx::Error::RowNotFound,
                ))
            },
            map_policy_error,
        )
        .await
        .expect_err("row-not-found should map to a service error");
    assert_eq!(failure.kind(), PolicyServiceErrorKind::Storage);
    assert_eq!(failure.code(), None);

    let unlabeled = service
        .run_unlabeled_data_operation(
            "indexer.test.data.unlabeled",
            async {
                Err::<(), revaer_data::DataError>(revaer_data::DataError::JobFailed {
                    operation: "test-op",
                    job_key: "test-job",
                    error_code: Some("P0001".to_string()),
                    error_detail: Some("conflict_not_found".to_string()),
                })
            },
            map_source_metadata_conflict_error,
        )
        .await
        .expect_err("job failure should map to a conflict service error");
    assert_eq!(
        unlabeled.kind(),
        SourceMetadataConflictServiceErrorKind::NotFound
    );
    assert_eq!(unlabeled.code(), Some("conflict_not_found"));

    let rendered = service.telemetry.render()?;
    assert!(rendered.contains("operation=\"indexer.test.data.success\",outcome=\"success\""));
    assert!(rendered.contains("operation=\"indexer.test.data.error\",outcome=\"error\""));
    assert!(rendered.contains("operation=\"indexer.test.data.unlabeled\",outcome=\"error\""));
    Ok(())
}

#[tokio::test]
async fn restore_backup_helpers_create_inventory_and_track_missing_secret_bindings()
-> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let definition_slug = unique_name("restore-helper-indexer");
    let definition_name = unique_name("Restore Helper Indexer");
    import_operator_definition_for_tests(&service, &definition_slug, &definition_name).await?;

    let tag_key = unique_name("restore-tag");
    let rate_limit_name = unique_name("restore-rate-limit");
    let routing_name = unique_name("restore-route");
    let instance_name = unique_name("restore-instance");
    let missing_route_secret = Uuid::new_v4();
    let missing_field_secret = Uuid::new_v4();

    restore_backup_tag_fixture(&service, &tag_key).await?;
    let (rate_limit_public_id, rate_limit_id_by_name) =
        restore_backup_rate_limit_fixture(&service, &rate_limit_name).await?;
    let (routing_policy_public_id, routing_policy_id_by_name) =
        restore_backup_routing_policy_fixture(
            &service,
            &routing_name,
            &rate_limit_name,
            rate_limit_public_id,
            missing_route_secret,
            &rate_limit_id_by_name,
        )
        .await?;
    restore_backup_indexer_instance_fixture(
        &service,
        &RestoreBackupIndexerInstanceFixture {
            definition_slug: &definition_slug,
            instance_name: &instance_name,
            tag_key: &tag_key,
            rate_limit_name: &rate_limit_name,
            rate_limit_public_id,
            rate_limit_id_by_name: &rate_limit_id_by_name,
            routing_name: &routing_name,
            routing_policy_public_id,
            routing_policy_id_by_name: &routing_policy_id_by_name,
            missing_field_secret,
        },
    )
    .await?;
    Ok(())
}

#[test]
fn parse_cardigann_definition_import_normalizes_settings() {
    let yaml = r"
id: Example-Tracker
name: Example Tracker
caps:
  search: [q]
settings:
  - name: ApiKey
    label: API key
    type: apikey
    required: true
  - name: sort
    type: select
    advanced: true
    default: seeders
    options:
      - value: seeders
        label: Seeders
      - date
";

    let prepared = parse_cardigann_definition_import(yaml).expect("cardigann import should parse");
    assert_eq!(prepared.upstream_slug, "example-tracker");
    assert_eq!(prepared.display_name, "Example Tracker");
    assert_eq!(prepared.fields.len(), 2);
    assert_eq!(prepared.fields[0].field_type, "api_key");
    assert_eq!(prepared.fields[1].field_type, "select_single");
    assert_eq!(prepared.fields[1].option_values, vec!["seeders", "date"]);
    assert!(
        prepared
            .canonical_definition_text
            .contains("example-tracker")
    );
}

#[test]
fn parse_cardigann_definition_import_rejects_unknown_setting_type() {
    let yaml = r"
id: example
name: Example
settings:
  - name: mode
    type: unsupported
";

    let err = parse_cardigann_definition_import(yaml).expect_err("expected invalid type");
    assert_eq!(err.kind(), IndexerDefinitionServiceErrorKind::Invalid);
    assert_eq!(err.code(), Some("cardigann_setting_type_unsupported"));
}

#[test]
fn parse_cardigann_definition_import_rejects_blank_payloads_and_invalid_shapes() {
    let err =
        parse_cardigann_definition_import(" \n\t ").expect_err("blank payload should be rejected");
    assert_eq!(err.kind(), IndexerDefinitionServiceErrorKind::Invalid);
    assert_eq!(err.code(), Some("cardigann_yaml_payload_missing"));

    let err = parse_cardigann_definition_import(
        r#"
id: "   "
name: Example
settings: []
"#,
    )
    .expect_err("blank slug should be rejected");
    assert_eq!(err.code(), Some("cardigann_definition_slug_missing"));

    let err = parse_cardigann_definition_import(
        r#"
id: example
name: "   "
settings: []
"#,
    )
    .expect_err("blank display name should be rejected");
    assert_eq!(err.code(), Some("cardigann_definition_name_missing"));

    let err = parse_cardigann_definition_import(
        r#"
id: example
name: Example
settings:
  - name: api_key
    type: select
    options:
      - value: "   "
        label: API key
"#,
    )
    .expect_err("blank option values should be rejected");
    assert_eq!(err.code(), Some("cardigann_setting_option_invalid"));

    let err = parse_cardigann_definition_import(
        r"
id: example
name: Example
settings:
  - name: query
    type: text
    default:
      - invalid
",
    )
    .expect_err("non-scalar defaults should be rejected");
    assert_eq!(err.code(), Some("cardigann_setting_default_invalid"));
}

#[test]
fn cardigann_helpers_cover_defaults_labels_and_validation() {
    assert_cardigann_normalization_helpers();
    assert_cardigann_prepare_field_helpers();
    assert_cardigann_default_population_helpers();
    assert_cardigann_validation_helpers();
}

fn assert_cardigann_normalization_helpers() {
    assert_eq!(
        normalize_cardigann_slug("  Example-Slug ").expect("slug should normalize"),
        "example-slug"
    );
    assert_eq!(
        normalize_cardigann_display_name("  Example Tracker ")
            .expect("display name should normalize"),
        "Example Tracker"
    );
    assert_eq!(
        normalize_cardigann_field_name(" ApiKey ").expect("field name should normalize"),
        "apikey"
    );
    assert_eq!(
        map_cardigann_setting_type("apikey").expect("setting alias should map"),
        "api_key"
    );
    assert_eq!(
        map_cardigann_setting_type("decimal").expect("decimal alias should map"),
        "number_decimal"
    );
    assert_eq!(
        cardigann_default_string(Some(&YamlValue::String("  seeded ".into()))),
        Some("seeded".to_string())
    );
    assert_eq!(
        cardigann_option_value(&CardigannSettingOptionDocument::Simple(" one ".into()))
            .expect("simple option value should normalize"),
        "one"
    );
    assert_eq!(
        cardigann_option_label(&CardigannSettingOptionDocument::Labeled {
            value: "one".into(),
            label: None,
            name: Some("  First ".into()),
        })
        .expect("labeled option should fall back to name"),
        "First"
    );
}

fn assert_cardigann_prepare_field_helpers() {
    let setting = CardigannSettingDocument {
        name: " ApiKey ".into(),
        label: None,
        setting_type: "apikey".into(),
        required: Some(true),
        advanced: Some(false),
        default: Some(YamlValue::String("  secret ".into())),
        options: Vec::new(),
    };
    let prepared = prepare_cardigann_field(&setting, 0).expect("field should prepare");
    assert_eq!(prepared.field_name, "apikey");
    assert_eq!(prepared.label, "apikey");
    assert_eq!(prepared.field_type, "api_key");
    assert_eq!(prepared.display_order, 1);
    assert_eq!(prepared.default_value_plain.as_deref(), Some("secret"));
}

fn assert_cardigann_default_population_helpers() {
    let mut bool_field = PreparedCardigannFieldImport {
        field_name: "enabled".into(),
        label: "Enabled".into(),
        field_type: "bool".into(),
        is_required: false,
        is_advanced: false,
        display_order: 1,
        default_value_plain: None,
        default_value_int: None,
        default_value_decimal: None,
        default_value_bool: None,
        option_values: Vec::new(),
        option_labels: Vec::new(),
    };
    populate_cardigann_default(&mut bool_field, Some(&YamlValue::Bool(true)))
        .expect("bool default should populate");
    assert_eq!(bool_field.default_value_bool, Some(true));

    let mut int_field = PreparedCardigannFieldImport {
        field_name: "limit".into(),
        label: "Limit".into(),
        field_type: "number_int".into(),
        is_required: false,
        is_advanced: false,
        display_order: 2,
        default_value_plain: None,
        default_value_int: None,
        default_value_decimal: None,
        default_value_bool: None,
        option_values: Vec::new(),
        option_labels: Vec::new(),
    };
    populate_cardigann_default(&mut int_field, Some(&YamlValue::Number(5_i64.into())))
        .expect("int default should populate");
    assert_eq!(int_field.default_value_int, Some(5));

    let mut decimal_field = PreparedCardigannFieldImport {
        field_name: "ratio".into(),
        label: "Ratio".into(),
        field_type: "number_decimal".into(),
        is_required: false,
        is_advanced: false,
        display_order: 3,
        default_value_plain: None,
        default_value_int: None,
        default_value_decimal: None,
        default_value_bool: None,
        option_values: Vec::new(),
        option_labels: Vec::new(),
    };
    populate_cardigann_default(
        &mut decimal_field,
        Some(&YamlValue::Number(serde_yaml::Number::from(3))),
    )
    .expect("decimal default should populate");
    assert_eq!(decimal_field.default_value_decimal.as_deref(), Some("3"));
}

fn assert_cardigann_validation_helpers() {
    let mut bool_field = PreparedCardigannFieldImport {
        field_name: "enabled".into(),
        label: "Enabled".into(),
        field_type: "bool".into(),
        is_required: false,
        is_advanced: false,
        display_order: 1,
        default_value_plain: None,
        default_value_int: None,
        default_value_decimal: None,
        default_value_bool: None,
        option_values: Vec::new(),
        option_labels: Vec::new(),
    };

    let err = normalize_cardigann_slug("   ").expect_err("blank slug should fail");
    assert_eq!(err.kind(), IndexerDefinitionServiceErrorKind::Invalid);
    assert_eq!(err.code(), Some("cardigann_definition_slug_missing"));

    let err = map_cardigann_setting_type("unsupported").expect_err("unsupported type");
    assert_eq!(err.code(), Some("cardigann_setting_type_unsupported"));

    let err = cardigann_option_label(&CardigannSettingOptionDocument::Simple("   ".into()))
        .expect_err("blank option label should fail");
    assert_eq!(err.code(), Some("cardigann_setting_option_invalid"));

    let err =
        populate_cardigann_default(&mut bool_field, Some(&YamlValue::String("not-bool".into())))
            .expect_err("wrong default type should fail");
    assert_eq!(err.code(), Some("cardigann_setting_default_invalid"));
}

#[test]
fn prepared_cardigann_field_to_data_import_preserves_values_and_option_labels() {
    let setting = CardigannSettingDocument {
        name: " ApiKey ".into(),
        label: Some("  API Key ".into()),
        setting_type: "apikey".into(),
        required: Some(true),
        advanced: Some(true),
        default: Some(YamlValue::String("  secret ".into())),
        options: vec![CardigannSettingOptionDocument::Labeled {
            value: " json ".into(),
            label: Some(" JSON ".into()),
            name: None,
        }],
    };
    let prepared = prepare_cardigann_field(&setting, 1).expect("field should prepare");
    let data = prepared.to_data_import();
    assert_eq!(prepared.label, "API Key");
    assert_eq!(prepared.display_order, 2);
    assert_eq!(prepared.default_value_plain.as_deref(), Some("secret"));
    assert_eq!(data.field_name, "apikey");
    assert_eq!(data.label, "API Key");
    assert_eq!(data.field_type, "api_key");
    assert!(data.is_required);
    assert!(data.is_advanced);
    assert_eq!(data.display_order, 2);
    assert_eq!(data.default_value_plain, Some("secret"));
    assert_eq!(data.option_values, vec!["json".to_string()]);
    assert_eq!(data.option_labels, vec!["JSON".to_string()]);

    let fallback_label_setting = CardigannSettingDocument {
        name: "token".into(),
        label: Some("   ".into()),
        setting_type: "token".into(),
        required: None,
        advanced: None,
        default: Some(YamlValue::Bool(true)),
        options: Vec::new(),
    };
    let fallback = prepare_cardigann_field(&fallback_label_setting, 0)
        .expect("blank labels should fall back to the field name");
    assert_eq!(fallback.label, "token");
    assert_eq!(fallback.field_type, "token");
    assert_eq!(
        cardigann_default_string(Some(&YamlValue::Sequence(Vec::new()))),
        None
    );
    assert_eq!(
        cardigann_option_label(&CardigannSettingOptionDocument::Labeled {
            value: " rss ".into(),
            label: None,
            name: None,
        })
        .expect("missing label and name should fall back to the value"),
        "rss"
    );
}

#[test]
fn row_mapping_helpers_copy_inventory_fields() {
    let now = Utc::now();
    assert_definition_and_health_rows(now);
    assert_explainability_and_import_rows(now);
    assert_search_page_item_rows(now);
}

fn assert_definition_and_health_rows(now: chrono::DateTime<Utc>) {
    let definition_row = IndexerDefinitionRow {
        upstream_source: "cardigann".into(),
        upstream_slug: "demo".into(),
        display_name: "Demo".into(),
        protocol: "torznab".into(),
        engine: "cardigann".into(),
        schema_version: 3,
        definition_hash: "abc".into(),
        is_deprecated: false,
        created_at: now,
        updated_at: now,
    };
    let definition = map_indexer_definition_row(definition_row.clone());
    assert_eq!(definition.upstream_slug, "demo");
    assert_eq!(definition.display_name, "Demo");

    let imported = map_imported_indexer_definition_row(ImportedIndexerDefinitionRow {
        upstream_source: definition_row.upstream_source,
        upstream_slug: definition_row.upstream_slug,
        display_name: definition_row.display_name,
        protocol: definition_row.protocol,
        engine: definition_row.engine,
        schema_version: definition_row.schema_version,
        definition_hash: definition_row.definition_hash,
        is_deprecated: definition_row.is_deprecated,
        created_at: definition_row.created_at,
        updated_at: definition_row.updated_at,
        field_count: 2,
        option_count: 3,
    });
    assert_eq!(imported.field_count, 2);
    assert_eq!(imported.option_count, 3);
    assert_eq!(imported.definition.upstream_slug, "demo");

    let hook = map_health_notification_hook_row(IndexerHealthNotificationHookRow {
        indexer_health_notification_hook_public_id: Uuid::new_v4(),
        channel: "webhook".into(),
        display_name: "Ops Pager".into(),
        status_threshold: "failing".into(),
        webhook_url: Some("https://hooks.example.test".into()),
        email: None,
        is_enabled: true,
        updated_at: now,
    });
    assert_eq!(hook.channel, "webhook");
    assert_eq!(hook.display_name, "Ops Pager");
}

fn assert_explainability_and_import_rows(now: chrono::DateTime<Utc>) {
    let explainability = map_search_request_explainability(&SearchRequestExplainabilityRow {
        zero_runnable_indexers: false,
        skipped_canceled_indexers: 1,
        skipped_failed_indexers: 2,
        blocked_results: 3,
        blocked_rule_public_ids: vec![Uuid::new_v4()],
        rate_limited_indexers: 4,
        retrying_indexers: 5,
    });
    assert_eq!(explainability.blocked_results, 3);
    assert_eq!(explainability.retrying_indexers, 5);

    let status = map_import_job_status(ImportJobStatusRow {
        status: "running".into(),
        result_total: 8,
        result_imported_ready: 3,
        result_imported_needs_secret: 1,
        result_imported_test_failed: 1,
        result_unmapped_definition: 2,
        result_skipped_duplicate: 1,
    });
    assert_eq!(status.result_imported_needs_secret, 1);

    let result = map_import_job_result(ImportJobResultRow {
        prowlarr_identifier: "demo".into(),
        upstream_slug: Some("slug".into()),
        indexer_instance_public_id: Some(Uuid::new_v4()),
        status: "imported_ready".into(),
        detail: Some("ready".into()),
        resolved_is_enabled: Some(true),
        resolved_priority: Some(7),
        missing_secret_fields: 0,
        media_domain_keys: vec!["movies".into()],
        tag_keys: vec!["anime".into()],
        created_at: now,
    });
    assert_eq!(result.media_domain_keys, vec!["movies".to_string()]);

    let conflict = map_source_metadata_conflict(SourceMetadataConflictRow {
        conflict_id: 9,
        conflict_type: "title".into(),
        existing_value: "A".into(),
        incoming_value: "B".into(),
        observed_at: now,
        resolved_at: None,
        resolution: None,
        resolution_note: None,
    });
    assert_eq!(conflict.conflict_id, 9);
}

fn assert_search_page_item_rows(now: chrono::DateTime<Utc>) {
    let page_item = map_search_page_item(&SearchPageFetchRow {
        page_number: 1,
        sealed_at: Some(now),
        item_count: 1,
        item_position: Some(2),
        canonical_torrent_public_id: Some(Uuid::new_v4()),
        title_display: Some("Demo Torrent".into()),
        size_bytes: Some(1024),
        infohash_v1: Some("a".repeat(40)),
        infohash_v2: None,
        magnet_hash: None,
        canonical_torrent_source_public_id: Some(Uuid::new_v4()),
        indexer_instance_public_id: Some(Uuid::new_v4()),
        indexer_display_name: Some("Indexer".into()),
        seeders: Some(7),
        leechers: Some(2),
        published_at: Some(now),
        download_url: Some("https://example.test/download".into()),
        magnet_uri: Some("magnet:?xt=urn:btih:demo".into()),
        details_url: Some("https://example.test/details".into()),
        tracker_name: Some("Tracker".into()),
        tracker_category: Some(2000),
        tracker_subcategory: Some(10),
    })
    .expect("complete page row should map");
    assert_eq!(page_item.position, 2);
    assert_eq!(page_item.indexer_display_name.as_deref(), Some("Indexer"));

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
}

#[test]
fn inventory_builders_dedupe_and_collect_secret_refs() {
    let routing_policy_public_id = Uuid::new_v4();
    let rate_limit_policy_public_id = Uuid::new_v4();
    let secret_public_id = Uuid::new_v4();
    let routing_rows = sample_inventory_builder_routing_rows(
        routing_policy_public_id,
        rate_limit_policy_public_id,
        secret_public_id,
    );
    assert_inventory_builder_routing_outputs(
        &routing_rows,
        secret_public_id,
        rate_limit_policy_public_id,
    );

    let instance_rows = sample_inventory_builder_instance_rows(
        routing_policy_public_id,
        rate_limit_policy_public_id,
        secret_public_id,
    );
    assert_inventory_builder_instance_outputs(&instance_rows);
    assert_inventory_builder_snapshot_outputs(
        &routing_rows,
        &instance_rows,
        rate_limit_policy_public_id,
    );
}

fn sample_inventory_builder_routing_rows(
    routing_policy_public_id: Uuid,
    rate_limit_policy_public_id: Uuid,
    secret_public_id: Uuid,
) -> Vec<BackupRoutingPolicyRow> {
    vec![
        BackupRoutingPolicyRow {
            routing_policy_public_id,
            display_name: "Beta".into(),
            mode: "http_proxy".into(),
            rate_limit_policy_public_id: Some(rate_limit_policy_public_id),
            rate_limit_display_name: Some("Global".into()),
            param_key: Some("proxy_url".into()),
            value_plain: Some("https://proxy.example".into()),
            value_int: None,
            value_bool: None,
            secret_public_id: None,
            secret_type: None,
        },
        BackupRoutingPolicyRow {
            routing_policy_public_id,
            display_name: "Beta".into(),
            mode: "http_proxy".into(),
            rate_limit_policy_public_id: None,
            rate_limit_display_name: None,
            param_key: Some("proxy_password".into()),
            value_plain: None,
            value_int: None,
            value_bool: None,
            secret_public_id: Some(secret_public_id),
            secret_type: Some("password".into()),
        },
        BackupRoutingPolicyRow {
            routing_policy_public_id: Uuid::new_v4(),
            display_name: "Alpha".into(),
            mode: "direct".into(),
            rate_limit_policy_public_id: None,
            rate_limit_display_name: None,
            param_key: None,
            value_plain: None,
            value_int: None,
            value_bool: None,
            secret_public_id: None,
            secret_type: None,
        },
    ]
}

fn assert_inventory_builder_routing_outputs(
    routing_rows: &[BackupRoutingPolicyRow],
    secret_public_id: Uuid,
    rate_limit_policy_public_id: Uuid,
) {
    let mut secret_refs = BTreeMap::new();
    let backup_routing = build_backup_routing_policies(routing_rows, &mut secret_refs);
    assert_eq!(secret_refs.len(), 1);
    assert_eq!(
        secret_refs
            .get(&secret_public_id)
            .map(|item| item.secret_type.as_str()),
        Some("password")
    );
    assert_eq!(backup_routing.len(), 2);
    assert_eq!(backup_routing[0].display_name, "Alpha");
    assert_eq!(backup_routing[1].parameters.len(), 2);

    let routing_inventory = build_routing_policy_inventory(routing_rows);
    assert_eq!(routing_inventory[0].display_name, "Alpha");
    assert_eq!(routing_inventory[1].parameter_count, 2);
    assert_eq!(routing_inventory[1].secret_binding_count, 1);
    assert_eq!(
        routing_inventory[1].rate_limit_policy_public_id,
        Some(rate_limit_policy_public_id)
    );
}

fn sample_inventory_builder_instance_rows(
    routing_policy_public_id: Uuid,
    rate_limit_policy_public_id: Uuid,
    secret_public_id: Uuid,
) -> Vec<BackupIndexerInstanceRow> {
    let indexer_instance_public_id = Uuid::new_v4();
    vec![
        BackupIndexerInstanceRow {
            indexer_instance_public_id,
            upstream_slug: "demo".into(),
            display_name: "Zulu".into(),
            instance_status: "enabled".into(),
            rss_status: "enabled".into(),
            automatic_search_status: "enabled".into(),
            interactive_search_status: "enabled".into(),
            priority: 5,
            trust_tier_key: Some("trusted".into()),
            routing_policy_public_id: Some(routing_policy_public_id),
            routing_policy_display_name: Some("Beta".into()),
            connect_timeout_ms: 1_000,
            read_timeout_ms: 2_000,
            max_parallel_requests: 3,
            rate_limit_policy_public_id: Some(rate_limit_policy_public_id),
            rate_limit_display_name: Some("Global".into()),
            rss_subscription_enabled: Some(true),
            rss_interval_seconds: Some(900),
            media_domain_key: Some("movies".into()),
            tag_key: Some("anime".into()),
            field_name: Some("api_key".into()),
            field_type: Some("api_key".into()),
            value_plain: None,
            value_int: None,
            value_decimal: None,
            value_bool: None,
            secret_public_id: Some(secret_public_id),
            secret_type: Some("api_key".into()),
        },
        BackupIndexerInstanceRow {
            indexer_instance_public_id,
            upstream_slug: "demo".into(),
            display_name: "Zulu".into(),
            instance_status: "enabled".into(),
            rss_status: "enabled".into(),
            automatic_search_status: "enabled".into(),
            interactive_search_status: "enabled".into(),
            priority: 5,
            trust_tier_key: Some("trusted".into()),
            routing_policy_public_id: None,
            routing_policy_display_name: None,
            connect_timeout_ms: 1_000,
            read_timeout_ms: 2_000,
            max_parallel_requests: 3,
            rate_limit_policy_public_id: None,
            rate_limit_display_name: None,
            rss_subscription_enabled: None,
            rss_interval_seconds: None,
            media_domain_key: Some("movies".into()),
            tag_key: Some("anime".into()),
            field_name: Some("sort".into()),
            field_type: Some("select_single".into()),
            value_plain: Some("seeders".into()),
            value_int: None,
            value_decimal: None,
            value_bool: None,
            secret_public_id: None,
            secret_type: None,
        },
    ]
}

fn assert_inventory_builder_instance_outputs(instance_rows: &[BackupIndexerInstanceRow]) {
    let inventory = build_indexer_instance_inventory(instance_rows);
    assert_eq!(inventory.len(), 1);
    assert_eq!(inventory[0].display_name, "Zulu");
    assert_eq!(inventory[0].media_domain_keys, vec!["movies".to_string()]);
    assert_eq!(inventory[0].tag_keys, vec!["anime".to_string()]);
    assert_eq!(inventory[0].fields.len(), 2);

    let mut backup_secret_refs = BTreeMap::new();
    let backup_instances = build_backup_indexer_instances(instance_rows, &mut backup_secret_refs);
    assert_eq!(backup_instances.len(), 1);
    assert_eq!(backup_instances[0].fields.len(), 2);
    assert_eq!(backup_secret_refs.len(), 1);
}

fn assert_inventory_builder_snapshot_outputs(
    routing_rows: &[BackupRoutingPolicyRow],
    instance_rows: &[BackupIndexerInstanceRow],
    rate_limit_policy_public_id: Uuid,
) {
    let snapshot = build_backup_snapshot(
        vec![BackupTagRow {
            tag_public_id: Uuid::new_v4(),
            tag_key: "anime".into(),
            display_name: "Anime".into(),
        }],
        vec![BackupRateLimitPolicyRow {
            rate_limit_policy_public_id,
            display_name: "Global".into(),
            requests_per_minute: 60,
            burst: 10,
            concurrent_requests: 3,
            is_system: false,
        }],
        routing_rows,
        instance_rows,
    );
    assert_eq!(snapshot.tags.len(), 1);
    assert_eq!(snapshot.rate_limit_policies.len(), 1);
    assert_eq!(snapshot.routing_policies.len(), 2);
    assert_eq!(snapshot.indexer_instances.len(), 1);
    assert_eq!(snapshot.secrets.len(), 1);
}

#[test]
fn inventory_row_mappers_cover_search_profiles_policy_sets_and_torznab_instances() {
    let search_profile_public_id = Uuid::new_v4();
    let policy_set_public_id = Uuid::new_v4();
    let torznab_instance_public_id = Uuid::new_v4();
    let allowed_indexer_public_id = Uuid::new_v4();
    let blocked_indexer_public_id = Uuid::new_v4();

    assert_search_profile_inventory_row_mapping(
        search_profile_public_id,
        policy_set_public_id,
        allowed_indexer_public_id,
        blocked_indexer_public_id,
    );
    assert_policy_set_inventory_row_mapping(policy_set_public_id);
    assert_torznab_inventory_row_mapping(torznab_instance_public_id, search_profile_public_id);
}

fn assert_search_profile_inventory_row_mapping(
    search_profile_public_id: Uuid,
    policy_set_public_id: Uuid,
    allowed_indexer_public_id: Uuid,
    blocked_indexer_public_id: Uuid,
) {
    let search_profile = build_search_profile_inventory_item(SearchProfileListRow {
        search_profile_public_id,
        display_name: "Movies".into(),
        is_default: true,
        page_size: Some(100),
        default_media_domain_key: Some("movie".into()),
        media_domain_keys: vec!["movie".into(), "tv".into()],
        policy_set_public_ids: vec![policy_set_public_id],
        policy_set_display_names: vec!["Blocklist".into()],
        allow_indexer_public_ids: vec![allowed_indexer_public_id],
        block_indexer_public_ids: vec![blocked_indexer_public_id],
        allow_tag_keys: vec!["4k".into()],
        block_tag_keys: vec!["cam".into()],
        prefer_tag_keys: vec!["hdr".into()],
    });
    assert_eq!(
        search_profile.search_profile_public_id,
        search_profile_public_id
    );
    assert_eq!(
        search_profile.default_media_domain_key.as_deref(),
        Some("movie")
    );
    assert_eq!(
        search_profile.allow_indexer_public_ids,
        vec![allowed_indexer_public_id]
    );
    assert_eq!(search_profile.prefer_tag_keys, vec!["hdr".to_string()]);
}

fn assert_policy_set_inventory_row_mapping(policy_set_public_id: Uuid) {
    let alpha_policy_set_public_id = Uuid::new_v4();
    let beta_primary_rule_public_id = Uuid::new_v4();
    let beta_secondary_rule_public_id = Uuid::new_v4();
    let beta_user_public_id = Uuid::new_v4();
    let beta_match_value_uuid = Uuid::new_v4();
    let expires_at = Utc::now();
    let policy_sets = build_policy_set_inventory(&[
        PolicySetRuleListRow {
            policy_set_public_id,
            policy_set_display_name: "Beta".into(),
            scope: "global".into(),
            is_enabled: true,
            user_public_id: Some(beta_user_public_id),
            policy_rule_public_id: Some(beta_secondary_rule_public_id),
            rule_type: Some("result".into()),
            match_field: Some("tracker".into()),
            match_operator: Some("eq".into()),
            sort_order: Some(2),
            match_value_text: Some("beta".into()),
            match_value_int: None,
            match_value_uuid: None,
            action: Some("allow".into()),
            severity: Some("info".into()),
            is_case_insensitive: Some(true),
            rationale: Some("keep tracker".into()),
            expires_at: Some(expires_at),
            is_rule_disabled: Some(false),
        },
        PolicySetRuleListRow {
            policy_set_public_id,
            policy_set_display_name: "Beta".into(),
            scope: "global".into(),
            is_enabled: true,
            user_public_id: Some(beta_user_public_id),
            policy_rule_public_id: Some(beta_primary_rule_public_id),
            rule_type: Some("result".into()),
            match_field: Some("source".into()),
            match_operator: Some("eq".into()),
            sort_order: Some(1),
            match_value_text: None,
            match_value_int: Some(5),
            match_value_uuid: Some(beta_match_value_uuid),
            action: Some("block".into()),
            severity: Some("warning".into()),
            is_case_insensitive: Some(false),
            rationale: Some("drop noisy source".into()),
            expires_at: None,
            is_rule_disabled: Some(true),
        },
        PolicySetRuleListRow {
            policy_set_public_id: alpha_policy_set_public_id,
            policy_set_display_name: "Alpha".into(),
            scope: "user".into(),
            is_enabled: false,
            user_public_id: None,
            policy_rule_public_id: None,
            rule_type: None,
            match_field: None,
            match_operator: None,
            sort_order: None,
            match_value_text: None,
            match_value_int: None,
            match_value_uuid: None,
            action: None,
            severity: None,
            is_case_insensitive: None,
            rationale: None,
            expires_at: None,
            is_rule_disabled: None,
        },
    ]);
    assert_eq!(policy_sets.len(), 2);
    assert_eq!(policy_sets[0].display_name, "Alpha");
    assert!(policy_sets[0].rules.is_empty());
    assert_eq!(policy_sets[1].display_name, "Beta");
    assert_eq!(policy_sets[1].rules.len(), 2);
    assert_eq!(policy_sets[1].rules[0].sort_order, 1);
    assert_eq!(
        policy_sets[1].rules[0].policy_rule_public_id,
        beta_primary_rule_public_id
    );
    assert_eq!(
        policy_sets[1].rules[0].match_value_uuid,
        Some(beta_match_value_uuid)
    );
    assert!(policy_sets[1].rules[0].is_disabled);
    assert_eq!(policy_sets[1].rules[1].sort_order, 2);
    assert!(policy_sets[1].rules[1].is_case_insensitive);
}

fn assert_torznab_inventory_row_mapping(
    torznab_instance_public_id: Uuid,
    search_profile_public_id: Uuid,
) {
    let torznab = build_torznab_instance_inventory_item(TorznabInstanceListRow {
        torznab_instance_public_id,
        display_name: "Bridge".into(),
        is_enabled: true,
        search_profile_public_id,
        search_profile_display_name: "Movies".into(),
    });
    assert_eq!(
        torznab.torznab_instance_public_id,
        torznab_instance_public_id
    );
    assert_eq!(torznab.search_profile_public_id, search_profile_public_id);
    assert_eq!(torznab.search_profile_display_name, "Movies");
}

#[test]
fn indexer_instance_inventory_helpers_merge_sparse_rows_without_overwriting_values() {
    let indexer_instance_public_id = Uuid::new_v4();
    let routing_policy_public_id = Uuid::new_v4();
    let rate_limit_policy_public_id = Uuid::new_v4();
    let secret_public_id = Uuid::new_v4();

    let inventory = build_indexer_instance_inventory(&sample_sparse_indexer_instance_rows(
        indexer_instance_public_id,
        routing_policy_public_id,
        rate_limit_policy_public_id,
        secret_public_id,
    ));
    assert_sparse_indexer_instance_inventory(
        &inventory,
        routing_policy_public_id,
        rate_limit_policy_public_id,
        secret_public_id,
    );
}

fn sample_sparse_indexer_instance_rows(
    indexer_instance_public_id: Uuid,
    routing_policy_public_id: Uuid,
    rate_limit_policy_public_id: Uuid,
    secret_public_id: Uuid,
) -> [BackupIndexerInstanceRow; 3] {
    [
        BackupIndexerInstanceRow {
            indexer_instance_public_id,
            upstream_slug: "demo".into(),
            display_name: "Delta".into(),
            instance_status: "enabled".into(),
            rss_status: "enabled".into(),
            automatic_search_status: "enabled".into(),
            interactive_search_status: "enabled".into(),
            priority: 10,
            trust_tier_key: Some("trusted".into()),
            routing_policy_public_id: Some(routing_policy_public_id),
            routing_policy_display_name: Some("Proxy".into()),
            connect_timeout_ms: 1_000,
            read_timeout_ms: 2_000,
            max_parallel_requests: 3,
            rate_limit_policy_public_id: Some(rate_limit_policy_public_id),
            rate_limit_display_name: Some("Burst".into()),
            rss_subscription_enabled: Some(true),
            rss_interval_seconds: Some(900),
            media_domain_key: Some("movie".into()),
            tag_key: Some("hdr".into()),
            field_name: Some("api_key".into()),
            field_type: Some("api_key".into()),
            value_plain: None,
            value_int: None,
            value_decimal: None,
            value_bool: None,
            secret_public_id: Some(secret_public_id),
            secret_type: Some("api_key".into()),
        },
        BackupIndexerInstanceRow {
            indexer_instance_public_id,
            upstream_slug: "demo".into(),
            display_name: "Delta".into(),
            instance_status: "enabled".into(),
            rss_status: "enabled".into(),
            automatic_search_status: "enabled".into(),
            interactive_search_status: "enabled".into(),
            priority: 10,
            trust_tier_key: Some("trusted".into()),
            routing_policy_public_id: None,
            routing_policy_display_name: None,
            connect_timeout_ms: 1_000,
            read_timeout_ms: 2_000,
            max_parallel_requests: 3,
            rate_limit_policy_public_id: None,
            rate_limit_display_name: None,
            rss_subscription_enabled: None,
            rss_interval_seconds: None,
            media_domain_key: Some("movie".into()),
            tag_key: Some("hdr".into()),
            field_name: Some("sort".into()),
            field_type: None,
            value_plain: Some("seeders".into()),
            value_int: None,
            value_decimal: None,
            value_bool: None,
            secret_public_id: None,
            secret_type: None,
        },
        BackupIndexerInstanceRow {
            indexer_instance_public_id,
            upstream_slug: "demo".into(),
            display_name: "Delta".into(),
            instance_status: "enabled".into(),
            rss_status: "enabled".into(),
            automatic_search_status: "enabled".into(),
            interactive_search_status: "enabled".into(),
            priority: 10,
            trust_tier_key: Some("trusted".into()),
            routing_policy_public_id: None,
            routing_policy_display_name: None,
            connect_timeout_ms: 1_000,
            read_timeout_ms: 2_000,
            max_parallel_requests: 3,
            rate_limit_policy_public_id: None,
            rate_limit_display_name: None,
            rss_subscription_enabled: None,
            rss_interval_seconds: None,
            media_domain_key: None,
            tag_key: None,
            field_name: None,
            field_type: None,
            value_plain: None,
            value_int: None,
            value_decimal: None,
            value_bool: None,
            secret_public_id: None,
            secret_type: None,
        },
    ]
}

fn assert_sparse_indexer_instance_inventory(
    inventory: &[IndexerInstanceListItemResponse],
    routing_policy_public_id: Uuid,
    rate_limit_policy_public_id: Uuid,
    secret_public_id: Uuid,
) {
    assert_eq!(inventory.len(), 1);
    let item = &inventory[0];
    assert_eq!(
        item.routing_policy_public_id,
        Some(routing_policy_public_id)
    );
    assert_eq!(item.routing_policy_display_name.as_deref(), Some("Proxy"));
    assert_eq!(
        item.rate_limit_policy_public_id,
        Some(rate_limit_policy_public_id)
    );
    assert_eq!(item.rate_limit_display_name.as_deref(), Some("Burst"));
    assert_eq!(item.rss_subscription_enabled, Some(true));
    assert_eq!(item.rss_interval_seconds, Some(900));
    assert_eq!(item.media_domain_keys, vec!["movie".to_string()]);
    assert_eq!(item.tag_keys, vec!["hdr".to_string()]);
    assert_eq!(item.fields.len(), 2);
    assert!(item.fields.iter().any(|field| {
        field.field_name == "api_key"
            && field.field_type == "api_key"
            && field.secret_public_id == Some(secret_public_id)
    }));
    assert!(item.fields.iter().any(|field| {
        field.field_name == "sort"
            && field.field_type.is_empty()
            && field.value_plain.as_deref() == Some("seeders")
    }));
}

#[test]
fn backup_inventory_helpers_merge_sparse_rows_and_preserve_first_field_values() {
    let routing_secret_public_id = Uuid::new_v4();
    let instance_secret_public_id = Uuid::new_v4();

    assert_sparse_backup_routing_policy_merge(routing_secret_public_id);
    assert_sparse_backup_indexer_instance_merge(instance_secret_public_id);
}

fn assert_sparse_backup_routing_policy_merge(routing_secret_public_id: Uuid) {
    let mut routing_secret_refs = BTreeMap::new();
    let routing_policies = build_backup_routing_policies(
        &[
            BackupRoutingPolicyRow {
                routing_policy_public_id: Uuid::new_v4(),
                display_name: "Gamma".into(),
                mode: "http_proxy".into(),
                rate_limit_policy_public_id: None,
                rate_limit_display_name: None,
                param_key: None,
                value_plain: None,
                value_int: None,
                value_bool: None,
                secret_public_id: None,
                secret_type: None,
            },
            BackupRoutingPolicyRow {
                routing_policy_public_id: Uuid::new_v4(),
                display_name: "Gamma".into(),
                mode: "http_proxy".into(),
                rate_limit_policy_public_id: None,
                rate_limit_display_name: Some("Burst".into()),
                param_key: Some("proxy_password".into()),
                value_plain: None,
                value_int: None,
                value_bool: None,
                secret_public_id: Some(routing_secret_public_id),
                secret_type: Some("password".into()),
            },
        ],
        &mut routing_secret_refs,
    );
    assert_eq!(routing_policies.len(), 1);
    assert_eq!(
        routing_policies[0].rate_limit_display_name.as_deref(),
        Some("Burst")
    );
    assert_eq!(routing_policies[0].parameters.len(), 1);
    assert_eq!(routing_secret_refs.len(), 1);
    assert_eq!(
        routing_secret_refs
            .get(&routing_secret_public_id)
            .map(|item| item.secret_type.as_str()),
        Some("password")
    );
}

fn assert_sparse_backup_indexer_instance_merge(instance_secret_public_id: Uuid) {
    let rows = sample_sparse_backup_indexer_instance_rows(instance_secret_public_id);
    let mut instance_secret_refs = BTreeMap::new();
    let backup_instances = build_backup_indexer_instances(&rows, &mut instance_secret_refs);
    assert_eq!(backup_instances.len(), 1);
    let item = &backup_instances[0];
    assert_sparse_backup_indexer_instance_fields(item);
    assert_sparse_backup_indexer_instance_secret_refs(
        &instance_secret_refs,
        instance_secret_public_id,
    );
}

fn assert_sparse_backup_indexer_instance_fields(item: &IndexerBackupIndexerInstanceItem) {
    assert_eq!(item.display_name, "Echo");
    assert_eq!(item.routing_policy_display_name.as_deref(), Some("Proxy"));
    assert_eq!(item.rate_limit_display_name.as_deref(), Some("Burst"));
    assert_eq!(item.rss_subscription_enabled, Some(true));
    assert_eq!(item.rss_interval_seconds, Some(600));
    assert_eq!(item.media_domain_keys, vec!["movie".to_string()]);
    assert_eq!(item.tag_keys, vec!["hdr".to_string()]);
    assert_eq!(item.fields.len(), 1);
    assert_eq!(item.fields[0].field_name, "sort");
    assert!(item.fields[0].field_type.is_empty());
    assert_eq!(item.fields[0].value_plain.as_deref(), Some("seeders"));
}

fn assert_sparse_backup_indexer_instance_secret_refs(
    instance_secret_refs: &BTreeMap<Uuid, IndexerBackupSecretRef>,
    instance_secret_public_id: Uuid,
) {
    assert_eq!(instance_secret_refs.len(), 1);
    assert_eq!(
        instance_secret_refs
            .get(&instance_secret_public_id)
            .map(|item| item.secret_type.as_str()),
        Some("api_key")
    );
}

fn sample_sparse_backup_indexer_instance_rows(
    instance_secret_public_id: Uuid,
) -> [BackupIndexerInstanceRow; 3] {
    [
        BackupIndexerInstanceRow {
            indexer_instance_public_id: Uuid::new_v4(),
            upstream_slug: "demo".into(),
            display_name: "Echo".into(),
            instance_status: "enabled".into(),
            rss_status: "enabled".into(),
            automatic_search_status: "enabled".into(),
            interactive_search_status: "enabled".into(),
            priority: 3,
            trust_tier_key: Some("trusted".into()),
            routing_policy_public_id: None,
            routing_policy_display_name: None,
            connect_timeout_ms: 1_000,
            read_timeout_ms: 2_000,
            max_parallel_requests: 4,
            rate_limit_policy_public_id: None,
            rate_limit_display_name: None,
            rss_subscription_enabled: None,
            rss_interval_seconds: None,
            media_domain_key: Some("movie".into()),
            tag_key: Some("hdr".into()),
            field_name: Some("sort".into()),
            field_type: None,
            value_plain: Some("seeders".into()),
            value_int: None,
            value_decimal: None,
            value_bool: None,
            secret_public_id: None,
            secret_type: None,
        },
        BackupIndexerInstanceRow {
            indexer_instance_public_id: Uuid::new_v4(),
            upstream_slug: "demo".into(),
            display_name: "Echo".into(),
            instance_status: "enabled".into(),
            rss_status: "enabled".into(),
            automatic_search_status: "enabled".into(),
            interactive_search_status: "enabled".into(),
            priority: 3,
            trust_tier_key: Some("trusted".into()),
            routing_policy_public_id: None,
            routing_policy_display_name: Some("Proxy".into()),
            connect_timeout_ms: 1_000,
            read_timeout_ms: 2_000,
            max_parallel_requests: 4,
            rate_limit_policy_public_id: None,
            rate_limit_display_name: Some("Burst".into()),
            rss_subscription_enabled: Some(true),
            rss_interval_seconds: Some(600),
            media_domain_key: Some("movie".into()),
            tag_key: Some("hdr".into()),
            field_name: Some("sort".into()),
            field_type: Some("select_single".into()),
            value_plain: Some("leechers".into()),
            value_int: None,
            value_decimal: None,
            value_bool: None,
            secret_public_id: Some(instance_secret_public_id),
            secret_type: Some("api_key".into()),
        },
        BackupIndexerInstanceRow {
            indexer_instance_public_id: Uuid::new_v4(),
            upstream_slug: "demo".into(),
            display_name: "Echo".into(),
            instance_status: "enabled".into(),
            rss_status: "enabled".into(),
            automatic_search_status: "enabled".into(),
            interactive_search_status: "enabled".into(),
            priority: 3,
            trust_tier_key: Some("trusted".into()),
            routing_policy_public_id: None,
            routing_policy_display_name: None,
            connect_timeout_ms: 1_000,
            read_timeout_ms: 2_000,
            max_parallel_requests: 4,
            rate_limit_policy_public_id: None,
            rate_limit_display_name: None,
            rss_subscription_enabled: None,
            rss_interval_seconds: None,
            media_domain_key: None,
            tag_key: None,
            field_name: None,
            field_type: None,
            value_plain: None,
            value_int: None,
            value_decimal: None,
            value_bool: None,
            secret_public_id: None,
            secret_type: None,
        },
    ]
}

#[test]
fn map_error_helpers_preserve_context_without_logging_side_effects() {
    let data_error = DataError::JobFailed {
        operation: "indexer_test",
        job_key: "indexer_test",
        error_code: Some("P0001".to_string()),
        error_detail: Some("actor_not_found".to_string()),
    };

    let definition_error = map_indexer_definition_error("definition_get", &data_error);
    assert_eq!(
        definition_error.kind(),
        IndexerDefinitionServiceErrorKind::Unauthorized
    );
    assert_eq!(definition_error.code(), Some("actor_not_found"));
    assert_eq!(definition_error.sqlstate(), Some("P0001"));

    let tag_error = map_tag_error("tag_create", &data_error);
    assert_eq!(tag_error.kind(), TagServiceErrorKind::Unauthorized);
    assert_eq!(tag_error.code(), Some("actor_not_found"));
    assert_eq!(tag_error.sqlstate(), Some("P0001"));

    let health_notification_error = map_health_notification_error("hook_create", &data_error);
    assert_eq!(
        health_notification_error.kind(),
        HealthNotificationServiceErrorKind::Unauthorized
    );
    assert_eq!(health_notification_error.code(), Some("actor_not_found"));
    assert_eq!(health_notification_error.sqlstate(), Some("P0001"));

    let field_error = map_indexer_instance_field_error("field_bind", &data_error);
    assert_eq!(
        field_error.kind(),
        IndexerInstanceFieldErrorKind::Unauthorized
    );
    assert_eq!(field_error.code(), Some("actor_not_found"));
    assert_eq!(field_error.sqlstate(), Some("P0001"));
}

#[test]
fn error_translation_helpers_preserve_codes_and_sqlstate_across_service_boundaries() {
    let sqlstate = "P0001".to_string();
    assert_domain_error_translations(&sqlstate);
    assert_backup_error_translations();
}

fn assert_domain_error_translations(sqlstate: &str) {
    let sqlstate = sqlstate.to_string();
    assert_policy_category_and_torznab_domain_errors(&sqlstate);
    assert_secret_and_indexer_domain_errors(&sqlstate);
}

fn assert_policy_category_and_torznab_domain_errors(sqlstate: &str) {
    let policy_error = map_policy_error(
        "policy_set_get",
        &DataError::JobFailed {
            operation: "policy_set_get",
            job_key: "policy_set_get",
            error_code: Some(sqlstate.to_string()),
            error_detail: Some("actor_not_found".to_string()),
        },
    );
    assert_eq!(policy_error.kind(), PolicyServiceErrorKind::Unauthorized);
    assert_eq!(policy_error.code(), Some("actor_not_found"));
    assert_eq!(policy_error.sqlstate(), Some("P0001"));

    let category_error = map_category_mapping_error(
        "category_mapping_get",
        &DataError::JobFailed {
            operation: "category_mapping_get",
            job_key: "category_mapping_get",
            error_code: Some(sqlstate.to_string()),
            error_detail: Some("unknown_key".to_string()),
        },
    );
    assert_eq!(
        category_error.kind(),
        CategoryMappingServiceErrorKind::NotFound
    );
    assert_eq!(category_error.code(), Some("unknown_key"));
    assert_eq!(category_error.sqlstate(), Some("P0001"));

    let torznab_instance_error = map_torznab_instance_error(
        "torznab_instance_get",
        &DataError::JobFailed {
            operation: "torznab_instance_get",
            job_key: "torznab_instance_get",
            error_code: Some(sqlstate.to_string()),
            error_detail: Some("display_name_already_exists".to_string()),
        },
    );
    assert_eq!(
        torznab_instance_error.kind(),
        TorznabInstanceServiceErrorKind::Conflict
    );
    assert_eq!(
        torznab_instance_error.code(),
        Some("display_name_already_exists")
    );
    assert_eq!(torznab_instance_error.sqlstate(), Some("P0001"));

    let torznab_access_error = map_torznab_access_error(
        "torznab_download_prepare",
        &DataError::JobFailed {
            operation: "torznab_download_prepare",
            job_key: "torznab_download_prepare",
            error_code: Some(sqlstate.to_string()),
            error_detail: Some("api_key_invalid".to_string()),
        },
    );
    assert_eq!(
        torznab_access_error.kind(),
        TorznabAccessErrorKind::Unauthorized
    );
    assert_eq!(torznab_access_error.code(), Some("api_key_invalid"));
    assert_eq!(torznab_access_error.sqlstate(), Some("P0001"));
}

fn assert_secret_and_indexer_domain_errors(sqlstate: &str) {
    let secret_error = map_secret_error(
        "secret_get",
        &DataError::JobFailed {
            operation: "secret_get",
            job_key: "secret_get",
            error_code: Some(sqlstate.to_string()),
            error_detail: Some("secret_not_found".to_string()),
        },
    );
    assert_eq!(secret_error.kind(), SecretServiceErrorKind::NotFound);
    assert_eq!(secret_error.code(), Some("secret_not_found"));
    assert_eq!(secret_error.sqlstate(), Some("P0001"));

    let indexer_error = map_indexer_instance_error(
        "indexer_instance_get",
        &DataError::JobFailed {
            operation: "indexer_instance_get",
            job_key: "indexer_instance_get",
            error_code: Some(sqlstate.to_string()),
            error_detail: Some("indexer_not_found".to_string()),
        },
    );
    assert_eq!(
        indexer_error.kind(),
        IndexerInstanceServiceErrorKind::NotFound
    );
    assert_eq!(indexer_error.code(), Some("indexer_not_found"));
    assert_eq!(indexer_error.sqlstate(), Some("P0001"));

    let indexer_field_error = map_indexer_instance_field_error(
        "indexer_instance_field_get",
        &DataError::JobFailed {
            operation: "indexer_instance_field_get",
            job_key: "indexer_instance_field_get",
            error_code: Some(sqlstate.to_string()),
            error_detail: Some("field_type_mismatch".to_string()),
        },
    );
    assert_eq!(
        indexer_field_error.kind(),
        IndexerInstanceFieldErrorKind::Conflict
    );
    assert_eq!(indexer_field_error.code(), Some("field_type_mismatch"));
    assert_eq!(indexer_field_error.sqlstate(), Some("P0001"));
}

fn assert_backup_error_translations() {
    let backup_tag_error = map_indexer_backup_tag_error(
        &TagServiceError::new(TagServiceErrorKind::Conflict)
            .with_code("tag_exists")
            .with_sqlstate("23505"),
    );
    assert_eq!(
        backup_tag_error.kind(),
        IndexerBackupServiceErrorKind::Conflict
    );
    assert_eq!(backup_tag_error.code(), Some("tag_exists"));
    assert_eq!(backup_tag_error.sqlstate(), Some("23505"));

    let backup_rate_limit_error = map_indexer_backup_rate_limit_error(
        &RateLimitPolicyServiceError::new(RateLimitPolicyServiceErrorKind::Unauthorized)
            .with_code("actor_not_found")
            .with_sqlstate("P0001"),
    );
    assert_eq!(
        backup_rate_limit_error.kind(),
        IndexerBackupServiceErrorKind::Unauthorized
    );
    assert_eq!(backup_rate_limit_error.code(), Some("actor_not_found"));
    assert_eq!(backup_rate_limit_error.sqlstate(), Some("P0001"));

    let backup_routing_error = map_indexer_backup_routing_error(
        &RoutingPolicyServiceError::new(RoutingPolicyServiceErrorKind::NotFound)
            .with_code("routing_policy_not_found")
            .with_sqlstate("P0001"),
    );
    assert_eq!(
        backup_routing_error.kind(),
        IndexerBackupServiceErrorKind::NotFound
    );
    assert_eq!(
        backup_routing_error.code(),
        Some("routing_policy_not_found")
    );
    assert_eq!(backup_routing_error.sqlstate(), Some("P0001"));

    let backup_indexer_error = map_indexer_backup_indexer_error(
        &IndexerInstanceServiceError::new(IndexerInstanceServiceErrorKind::Invalid)
            .with_code("display_name_missing")
            .with_sqlstate("P0001"),
    );
    assert_eq!(
        backup_indexer_error.kind(),
        IndexerBackupServiceErrorKind::Invalid
    );
    assert_eq!(backup_indexer_error.code(), Some("display_name_missing"));
    assert_eq!(backup_indexer_error.sqlstate(), Some("P0001"));

    let backup_field_error = map_indexer_backup_field_error(
        &IndexerInstanceFieldError::new(IndexerInstanceFieldErrorKind::Storage)
            .with_code("field_write_failed")
            .with_sqlstate("XX001"),
    );
    assert_eq!(
        backup_field_error.kind(),
        IndexerBackupServiceErrorKind::Storage
    );
    assert_eq!(backup_field_error.code(), Some("field_write_failed"));
    assert_eq!(backup_field_error.sqlstate(), Some("XX001"));
}

#[test]
fn error_kind_helpers_cover_classification_matrix() {
    assert_definition_notification_and_tag_kind_matrix();
    assert_routing_search_and_import_kind_matrix();
    assert_policy_category_and_torznab_kind_matrix();
    assert_indexer_and_backup_kind_matrix();
}

fn assert_definition_notification_and_tag_kind_matrix() {
    macro_rules! assert_kind {
            ($func:path, $expected:expr, [$($detail:expr),+ $(,)?]) => {
                $(
                    assert_eq!($func(Some($detail)), $expected, "detail={}", $detail);
                )+
            };
        }

    assert_kind!(
        indexer_definition_error_kind,
        IndexerDefinitionServiceErrorKind::Invalid,
        [
            "definition_field_name_missing",
            "definition_option_length_mismatch"
        ]
    );
    assert_kind!(
        tag_error_kind,
        TagServiceErrorKind::Conflict,
        ["tag_key_already_exists", "tag_deleted"]
    );
    assert_kind!(
        health_notification_error_kind,
        HealthNotificationServiceErrorKind::Invalid,
        ["channel_invalid", "channel_payload_mismatch"]
    );
}

fn assert_routing_search_and_import_kind_matrix() {
    macro_rules! assert_kind {
            ($func:path, $expected:expr, [$($detail:expr),+ $(,)?]) => {
                $(
                    assert_eq!($func(Some($detail)), $expected, "detail={}", $detail);
                )+
            };
        }

    assert_kind!(
        routing_policy_error_kind,
        RoutingPolicyServiceErrorKind::Invalid,
        ["param_requires_secret", "param_value_out_of_range"]
    );
    assert_kind!(
        rate_limit_policy_error_kind,
        RateLimitPolicyServiceErrorKind::Invalid,
        ["rpm_out_of_range", "scope_id_missing"]
    );
    assert_kind!(
        search_profile_error_kind,
        SearchProfileServiceErrorKind::Conflict,
        ["indexer_block_conflict", "tag_allow_conflict"]
    );
    assert_kind!(
        search_request_error_kind,
        SearchRequestServiceErrorKind::Invalid,
        ["invalid_identifier_combo", "page_number_invalid"]
    );
    assert_kind!(
        import_job_error_kind,
        ImportJobServiceErrorKind::Invalid,
        ["source_missing", "backup_blob_too_long"]
    );
}

fn assert_policy_category_and_torznab_kind_matrix() {
    macro_rules! assert_kind {
            ($func:path, $expected:expr, [$($detail:expr),+ $(,)?]) => {
                $(
                    assert_eq!($func(Some($detail)), $expected, "detail={}", $detail);
                )+
            };
        }

    assert_kind!(
        policy_error_kind,
        PolicyServiceErrorKind::Invalid,
        ["value_set_duplicate", "action_invalid"]
    );
    assert_kind!(
        category_mapping_error_kind,
        CategoryMappingServiceErrorKind::NotFound,
        ["torznab_instance_not_found", "unknown_key"]
    );
    assert_kind!(
        torznab_instance_error_kind,
        TorznabInstanceServiceErrorKind::Conflict,
        ["display_name_already_exists", "torznab_instance_deleted"]
    );
    assert_kind!(
        torznab_access_error_kind,
        TorznabAccessErrorKind::NotFound,
        ["torznab_instance_disabled", "canonical_not_found"]
    );
}

fn assert_indexer_and_backup_kind_matrix() {
    macro_rules! assert_kind {
            ($func:path, $expected:expr, [$($detail:expr),+ $(,)?]) => {
                $(
                    assert_eq!($func(Some($detail)), $expected, "detail={}", $detail);
                )+
            };
        }

    assert_kind!(
        secret_error_kind,
        SecretServiceErrorKind::Invalid,
        ["secret_type_missing", "secret_missing"]
    );
    assert_kind!(
        indexer_instance_error_kind,
        IndexerInstanceServiceErrorKind::Invalid,
        ["priority_out_of_range", "rss_item_identifier_missing"]
    );
    assert_kind!(
        indexer_instance_field_error_kind,
        IndexerInstanceFieldErrorKind::Conflict,
        ["field_type_mismatch", "field_requires_secret"]
    );
    assert_kind!(
        indexer_backup_error_kind,
        IndexerBackupServiceErrorKind::Invalid,
        [
            "rate_limit_reference_missing",
            "routing_policy_reference_missing"
        ]
    );
    assert_eq!(
        map_source_metadata_conflict_error(&DataError::JobFailed {
            operation: "indexer_test",
            job_key: "indexer_test",
            error_code: Some("P0001".to_string()),
            error_detail: Some("conflict_already_resolved".to_string()),
        })
        .kind(),
        SourceMetadataConflictServiceErrorKind::Conflict
    );
    assert_eq!(
        lookup_backup_reference(&BTreeMap::new(), "missing", "missing_ref")
            .expect_err("missing reference should fail")
            .code(),
        Some("missing_ref")
    );
    assert!(is_missing_secret_error(Some("secret_not_found")));
    assert!(!is_missing_secret_error(Some("different_error")));
}

#[test]
fn helper_mappings_cover_remaining_storage_and_reference_paths() {
    assert_search_page_summary_helper_mapping();
    assert_source_metadata_conflict_helper_mapping();
    assert_backup_storage_and_authorization_mapping();
    assert_backup_reference_and_default_codes();
}

fn assert_search_page_summary_helper_mapping() {
    let summary = map_search_page_summary(&SearchPageSummaryRow {
        page_number: 4,
        sealed_at: None,
        item_count: 27,
    });
    assert_eq!(summary.page_number, 4);
    assert_eq!(summary.item_count, 27);
    assert!(summary.sealed_at.is_none());
}

fn assert_source_metadata_conflict_helper_mapping() {
    let conflict_not_found = map_source_metadata_conflict_error(&DataError::JobFailed {
        operation: "conflict_get",
        job_key: "conflict_get",
        error_code: Some("P0001".to_string()),
        error_detail: Some("conflict_not_found".to_string()),
    });
    assert_eq!(
        conflict_not_found.kind(),
        SourceMetadataConflictServiceErrorKind::NotFound
    );
    assert_eq!(conflict_not_found.code(), Some("conflict_not_found"));
    assert_eq!(conflict_not_found.sqlstate(), Some("P0001"));

    let conflict_invalid = map_source_metadata_conflict_error(&DataError::JobFailed {
        operation: "conflict_resolve",
        job_key: "conflict_resolve",
        error_code: None,
        error_detail: Some("resolution_missing".to_string()),
    });
    assert_eq!(
        conflict_invalid.kind(),
        SourceMetadataConflictServiceErrorKind::Invalid
    );
    assert_eq!(conflict_invalid.code(), Some("resolution_missing"));
    assert!(conflict_invalid.sqlstate().is_none());

    let conflict_storage = map_source_metadata_conflict_error(&DataError::JobFailed {
        operation: "conflict_other",
        job_key: "conflict_other",
        error_code: None,
        error_detail: None,
    });
    assert_eq!(
        conflict_storage.kind(),
        SourceMetadataConflictServiceErrorKind::Storage
    );
    assert!(conflict_storage.code().is_none());
}

fn assert_backup_storage_and_authorization_mapping() {
    let backup_storage = map_indexer_backup_error(
        "backup_restore",
        &DataError::JobFailed {
            operation: "backup_restore",
            job_key: "backup_restore",
            error_code: None,
            error_detail: None,
        },
    );
    assert_eq!(
        backup_storage.kind(),
        IndexerBackupServiceErrorKind::Storage
    );
    assert!(backup_storage.code().is_none());
    assert!(backup_storage.sqlstate().is_none());

    let backup_unauthorized = map_indexer_backup_error(
        "backup_restore",
        &DataError::JobFailed {
            operation: "backup_restore",
            job_key: "backup_restore",
            error_code: Some("P0001".to_string()),
            error_detail: Some("actor_unverified".to_string()),
        },
    );
    assert_eq!(
        backup_unauthorized.kind(),
        IndexerBackupServiceErrorKind::Unauthorized
    );
    assert_eq!(backup_unauthorized.code(), Some("actor_unverified"));
    assert_eq!(backup_unauthorized.sqlstate(), Some("P0001"));
}

fn assert_backup_reference_and_default_codes() {
    let reference_id = Uuid::new_v4();
    let refs = BTreeMap::from([(String::from("Global"), reference_id)]);
    assert_eq!(
        lookup_backup_reference(&refs, "Global", "missing_ref")
            .expect("known references should resolve"),
        reference_id
    );

    let tag_default =
        map_indexer_backup_tag_error(&TagServiceError::new(TagServiceErrorKind::Storage));
    assert_eq!(tag_default.code(), Some("tag_backup_restore_failed"));

    let rate_limit_default = map_indexer_backup_rate_limit_error(
        &RateLimitPolicyServiceError::new(RateLimitPolicyServiceErrorKind::Storage),
    );
    assert_eq!(
        rate_limit_default.code(),
        Some("rate_limit_backup_restore_failed")
    );

    let routing_default = map_indexer_backup_routing_error(&RoutingPolicyServiceError::new(
        RoutingPolicyServiceErrorKind::Storage,
    ));
    assert_eq!(
        routing_default.code(),
        Some("routing_backup_restore_failed")
    );

    let indexer_default = map_indexer_backup_indexer_error(&IndexerInstanceServiceError::new(
        IndexerInstanceServiceErrorKind::Storage,
    ));
    assert_eq!(
        indexer_default.code(),
        Some("indexer_backup_restore_failed")
    );

    let field_default = map_indexer_backup_field_error(&IndexerInstanceFieldError::new(
        IndexerInstanceFieldErrorKind::Storage,
    ));
    assert_eq!(
        field_default.code(),
        Some("indexer_field_backup_restore_failed")
    );
}

#[test]
fn helper_error_kind_matrices_cover_remaining_outcomes() {
    assert_primary_service_error_kind_outcomes();
    assert_routing_search_and_import_kind_outcomes();
    assert_policy_category_and_torznab_kind_outcomes();
    assert_secret_and_indexer_kind_outcomes();
}

fn assert_primary_service_error_kind_outcomes() {
    assert_eq!(
        indexer_definition_error_kind(Some("actor_missing")),
        IndexerDefinitionServiceErrorKind::Unauthorized
    );
    assert_eq!(
        indexer_definition_error_kind(Some("unknown")),
        IndexerDefinitionServiceErrorKind::Storage
    );

    assert_eq!(
        tag_error_kind(Some("tag_not_found")),
        TagServiceErrorKind::NotFound
    );
    assert_eq!(
        tag_error_kind(Some("actor_missing")),
        TagServiceErrorKind::Unauthorized
    );
    assert_eq!(tag_error_kind(None), TagServiceErrorKind::Storage);

    assert_eq!(
        health_notification_error_kind(Some("hook_not_found")),
        HealthNotificationServiceErrorKind::NotFound
    );
    assert_eq!(
        health_notification_error_kind(Some("actor_missing")),
        HealthNotificationServiceErrorKind::Unauthorized
    );
    assert_eq!(
        health_notification_error_kind(None),
        HealthNotificationServiceErrorKind::Storage
    );
}

fn assert_routing_search_and_import_kind_outcomes() {
    assert_eq!(
        routing_policy_error_kind(Some("routing_policy_not_found")),
        RoutingPolicyServiceErrorKind::NotFound
    );
    assert_eq!(
        routing_policy_error_kind(Some("display_name_already_exists")),
        RoutingPolicyServiceErrorKind::Conflict
    );
    assert_eq!(
        routing_policy_error_kind(Some("actor_missing")),
        RoutingPolicyServiceErrorKind::Unauthorized
    );
    assert_eq!(
        routing_policy_error_kind(None),
        RoutingPolicyServiceErrorKind::Storage
    );

    assert_eq!(
        rate_limit_policy_error_kind(Some("policy_not_found")),
        RateLimitPolicyServiceErrorKind::NotFound
    );
    assert_eq!(
        rate_limit_policy_error_kind(Some("policy_deleted")),
        RateLimitPolicyServiceErrorKind::Conflict
    );
    assert_eq!(
        rate_limit_policy_error_kind(Some("actor_missing")),
        RateLimitPolicyServiceErrorKind::Unauthorized
    );
    assert_eq!(
        rate_limit_policy_error_kind(None),
        RateLimitPolicyServiceErrorKind::Storage
    );

    assert_eq!(
        search_profile_error_kind(Some("search_profile_not_found")),
        SearchProfileServiceErrorKind::NotFound
    );
    assert_eq!(
        search_profile_error_kind(Some("actor_missing")),
        SearchProfileServiceErrorKind::Unauthorized
    );
    assert_eq!(
        search_profile_error_kind(None),
        SearchProfileServiceErrorKind::Storage
    );

    assert_eq!(
        search_request_error_kind(Some("search_page_not_found")),
        SearchRequestServiceErrorKind::NotFound
    );
    assert_eq!(
        search_request_error_kind(Some("actor_missing")),
        SearchRequestServiceErrorKind::Unauthorized
    );
    assert_eq!(
        search_request_error_kind(None),
        SearchRequestServiceErrorKind::Storage
    );

    assert_eq!(
        import_job_error_kind(Some("import_job_not_found")),
        ImportJobServiceErrorKind::NotFound
    );
    assert_eq!(
        import_job_error_kind(Some("import_source_mismatch")),
        ImportJobServiceErrorKind::Conflict
    );
    assert_eq!(
        import_job_error_kind(Some("actor_missing")),
        ImportJobServiceErrorKind::Unauthorized
    );
    assert_eq!(
        import_job_error_kind(None),
        ImportJobServiceErrorKind::Storage
    );
}

fn assert_policy_category_and_torznab_kind_outcomes() {
    assert_eq!(
        policy_error_kind(Some("policy_set_not_found")),
        PolicyServiceErrorKind::NotFound
    );
    assert_eq!(
        policy_error_kind(Some("global_policy_set_exists")),
        PolicyServiceErrorKind::Conflict
    );
    assert_eq!(
        policy_error_kind(Some("actor_missing")),
        PolicyServiceErrorKind::Unauthorized
    );
    assert_eq!(policy_error_kind(None), PolicyServiceErrorKind::Storage);

    assert_eq!(
        category_mapping_error_kind(Some("mapping_not_found")),
        CategoryMappingServiceErrorKind::NotFound
    );
    assert_eq!(
        category_mapping_error_kind(Some("actor_missing")),
        CategoryMappingServiceErrorKind::Unauthorized
    );
    assert_eq!(
        category_mapping_error_kind(Some("tracker_category_missing")),
        CategoryMappingServiceErrorKind::Invalid
    );
    assert_eq!(
        category_mapping_error_kind(None),
        CategoryMappingServiceErrorKind::Storage
    );

    assert_eq!(
        torznab_instance_error_kind(Some("torznab_instance_not_found")),
        TorznabInstanceServiceErrorKind::NotFound
    );
    assert_eq!(
        torznab_instance_error_kind(Some("display_name_already_exists")),
        TorznabInstanceServiceErrorKind::Conflict
    );
    assert_eq!(
        torznab_instance_error_kind(Some("search_profile_missing")),
        TorznabInstanceServiceErrorKind::Invalid
    );
    assert_eq!(
        torznab_instance_error_kind(None),
        TorznabInstanceServiceErrorKind::Storage
    );

    assert_eq!(
        torznab_access_error_kind(Some("api_key_missing")),
        TorznabAccessErrorKind::Unauthorized
    );
    assert_eq!(
        torznab_access_error_kind(Some("canonical_not_found")),
        TorznabAccessErrorKind::NotFound
    );
    assert_eq!(
        torznab_access_error_kind(None),
        TorznabAccessErrorKind::Storage
    );
}

fn assert_secret_and_indexer_kind_outcomes() {
    assert_eq!(
        secret_error_kind(Some("actor_missing")),
        SecretServiceErrorKind::Unauthorized
    );
    assert_eq!(
        secret_error_kind(Some("secret_not_found")),
        SecretServiceErrorKind::NotFound
    );
    assert_eq!(secret_error_kind(None), SecretServiceErrorKind::Storage);

    assert_eq!(
        indexer_instance_error_kind(Some("indexer_not_found")),
        IndexerInstanceServiceErrorKind::NotFound
    );
    assert_eq!(
        indexer_instance_error_kind(Some("display_name_already_exists")),
        IndexerInstanceServiceErrorKind::Conflict
    );
    assert_eq!(
        indexer_instance_error_kind(Some("actor_missing")),
        IndexerInstanceServiceErrorKind::Invalid
    );
    assert_eq!(
        indexer_instance_error_kind(None),
        IndexerInstanceServiceErrorKind::Storage
    );

    assert_eq!(
        indexer_instance_field_error_kind(Some("indexer_not_found")),
        IndexerInstanceFieldErrorKind::NotFound
    );
    assert_eq!(
        indexer_instance_field_error_kind(Some("field_requires_secret")),
        IndexerInstanceFieldErrorKind::Conflict
    );
    assert_eq!(
        indexer_instance_field_error_kind(Some("actor_missing")),
        IndexerInstanceFieldErrorKind::Unauthorized
    );
    assert_eq!(
        indexer_instance_field_error_kind(None),
        IndexerInstanceFieldErrorKind::Storage
    );
}
