use super::super::*;
use super::{SYSTEM_USER_PUBLIC_ID, build_service, unique_name};
use anyhow::Context;
use chrono::Utc;
use revaer_data::indexers::conflicts::log_source_metadata_conflict;
use revaer_data::indexers::search_requests::{SearchRequestCreateInput, search_request_create};
use revaer_data::indexers::search_results::{SearchResultIngestInput, search_result_ingest};
use sqlx::{query, query_scalar};
use std::collections::BTreeMap;

#[derive(Debug)]
struct IndexerFixture {
    tag_key: String,
    route_secret_public_id: Uuid,
    field_secret_public_id: Uuid,
    rate_limit_policy_public_id: Uuid,
    rate_limit_policy_name: String,
    routing_policy_public_id: Uuid,
    routing_policy_name: String,
    indexer_instance_public_id: Uuid,
    indexer_instance_name: String,
}

#[derive(Debug)]
struct SeedFixtureResources {
    tag_key: String,
    route_secret_public_id: Uuid,
    field_secret_public_id: Uuid,
    rate_limit_policy_public_id: Uuid,
    rate_limit_policy_name: String,
    routing_policy_public_id: Uuid,
    routing_policy_name: String,
    indexer_instance_name: String,
}

async fn import_operator_definition(
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

async fn import_restore_operator_definition(
    service: &IndexerService,
    upstream_slug: &str,
    display_name: &str,
) -> anyhow::Result<()> {
    let yaml = format!(
        "id: {upstream_slug}\nname: {display_name}\nsettings:\n  - name: base_url\n    label: Base URL\n    type: text\n    required: true\n  - name: api_key\n    label: API key\n    type: apikey\n    required: true\n  - name: password\n    label: Password\n    type: password\n  - name: weight\n    label: Weight\n    type: number\n  - name: ratio\n    label: Ratio\n    type: float\n  - name: enabled\n    label: Enabled\n    type: bool\n"
    );

    let _response = service
        .indexer_definition_import_cardigann(SYSTEM_USER_PUBLIC_ID, &yaml, Some(false))
        .await?;
    Ok(())
}

async fn create_seed_fixture_resources(
    service: &IndexerService,
) -> anyhow::Result<SeedFixtureResources> {
    let tag_key = unique_name("backup-tag");
    let routing_policy_name = unique_name("Backup Route");
    let rate_limit_policy_name = unique_name("Backup Limit");
    let indexer_instance_name = unique_name("Backup Instance");

    let _tag_public_id = service
        .tag_create(SYSTEM_USER_PUBLIC_ID, &tag_key, "Backup")
        .await?;
    let route_secret_public_id = service
        .secret_create(SYSTEM_USER_PUBLIC_ID, "password", "proxy-pass")
        .await?;
    let field_secret_public_id = service
        .secret_create(SYSTEM_USER_PUBLIC_ID, "api_key", "indexer-key")
        .await?;
    let rate_limit_policy_public_id = service
        .rate_limit_policy_create(SYSTEM_USER_PUBLIC_ID, &rate_limit_policy_name, 90, 30, 3)
        .await?;
    let routing_policy_public_id = service
        .routing_policy_create(SYSTEM_USER_PUBLIC_ID, &routing_policy_name, "http_proxy")
        .await?;
    service
        .routing_policy_set_rate_limit_policy(
            SYSTEM_USER_PUBLIC_ID,
            routing_policy_public_id,
            Some(rate_limit_policy_public_id),
        )
        .await?;
    service
        .routing_policy_set_param(
            SYSTEM_USER_PUBLIC_ID,
            routing_policy_public_id,
            "proxy_host",
            Some("proxy.internal"),
            None,
            None,
        )
        .await?;
    service
        .routing_policy_bind_secret(
            SYSTEM_USER_PUBLIC_ID,
            routing_policy_public_id,
            "http_proxy_auth",
            route_secret_public_id,
        )
        .await?;

    Ok(SeedFixtureResources {
        tag_key,
        route_secret_public_id,
        field_secret_public_id,
        rate_limit_policy_public_id,
        rate_limit_policy_name,
        routing_policy_public_id,
        routing_policy_name,
        indexer_instance_name,
    })
}

async fn create_seeded_indexer_instance(
    service: &IndexerService,
    definition_slug: &str,
    resources: &SeedFixtureResources,
) -> anyhow::Result<Uuid> {
    let indexer_instance_public_id = service
        .indexer_instance_create(
            SYSTEM_USER_PUBLIC_ID,
            definition_slug,
            &resources.indexer_instance_name,
            Some(55),
            Some("public"),
            Some(resources.routing_policy_public_id),
        )
        .await?;
    let _updated_public_id = service
        .indexer_instance_update(IndexerInstanceUpdateParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id,
            display_name: None,
            priority: Some(55),
            trust_tier_key: Some("public"),
            routing_policy_public_id: Some(resources.routing_policy_public_id),
            is_enabled: Some(true),
            enable_rss: Some(true),
            enable_automatic_search: Some(false),
            enable_interactive_search: Some(true),
        })
        .await?;
    service
        .indexer_instance_set_tags(
            SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id,
            None,
            Some(std::slice::from_ref(&resources.tag_key)),
        )
        .await?;
    service
        .indexer_instance_set_media_domains(
            SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id,
            &[String::from("tv")],
        )
        .await?;
    service
        .indexer_instance_set_rate_limit_policy(
            SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id,
            Some(resources.rate_limit_policy_public_id),
        )
        .await?;
    service
        .indexer_instance_field_set_value(IndexerInstanceFieldValueParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id,
            field_name: "base_url",
            value_plain: Some("https://indexer.example"),
            value_int: None,
            value_decimal: None,
            value_bool: None,
        })
        .await?;
    let _subscription = service
        .indexer_rss_subscription_set(IndexerRssSubscriptionParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id,
            is_enabled: true,
            interval_seconds: Some(1800),
        })
        .await?;
    Ok(indexer_instance_public_id)
}

async fn seed_operator_indexer_fixture(
    service: &IndexerService,
    definition_slug: &str,
) -> anyhow::Result<IndexerFixture> {
    let resources = create_seed_fixture_resources(service).await?;
    let indexer_instance_public_id =
        create_seeded_indexer_instance(service, definition_slug, &resources).await?;

    Ok(IndexerFixture {
        tag_key: resources.tag_key,
        route_secret_public_id: resources.route_secret_public_id,
        field_secret_public_id: resources.field_secret_public_id,
        rate_limit_policy_public_id: resources.rate_limit_policy_public_id,
        rate_limit_policy_name: resources.rate_limit_policy_name,
        routing_policy_public_id: resources.routing_policy_public_id,
        routing_policy_name: resources.routing_policy_name,
        indexer_instance_public_id,
        indexer_instance_name: resources.indexer_instance_name,
    })
}

async fn insert_snapshot_import_result(
    pool: &sqlx::PgPool,
    job_id: Uuid,
    tag_alpha: Uuid,
    tag_beta: Uuid,
) -> anyhow::Result<()> {
    let import_job_id: i64 =
        query_scalar("SELECT import_job_id FROM import_job WHERE import_job_public_id = $1")
            .bind(job_id)
            .fetch_one(pool)
            .await?;
    let upstream_slug = unique_name("import-snapshot");
    query(include_str!("sql/import_snapshot_insert_definition.sql"))
        .bind("prowlarr_indexers")
        .bind(&upstream_slug)
        .bind("Import Snapshot Definition")
        .bind("torrent")
        .bind("torznab")
        .bind(1_i32)
        .bind("e".repeat(64))
        .bind(false)
        .execute(pool)
        .await?;

    let tag_alpha_id: i64 = query_scalar("SELECT tag_id FROM tag WHERE tag_public_id = $1")
        .bind(tag_alpha)
        .fetch_one(pool)
        .await?;
    let tag_beta_id: i64 = query_scalar("SELECT tag_id FROM tag WHERE tag_public_id = $1")
        .bind(tag_beta)
        .fetch_one(pool)
        .await?;
    let import_result_id: i64 = query_scalar(include_str!("sql/import_snapshot_insert_result.sql"))
        .bind(import_job_id)
        .bind(upstream_slug)
        .fetch_one(pool)
        .await?;

    query(include_str!("sql/import_snapshot_insert_media_domains.sql"))
        .bind(import_result_id)
        .execute(pool)
        .await?;

    query(include_str!("sql/import_snapshot_insert_tags.sql"))
        .bind(import_result_id)
        .bind(tag_alpha_id)
        .bind(tag_beta_id)
        .execute(pool)
        .await?;

    Ok(())
}

async fn fetch_policy_sort_orders(
    pool: &sqlx::PgPool,
    first_policy_public_id: Uuid,
    second_policy_public_id: Uuid,
) -> anyhow::Result<(i32, i32)> {
    let first_policy_sort_order: i32 = query_scalar(
        "SELECT sort_order
         FROM policy_set
         WHERE policy_set_public_id = $1",
    )
    .bind(first_policy_public_id)
    .fetch_one(pool)
    .await?;
    let second_policy_sort_order: i32 = query_scalar(
        "SELECT sort_order
         FROM policy_set
         WHERE policy_set_public_id = $1",
    )
    .bind(second_policy_public_id)
    .fetch_one(pool)
    .await?;
    Ok((first_policy_sort_order, second_policy_sort_order))
}

async fn assert_seeded_indexer_instance_inventory(
    service: &IndexerService,
    fixture: &IndexerFixture,
    definition_slug: &str,
) -> anyhow::Result<()> {
    let instances = service.indexer_instance_list(SYSTEM_USER_PUBLIC_ID).await?;
    let instance = instances
        .iter()
        .find(|item| item.indexer_instance_public_id == fixture.indexer_instance_public_id)
        .expect("created instance should be listed");
    assert_eq!(instance.upstream_slug, definition_slug);
    assert_eq!(instance.display_name, fixture.indexer_instance_name);
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
        instance.routing_policy_display_name.as_deref(),
        Some(fixture.routing_policy_name.as_str())
    );
    assert_eq!(
        instance.rate_limit_policy_public_id,
        Some(fixture.rate_limit_policy_public_id)
    );
    assert_eq!(instance.media_domain_keys, vec!["tv".to_string()]);
    assert_eq!(instance.tag_keys, vec![fixture.tag_key.clone()]);
    assert!(instance.fields.iter().any(|field| {
        field.field_name == "base_url"
            && field.value_plain.as_deref() == Some("https://indexer.example")
    }));
    Ok(())
}

async fn assert_fixture_subscription_and_prepare_failure(
    service: &IndexerService,
    fixture: &IndexerFixture,
) -> anyhow::Result<()> {
    let subscription = service
        .indexer_rss_subscription_get(SYSTEM_USER_PUBLIC_ID, fixture.indexer_instance_public_id)
        .await?;
    assert_eq!(subscription.subscription_status, "enabled");
    assert_eq!(subscription.interval_seconds, 1800);

    let prepare = service
        .indexer_instance_test_prepare(SYSTEM_USER_PUBLIC_ID, fixture.indexer_instance_public_id)
        .await?;
    assert!(!prepare.can_execute);
    assert_eq!(prepare.error_class.as_deref(), Some("auth_error"));
    assert_eq!(prepare.error_code.as_deref(), Some("missing_secret"));
    assert_eq!(prepare.detail.as_deref(), Some("api_key"));
    Ok(())
}

async fn bind_fixture_secret_and_assert(
    service: &IndexerService,
    fixture: &IndexerFixture,
) -> anyhow::Result<()> {
    service
        .indexer_instance_field_bind_secret(
            SYSTEM_USER_PUBLIC_ID,
            fixture.indexer_instance_public_id,
            "api_key",
            fixture.field_secret_public_id,
        )
        .await?;

    let instances = service.indexer_instance_list(SYSTEM_USER_PUBLIC_ID).await?;
    let instance = instances
        .iter()
        .find(|item| item.indexer_instance_public_id == fixture.indexer_instance_public_id)
        .expect("updated instance should be listed");
    assert!(instance.fields.iter().any(|field| {
        field.field_name == "api_key"
            && field.secret_public_id == Some(fixture.field_secret_public_id)
    }));
    Ok(())
}

async fn assert_fixture_rss_seen_roundtrip(
    service: &IndexerService,
    fixture: &IndexerFixture,
) -> anyhow::Result<()> {
    let mark = service
        .indexer_rss_seen_mark(IndexerRssSeenMarkParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id: fixture.indexer_instance_public_id,
            item_guid: Some("GUID-123"),
            infohash_v1: None,
            infohash_v2: None,
            magnet_hash: None,
        })
        .await?;
    assert!(mark.inserted);
    assert_eq!(mark.item.item_guid.as_deref(), Some("guid-123"));

    let duplicate = service
        .indexer_rss_seen_mark(IndexerRssSeenMarkParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id: fixture.indexer_instance_public_id,
            item_guid: Some("guid-123"),
            infohash_v1: None,
            infohash_v2: None,
            magnet_hash: None,
        })
        .await?;
    assert!(!duplicate.inserted);
    assert_eq!(duplicate.item.first_seen_at, mark.item.first_seen_at);

    let seen_items = service
        .indexer_rss_seen_list(IndexerRssSeenListParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id: fixture.indexer_instance_public_id,
            limit: Some(10),
        })
        .await?;
    assert_eq!(seen_items.len(), 1);
    assert_eq!(seen_items[0].item_guid.as_deref(), Some("guid-123"));
    Ok(())
}

async fn assert_fixture_prepare_finalize_and_secret_metadata(
    service: &IndexerService,
    fixture: &IndexerFixture,
) -> anyhow::Result<()> {
    let prepare = service
        .indexer_instance_test_prepare(SYSTEM_USER_PUBLIC_ID, fixture.indexer_instance_public_id)
        .await?;
    assert!(prepare.can_execute);
    assert_eq!(
        prepare.routing_policy_public_id,
        Some(fixture.routing_policy_public_id)
    );
    assert_eq!(
        prepare.field_names,
        Some(vec!["api_key".to_string(), "base_url".to_string()])
    );
    assert_eq!(
        prepare.field_types,
        Some(vec!["api_key".to_string(), "string".to_string()])
    );
    assert_eq!(
        prepare.secret_public_ids,
        Some(vec![Some(fixture.field_secret_public_id), None])
    );

    let finalized = service
        .indexer_instance_test_finalize(IndexerInstanceTestFinalizeParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id: fixture.indexer_instance_public_id,
            ok: true,
            error_class: None,
            error_code: None,
            detail: None,
            result_count: Some(3),
        })
        .await
        .unwrap_or_else(|err| {
            panic!(
                "indexer_instance_test_finalize failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    assert!(finalized.ok);
    assert_eq!(finalized.result_count, Some(3));

    let metadata = service
        .secret_metadata_list(SYSTEM_USER_PUBLIC_ID)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "secret_metadata_list failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    let route_secret = metadata
        .iter()
        .find(|item| item.secret_public_id == fixture.route_secret_public_id)
        .expect("routing secret metadata should be listed");
    assert_eq!(route_secret.binding_count, 1);
    let field_secret = metadata
        .iter()
        .find(|item| item.secret_public_id == fixture.field_secret_public_id)
        .expect("field secret metadata should be listed");
    assert_eq!(field_secret.binding_count, 1);
    Ok(())
}

async fn seed_fixture_connectivity_rows(
    service: &IndexerService,
    fixture: &IndexerFixture,
    now: chrono::DateTime<Utc>,
) -> anyhow::Result<()> {
    let instance_id: i64 = query_scalar(
        "SELECT indexer_instance_id
         FROM indexer_instance
         WHERE indexer_instance_public_id = $1",
    )
    .bind(fixture.indexer_instance_public_id)
    .fetch_one(service.config.pool())
    .await?;

    query(include_str!("sql/connectivity_insert_profile.sql"))
        .bind(instance_id)
        .bind(now)
        .execute(service.config.pool())
        .await?;
    query(include_str!("sql/connectivity_insert_reputation.sql"))
        .bind(instance_id)
        .bind(now)
        .execute(service.config.pool())
        .await?;
    query(include_str!("sql/connectivity_insert_health_event.sql"))
        .bind(instance_id)
        .bind(now)
        .execute(service.config.pool())
        .await?;
    Ok(())
}

async fn assert_fixture_connectivity_views(
    service: &IndexerService,
    fixture: &IndexerFixture,
) -> anyhow::Result<()> {
    let cf_state = service
        .indexer_cf_state_get(SYSTEM_USER_PUBLIC_ID, fixture.indexer_instance_public_id)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "indexer_cf_state_get failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    assert_eq!(cf_state.state, "clear");
    service
        .indexer_cf_state_reset(IndexerCfStateResetParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id: fixture.indexer_instance_public_id,
            reason: "manual_reset",
        })
        .await
        .unwrap_or_else(|err| {
            panic!(
                "indexer_cf_state_reset failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    let cf_state_after_reset = service
        .indexer_cf_state_get(SYSTEM_USER_PUBLIC_ID, fixture.indexer_instance_public_id)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "indexer_cf_state_get after reset failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    assert_eq!(cf_state_after_reset.state, "clear");

    let connectivity = service
        .indexer_connectivity_profile_get(SYSTEM_USER_PUBLIC_ID, fixture.indexer_instance_public_id)
        .await?;
    assert!(connectivity.profile_exists);
    assert_eq!(connectivity.status.as_deref(), Some("healthy"));
    assert_eq!(connectivity.latency_p50_ms, Some(110));

    let reputation = service
        .indexer_source_reputation_list(IndexerSourceReputationListParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id: fixture.indexer_instance_public_id,
            window_key: Some("24h"),
            limit: Some(5),
        })
        .await?;
    assert_eq!(reputation.len(), 1);
    assert_eq!(reputation[0].window_key, "24h");
    assert_eq!(reputation[0].request_count, 40);

    let health_events = service
        .indexer_health_event_list(IndexerHealthEventListParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id: fixture.indexer_instance_public_id,
            limit: Some(5),
        })
        .await?;
    assert_eq!(health_events.len(), 1);
    assert_eq!(health_events[0].event_type, "identity_conflict");
    assert_eq!(health_events[0].http_status, Some(429));
    Ok(())
}

#[tokio::test]
async fn indexer_instance_operator_roundtrip_covers_inventory_and_executor_flows()
-> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let definition_slug = unique_name("operator-indexer");
    let definition_name = unique_name("Operator Indexer");
    import_operator_definition(&service, &definition_slug, &definition_name).await?;
    let fixture = seed_operator_indexer_fixture(&service, &definition_slug).await?;
    let now = Utc::now();

    assert_seeded_indexer_instance_inventory(&service, &fixture, &definition_slug).await?;
    assert_fixture_subscription_and_prepare_failure(&service, &fixture).await?;
    bind_fixture_secret_and_assert(&service, &fixture).await?;
    assert_fixture_rss_seen_roundtrip(&service, &fixture).await?;
    assert_fixture_prepare_finalize_and_secret_metadata(&service, &fixture).await?;
    seed_fixture_connectivity_rows(&service, &fixture, now).await?;
    assert_fixture_connectivity_views(&service, &fixture).await?;
    Ok(())
}

#[tokio::test]
async fn search_profiles_policy_sets_and_torznab_roundtrip_cover_operator_workflows()
-> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };
    let fixture = create_search_profile_inventory_fixture(&service).await?;
    assert_search_profile_inventory(&service, &fixture).await?;
    let torznab_fixture = create_torznab_operator_fixture(&service, &fixture).await?;
    assert_torznab_category_mapping_roundtrip(&service, &fixture, &torznab_fixture).await?;
    assert_search_profile_torznab_cleanup(&service, &fixture, &torznab_fixture).await?;
    Ok(())
}

struct SearchProfileInventoryFixture {
    fixture: IndexerFixture,
    blocked_tag_key: String,
    blocked_instance_public_id: Uuid,
    profile_public_id: Uuid,
    updated_profile_name: String,
    policy_set_public_id: Uuid,
    updated_policy_name: String,
    first_rule_public_id: Uuid,
    second_rule_public_id: Uuid,
}

struct PolicySetFixture {
    policy_set_public_id: Uuid,
    updated_policy_name: String,
    first_rule_public_id: Uuid,
    second_rule_public_id: Uuid,
}

struct SearchProfileFixture {
    profile_public_id: Uuid,
    updated_profile_name: String,
}

struct TorznabOperatorFixture {
    torznab_instance_public_id: Uuid,
    torznab_name: String,
}

async fn create_search_profile_inventory_fixture(
    service: &IndexerService,
) -> anyhow::Result<SearchProfileInventoryFixture> {
    let definition_slug = unique_name("search-indexer");
    let definition_name = unique_name("Search Indexer");
    import_operator_definition(service, &definition_slug, &definition_name).await?;
    let fixture = seed_operator_indexer_fixture(service, &definition_slug).await?;
    let (blocked_tag_key, blocked_instance_public_id) =
        create_blocked_profile_inputs(service, &definition_slug).await?;
    let search_profile = create_search_profile_fixture(service).await?;
    let policy_set = create_policy_set_fixture(service).await?;
    assign_search_profile_operator_links(
        service,
        &fixture,
        search_profile.profile_public_id,
        policy_set.policy_set_public_id,
        blocked_instance_public_id,
        &blocked_tag_key,
    )
    .await?;
    Ok(SearchProfileInventoryFixture {
        fixture,
        blocked_tag_key,
        blocked_instance_public_id,
        profile_public_id: search_profile.profile_public_id,
        updated_profile_name: search_profile.updated_profile_name,
        policy_set_public_id: policy_set.policy_set_public_id,
        updated_policy_name: policy_set.updated_policy_name,
        first_rule_public_id: policy_set.first_rule_public_id,
        second_rule_public_id: policy_set.second_rule_public_id,
    })
}

async fn create_blocked_profile_inputs(
    service: &IndexerService,
    definition_slug: &str,
) -> anyhow::Result<(String, Uuid)> {
    let blocked_tag_key = unique_name("blocked-tag");
    let _blocked_tag_public_id = service
        .tag_create(SYSTEM_USER_PUBLIC_ID, &blocked_tag_key, "Blocked")
        .await?;
    let blocked_instance_public_id = service
        .indexer_instance_create(
            SYSTEM_USER_PUBLIC_ID,
            definition_slug,
            &unique_name("Blocked Instance"),
            Some(40),
            Some("public"),
            None,
        )
        .await?;
    Ok((blocked_tag_key, blocked_instance_public_id))
}

async fn create_search_profile_fixture(
    service: &IndexerService,
) -> anyhow::Result<SearchProfileFixture> {
    let profile_name = unique_name("Search Profile");
    let updated_profile_name = unique_name("Search Profile Updated");
    let profile_public_id = service
        .search_profile_create(
            SYSTEM_USER_PUBLIC_ID,
            &profile_name,
            Some(true),
            Some(50),
            Some("tv"),
            None,
        )
        .await?;
    service
        .search_profile_update(
            SYSTEM_USER_PUBLIC_ID,
            profile_public_id,
            Some(&updated_profile_name),
            Some(75),
        )
        .await?;
    service
        .search_profile_set_default(SYSTEM_USER_PUBLIC_ID, profile_public_id, Some(80))
        .await?;
    service
        .search_profile_set_default_domain(SYSTEM_USER_PUBLIC_ID, profile_public_id, Some("movies"))
        .await?;
    service
        .search_profile_set_domain_allowlist(
            SYSTEM_USER_PUBLIC_ID,
            profile_public_id,
            &["movies".to_string(), "tv".to_string()],
        )
        .await?;
    Ok(SearchProfileFixture {
        profile_public_id,
        updated_profile_name,
    })
}

async fn create_policy_set_fixture(service: &IndexerService) -> anyhow::Result<PolicySetFixture> {
    let policy_name = unique_name("Profile Policy");
    let updated_policy_name = unique_name("Profile Policy Updated");
    let policy_set_public_id = service
        .policy_set_create(SYSTEM_USER_PUBLIC_ID, &policy_name, "profile", Some(false))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "policy_set_create failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    service
        .policy_set_update(
            SYSTEM_USER_PUBLIC_ID,
            policy_set_public_id,
            Some(&updated_policy_name),
        )
        .await
        .unwrap_or_else(|err| {
            panic!(
                "policy_set_update failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    let first_rule_public_id =
        create_policy_rule(service, policy_set_public_id, 10, "cam", "low quality").await;
    let second_rule_public_id =
        create_policy_rule(service, policy_set_public_id, 20, "ts", "cam release").await;
    service
        .policy_rule_disable(SYSTEM_USER_PUBLIC_ID, second_rule_public_id)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "policy_rule_disable failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    service
        .policy_rule_enable(SYSTEM_USER_PUBLIC_ID, second_rule_public_id)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "policy_rule_enable failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    service
        .policy_rule_reorder(
            SYSTEM_USER_PUBLIC_ID,
            policy_set_public_id,
            &[second_rule_public_id, first_rule_public_id],
        )
        .await
        .unwrap_or_else(|err| {
            panic!(
                "policy_rule_reorder failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    Ok(PolicySetFixture {
        policy_set_public_id,
        updated_policy_name,
        first_rule_public_id,
        second_rule_public_id,
    })
}

async fn create_policy_rule(
    service: &IndexerService,
    policy_set_public_id: Uuid,
    sort_order: i32,
    match_value_text: &str,
    rationale: &str,
) -> Uuid {
    service
        .policy_rule_create(PolicyRuleCreateParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            policy_set_public_id,
            rule_type: "block_title_regex".to_string(),
            match_field: "title".to_string(),
            match_operator: "regex".to_string(),
            sort_order,
            match_value_text: Some(match_value_text.to_string()),
            match_value_int: None,
            match_value_uuid: None,
            value_set_items: None,
            action: "drop_canonical".to_string(),
            severity: "hard".to_string(),
            is_case_insensitive: Some(true),
            rationale: Some(rationale.to_string()),
            expires_at: None,
        })
        .await
        .unwrap_or_else(|err| {
            panic!(
                "policy_rule_create failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        })
}

async fn assign_search_profile_operator_links(
    service: &IndexerService,
    fixture: &IndexerFixture,
    profile_public_id: Uuid,
    policy_set_public_id: Uuid,
    blocked_instance_public_id: Uuid,
    blocked_tag_key: &str,
) -> anyhow::Result<()> {
    service
        .search_profile_add_policy_set(
            SYSTEM_USER_PUBLIC_ID,
            profile_public_id,
            policy_set_public_id,
        )
        .await?;
    toggle_policy_set_for_search_profile(service, policy_set_public_id).await;
    service
        .search_profile_indexer_allow(
            SYSTEM_USER_PUBLIC_ID,
            profile_public_id,
            &[fixture.indexer_instance_public_id],
        )
        .await?;
    service
        .search_profile_indexer_block(
            SYSTEM_USER_PUBLIC_ID,
            profile_public_id,
            &[blocked_instance_public_id],
        )
        .await?;
    let tags = service.tag_list(SYSTEM_USER_PUBLIC_ID).await?;
    let fixture_tag_public_id = tags
        .iter()
        .find(|tag| tag.tag_key == fixture.tag_key)
        .expect("fixture tag should exist")
        .tag_public_id;
    service
        .search_profile_tag_allow(
            SYSTEM_USER_PUBLIC_ID,
            profile_public_id,
            Some(&[fixture_tag_public_id]),
            None,
        )
        .await?;
    service
        .search_profile_tag_block(
            SYSTEM_USER_PUBLIC_ID,
            profile_public_id,
            None,
            Some(&[blocked_tag_key.to_string()]),
        )
        .await?;
    service
        .search_profile_tag_prefer(
            SYSTEM_USER_PUBLIC_ID,
            profile_public_id,
            None,
            Some(std::slice::from_ref(&fixture.tag_key)),
        )
        .await?;
    Ok(())
}

async fn toggle_policy_set_for_search_profile(
    service: &IndexerService,
    policy_set_public_id: Uuid,
) {
    service
        .policy_set_enable(SYSTEM_USER_PUBLIC_ID, policy_set_public_id)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "policy_set_enable failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    service
        .policy_set_disable(SYSTEM_USER_PUBLIC_ID, policy_set_public_id)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "policy_set_disable failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    service
        .policy_set_enable(SYSTEM_USER_PUBLIC_ID, policy_set_public_id)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "policy_set_enable after link failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
}

async fn assert_search_profile_inventory(
    service: &IndexerService,
    fixture: &SearchProfileInventoryFixture,
) -> anyhow::Result<()> {
    let profiles = service.search_profile_list(SYSTEM_USER_PUBLIC_ID).await?;
    let profile = profiles
        .iter()
        .find(|item| item.search_profile_public_id == fixture.profile_public_id)
        .expect("created profile should be listed");
    assert_eq!(profile.display_name, fixture.updated_profile_name);
    assert!(profile.is_default);
    assert_eq!(profile.page_size, Some(80));
    assert_eq!(profile.default_media_domain_key.as_deref(), Some("movies"));
    assert_eq!(
        profile.media_domain_keys,
        vec!["movies".to_string(), "tv".to_string()]
    );
    assert!(
        profile
            .policy_set_public_ids
            .contains(&fixture.policy_set_public_id)
    );
    assert!(
        profile
            .allow_indexer_public_ids
            .contains(&fixture.fixture.indexer_instance_public_id)
    );
    assert!(
        profile
            .block_indexer_public_ids
            .contains(&fixture.blocked_instance_public_id)
    );
    assert!(profile.allow_tag_keys.contains(&fixture.fixture.tag_key));
    assert!(profile.block_tag_keys.contains(&fixture.blocked_tag_key));
    assert!(profile.prefer_tag_keys.contains(&fixture.fixture.tag_key));

    let policy_sets = service.policy_set_list(SYSTEM_USER_PUBLIC_ID).await?;
    let policy_set = policy_sets
        .iter()
        .find(|item| item.policy_set_public_id == fixture.policy_set_public_id)
        .expect("policy set should be listed");
    assert_eq!(policy_set.display_name, fixture.updated_policy_name);
    assert!(policy_set.is_enabled);
    assert_eq!(policy_set.rules.len(), 2);
    assert_eq!(
        policy_set.rules[0].policy_rule_public_id,
        fixture.second_rule_public_id
    );
    assert_eq!(
        policy_set.rules[1].policy_rule_public_id,
        fixture.first_rule_public_id
    );
    Ok(())
}

async fn create_torznab_operator_fixture(
    service: &IndexerService,
    fixture: &SearchProfileInventoryFixture,
) -> anyhow::Result<TorznabOperatorFixture> {
    let (torznab_name, torznab_instance_public_id, api_key_plaintext) =
        create_torznab_instance_credentials(service, fixture).await?;
    assert_torznab_auth_lifecycle(
        service,
        torznab_instance_public_id,
        &torznab_name,
        &api_key_plaintext,
    )
    .await?;
    Ok(TorznabOperatorFixture {
        torznab_instance_public_id,
        torznab_name,
    })
}

async fn create_torznab_instance_credentials(
    service: &IndexerService,
    fixture: &SearchProfileInventoryFixture,
) -> anyhow::Result<(String, Uuid, String)> {
    let torznab_name = unique_name("Torznab");
    let torznab_credentials = service
        .torznab_instance_create(
            SYSTEM_USER_PUBLIC_ID,
            fixture.profile_public_id,
            &torznab_name,
        )
        .await
        .unwrap_or_else(|err| {
            panic!(
                "torznab_instance_create failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    let torznab_instances = service.torznab_instance_list(SYSTEM_USER_PUBLIC_ID).await?;
    let torznab_instance = torznab_instances
        .iter()
        .find(|item| {
            item.torznab_instance_public_id == torznab_credentials.torznab_instance_public_id
        })
        .expect("torznab instance should be listed");
    assert_eq!(torznab_instance.display_name, torznab_name);
    assert_eq!(
        torznab_instance.search_profile_public_id,
        fixture.profile_public_id
    );
    assert_eq!(
        torznab_instance.search_profile_display_name,
        fixture.updated_profile_name
    );
    Ok((
        torznab_name,
        torznab_credentials.torznab_instance_public_id,
        torznab_credentials.api_key_plaintext,
    ))
}

async fn assert_torznab_auth_lifecycle(
    service: &IndexerService,
    torznab_instance_public_id: Uuid,
    torznab_name: &str,
    api_key_plaintext: &str,
) -> anyhow::Result<()> {
    let auth = service
        .torznab_instance_authenticate(torznab_instance_public_id, api_key_plaintext)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "torznab_instance_authenticate failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    assert_eq!(auth.display_name, torznab_name);

    let rotated_credentials = service
        .torznab_instance_rotate_key(SYSTEM_USER_PUBLIC_ID, torznab_instance_public_id)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "torznab_instance_rotate_key failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    assert_ne!(rotated_credentials.api_key_plaintext, api_key_plaintext);
    let stale_key_error = service
        .torznab_instance_authenticate(torznab_instance_public_id, api_key_plaintext)
        .await
        .expect_err("rotated api key should invalidate previous credential");
    assert_eq!(stale_key_error.kind(), TorznabAccessErrorKind::Unauthorized);
    service
        .torznab_instance_enable_disable(SYSTEM_USER_PUBLIC_ID, torznab_instance_public_id, false)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "torznab_instance_enable_disable false failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    let disabled_error = service
        .torznab_instance_authenticate(
            torznab_instance_public_id,
            &rotated_credentials.api_key_plaintext,
        )
        .await
        .expect_err("disabled torznab instance should not authenticate");
    assert_eq!(disabled_error.kind(), TorznabAccessErrorKind::NotFound);
    service
        .torznab_instance_enable_disable(SYSTEM_USER_PUBLIC_ID, torznab_instance_public_id, true)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "torznab_instance_enable_disable true failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    let restored_auth = service
        .torznab_instance_authenticate(
            torznab_instance_public_id,
            &rotated_credentials.api_key_plaintext,
        )
        .await?;
    assert_eq!(restored_auth.display_name, torznab_name);
    Ok(())
}

async fn assert_torznab_category_mapping_roundtrip(
    service: &IndexerService,
    fixture: &SearchProfileInventoryFixture,
    torznab_fixture: &TorznabOperatorFixture,
) -> anyhow::Result<()> {
    let categories = service.torznab_category_list().await?;
    assert!(!categories.is_empty());
    service
        .media_domain_mapping_upsert(SYSTEM_USER_PUBLIC_ID, "tv", 5040, Some(true))
        .await?;
    service
        .tracker_category_mapping_upsert(TrackerCategoryMappingUpsertParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            torznab_instance_public_id: Some(torznab_fixture.torznab_instance_public_id),
            indexer_definition_upstream_slug: None,
            indexer_instance_public_id: Some(fixture.fixture.indexer_instance_public_id),
            tracker_category: 2000,
            tracker_subcategory: Some(2040),
            torznab_cat_id: 5040,
            media_domain_key: Some("tv"),
        })
        .await?;
    let resolved_category_ids = service
        .torznab_feed_category_ids(
            torznab_fixture.torznab_instance_public_id,
            fixture.fixture.indexer_instance_public_id,
            Some(2000),
            Some(2040),
        )
        .await?;
    assert!(resolved_category_ids.contains(&5040));
    service
        .tracker_category_mapping_delete(TrackerCategoryMappingDeleteParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            torznab_instance_public_id: Some(torznab_fixture.torznab_instance_public_id),
            indexer_definition_upstream_slug: None,
            indexer_instance_public_id: Some(fixture.fixture.indexer_instance_public_id),
            tracker_category: 2000,
            tracker_subcategory: Some(2040),
        })
        .await?;
    service
        .media_domain_mapping_delete(SYSTEM_USER_PUBLIC_ID, "tv", 5040)
        .await?;
    Ok(())
}

async fn assert_search_profile_torznab_cleanup(
    service: &IndexerService,
    fixture: &SearchProfileInventoryFixture,
    torznab_fixture: &TorznabOperatorFixture,
) -> anyhow::Result<()> {
    service
        .search_profile_remove_policy_set(
            SYSTEM_USER_PUBLIC_ID,
            fixture.profile_public_id,
            fixture.policy_set_public_id,
        )
        .await?;
    let profiles = service.search_profile_list(SYSTEM_USER_PUBLIC_ID).await?;
    let profile = profiles
        .iter()
        .find(|item| item.search_profile_public_id == fixture.profile_public_id)
        .expect("updated profile should remain listed");
    assert!(
        !profile
            .policy_set_public_ids
            .contains(&fixture.policy_set_public_id)
    );

    service
        .torznab_instance_soft_delete(
            SYSTEM_USER_PUBLIC_ID,
            torznab_fixture.torznab_instance_public_id,
        )
        .await?;
    let torznab_instances = service.torznab_instance_list(SYSTEM_USER_PUBLIC_ID).await?;
    assert!(torznab_instances.iter().all(|item| {
        item.torznab_instance_public_id != torznab_fixture.torznab_instance_public_id
    }));
    assert!(
        torznab_instances
            .iter()
            .all(|item| item.display_name != torznab_fixture.torznab_name)
    );
    Ok(())
}

#[tokio::test]
async fn search_requests_import_jobs_and_conflicts_cover_wrapper_paths() -> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };
    let fixture = create_search_request_import_fixture(&service).await?;
    assert_empty_search_request_pages(&service, fixture.search_profile_public_id).await?;
    let populated_request = ingest_catalog_search_request_result(&service, &fixture).await?;
    assert_import_job_wrapper_paths(&service, fixture.search_profile_public_id).await?;
    assert_source_metadata_conflict_roundtrip(&service, &populated_request).await?;
    Ok(())
}

struct SearchRequestImportFixture {
    search_profile_public_id: Uuid,
    definition_slug: String,
}

struct PopulatedSearchRequestFixture {
    canonical_torrent_source_public_id: Uuid,
    indexer_instance_public_id: Uuid,
}

async fn create_search_request_import_fixture(
    service: &IndexerService,
) -> anyhow::Result<SearchRequestImportFixture> {
    let definition_slug = unique_name("catalog-indexer");
    let definition_name = unique_name("Catalog Indexer");
    import_operator_definition(service, &definition_slug, &definition_name).await?;
    let definitions = service
        .indexer_definition_list(SYSTEM_USER_PUBLIC_ID)
        .await?;
    assert!(definitions.iter().any(|definition| {
        definition.upstream_slug == definition_slug && definition.display_name == definition_name
    }));

    let search_profile_public_id = service
        .search_profile_create(
            SYSTEM_USER_PUBLIC_ID,
            &unique_name("Request Profile"),
            Some(true),
            Some(25),
            Some("tv"),
            None,
        )
        .await?;
    Ok(SearchRequestImportFixture {
        search_profile_public_id,
        definition_slug,
    })
}

async fn assert_empty_search_request_pages(
    service: &IndexerService,
    search_profile_public_id: Uuid,
) -> anyhow::Result<()> {
    let empty_request = service
        .search_request_create(SearchRequestCreateParams {
            actor_user_public_id: None,
            query_text: "ubuntu",
            query_type: "free_text",
            torznab_mode: Some("generic"),
            requested_media_domain_key: Some("tv"),
            page_size: Some(25),
            search_profile_public_id: Some(search_profile_public_id),
            request_policy_set_public_id: None,
            season_number: None,
            episode_number: None,
            identifier_types: None,
            identifier_values: None,
            torznab_cat_ids: None,
        })
        .await
        .unwrap_or_else(|err| {
            panic!(
                "search_request_create failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    let page_list = service
        .search_page_list(
            SYSTEM_USER_PUBLIC_ID,
            empty_request.search_request_public_id,
        )
        .await
        .context("search page list should succeed for a newly created request")?;
    assert_eq!(page_list.pages.len(), 1);
    assert_eq!(page_list.pages[0].page_number, 1);
    assert_eq!(page_list.pages[0].item_count, 0);
    assert!(page_list.explainability.zero_runnable_indexers);

    let page = service
        .search_page_fetch(
            SYSTEM_USER_PUBLIC_ID,
            empty_request.search_request_public_id,
            1,
        )
        .await
        .context("search page fetch should succeed for the first request page")?;
    assert_eq!(page.page_number, 1);
    assert_eq!(page.item_count, 0);
    assert!(page.items.is_empty());

    let missing_page_error = service
        .search_page_fetch(
            SYSTEM_USER_PUBLIC_ID,
            empty_request.search_request_public_id,
            2,
        )
        .await
        .expect_err("missing page should fail");
    assert_eq!(
        missing_page_error.kind(),
        SearchRequestServiceErrorKind::NotFound
    );
    assert_eq!(missing_page_error.code(), Some("search_page_not_found"));
    Ok(())
}

async fn ingest_catalog_search_request_result(
    service: &IndexerService,
    fixture: &SearchRequestImportFixture,
) -> anyhow::Result<PopulatedSearchRequestFixture> {
    let operator_fixture = seed_operator_indexer_fixture(service, &fixture.definition_slug).await?;
    let request_public_id =
        create_runnable_search_request(service, fixture.search_profile_public_id).await;
    let canonical_torrent_source_public_id =
        ingest_catalog_result_for_request(service, &operator_fixture, request_public_id).await?;
    assert_catalog_search_request_page(
        service,
        &operator_fixture,
        request_public_id,
        canonical_torrent_source_public_id,
    )
    .await?;
    service
        .search_request_cancel(SYSTEM_USER_PUBLIC_ID, request_public_id)
        .await
        .context("search request cancel should succeed after ingest assertions")?;
    Ok(PopulatedSearchRequestFixture {
        canonical_torrent_source_public_id,
        indexer_instance_public_id: operator_fixture.indexer_instance_public_id,
    })
}

async fn create_runnable_search_request(
    service: &IndexerService,
    search_profile_public_id: Uuid,
) -> Uuid {
    service
        .search_request_create(SearchRequestCreateParams {
            actor_user_public_id: None,
            query_text: "ubuntu",
            query_type: "free_text",
            torznab_mode: Some("generic"),
            requested_media_domain_key: Some("tv"),
            page_size: Some(25),
            search_profile_public_id: Some(search_profile_public_id),
            request_policy_set_public_id: None,
            season_number: None,
            episode_number: None,
            identifier_types: None,
            identifier_values: None,
            torznab_cat_ids: None,
        })
        .await
        .unwrap_or_else(|err| {
            panic!(
                "search_request_create with runnable indexer failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        })
        .search_request_public_id
}

async fn ingest_catalog_result_for_request(
    service: &IndexerService,
    fixture: &IndexerFixture,
    search_request_public_id: Uuid,
) -> anyhow::Result<Uuid> {
    let ingested = search_result_ingest(
        service.config.pool(),
        &SearchResultIngestInput {
            search_request_public_id,
            indexer_instance_public_id: fixture.indexer_instance_public_id,
            source_guid: Some("catalog-guid-1"),
            details_url: Some("https://example.test/details/catalog-guid-1"),
            download_url: Some("https://example.test/download/catalog-guid-1"),
            magnet_uri: Some("magnet:?xt=urn:btih:89abcdef0123456789abcdef0123456789abcdef"),
            title_raw: "Catalog result",
            size_bytes: Some(7 * 1024_i64 * 1024_i64),
            infohash_v1: Some("89abcdef0123456789abcdef0123456789abcdef"),
            infohash_v2: None,
            magnet_hash: None,
            seeders: Some(33),
            leechers: Some(4),
            published_at: Some(Utc::now()),
            uploader: Some("catalog-uploader"),
            observed_at: Utc::now(),
            attr_keys: None,
            attr_types: None,
            attr_value_text: None,
            attr_value_int: None,
            attr_value_bigint: None,
            attr_value_numeric: None,
            attr_value_bool: None,
            attr_value_uuid: None,
        },
    )
    .await?;
    assert!(ingested.observation_created);
    Ok(ingested.canonical_torrent_source_public_id)
}

async fn assert_catalog_search_request_page(
    service: &IndexerService,
    fixture: &IndexerFixture,
    request_public_id: Uuid,
    canonical_torrent_source_public_id: Uuid,
) -> anyhow::Result<()> {
    let populated_pages = service
        .search_page_list(SYSTEM_USER_PUBLIC_ID, request_public_id)
        .await
        .context("search page list should reflect ingested catalog results")?;
    assert_eq!(populated_pages.pages.len(), 1);
    assert_eq!(populated_pages.pages[0].item_count, 1);

    let populated_page = service
        .search_page_fetch(SYSTEM_USER_PUBLIC_ID, request_public_id, 1)
        .await
        .context("search page fetch should return the ingested catalog result")?;
    assert_eq!(populated_page.item_count, 1);
    let item = populated_page
        .items
        .first()
        .expect("ingested item should be returned");
    assert_eq!(item.position, 1);
    assert_eq!(item.title_display, "Catalog result");
    assert_eq!(
        item.canonical_torrent_source_public_id,
        Some(canonical_torrent_source_public_id)
    );
    assert_eq!(
        item.indexer_instance_public_id,
        Some(fixture.indexer_instance_public_id)
    );
    assert_eq!(item.seeders, Some(33));
    assert_eq!(item.leechers, Some(4));
    assert_eq!(
        item.download_url.as_deref(),
        Some("https://example.test/download/catalog-guid-1")
    );
    assert_eq!(
        item.magnet_uri.as_deref(),
        Some("magnet:?xt=urn:btih:89abcdef0123456789abcdef0123456789abcdef")
    );
    assert_eq!(
        item.details_url.as_deref(),
        Some("https://example.test/details/catalog-guid-1")
    );
    assert_eq!(
        item.indexer_display_name.as_deref(),
        Some(fixture.indexer_instance_name.as_str())
    );
    Ok(())
}

async fn assert_import_job_wrapper_paths(
    service: &IndexerService,
    search_profile_public_id: Uuid,
) -> anyhow::Result<()> {
    let api_job_public_id = service
        .import_job_create(
            SYSTEM_USER_PUBLIC_ID,
            "prowlarr_api",
            Some(true),
            Some(search_profile_public_id),
            None,
        )
        .await?;
    let api_status = service.import_job_get_status(api_job_public_id).await?;
    assert_eq!(api_status.status, "pending");
    assert_eq!(api_status.result_total, 0);
    assert!(
        service
            .import_job_list_results(api_job_public_id)
            .await?
            .is_empty()
    );

    let backup_job_public_id = service
        .import_job_create(
            SYSTEM_USER_PUBLIC_ID,
            "prowlarr_backup",
            Some(false),
            Some(search_profile_public_id),
            None,
        )
        .await?;
    let tag_alpha_key = unique_name("alpha");
    let tag_beta_key = unique_name("beta");
    let tag_alpha = service
        .tag_create(SYSTEM_USER_PUBLIC_ID, &tag_alpha_key, "Alpha")
        .await?;
    let tag_beta = service
        .tag_create(SYSTEM_USER_PUBLIC_ID, &tag_beta_key, "Beta")
        .await?;
    let snapshot_job_public_id = service
        .import_job_create(
            SYSTEM_USER_PUBLIC_ID,
            "prowlarr_api",
            Some(false),
            Some(search_profile_public_id),
            None,
        )
        .await?;
    let mismatch_error = service
        .import_job_run_prowlarr_backup(api_job_public_id, "snapshot-ref")
        .await
        .expect_err("source mismatch should fail");
    assert_eq!(mismatch_error.kind(), ImportJobServiceErrorKind::Conflict);
    assert_eq!(mismatch_error.code(), Some("import_source_mismatch"));

    let secret_public_id = service
        .secret_create(SYSTEM_USER_PUBLIC_ID, "api_key", "prowlarr-secret")
        .await?;
    service
        .import_job_run_prowlarr_api(api_job_public_id, "http://localhost:9696", secret_public_id)
        .await?;
    service
        .import_job_run_prowlarr_backup(backup_job_public_id, "snapshot-ref")
        .await?;
    insert_snapshot_import_result(
        service.config.pool(),
        snapshot_job_public_id,
        tag_alpha,
        tag_beta,
    )
    .await?;

    let snapshot_status = service
        .import_job_get_status(snapshot_job_public_id)
        .await?;
    assert_eq!(snapshot_status.status, "pending");
    assert_eq!(snapshot_status.result_total, 1);
    assert_eq!(snapshot_status.result_imported_needs_secret, 1);
    let snapshot_results = service
        .import_job_list_results(snapshot_job_public_id)
        .await?;
    assert_eq!(snapshot_results.len(), 1);
    assert_eq!(snapshot_results[0].prowlarr_identifier, "prowlarr-snapshot");
    assert_eq!(snapshot_results[0].status, "imported_needs_secret");
    assert_eq!(
        snapshot_results[0].detail.as_deref(),
        Some("missing_secret_bindings")
    );
    assert_eq!(snapshot_results[0].resolved_is_enabled, Some(false));
    assert_eq!(snapshot_results[0].resolved_priority, Some(73));
    assert_eq!(snapshot_results[0].missing_secret_fields, 2);
    assert_eq!(snapshot_results[0].media_domain_keys, vec!["movies", "tv"]);
    assert_eq!(
        snapshot_results[0].tag_keys,
        vec![tag_alpha_key.clone(), tag_beta_key.clone()]
    );
    Ok(())
}

async fn assert_source_metadata_conflict_roundtrip(
    service: &IndexerService,
    populated_request: &PopulatedSearchRequestFixture,
) -> anyhow::Result<()> {
    let conflict_id = create_source_metadata_conflict(service, populated_request).await?;
    assert_resolved_source_metadata_conflict(service, conflict_id).await?;
    assert_reopened_source_metadata_conflict(service, conflict_id).await?;
    assert_missing_source_metadata_conflict_errors(service, conflict_id).await?;
    Ok(())
}

async fn create_source_metadata_conflict(
    service: &IndexerService,
    populated_request: &PopulatedSearchRequestFixture,
) -> anyhow::Result<i64> {
    let canonical_torrent_source_id: i64 = query_scalar(
        "SELECT canonical_torrent_source_id
         FROM canonical_torrent_source
         WHERE canonical_torrent_source_public_id = $1",
    )
    .bind(populated_request.canonical_torrent_source_public_id)
    .fetch_one(service.config.pool())
    .await?;
    let indexer_instance_id: i64 = query_scalar(
        "SELECT indexer_instance_id
         FROM indexer_instance
         WHERE indexer_instance_public_id = $1",
    )
    .bind(populated_request.indexer_instance_public_id)
    .fetch_one(service.config.pool())
    .await?;
    log_source_metadata_conflict(
        service.config.pool(),
        canonical_torrent_source_id,
        indexer_instance_id,
        "hash",
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        Some(Utc::now()),
    )
    .await?;
    let conflicts = service
        .source_metadata_conflict_list(SYSTEM_USER_PUBLIC_ID, Some(true), Some(10))
        .await?;
    assert_eq!(conflicts.len(), 1);
    let conflict = &conflicts[0];
    assert_eq!(conflict.conflict_type, "hash");
    assert_eq!(
        conflict.existing_value,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(
        conflict.incoming_value,
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );
    assert_eq!(conflict.resolved_at, None);
    assert_eq!(conflict.resolution, None);
    Ok(conflict.conflict_id)
}

async fn assert_resolved_source_metadata_conflict(
    service: &IndexerService,
    conflict_id: i64,
) -> anyhow::Result<()> {
    service
        .source_metadata_conflict_resolve(
            SYSTEM_USER_PUBLIC_ID,
            conflict_id,
            "kept_existing",
            Some("reviewed by operator"),
        )
        .await?;
    let resolved_conflicts = service
        .source_metadata_conflict_list(SYSTEM_USER_PUBLIC_ID, Some(true), Some(10))
        .await?;
    assert_eq!(resolved_conflicts.len(), 1);
    assert_eq!(
        resolved_conflicts[0].resolution.as_deref(),
        Some("kept_existing")
    );
    assert_eq!(
        resolved_conflicts[0].resolution_note.as_deref(),
        Some("reviewed by operator")
    );
    assert!(resolved_conflicts[0].resolved_at.is_some());
    Ok(())
}

async fn assert_reopened_source_metadata_conflict(
    service: &IndexerService,
    conflict_id: i64,
) -> anyhow::Result<()> {
    service
        .source_metadata_conflict_reopen(
            SYSTEM_USER_PUBLIC_ID,
            conflict_id,
            Some("need another pass"),
        )
        .await?;
    let unresolved_conflicts = service
        .source_metadata_conflict_list(SYSTEM_USER_PUBLIC_ID, Some(false), Some(10))
        .await?;
    assert_eq!(unresolved_conflicts.len(), 1);
    assert_eq!(unresolved_conflicts[0].conflict_id, conflict_id);
    assert_eq!(unresolved_conflicts[0].resolved_at, None);
    assert_eq!(unresolved_conflicts[0].resolution, None);
    assert_eq!(unresolved_conflicts[0].resolution_note, None);
    Ok(())
}

async fn assert_missing_source_metadata_conflict_errors(
    service: &IndexerService,
    conflict_id: i64,
) -> anyhow::Result<()> {
    let resolve_error = service
        .source_metadata_conflict_resolve(SYSTEM_USER_PUBLIC_ID, conflict_id + 1, "ignored", None)
        .await
        .expect_err("missing conflict should not resolve");
    assert_eq!(
        resolve_error.kind(),
        SourceMetadataConflictServiceErrorKind::NotFound
    );
    assert_eq!(resolve_error.code(), Some("conflict_not_found"));

    let reopen_error = service
        .source_metadata_conflict_reopen(SYSTEM_USER_PUBLIC_ID, 1, None)
        .await
        .expect_err("missing conflict should not reopen");
    assert_eq!(
        reopen_error.kind(),
        SourceMetadataConflictServiceErrorKind::Conflict
    );
    assert_eq!(reopen_error.code(), Some("conflict_not_resolved"));
    Ok(())
}

#[tokio::test]
async fn indexer_backup_export_and_restore_roundtrip_preserves_inventory_shapes()
-> anyhow::Result<()> {
    let Ok((source_service, _source_db)) = build_service().await else {
        return Ok(());
    };
    let Ok((restore_service, _restore_db)) = build_service().await else {
        return Ok(());
    };

    let definition_slug = unique_name("backup-indexer");
    let definition_name = unique_name("Backup Indexer");
    import_operator_definition(&source_service, &definition_slug, &definition_name).await?;
    import_operator_definition(&restore_service, &definition_slug, &definition_name).await?;

    let fixture = seed_indexer_backup_export_source(&source_service, &definition_slug).await?;

    let export = source_service
        .indexer_backup_export(SYSTEM_USER_PUBLIC_ID)
        .await
        .context("indexer backup export should succeed for the seeded source inventory")?;
    assert_indexer_backup_export_snapshot(&export.snapshot, &fixture, &definition_slug);

    let restore = restore_service
        .indexer_backup_restore(SYSTEM_USER_PUBLIC_ID, &export.snapshot)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "indexer_backup_restore failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    assert_indexer_backup_restore_roundtrip_inventory(
        &restore_service,
        &restore,
        &fixture,
        &definition_slug,
    )
    .await?;
    Ok(())
}

async fn seed_indexer_backup_export_source(
    source_service: &IndexerService,
    definition_slug: &str,
) -> anyhow::Result<IndexerFixture> {
    let fixture = seed_operator_indexer_fixture(source_service, definition_slug).await?;
    source_service
        .indexer_instance_field_bind_secret(
            SYSTEM_USER_PUBLIC_ID,
            fixture.indexer_instance_public_id,
            "api_key",
            fixture.field_secret_public_id,
        )
        .await?;
    Ok(fixture)
}

fn assert_indexer_backup_export_snapshot(
    snapshot: &IndexerBackupSnapshot,
    fixture: &IndexerFixture,
    definition_slug: &str,
) {
    assert!(
        snapshot
            .tags
            .iter()
            .any(|tag| tag.tag_key == fixture.tag_key)
    );
    assert!(snapshot.rate_limit_policies.iter().any(|policy| {
        policy.display_name == fixture.rate_limit_policy_name
            && policy.requests_per_minute == 90
            && policy.burst == 30
            && policy.concurrent_requests == 3
    }));
    assert!(snapshot.routing_policies.iter().any(|policy| {
        policy.display_name == fixture.routing_policy_name
            && policy.parameters.iter().any(|parameter| {
                parameter.param_key == "proxy_host"
                    && parameter.value_plain.as_deref() == Some("proxy.internal")
            })
    }));
    assert!(snapshot.routing_policies.iter().any(|policy| {
        policy.display_name == fixture.routing_policy_name
            && policy.parameters.iter().any(|parameter| {
                parameter.param_key == "http_proxy_auth"
                    && parameter.secret_public_id == Some(fixture.route_secret_public_id)
            })
    }));
    assert!(snapshot.indexer_instances.iter().any(|instance| {
        instance.display_name == fixture.indexer_instance_name
            && instance.upstream_slug == definition_slug
            && instance.media_domain_keys == vec!["tv".to_string()]
            && instance.tag_keys == vec![fixture.tag_key.clone()]
    }));
    assert!(snapshot.secrets.iter().any(|secret| {
        secret.secret_public_id == fixture.route_secret_public_id
            && secret.secret_type == "password"
    }));
    assert!(snapshot.secrets.iter().any(|secret| {
        secret.secret_public_id == fixture.field_secret_public_id && secret.secret_type == "api_key"
    }));
}

async fn assert_indexer_backup_restore_roundtrip_inventory(
    restore_service: &IndexerService,
    restore: &IndexerBackupRestoreResponse,
    fixture: &IndexerFixture,
    definition_slug: &str,
) -> anyhow::Result<()> {
    assert_eq!(restore.created_tag_count, 1);
    assert_eq!(restore.created_rate_limit_policy_count, 1);
    assert_eq!(restore.created_routing_policy_count, 1);
    assert_eq!(restore.created_indexer_instance_count, 1);
    assert_eq!(restore.unresolved_secret_bindings.len(), 2);
    assert!(restore.unresolved_secret_bindings.iter().any(|binding| {
        binding.entity_type == "routing_policy"
            && binding.entity_display_name == fixture.routing_policy_name
            && binding.binding_key == "http_proxy_auth"
            && binding.secret_public_id == fixture.route_secret_public_id
    }));
    assert!(restore.unresolved_secret_bindings.iter().any(|binding| {
        binding.entity_type == "indexer_instance"
            && binding.entity_display_name == fixture.indexer_instance_name
            && binding.binding_key == "api_key"
            && binding.secret_public_id == fixture.field_secret_public_id
    }));

    let routing_inventory = restore_service
        .routing_policy_list(SYSTEM_USER_PUBLIC_ID)
        .await?;
    assert!(routing_inventory.iter().any(|policy| {
        policy.display_name == fixture.routing_policy_name
            && policy.rate_limit_policy_public_id.is_some()
            && policy.parameter_count >= 1
    }));

    let instances = restore_service
        .indexer_instance_list(SYSTEM_USER_PUBLIC_ID)
        .await?;
    let restored = instances
        .iter()
        .find(|instance| instance.display_name == fixture.indexer_instance_name)
        .expect("restored indexer instance should be listed");
    assert_eq!(restored.upstream_slug, definition_slug);
    assert_eq!(restored.media_domain_keys, vec!["tv".to_string()]);
    assert_eq!(restored.tag_keys, vec![fixture.tag_key.clone()]);
    assert!(restored.fields.iter().any(|field| {
        field.field_name == "base_url"
            && field.value_plain.as_deref() == Some("https://indexer.example")
    }));
    assert!(
        restored
            .fields
            .iter()
            .all(|field| field.secret_public_id != Some(fixture.field_secret_public_id))
    );
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
    import_operator_definition(&service, &definition_slug, &definition_name).await?;

    let tag_key = unique_name("restore-tag");
    let rate_limit_name = unique_name("restore-rate-limit");
    let routing_name = unique_name("restore-route");
    let instance_name = unique_name("restore-instance");
    let missing_route_secret = Uuid::new_v4();
    let missing_field_secret = Uuid::new_v4();
    super::restore_backup_tag_fixture(&service, &tag_key).await?;
    let (rate_limit_public_id, rate_limit_id_by_name) =
        super::restore_backup_rate_limit_fixture(&service, &rate_limit_name).await?;
    let (routing_policy_public_id, routing_policy_id_by_name) =
        super::restore_backup_routing_policy_fixture(
            &service,
            &routing_name,
            &rate_limit_name,
            rate_limit_public_id,
            missing_route_secret,
            &rate_limit_id_by_name,
        )
        .await?;
    super::restore_backup_indexer_instance_fixture(
        &service,
        &super::RestoreBackupIndexerInstanceFixture {
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

#[tokio::test]
async fn restore_backup_rate_limits_reuse_existing_system_policies() -> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let system_policy_name = unique_name("restore-system-policy");
    let system_policy_public_id = service
        .rate_limit_policy_create(SYSTEM_USER_PUBLIC_ID, &system_policy_name, 120, 40, 4)
        .await?;
    query(include_str!("sql/rate_limit_mark_system.sql"))
        .bind(system_policy_public_id)
        .execute(service.config.pool())
        .await?;

    let custom_policy_name = unique_name("restore-custom-policy");
    let (created_count, policy_ids) = service
        .restore_backup_rate_limits(
            SYSTEM_USER_PUBLIC_ID,
            &[
                IndexerBackupRateLimitPolicyItem {
                    display_name: system_policy_name.clone(),
                    requests_per_minute: 999,
                    burst: 999,
                    concurrent_requests: 99,
                    is_system: true,
                },
                IndexerBackupRateLimitPolicyItem {
                    display_name: custom_policy_name.clone(),
                    requests_per_minute: 75,
                    burst: 25,
                    concurrent_requests: 5,
                    is_system: false,
                },
            ],
        )
        .await?;

    assert_eq!(created_count, 1);
    assert_eq!(
        policy_ids.get(&system_policy_name),
        Some(&system_policy_public_id)
    );

    let custom_policy_public_id = policy_ids
        .get(&custom_policy_name)
        .copied()
        .context("restored custom rate-limit should be indexed")?;
    let policies = service
        .rate_limit_policy_list(SYSTEM_USER_PUBLIC_ID)
        .await?;
    let custom = policies
        .iter()
        .find(|policy| policy.rate_limit_policy_public_id == custom_policy_public_id)
        .context("restored custom rate-limit should be listed")?;
    assert_eq!(custom.display_name, custom_policy_name);
    assert_eq!(custom.requests_per_minute, 75);
    assert_eq!(custom.burst, 25);
    assert_eq!(custom.concurrent_requests, 5);
    assert!(!custom.is_system);
    Ok(())
}

#[tokio::test]
async fn restore_backup_routing_policies_bind_existing_secrets_and_boolean_params()
-> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let rate_limit_name = unique_name("restore-bool-rate-limit");
    let rate_limit_policy_public_id = service
        .rate_limit_policy_create(SYSTEM_USER_PUBLIC_ID, &rate_limit_name, 80, 20, 3)
        .await?;
    let route_secret_public_id = service
        .secret_create(SYSTEM_USER_PUBLIC_ID, "password", "proxy-pass")
        .await?;

    let (created_count, routing_policy_ids, unresolved_bindings) = service
        .restore_backup_routing_policies(
            SYSTEM_USER_PUBLIC_ID,
            &[IndexerBackupRoutingPolicyItem {
                display_name: unique_name("restore-bool-route"),
                mode: "http_proxy".to_string(),
                rate_limit_display_name: Some(rate_limit_name.clone()),
                parameters: vec![
                    IndexerBackupRoutingParameterItem {
                        param_key: "proxy_host".to_string(),
                        value_plain: Some("proxy.internal".to_string()),
                        value_int: None,
                        value_bool: None,
                        secret_public_id: None,
                    },
                    IndexerBackupRoutingParameterItem {
                        param_key: "proxy_use_tls".to_string(),
                        value_plain: None,
                        value_int: None,
                        value_bool: Some(true),
                        secret_public_id: None,
                    },
                    IndexerBackupRoutingParameterItem {
                        param_key: "http_proxy_auth".to_string(),
                        value_plain: None,
                        value_int: None,
                        value_bool: None,
                        secret_public_id: Some(route_secret_public_id),
                    },
                ],
            }],
            &BTreeMap::from([(rate_limit_name, rate_limit_policy_public_id)]),
        )
        .await?;

    assert_eq!(created_count, 1);
    assert!(
        unresolved_bindings.is_empty(),
        "existing secrets should bind during routing policy restore"
    );

    let routing_policy_public_id = routing_policy_ids
        .values()
        .copied()
        .next()
        .context("restored routing policy should be indexed")?;
    let detail = service
        .routing_policy_get(SYSTEM_USER_PUBLIC_ID, routing_policy_public_id)
        .await?;
    assert!(detail.parameters.iter().any(|parameter| {
        parameter.param_key == "proxy_use_tls" && parameter.value_bool == Some(true)
    }));
    assert!(detail.parameters.iter().any(|parameter| {
        parameter.param_key == "http_proxy_auth"
            && parameter.secret_public_id == Some(route_secret_public_id)
    }));
    Ok(())
}

#[tokio::test]
async fn restore_backup_indexer_instances_apply_links_fields_and_disabled_subscriptions()
-> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let fixture = create_restore_rich_indexer_fixture(&service).await?;
    assert_restore_rich_indexer_inventory(&service, &fixture).await?;
    Ok(())
}

struct RestoreRichIndexerFixture {
    definition_slug: String,
    tag_key: String,
    instance_name: String,
    rate_limit_policy_public_id: Uuid,
    api_key_secret_public_id: Uuid,
    missing_password_secret_public_id: Uuid,
}

struct RestoreRichIndexerDependencies {
    definition_slug: String,
    tag_key: String,
    rate_limit_name: String,
    rate_limit_policy_public_id: Uuid,
    api_key_secret_public_id: Uuid,
}

async fn create_restore_rich_indexer_fixture(
    service: &IndexerService,
) -> anyhow::Result<RestoreRichIndexerFixture> {
    let dependencies = prepare_restore_rich_indexer_dependencies(service).await?;
    restore_rich_indexer_with_missing_secret(service, &dependencies).await
}

async fn prepare_restore_rich_indexer_dependencies(
    service: &IndexerService,
) -> anyhow::Result<RestoreRichIndexerDependencies> {
    let definition_slug = unique_name("restore-rich-indexer");
    let definition_name = unique_name("Restore Rich Indexer");
    import_restore_operator_definition(service, &definition_slug, &definition_name).await?;

    let tag_key = unique_name("restore-rich-tag");
    service
        .tag_create(SYSTEM_USER_PUBLIC_ID, &tag_key, "Restore Rich Tag")
        .await?;
    let rate_limit_name = unique_name("restore-rich-rate-limit");
    let rate_limit_policy_public_id = service
        .rate_limit_policy_create(SYSTEM_USER_PUBLIC_ID, &rate_limit_name, 60, 10, 2)
        .await?;
    let api_key_secret_public_id = service
        .secret_create(SYSTEM_USER_PUBLIC_ID, "api_key", "rich-api-key")
        .await?;
    Ok(RestoreRichIndexerDependencies {
        definition_slug,
        tag_key,
        rate_limit_name,
        rate_limit_policy_public_id,
        api_key_secret_public_id,
    })
}

async fn restore_rich_indexer_with_missing_secret(
    service: &IndexerService,
    dependencies: &RestoreRichIndexerDependencies,
) -> anyhow::Result<RestoreRichIndexerFixture> {
    let missing_password_secret_public_id = Uuid::new_v4();
    let instance_name = unique_name("restore-rich-instance");
    let backup_item = build_restore_rich_indexer_item(
        dependencies,
        &instance_name,
        missing_password_secret_public_id,
    );
    let (created_count, unresolved_bindings) = service
        .restore_backup_indexer_instances(
            SYSTEM_USER_PUBLIC_ID,
            &[backup_item],
            &BTreeMap::from([(
                dependencies.rate_limit_name.clone(),
                dependencies.rate_limit_policy_public_id,
            )]),
            &BTreeMap::new(),
        )
        .await?;
    assert_restore_rich_missing_secret_binding(
        created_count,
        &unresolved_bindings,
        &instance_name,
        missing_password_secret_public_id,
    );

    Ok(RestoreRichIndexerFixture {
        definition_slug: dependencies.definition_slug.clone(),
        tag_key: dependencies.tag_key.clone(),
        instance_name,
        rate_limit_policy_public_id: dependencies.rate_limit_policy_public_id,
        api_key_secret_public_id: dependencies.api_key_secret_public_id,
        missing_password_secret_public_id,
    })
}

fn build_restore_rich_indexer_item(
    dependencies: &RestoreRichIndexerDependencies,
    instance_name: &str,
    missing_password_secret_public_id: Uuid,
) -> IndexerBackupIndexerInstanceItem {
    IndexerBackupIndexerInstanceItem {
        upstream_slug: dependencies.definition_slug.clone(),
        display_name: instance_name.to_string(),
        instance_status: "disabled".to_string(),
        rss_status: "enabled".to_string(),
        automatic_search_status: "enabled".to_string(),
        interactive_search_status: "disabled".to_string(),
        priority: 27,
        trust_tier_key: Some("public".to_string()),
        routing_policy_display_name: None,
        connect_timeout_ms: 4_500,
        read_timeout_ms: 12_000,
        max_parallel_requests: 6,
        rate_limit_display_name: Some(dependencies.rate_limit_name.clone()),
        rss_subscription_enabled: Some(false),
        rss_interval_seconds: Some(300),
        media_domain_keys: vec!["movies".to_string(), "tv".to_string()],
        tag_keys: vec![dependencies.tag_key.clone()],
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
                field_name: "weight".to_string(),
                field_type: "number".to_string(),
                value_plain: None,
                value_int: Some(7),
                value_decimal: None,
                value_bool: None,
                secret_public_id: None,
            },
            IndexerBackupFieldItem {
                field_name: "ratio".to_string(),
                field_type: "float".to_string(),
                value_plain: None,
                value_int: None,
                value_decimal: Some("1.25".to_string()),
                value_bool: None,
                secret_public_id: None,
            },
            IndexerBackupFieldItem {
                field_name: "enabled".to_string(),
                field_type: "bool".to_string(),
                value_plain: None,
                value_int: None,
                value_decimal: None,
                value_bool: Some(true),
                secret_public_id: None,
            },
            IndexerBackupFieldItem {
                field_name: "api_key".to_string(),
                field_type: "api_key".to_string(),
                value_plain: None,
                value_int: None,
                value_decimal: None,
                value_bool: None,
                secret_public_id: Some(dependencies.api_key_secret_public_id),
            },
            IndexerBackupFieldItem {
                field_name: "password".to_string(),
                field_type: "password".to_string(),
                value_plain: None,
                value_int: None,
                value_decimal: None,
                value_bool: None,
                secret_public_id: Some(missing_password_secret_public_id),
            },
        ],
    }
}

fn assert_restore_rich_missing_secret_binding(
    created_count: i32,
    unresolved_bindings: &[IndexerBackupUnresolvedSecretBinding],
    instance_name: &str,
    missing_password_secret_public_id: Uuid,
) {
    assert_eq!(created_count, 1);
    assert_eq!(unresolved_bindings.len(), 1);
    assert_eq!(unresolved_bindings[0].entity_type, "indexer_instance");
    assert_eq!(unresolved_bindings[0].entity_display_name, instance_name);
    assert_eq!(unresolved_bindings[0].binding_key, "password");
    assert_eq!(
        unresolved_bindings[0].secret_public_id,
        missing_password_secret_public_id
    );
}

async fn assert_restore_rich_indexer_inventory(
    service: &IndexerService,
    fixture: &RestoreRichIndexerFixture,
) -> anyhow::Result<()> {
    let instances = service.indexer_instance_list(SYSTEM_USER_PUBLIC_ID).await?;
    let instance = instances
        .iter()
        .find(|item| item.display_name == fixture.instance_name)
        .context("restored rich indexer instance should be listed")?;
    assert_eq!(instance.upstream_slug, fixture.definition_slug);
    assert_eq!(instance.instance_status, "disabled");
    assert_eq!(instance.rss_status, "enabled");
    assert_eq!(instance.automatic_search_status, "enabled");
    assert_eq!(instance.interactive_search_status, "disabled");
    assert_eq!(instance.priority, 27);
    assert_eq!(instance.trust_tier_key.as_deref(), Some("public"));
    assert_eq!(
        instance.rate_limit_policy_public_id,
        Some(fixture.rate_limit_policy_public_id)
    );
    assert_eq!(instance.media_domain_keys, vec!["movies", "tv"]);
    assert_eq!(instance.tag_keys, vec![fixture.tag_key.clone()]);
    assert!(instance.fields.iter().any(|field| {
        field.field_name == "base_url"
            && field.value_plain.as_deref() == Some("https://indexer.example")
    }));
    assert!(
        instance
            .fields
            .iter()
            .any(|field| { field.field_name == "weight" && field.value_int == Some(7) })
    );
    let ratio_field = instance
        .fields
        .iter()
        .find(|field| field.field_name == "ratio")
        .context("restored decimal field should be listed")?;
    assert_eq!(ratio_field.field_type, "number_decimal");
    let ratio_value = ratio_field
        .value_decimal
        .as_deref()
        .context("restored decimal field should include a decimal value")?;
    let parsed_ratio = ratio_value
        .parse::<f64>()
        .context("restored decimal field should parse as a numeric value")?;
    assert!((parsed_ratio - 1.25).abs() < f64::EPSILON);
    assert!(
        instance
            .fields
            .iter()
            .any(|field| { field.field_name == "enabled" && field.value_bool == Some(true) })
    );
    assert!(instance.fields.iter().any(|field| {
        field.field_name == "api_key"
            && field.secret_public_id == Some(fixture.api_key_secret_public_id)
    }));
    assert!(!instance.fields.iter().any(|field| {
        field.field_name == "password"
            && field.secret_public_id == Some(fixture.missing_password_secret_public_id)
    }));

    let subscription = service
        .indexer_rss_subscription_get(SYSTEM_USER_PUBLIC_ID, instance.indexer_instance_public_id)
        .await?;
    assert_eq!(subscription.subscription_status, "disabled");
    assert_eq!(subscription.interval_seconds, 300);
    Ok(())
}

#[tokio::test]
async fn restore_backup_helpers_bind_existing_secrets_without_unresolved_bindings()
-> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let fixture = create_restore_bound_secret_fixture(&service).await?;
    assert_restore_bound_secret_inventory(&service, &fixture).await?;
    Ok(())
}

struct RestoreBoundSecretFixture {
    definition_slug: String,
    tag_key: String,
    instance_name: String,
    route_secret_public_id: Uuid,
    field_secret_public_id: Uuid,
    routing_policy_public_id: Uuid,
}

struct RestoreBoundSecretDependencies {
    definition_slug: String,
    tag_key: String,
    route_secret_public_id: Uuid,
    field_secret_public_id: Uuid,
    rate_limit_name: String,
    routing_name: String,
    rate_limit_id_by_name: BTreeMap<String, Uuid>,
}

async fn create_restore_bound_secret_fixture(
    service: &IndexerService,
) -> anyhow::Result<RestoreBoundSecretFixture> {
    let dependencies = prepare_restore_bound_secret_dependencies(service).await?;
    restore_bound_secret_inventory_fixture(service, &dependencies).await
}

async fn prepare_restore_bound_secret_dependencies(
    service: &IndexerService,
) -> anyhow::Result<RestoreBoundSecretDependencies> {
    let definition_slug = unique_name("restore-secret-indexer");
    let definition_name = unique_name("Restore Secret Indexer");
    import_operator_definition(service, &definition_slug, &definition_name).await?;

    let tag_key = unique_name("restore-secret-tag");
    service
        .tag_create(SYSTEM_USER_PUBLIC_ID, &tag_key, "Restore Secret Tag")
        .await?;
    let route_secret_public_id = service
        .secret_create(SYSTEM_USER_PUBLIC_ID, "password", "proxy-pass")
        .await?;
    let field_secret_public_id = service
        .secret_create(SYSTEM_USER_PUBLIC_ID, "api_key", "indexer-key")
        .await?;

    let rate_limit_name = unique_name("restore-secret-rate-limit");
    let routing_name = unique_name("restore-secret-route");

    let (_, rate_limit_id_by_name) = service
        .restore_backup_rate_limits(
            SYSTEM_USER_PUBLIC_ID,
            &[IndexerBackupRateLimitPolicyItem {
                display_name: rate_limit_name.clone(),
                requests_per_minute: 45,
                burst: 12,
                concurrent_requests: 2,
                is_system: false,
            }],
        )
        .await?;
    Ok(RestoreBoundSecretDependencies {
        definition_slug,
        tag_key,
        route_secret_public_id,
        field_secret_public_id,
        rate_limit_name,
        routing_name,
        rate_limit_id_by_name,
    })
}

async fn restore_bound_secret_inventory_fixture(
    service: &IndexerService,
    dependencies: &RestoreBoundSecretDependencies,
) -> anyhow::Result<RestoreBoundSecretFixture> {
    let instance_name = unique_name("restore-secret-instance");
    let (_, routing_policy_id_by_name, unresolved_routing_bindings) = service
        .restore_backup_routing_policies(
            SYSTEM_USER_PUBLIC_ID,
            &[IndexerBackupRoutingPolicyItem {
                display_name: dependencies.routing_name.clone(),
                mode: "http_proxy".to_string(),
                rate_limit_display_name: Some(dependencies.rate_limit_name.clone()),
                parameters: vec![
                    IndexerBackupRoutingParameterItem {
                        param_key: "proxy_host".to_string(),
                        value_plain: Some("proxy.internal".to_string()),
                        value_int: None,
                        value_bool: None,
                        secret_public_id: None,
                    },
                    IndexerBackupRoutingParameterItem {
                        param_key: "http_proxy_auth".to_string(),
                        value_plain: None,
                        value_int: None,
                        value_bool: None,
                        secret_public_id: Some(dependencies.route_secret_public_id),
                    },
                ],
            }],
            &dependencies.rate_limit_id_by_name,
        )
        .await?;
    assert!(
        unresolved_routing_bindings.is_empty(),
        "existing secrets should bind without unresolved routing bindings"
    );

    let (_, unresolved_field_bindings) = service
        .restore_backup_indexer_instances(
            SYSTEM_USER_PUBLIC_ID,
            &[IndexerBackupIndexerInstanceItem {
                upstream_slug: dependencies.definition_slug.clone(),
                display_name: instance_name.clone(),
                instance_status: "enabled".to_string(),
                rss_status: "enabled".to_string(),
                automatic_search_status: "enabled".to_string(),
                interactive_search_status: "enabled".to_string(),
                priority: 21,
                trust_tier_key: Some("public".to_string()),
                routing_policy_display_name: Some(dependencies.routing_name.clone()),
                connect_timeout_ms: 5_000,
                read_timeout_ms: 15_000,
                max_parallel_requests: 4,
                rate_limit_display_name: Some(dependencies.rate_limit_name.clone()),
                rss_subscription_enabled: Some(true),
                rss_interval_seconds: Some(1_200),
                media_domain_keys: vec!["movies".to_string()],
                tag_keys: vec![dependencies.tag_key.clone()],
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
                        secret_public_id: Some(dependencies.field_secret_public_id),
                    },
                ],
            }],
            &dependencies.rate_limit_id_by_name,
            &routing_policy_id_by_name,
        )
        .await?;
    assert!(
        unresolved_field_bindings.is_empty(),
        "existing secrets should bind without unresolved field bindings"
    );

    let routing_policy_public_id = routing_policy_id_by_name
        .get(&dependencies.routing_name)
        .copied()
        .ok_or_else(|| anyhow::anyhow!("restored routing policy should be indexed"))?;
    Ok(RestoreBoundSecretFixture {
        definition_slug: dependencies.definition_slug.clone(),
        tag_key: dependencies.tag_key.clone(),
        instance_name,
        route_secret_public_id: dependencies.route_secret_public_id,
        field_secret_public_id: dependencies.field_secret_public_id,
        routing_policy_public_id,
    })
}

async fn assert_restore_bound_secret_inventory(
    service: &IndexerService,
    fixture: &RestoreBoundSecretFixture,
) -> anyhow::Result<()> {
    let routing_detail = service
        .routing_policy_get(SYSTEM_USER_PUBLIC_ID, fixture.routing_policy_public_id)
        .await?;
    assert!(routing_detail.parameters.iter().any(|parameter| {
        parameter.param_key == "http_proxy_auth"
            && parameter.secret_public_id == Some(fixture.route_secret_public_id)
    }));

    let instances = service.indexer_instance_list(SYSTEM_USER_PUBLIC_ID).await?;
    let instance = instances
        .iter()
        .find(|item| item.display_name == fixture.instance_name)
        .ok_or_else(|| anyhow::anyhow!("restored indexer instance should be listed"))?;
    assert_eq!(instance.upstream_slug, fixture.definition_slug);
    assert_eq!(instance.media_domain_keys, vec!["movies".to_string()]);
    assert_eq!(instance.tag_keys, vec![fixture.tag_key.clone()]);
    assert!(instance.fields.iter().any(|field| {
        field.field_name == "base_url"
            && field.value_plain.as_deref() == Some("https://indexer.example")
    }));
    assert!(instance.fields.iter().any(|field| {
        field.field_name == "api_key"
            && field.secret_public_id == Some(fixture.field_secret_public_id)
    }));

    let metadata = service.secret_metadata_list(SYSTEM_USER_PUBLIC_ID).await?;
    assert!(metadata.iter().any(|item| {
        item.secret_public_id == fixture.route_secret_public_id && item.binding_count >= 1
    }));
    assert!(metadata.iter().any(|item| {
        item.secret_public_id == fixture.field_secret_public_id && item.binding_count >= 1
    }));
    Ok(())
}

#[tokio::test]
async fn restore_backup_helpers_surface_missing_reference_errors() -> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };
    assert_restore_backup_missing_rate_limit_error(&service).await?;
    let (definition_slug, routing_name, routing_policy_id_by_name) =
        create_missing_reference_restore_fixture(&service).await?;
    assert_restore_backup_missing_routing_error(
        &service,
        &definition_slug,
        &routing_policy_id_by_name,
    )
    .await?;
    assert_restore_backup_missing_rate_limit_link_error(
        &service,
        &definition_slug,
        &routing_name,
        &routing_policy_id_by_name,
    )
    .await?;
    Ok(())
}

async fn assert_restore_backup_missing_rate_limit_error(
    service: &IndexerService,
) -> anyhow::Result<()> {
    let missing_rate_limit_error = service
        .restore_backup_routing_policies(
            SYSTEM_USER_PUBLIC_ID,
            &[IndexerBackupRoutingPolicyItem {
                display_name: unique_name("Missing Rate Limit Route"),
                mode: "http_proxy".to_string(),
                rate_limit_display_name: Some("missing-rate-limit".to_string()),
                parameters: Vec::new(),
            }],
            &BTreeMap::new(),
        )
        .await
        .expect_err("missing rate-limit reference should fail before restore");
    assert_eq!(
        missing_rate_limit_error.kind(),
        IndexerBackupServiceErrorKind::Invalid
    );
    assert_eq!(
        missing_rate_limit_error.code(),
        Some("rate_limit_reference_missing")
    );
    Ok(())
}

async fn create_missing_reference_restore_fixture(
    service: &IndexerService,
) -> anyhow::Result<(String, String, BTreeMap<String, Uuid>)> {
    let definition_slug = unique_name("missing-ref-indexer");
    let definition_name = unique_name("Missing Ref Indexer");
    import_operator_definition(service, &definition_slug, &definition_name).await?;

    let routing_name = unique_name("Existing Route");
    let (_, routing_policy_id_by_name, _) = service
        .restore_backup_routing_policies(
            SYSTEM_USER_PUBLIC_ID,
            &[IndexerBackupRoutingPolicyItem {
                display_name: routing_name.clone(),
                mode: "http_proxy".to_string(),
                rate_limit_display_name: None,
                parameters: Vec::new(),
            }],
            &BTreeMap::new(),
        )
        .await?;
    Ok((definition_slug, routing_name, routing_policy_id_by_name))
}

async fn assert_restore_backup_missing_routing_error(
    service: &IndexerService,
    definition_slug: &str,
    routing_policy_id_by_name: &BTreeMap<String, Uuid>,
) -> anyhow::Result<()> {
    let missing_routing_error = service
        .restore_backup_indexer_instances(
            SYSTEM_USER_PUBLIC_ID,
            &[IndexerBackupIndexerInstanceItem {
                upstream_slug: definition_slug.to_string(),
                display_name: unique_name("Missing Route Instance"),
                instance_status: "enabled".to_string(),
                rss_status: "disabled".to_string(),
                automatic_search_status: "disabled".to_string(),
                interactive_search_status: "enabled".to_string(),
                priority: 10,
                trust_tier_key: Some("public".to_string()),
                routing_policy_display_name: Some("missing-routing-policy".to_string()),
                connect_timeout_ms: 5_000,
                read_timeout_ms: 15_000,
                max_parallel_requests: 4,
                rate_limit_display_name: None,
                rss_subscription_enabled: None,
                rss_interval_seconds: None,
                media_domain_keys: Vec::new(),
                tag_keys: Vec::new(),
                fields: Vec::new(),
            }],
            &BTreeMap::new(),
            routing_policy_id_by_name,
        )
        .await
        .expect_err("missing routing reference should fail before indexer creation");
    assert_eq!(
        missing_routing_error.kind(),
        IndexerBackupServiceErrorKind::Invalid
    );
    assert_eq!(
        missing_routing_error.code(),
        Some("routing_policy_reference_missing")
    );
    Ok(())
}

async fn assert_restore_backup_missing_rate_limit_link_error(
    service: &IndexerService,
    definition_slug: &str,
    routing_name: &str,
    routing_policy_id_by_name: &BTreeMap<String, Uuid>,
) -> anyhow::Result<()> {
    let missing_rate_limit_link_error = service
        .restore_backup_indexer_instances(
            SYSTEM_USER_PUBLIC_ID,
            &[IndexerBackupIndexerInstanceItem {
                upstream_slug: definition_slug.to_string(),
                display_name: unique_name("Missing Link Rate Limit"),
                instance_status: "enabled".to_string(),
                rss_status: "disabled".to_string(),
                automatic_search_status: "disabled".to_string(),
                interactive_search_status: "enabled".to_string(),
                priority: 10,
                trust_tier_key: Some("public".to_string()),
                routing_policy_display_name: Some(routing_name.to_string()),
                connect_timeout_ms: 5_000,
                read_timeout_ms: 15_000,
                max_parallel_requests: 4,
                rate_limit_display_name: Some("missing-rate-limit".to_string()),
                rss_subscription_enabled: None,
                rss_interval_seconds: None,
                media_domain_keys: Vec::new(),
                tag_keys: Vec::new(),
                fields: Vec::new(),
            }],
            &BTreeMap::new(),
            routing_policy_id_by_name,
        )
        .await
        .expect_err("missing rate-limit link should fail during instance restore");
    assert_eq!(
        missing_rate_limit_link_error.kind(),
        IndexerBackupServiceErrorKind::Invalid
    );
    assert_eq!(
        missing_rate_limit_link_error.code(),
        Some("rate_limit_reference_missing")
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
async fn cardigann_definition_reimport_replaces_existing_field_shape() -> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let upstream_slug = unique_name("reimport-cardigann");
    let first_name = unique_name("First Cardigann Import");
    let second_name = unique_name("Second Cardigann Import");

    let first_yaml = format!(
        "id: {upstream_slug}\nname: {first_name}\nsettings:\n  - name: apiKey\n    label: API key\n    type: apikey\n    required: true\n  - name: sort\n    label: Sort\n    type: select\n    options:\n      - value: seeders\n        label: Seeders\n      - value: date\n        label: Date\n"
    );
    let first_response = service
        .indexer_definition_import_cardigann(SYSTEM_USER_PUBLIC_ID, &first_yaml, Some(false))
        .await?;
    assert_eq!(first_response.field_count, 2);
    assert_eq!(first_response.option_count, 2);

    let second_yaml = format!(
        "id: {upstream_slug}\nname: {second_name}\nsettings:\n  - name: baseUrl\n    label: Base URL\n    type: text\n    required: true\n"
    );
    let second_response = service
        .indexer_definition_import_cardigann(SYSTEM_USER_PUBLIC_ID, &second_yaml, Some(true))
        .await?;
    assert_eq!(second_response.definition.upstream_slug, upstream_slug);
    assert_eq!(second_response.definition.display_name, second_name);
    assert!(second_response.definition.is_deprecated);
    assert_eq!(second_response.field_count, 1);
    assert_eq!(second_response.option_count, 0);

    let definitions = service
        .indexer_definition_list(SYSTEM_USER_PUBLIC_ID)
        .await?;
    let definition = definitions
        .iter()
        .find(|item| item.upstream_slug == upstream_slug)
        .ok_or_else(|| anyhow::anyhow!("reimported definition should be listed"))?;
    assert_eq!(definition.display_name, second_name);
    assert!(definition.is_deprecated);
    Ok(())
}

#[tokio::test]
async fn indexer_backup_restore_surfaces_top_level_reference_errors() -> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let missing_rate_limit_error = service
        .indexer_backup_restore(
            SYSTEM_USER_PUBLIC_ID,
            &IndexerBackupSnapshot {
                version: "revaer.indexers.backup.v1".to_string(),
                exported_at: Utc::now(),
                tags: Vec::new(),
                rate_limit_policies: Vec::new(),
                routing_policies: vec![IndexerBackupRoutingPolicyItem {
                    display_name: unique_name("Top Level Missing Rate Limit"),
                    mode: "http_proxy".to_string(),
                    rate_limit_display_name: Some("missing-rate-limit".to_string()),
                    parameters: Vec::new(),
                }],
                indexer_instances: Vec::new(),
                secrets: Vec::new(),
            },
        )
        .await
        .expect_err("restore should fail when routing policies reference a missing rate limit");
    assert_eq!(
        missing_rate_limit_error.kind(),
        IndexerBackupServiceErrorKind::Invalid
    );
    assert_eq!(
        missing_rate_limit_error.code(),
        Some("rate_limit_reference_missing")
    );

    let definition_slug = unique_name("top-level-missing-routing");
    let definition_name = unique_name("Top Level Missing Routing");
    import_operator_definition(&service, &definition_slug, &definition_name).await?;

    let rate_limit_name = unique_name("Top Level Restore Rate");
    let missing_routing_error = service
        .indexer_backup_restore(
            SYSTEM_USER_PUBLIC_ID,
            &IndexerBackupSnapshot {
                version: "revaer.indexers.backup.v1".to_string(),
                exported_at: Utc::now(),
                tags: Vec::new(),
                rate_limit_policies: vec![IndexerBackupRateLimitPolicyItem {
                    display_name: rate_limit_name.clone(),
                    requests_per_minute: 60,
                    burst: 10,
                    concurrent_requests: 3,
                    is_system: false,
                }],
                routing_policies: Vec::new(),
                indexer_instances: vec![IndexerBackupIndexerInstanceItem {
                    upstream_slug: definition_slug,
                    display_name: unique_name("Top Level Missing Routing Instance"),
                    instance_status: "enabled".to_string(),
                    rss_status: "disabled".to_string(),
                    automatic_search_status: "disabled".to_string(),
                    interactive_search_status: "enabled".to_string(),
                    priority: 10,
                    trust_tier_key: Some("public".to_string()),
                    routing_policy_display_name: Some("missing-routing-policy".to_string()),
                    connect_timeout_ms: 5_000,
                    read_timeout_ms: 15_000,
                    max_parallel_requests: 4,
                    rate_limit_display_name: Some(rate_limit_name),
                    rss_subscription_enabled: None,
                    rss_interval_seconds: None,
                    media_domain_keys: Vec::new(),
                    tag_keys: Vec::new(),
                    fields: Vec::new(),
                }],
                secrets: Vec::new(),
            },
        )
        .await
        .expect_err("restore should fail when indexers reference a missing routing policy");
    assert_eq!(
        missing_routing_error.kind(),
        IndexerBackupServiceErrorKind::Invalid
    );
    assert_eq!(
        missing_routing_error.code(),
        Some("routing_policy_reference_missing")
    );
    Ok(())
}

#[tokio::test]
async fn missing_indexer_operations_surface_not_found_across_runtime_views() -> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let missing_indexer_public_id = Uuid::new_v4();
    assert_missing_indexer_cf_and_connectivity_errors(&service, missing_indexer_public_id).await;
    assert_missing_indexer_runtime_operation_errors(&service, missing_indexer_public_id).await;
    assert_missing_indexer_rss_errors(&service, missing_indexer_public_id).await;
    Ok(())
}

async fn assert_missing_indexer_cf_and_connectivity_errors(
    service: &IndexerService,
    missing_indexer_public_id: Uuid,
) {
    let cf_state_error = service
        .indexer_cf_state_get(SYSTEM_USER_PUBLIC_ID, missing_indexer_public_id)
        .await
        .expect_err("missing indexer should not have cf state");
    assert_eq!(
        cf_state_error.kind(),
        IndexerInstanceServiceErrorKind::NotFound
    );
    assert_eq!(cf_state_error.code(), Some("indexer_not_found"));

    let reset_error = service
        .indexer_cf_state_reset(IndexerCfStateResetParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id: missing_indexer_public_id,
            reason: "manual_reset",
        })
        .await
        .expect_err("missing indexer should not reset cf state");
    assert_eq!(
        reset_error.kind(),
        IndexerInstanceServiceErrorKind::NotFound
    );
    assert_eq!(reset_error.code(), Some("indexer_not_found"));

    let connectivity_error = service
        .indexer_connectivity_profile_get(SYSTEM_USER_PUBLIC_ID, missing_indexer_public_id)
        .await
        .expect_err("missing indexer should not have a connectivity profile");
    assert_eq!(
        connectivity_error.kind(),
        IndexerInstanceServiceErrorKind::NotFound
    );
    assert_eq!(connectivity_error.code(), Some("indexer_not_found"));
}

async fn assert_missing_indexer_runtime_operation_errors(
    service: &IndexerService,
    missing_indexer_public_id: Uuid,
) {
    let reputation_error = service
        .indexer_source_reputation_list(IndexerSourceReputationListParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id: missing_indexer_public_id,
            window_key: Some("24h"),
            limit: Some(5),
        })
        .await
        .expect_err("missing indexer should not list reputation windows");
    assert_eq!(
        reputation_error.kind(),
        IndexerInstanceServiceErrorKind::NotFound
    );
    assert_eq!(reputation_error.code(), Some("indexer_not_found"));

    let health_event_error = service
        .indexer_health_event_list(IndexerHealthEventListParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id: missing_indexer_public_id,
            limit: Some(5),
        })
        .await
        .expect_err("missing indexer should not list health events");
    assert_eq!(
        health_event_error.kind(),
        IndexerInstanceServiceErrorKind::NotFound
    );
    assert_eq!(health_event_error.code(), Some("indexer_not_found"));

    let prepare_error = service
        .indexer_instance_test_prepare(SYSTEM_USER_PUBLIC_ID, missing_indexer_public_id)
        .await
        .expect_err("missing indexer should not prepare test execution");
    assert_eq!(
        prepare_error.kind(),
        IndexerInstanceServiceErrorKind::NotFound
    );
    assert_eq!(prepare_error.code(), Some("indexer_not_found"));

    let finalize_error = service
        .indexer_instance_test_finalize(IndexerInstanceTestFinalizeParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id: missing_indexer_public_id,
            ok: false,
            error_class: Some("timeout"),
            error_code: Some("connect_timeout"),
            detail: Some("missing indexer"),
            result_count: Some(0),
        })
        .await
        .expect_err("missing indexer should not finalize test execution");
    assert_eq!(
        finalize_error.kind(),
        IndexerInstanceServiceErrorKind::NotFound
    );
    assert_eq!(finalize_error.code(), Some("indexer_not_found"));
}

async fn assert_missing_indexer_rss_errors(
    service: &IndexerService,
    missing_indexer_public_id: Uuid,
) {
    let rss_subscription_error = service
        .indexer_rss_subscription_get(SYSTEM_USER_PUBLIC_ID, missing_indexer_public_id)
        .await
        .expect_err("missing indexer should not have rss subscription state");
    assert_eq!(
        rss_subscription_error.kind(),
        IndexerInstanceServiceErrorKind::NotFound
    );
    assert_eq!(rss_subscription_error.code(), Some("indexer_not_found"));

    let rss_subscription_set_error = service
        .indexer_rss_subscription_set(IndexerRssSubscriptionParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id: missing_indexer_public_id,
            is_enabled: true,
            interval_seconds: Some(900),
        })
        .await
        .expect_err("missing indexer should not update rss subscription state");
    assert_eq!(
        rss_subscription_set_error.kind(),
        IndexerInstanceServiceErrorKind::NotFound
    );
    assert_eq!(rss_subscription_set_error.code(), Some("indexer_not_found"));

    let rss_seen_list_error = service
        .indexer_rss_seen_list(IndexerRssSeenListParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id: missing_indexer_public_id,
            limit: Some(10),
        })
        .await
        .expect_err("missing indexer should not list seen rss items");
    assert_eq!(
        rss_seen_list_error.kind(),
        IndexerInstanceServiceErrorKind::NotFound
    );
    assert_eq!(rss_seen_list_error.code(), Some("indexer_not_found"));

    let rss_seen_mark_error = service
        .indexer_rss_seen_mark(IndexerRssSeenMarkParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            indexer_instance_public_id: missing_indexer_public_id,
            item_guid: Some("guid-404"),
            infohash_v1: None,
            infohash_v2: None,
            magnet_hash: None,
        })
        .await
        .expect_err("missing indexer should not mark rss items as seen");
    assert_eq!(
        rss_seen_mark_error.kind(),
        IndexerInstanceServiceErrorKind::NotFound
    );
    assert_eq!(rss_seen_mark_error.code(), Some("indexer_not_found"));
}

struct TorznabDownloadPrepareFixture {
    torznab_instance_public_id: Uuid,
    canonical_torrent_source_public_id: Uuid,
}

#[tokio::test]
async fn policy_reorder_and_torznab_download_prepare_cover_operator_contracts() -> anyhow::Result<()>
{
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };
    reorder_policy_sets_for_operator_contracts(&service).await?;
    let fixture = create_torznab_download_prepare_fixture(&service).await?;
    assert_torznab_download_prepare_contracts(&service, &fixture).await?;
    Ok(())
}

async fn reorder_policy_sets_for_operator_contracts(
    service: &IndexerService,
) -> anyhow::Result<()> {
    let first_policy_name = unique_name("Policy A");
    let second_policy_name = unique_name("Policy B");
    let first_policy_public_id = service
        .policy_set_create(
            SYSTEM_USER_PUBLIC_ID,
            &first_policy_name,
            "user",
            Some(false),
        )
        .await
        .unwrap_or_else(|err| {
            panic!(
                "policy_set_create first policy failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    let second_policy_public_id = service
        .policy_set_create(
            SYSTEM_USER_PUBLIC_ID,
            &second_policy_name,
            "user",
            Some(false),
        )
        .await
        .unwrap_or_else(|err| {
            panic!(
                "policy_set_create second policy failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });

    let mut reordered = service
        .policy_set_list(SYSTEM_USER_PUBLIC_ID)
        .await?
        .into_iter()
        .map(|item| item.policy_set_public_id)
        .filter(|id| *id != first_policy_public_id && *id != second_policy_public_id)
        .collect::<Vec<_>>();
    reordered.insert(0, second_policy_public_id);
    reordered.insert(1, first_policy_public_id);
    service
        .policy_set_reorder(SYSTEM_USER_PUBLIC_ID, &reordered)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "policy_set_reorder failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });

    let reordered_sort_orders = fetch_policy_sort_orders(
        service.config.pool(),
        first_policy_public_id,
        second_policy_public_id,
    )
    .await?;
    assert!(
        reordered_sort_orders.1 < reordered_sort_orders.0,
        "second policy should sort ahead of first policy after reorder"
    );

    query(include_str!("sql/disable_policy_sets.sql"))
        .execute(service.config.pool())
        .await?;
    Ok(())
}

async fn create_torznab_download_prepare_fixture(
    service: &IndexerService,
) -> anyhow::Result<TorznabDownloadPrepareFixture> {
    let definition_slug = unique_name("torznab-download-indexer");
    let definition_name = unique_name("Torznab Download Indexer");
    import_operator_definition(service, &definition_slug, &definition_name).await?;
    let fixture = seed_operator_indexer_fixture(service, &definition_slug).await?;
    let search_profile_public_id = service
        .search_profile_create(
            SYSTEM_USER_PUBLIC_ID,
            &unique_name("Torznab Download Profile"),
            Some(false),
            Some(50),
            Some("movies"),
            None,
        )
        .await?;
    let torznab_credentials = service
        .torznab_instance_create(
            SYSTEM_USER_PUBLIC_ID,
            search_profile_public_id,
            &unique_name("Torznab Download Feed"),
        )
        .await?;

    let request = search_request_create(
        service.config.pool(),
        &SearchRequestCreateInput {
            actor_user_public_id: None,
            query_text: "torznab download",
            query_type: "free_text",
            torznab_mode: Some("generic"),
            requested_media_domain_key: Some("tv"),
            page_size: Some(50),
            search_profile_public_id: Some(search_profile_public_id),
            request_policy_set_public_id: None,
            season_number: None,
            episode_number: None,
            identifier_types: None,
            identifier_values: None,
            torznab_cat_ids: None,
        },
    )
    .await?;

    let ingested = search_result_ingest(
        service.config.pool(),
        &SearchResultIngestInput {
            search_request_public_id: request.search_request_public_id,
            indexer_instance_public_id: fixture.indexer_instance_public_id,
            source_guid: Some("torznab-download-source"),
            details_url: Some("https://example.test/details/1"),
            download_url: Some("https://example.test/download/1"),
            magnet_uri: Some("magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567"),
            title_raw: "Torznab Download Result",
            size_bytes: Some(1024_i64 * 1024_i64),
            infohash_v1: Some("0123456789abcdef0123456789abcdef01234567"),
            infohash_v2: None,
            magnet_hash: None,
            seeders: Some(10),
            leechers: Some(2),
            published_at: None,
            uploader: Some("uploader-a"),
            observed_at: Utc::now(),
            attr_keys: None,
            attr_types: None,
            attr_value_text: None,
            attr_value_int: None,
            attr_value_bigint: None,
            attr_value_numeric: None,
            attr_value_bool: None,
            attr_value_uuid: None,
        },
    )
    .await
    .unwrap_or_else(|err| {
        panic!(
            "search_result_ingest failed: detail={:?} sqlstate={:?}",
            err.database_detail(),
            err.database_code()
        )
    });
    Ok(TorznabDownloadPrepareFixture {
        torznab_instance_public_id: torznab_credentials.torznab_instance_public_id,
        canonical_torrent_source_public_id: ingested.canonical_torrent_source_public_id,
    })
}

async fn assert_torznab_download_prepare_contracts(
    service: &IndexerService,
    fixture: &TorznabDownloadPrepareFixture,
) -> anyhow::Result<()> {
    let redirect = service
        .torznab_download_prepare(
            fixture.torznab_instance_public_id,
            fixture.canonical_torrent_source_public_id,
        )
        .await?;
    assert_eq!(
        redirect.as_deref(),
        Some("magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567")
    );

    let missing_instance = service
        .torznab_download_prepare(Uuid::new_v4(), fixture.canonical_torrent_source_public_id)
        .await
        .expect_err("missing torznab instance should fail");
    assert_eq!(missing_instance.kind(), TorznabAccessErrorKind::NotFound);
    assert_eq!(missing_instance.code(), Some("torznab_instance_not_found"));
    Ok(())
}

#[tokio::test]
async fn secret_create_rotate_revoke_roundtrip() -> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let secret_id = service
        .secret_create(SYSTEM_USER_PUBLIC_ID, "api_key", "initial-value")
        .await
        .unwrap_or_else(|err| {
            panic!(
                "secret_create failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    let rotated = service
        .secret_rotate(SYSTEM_USER_PUBLIC_ID, secret_id, "rotated-value")
        .await
        .unwrap_or_else(|err| {
            panic!(
                "secret_rotate failed: kind={:?} code={:?}",
                err.kind(),
                err.code()
            )
        });
    assert_eq!(rotated, secret_id);

    service
        .secret_revoke(SYSTEM_USER_PUBLIC_ID, secret_id)
        .await?;

    let err = service
        .secret_rotate(SYSTEM_USER_PUBLIC_ID, secret_id, "after-revoke")
        .await
        .unwrap_err();
    assert_eq!(err.kind(), SecretServiceErrorKind::NotFound);
    Ok(())
}

#[tokio::test]
async fn cardigann_definition_import_roundtrip() -> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let upstream_slug = unique_name("cardigann-app");
    let display_name = unique_name("Cardigann Import");
    let yaml = format!(
        "id: cardigann-app-{upstream_slug}\nname: Cardigann Import {display_name}\nsettings:\n  - name: apiKey\n    label: API key\n    type: apikey\n    required: true\n  - name: sort\n    type: select\n    options:\n      - value: seeders\n        label: Seeders\n      - date\n",
    );

    let response = service
        .indexer_definition_import_cardigann(SYSTEM_USER_PUBLIC_ID, &yaml, Some(false))
        .await?;

    assert_eq!(response.definition.upstream_source, "cardigann");
    assert_eq!(response.definition.engine, "cardigann");
    assert_eq!(response.field_count, 2);
    assert_eq!(response.option_count, 2);

    let definitions = service
        .indexer_definition_list(SYSTEM_USER_PUBLIC_ID)
        .await?;
    let imported = definitions
        .iter()
        .find(|definition| definition.upstream_slug == response.definition.upstream_slug)
        .expect("imported definition should be listed");
    assert_eq!(imported.display_name, response.definition.display_name);
    Ok(())
}

#[tokio::test]
async fn tag_crud_roundtrip_and_reference_validation() -> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let tag_key = unique_name("favorites");
    let display_name = unique_name("Favorites");
    let updated_name = unique_name("Favorites Updated");

    let tag_public_id = service
        .tag_create(SYSTEM_USER_PUBLIC_ID, &tag_key, &display_name)
        .await?;
    let list = service.tag_list(SYSTEM_USER_PUBLIC_ID).await?;
    let created = list
        .iter()
        .find(|tag| tag.tag_public_id == tag_public_id)
        .expect("created tag should be listed");
    assert_eq!(created.tag_key, tag_key);
    assert_eq!(created.display_name, display_name);

    let updated_public_id = service
        .tag_update(SYSTEM_USER_PUBLIC_ID, None, Some(&tag_key), &updated_name)
        .await?;
    assert_eq!(updated_public_id, tag_public_id);

    let list = service.tag_list(SYSTEM_USER_PUBLIC_ID).await?;
    let updated = list
        .iter()
        .find(|tag| tag.tag_public_id == tag_public_id)
        .expect("updated tag should remain listed");
    assert_eq!(updated.display_name, updated_name);

    let err = service
        .tag_update(SYSTEM_USER_PUBLIC_ID, None, None, &updated_name)
        .await
        .expect_err("missing tag reference should fail");
    assert_eq!(err.kind(), TagServiceErrorKind::Invalid);
    assert_eq!(err.code(), Some("tag_reference_missing"));

    service
        .tag_delete(SYSTEM_USER_PUBLIC_ID, Some(tag_public_id), None)
        .await?;

    let list = service.tag_list(SYSTEM_USER_PUBLIC_ID).await?;
    assert!(list.iter().all(|tag| tag.tag_public_id != tag_public_id));
    Ok(())
}

#[tokio::test]
async fn health_notification_hook_crud_and_validation_roundtrip() -> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };
    let fixture = create_health_notification_hooks(&service).await?;
    assert_health_notification_hook_inventory(&service, &fixture).await?;
    assert_health_notification_hook_update_and_delete(&service, &fixture).await?;
    Ok(())
}

struct HealthNotificationHookFixture {
    webhook_public_id: Uuid,
    email_public_id: Uuid,
    webhook_name: String,
    updated_name: String,
}

async fn create_health_notification_hooks(
    service: &IndexerService,
) -> anyhow::Result<HealthNotificationHookFixture> {
    let webhook_name = unique_name("Pager webhook");
    let email_name = unique_name("Ops inbox");
    let updated_name = unique_name("Pager escalation");
    let webhook_public_id = service
        .indexer_health_notification_hook_create(
            SYSTEM_USER_PUBLIC_ID,
            "webhook",
            &webhook_name,
            "failing",
            Some("https://hooks.example.test/indexers"),
            None,
        )
        .await?;
    let email_public_id = service
        .indexer_health_notification_hook_create(
            SYSTEM_USER_PUBLIC_ID,
            "email",
            &email_name,
            "degraded",
            None,
            Some("ops@example.test"),
        )
        .await?;
    Ok(HealthNotificationHookFixture {
        webhook_public_id,
        email_public_id,
        webhook_name,
        updated_name,
    })
}

async fn assert_health_notification_hook_inventory(
    service: &IndexerService,
    fixture: &HealthNotificationHookFixture,
) -> anyhow::Result<()> {
    let list = service
        .indexer_health_notification_hook_list(SYSTEM_USER_PUBLIC_ID)
        .await?;
    assert_eq!(list.len(), 2);
    assert!(list.iter().any(|hook| {
        hook.indexer_health_notification_hook_public_id == fixture.webhook_public_id
            && hook.channel == "webhook"
    }));
    assert!(list.iter().any(|hook| {
        hook.indexer_health_notification_hook_public_id == fixture.email_public_id
            && hook.email.as_deref() == Some("ops@example.test")
    }));

    let webhook = service
        .indexer_health_notification_hook_get(SYSTEM_USER_PUBLIC_ID, fixture.webhook_public_id)
        .await?;
    assert_eq!(webhook.display_name, fixture.webhook_name);
    assert_eq!(
        webhook.webhook_url.as_deref(),
        Some("https://hooks.example.test/indexers")
    );
    Ok(())
}

async fn assert_health_notification_hook_update_and_delete(
    service: &IndexerService,
    fixture: &HealthNotificationHookFixture,
) -> anyhow::Result<()> {
    let updated_public_id = service
        .indexer_health_notification_hook_update(HealthNotificationHookUpdateParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            hook_public_id: fixture.webhook_public_id,
            display_name: Some(&fixture.updated_name),
            status_threshold: Some("quarantined"),
            webhook_url: Some("https://hooks.example.test/escalation"),
            email: None,
            is_enabled: Some(false),
        })
        .await?;
    assert_eq!(updated_public_id, fixture.webhook_public_id);

    let webhook = service
        .indexer_health_notification_hook_get(SYSTEM_USER_PUBLIC_ID, fixture.webhook_public_id)
        .await?;
    assert_eq!(webhook.display_name, fixture.updated_name);
    assert_eq!(webhook.status_threshold, "quarantined");
    assert_eq!(
        webhook.webhook_url.as_deref(),
        Some("https://hooks.example.test/escalation")
    );
    assert!(!webhook.is_enabled);

    let err = service
        .indexer_health_notification_hook_update(HealthNotificationHookUpdateParams {
            actor_user_public_id: SYSTEM_USER_PUBLIC_ID,
            hook_public_id: fixture.email_public_id,
            display_name: None,
            status_threshold: None,
            webhook_url: Some("https://hooks.example.test/wrong"),
            email: None,
            is_enabled: None,
        })
        .await
        .expect_err("email hook update should reject webhook payload");
    assert_eq!(err.kind(), HealthNotificationServiceErrorKind::Invalid);
    assert_eq!(err.code(), Some("channel_payload_mismatch"));

    service
        .indexer_health_notification_hook_delete(SYSTEM_USER_PUBLIC_ID, fixture.email_public_id)
        .await?;
    let list = service
        .indexer_health_notification_hook_list(SYSTEM_USER_PUBLIC_ID)
        .await?;
    assert_eq!(list.len(), 1);
    assert_eq!(
        list[0].indexer_health_notification_hook_public_id,
        fixture.webhook_public_id
    );

    service
        .indexer_health_notification_hook_delete(SYSTEM_USER_PUBLIC_ID, fixture.webhook_public_id)
        .await?;
    let err = service
        .indexer_health_notification_hook_get(SYSTEM_USER_PUBLIC_ID, fixture.webhook_public_id)
        .await
        .expect_err("deleted hook should not be returned");
    assert_eq!(err.kind(), HealthNotificationServiceErrorKind::NotFound);
    assert_eq!(err.code(), Some("hook_not_found"));
    Ok(())
}

#[tokio::test]
async fn routing_and_rate_limit_policy_roundtrip() -> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };
    let fixture = create_routing_policy_roundtrip_fixture(&service).await?;
    assert_routing_policy_roundtrip_detail(&service, &fixture).await?;
    assert_routing_policy_roundtrip_inventory(&service, &fixture).await?;
    assert_routing_policy_roundtrip_rate_limit_lifecycle(&service, &fixture).await?;
    Ok(())
}

struct RoutingPolicyRoundtripFixture {
    routing_policy_public_id: Uuid,
    rate_limit_policy_public_id: Uuid,
    secret_public_id: Uuid,
    routing_name: String,
    policy_name: String,
    updated_policy_name: String,
}

async fn create_routing_policy_roundtrip_fixture(
    service: &IndexerService,
) -> anyhow::Result<RoutingPolicyRoundtripFixture> {
    let routing_name = unique_name("Proxy routing");
    let policy_name = unique_name("Proxy rate");
    let updated_policy_name = unique_name("Proxy rate updated");
    let routing_policy_public_id = service
        .routing_policy_create(SYSTEM_USER_PUBLIC_ID, &routing_name, "http_proxy")
        .await?;

    service
        .routing_policy_set_param(
            SYSTEM_USER_PUBLIC_ID,
            routing_policy_public_id,
            "proxy_host",
            Some("proxy.internal"),
            None,
            None,
        )
        .await?;
    service
        .routing_policy_set_param(
            SYSTEM_USER_PUBLIC_ID,
            routing_policy_public_id,
            "proxy_port",
            None,
            Some(8443),
            None,
        )
        .await?;

    let secret_public_id = service
        .secret_create(SYSTEM_USER_PUBLIC_ID, "password", "proxy-pass")
        .await?;
    service
        .routing_policy_bind_secret(
            SYSTEM_USER_PUBLIC_ID,
            routing_policy_public_id,
            "http_proxy_auth",
            secret_public_id,
        )
        .await?;

    let rate_limit_policy_public_id = service
        .rate_limit_policy_create(SYSTEM_USER_PUBLIC_ID, &policy_name, 90, 15, 3)
        .await?;
    service
        .routing_policy_set_rate_limit_policy(
            SYSTEM_USER_PUBLIC_ID,
            routing_policy_public_id,
            Some(rate_limit_policy_public_id),
        )
        .await?;
    Ok(RoutingPolicyRoundtripFixture {
        routing_policy_public_id,
        rate_limit_policy_public_id,
        secret_public_id,
        routing_name,
        policy_name,
        updated_policy_name,
    })
}

async fn assert_routing_policy_roundtrip_detail(
    service: &IndexerService,
    fixture: &RoutingPolicyRoundtripFixture,
) -> anyhow::Result<()> {
    let detail = service
        .routing_policy_get(SYSTEM_USER_PUBLIC_ID, fixture.routing_policy_public_id)
        .await?;
    assert_eq!(detail.display_name, fixture.routing_name);
    assert_eq!(detail.mode, "http_proxy");
    assert_eq!(
        detail.rate_limit_policy_public_id,
        Some(fixture.rate_limit_policy_public_id)
    );
    assert_eq!(
        detail.rate_limit_display_name.as_deref(),
        Some(fixture.policy_name.as_str())
    );
    assert_eq!(detail.rate_limit_requests_per_minute, Some(90));
    assert!(detail.parameters.iter().any(|parameter| {
        parameter.param_key == "proxy_host"
            && parameter.value_plain.as_deref() == Some("proxy.internal")
    }));
    assert!(detail.parameters.iter().any(|parameter| {
        parameter.param_key == "proxy_port" && parameter.value_int == Some(8443)
    }));
    assert!(detail.parameters.iter().any(|parameter| {
        parameter.param_key == "http_proxy_auth"
            && parameter.secret_public_id == Some(fixture.secret_public_id)
            && parameter.secret_binding_name.as_deref() == Some("proxy_password")
    }));
    Ok(())
}

async fn assert_routing_policy_roundtrip_inventory(
    service: &IndexerService,
    fixture: &RoutingPolicyRoundtripFixture,
) -> anyhow::Result<()> {
    let routing_inventory = service.routing_policy_list(SYSTEM_USER_PUBLIC_ID).await?;
    let inventory_item = routing_inventory
        .iter()
        .find(|policy| policy.routing_policy_public_id == fixture.routing_policy_public_id)
        .expect("routing policy should be listed");
    assert_eq!(inventory_item.display_name, fixture.routing_name);
    assert_eq!(inventory_item.parameter_count, 4);
    assert_eq!(inventory_item.secret_binding_count, 1);
    assert_eq!(
        inventory_item.rate_limit_policy_public_id,
        Some(fixture.rate_limit_policy_public_id)
    );

    let rate_limit_policies = service
        .rate_limit_policy_list(SYSTEM_USER_PUBLIC_ID)
        .await?;
    let created_policy = rate_limit_policies
        .iter()
        .find(|policy| policy.rate_limit_policy_public_id == fixture.rate_limit_policy_public_id)
        .expect("rate-limit policy should be listed");
    assert_eq!(created_policy.display_name, fixture.policy_name);
    assert_eq!(created_policy.requests_per_minute, 90);
    assert_eq!(created_policy.burst, 15);
    assert_eq!(created_policy.concurrent_requests, 3);
    assert!(!created_policy.is_system);
    Ok(())
}

async fn assert_routing_policy_roundtrip_rate_limit_lifecycle(
    service: &IndexerService,
    fixture: &RoutingPolicyRoundtripFixture,
) -> anyhow::Result<()> {
    service
        .rate_limit_policy_update(
            SYSTEM_USER_PUBLIC_ID,
            fixture.rate_limit_policy_public_id,
            Some(&fixture.updated_policy_name),
            Some(120),
            Some(20),
            Some(4),
        )
        .await?;
    let rate_limit_policies = service
        .rate_limit_policy_list(SYSTEM_USER_PUBLIC_ID)
        .await?;
    let updated_policy = rate_limit_policies
        .iter()
        .find(|policy| policy.rate_limit_policy_public_id == fixture.rate_limit_policy_public_id)
        .expect("updated rate-limit policy should be listed");
    assert_eq!(updated_policy.display_name, fixture.updated_policy_name);
    assert_eq!(updated_policy.requests_per_minute, 120);
    assert_eq!(updated_policy.burst, 20);
    assert_eq!(updated_policy.concurrent_requests, 4);

    service
        .routing_policy_set_rate_limit_policy(
            SYSTEM_USER_PUBLIC_ID,
            fixture.routing_policy_public_id,
            None,
        )
        .await?;
    let detail = service
        .routing_policy_get(SYSTEM_USER_PUBLIC_ID, fixture.routing_policy_public_id)
        .await?;
    assert_eq!(detail.rate_limit_policy_public_id, None);

    let err = service
        .indexer_instance_set_rate_limit_policy(
            SYSTEM_USER_PUBLIC_ID,
            Uuid::new_v4(),
            Some(fixture.rate_limit_policy_public_id),
        )
        .await
        .expect_err("missing indexer instance should fail");
    assert_eq!(err.kind(), RateLimitPolicyServiceErrorKind::NotFound);
    assert_eq!(err.code(), Some("indexer_not_found"));

    service
        .rate_limit_policy_soft_delete(SYSTEM_USER_PUBLIC_ID, fixture.rate_limit_policy_public_id)
        .await?;
    let rate_limit_policies = service
        .rate_limit_policy_list(SYSTEM_USER_PUBLIC_ID)
        .await?;
    assert!(rate_limit_policies.iter().all(|policy| {
        policy.rate_limit_policy_public_id != fixture.rate_limit_policy_public_id
    }));
    Ok(())
}

#[tokio::test]
async fn secret_create_rejects_empty_value() -> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let err = service
        .secret_create(SYSTEM_USER_PUBLIC_ID, "api_key", "")
        .await
        .unwrap_err();
    assert_eq!(err.kind(), SecretServiceErrorKind::Invalid);
    Ok(())
}

#[tokio::test]
async fn torznab_instance_create_requires_profile() -> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let err = service
        .torznab_instance_create(SYSTEM_USER_PUBLIC_ID, Uuid::new_v4(), "Torznab")
        .await
        .unwrap_err();
    assert_eq!(err.kind(), TorznabInstanceServiceErrorKind::NotFound);
    Ok(())
}

#[tokio::test]
async fn torznab_instance_rotate_requires_instance() -> anyhow::Result<()> {
    let Ok((service, _db)) = build_service().await else {
        return Ok(());
    };

    let err = service
        .torznab_instance_rotate_key(SYSTEM_USER_PUBLIC_ID, Uuid::new_v4())
        .await
        .unwrap_err();
    assert_eq!(err.kind(), TorznabInstanceServiceErrorKind::NotFound);
    Ok(())
}
