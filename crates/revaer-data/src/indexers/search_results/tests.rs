use super::*;
use crate::DataError;
use crate::indexers::search_requests::{SearchRequestCreateInput, search_request_create};
use crate::indexers::search_requests::{
    search_indexer_run_mark_finished, search_indexer_run_mark_started,
};
use chrono::Duration;
use sqlx::PgConnection;

async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
    crate::indexers::setup_indexer_db("indexer tests").await
}

async fn insert_indexer_instance(pool: &PgPool) -> anyhow::Result<Uuid> {
    let definition_id: i64 = sqlx::query_scalar(
        "INSERT INTO indexer_definition (
            upstream_source,
            upstream_slug,
            display_name,
            protocol,
            engine,
            schema_version,
            definition_hash,
            is_deprecated
        )
        VALUES ($1::upstream_source, $2, $3, $4::protocol, $5::engine, $6, $7, $8)
        RETURNING indexer_definition_id",
    )
    .bind("prowlarr_indexers")
    .bind(format!("search-result-{}", Uuid::new_v4().simple()))
    .bind("Search Result Definition")
    .bind("torrent")
    .bind("torznab")
    .bind(1_i32)
    .bind("d".repeat(64))
    .bind(false)
    .fetch_one(pool)
    .await?;

    let instance_public_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO indexer_instance (
            indexer_instance_public_id,
            indexer_definition_id,
            display_name,
            is_enabled,
            migration_state,
            enable_rss,
            enable_automatic_search,
            enable_interactive_search,
            priority,
            trust_tier_key,
            created_by_user_id,
            updated_by_user_id
        )
        VALUES (
            $1,
            $2,
            $3,
            TRUE,
            $4::indexer_instance_migration_state,
            TRUE,
            TRUE,
            TRUE,
            100,
            'public',
            0,
            0
        )",
    )
    .bind(instance_public_id)
    .bind(definition_id)
    .bind("Search Result Instance")
    .bind("ready")
    .execute(pool)
    .await?;
    Ok(instance_public_id)
}

async fn setup_ingest_scope_with_page_size(
    pool: &PgPool,
    page_size: i32,
) -> anyhow::Result<(Uuid, Uuid)> {
    sqlx::query("UPDATE policy_set SET is_enabled = FALSE")
        .execute(pool)
        .await?;
    let indexer_instance_public_id = insert_indexer_instance(pool).await?;
    let create = search_request_create(
        pool,
        &SearchRequestCreateInput {
            actor_user_public_id: None,
            query_text: "result ingest",
            query_type: "free_text",
            torznab_mode: Some("generic"),
            requested_media_domain_key: None,
            page_size: Some(page_size),
            search_profile_public_id: None,
            request_policy_set_public_id: None,
            season_number: None,
            episode_number: None,
            identifier_types: None,
            identifier_values: None,
            torznab_cat_ids: None,
        },
    )
    .await?;
    let search_request_public_id = create.search_request_public_id;
    Ok((search_request_public_id, indexer_instance_public_id))
}

async fn setup_ingest_scope(pool: &PgPool) -> anyhow::Result<(Uuid, Uuid)> {
    setup_ingest_scope_with_page_size(pool, 50).await
}

async fn insert_request_policy_set_with_drop_title_rule(
    pool: &PgPool,
    title_pattern: &str,
) -> anyhow::Result<Uuid> {
    let policy_set_public_id = Uuid::new_v4();
    let policy_set_id: i64 = sqlx::query_scalar(
        "INSERT INTO policy_set (
            policy_set_public_id,
            user_id,
            display_name,
            scope,
            is_enabled,
            sort_order,
            is_auto_created,
            created_for_search_request_id,
            created_by_user_id,
            updated_by_user_id
        )
        VALUES ($1, NULL, $2, 'request', TRUE, 1000, FALSE, NULL, 0, 0)
        RETURNING policy_set_id",
    )
    .bind(policy_set_public_id)
    .bind(format!("Request Policy {}", policy_set_public_id.simple()))
    .fetch_one(pool)
    .await?;
    sqlx::query(
        "INSERT INTO policy_rule (
            policy_set_id,
            policy_rule_public_id,
            rule_type,
            match_field,
            match_operator,
            sort_order,
            match_value_text,
            action,
            severity,
            is_case_insensitive,
            rationale,
            created_by_user_id,
            updated_by_user_id
        )
        VALUES (
            $1,
            $2,
            'block_title_regex',
            'title',
            'regex',
            1000,
            $3,
            'drop_canonical',
            'hard',
            TRUE,
            $4,
            0,
            0
        )",
    )
    .bind(policy_set_id)
    .bind(Uuid::new_v4())
    .bind(title_pattern)
    .bind("Drop blocked title in request scope")
    .execute(pool)
    .await?;
    Ok(policy_set_public_id)
}

async fn setup_ingest_scope_with_request_drop_policy(
    pool: &PgPool,
    title_pattern: &str,
) -> anyhow::Result<(Uuid, Uuid, Option<&'static str>)> {
    sqlx::query("UPDATE policy_set SET is_enabled = FALSE")
        .execute(pool)
        .await?;
    let request_policy_set_public_id =
        insert_request_policy_set_with_drop_title_rule(pool, title_pattern).await?;
    let indexer_instance_public_id = insert_indexer_instance(pool).await?;
    let create = search_request_create(
        pool,
        &SearchRequestCreateInput {
            actor_user_public_id: None,
            query_text: "blocked request",
            query_type: "free_text",
            torznab_mode: Some("generic"),
            requested_media_domain_key: None,
            page_size: Some(25),
            search_profile_public_id: None,
            request_policy_set_public_id: Some(request_policy_set_public_id),
            season_number: None,
            episode_number: None,
            identifier_types: None,
            identifier_values: None,
            torznab_cat_ids: None,
        },
    )
    .await?;
    Ok((
        create.search_request_public_id,
        indexer_instance_public_id,
        Some("blocked-guid-1"),
    ))
}

async fn fetch_dropped_source_audit_row(
    pool: &PgPool,
    search_request_public_id: Uuid,
    source_guid: Option<&str>,
    canonical_torrent_public_id: Uuid,
    canonical_torrent_source_public_id: Uuid,
) -> anyhow::Result<(bool, String, bool, bool)> {
    sqlx::query_as(
        "SELECT
            cs.is_dropped,
            fd.decision::text,
            fd.observation_id IS NOT NULL,
            EXISTS (
                SELECT 1
                FROM search_request_source_observation obs
                JOIN search_request req
                  ON req.search_request_id = obs.search_request_id
                WHERE req.search_request_public_id = $1
                  AND obs.source_guid = $2
                  AND obs.canonical_torrent_id = fd.canonical_torrent_id
                  AND obs.canonical_torrent_source_id = fd.canonical_torrent_source_id
            )
         FROM search_request sr
         JOIN canonical_torrent c
           ON c.canonical_torrent_public_id = $3
         JOIN canonical_torrent_source cs_source
           ON cs_source.canonical_torrent_source_public_id = $4
         JOIN canonical_torrent_source_context_score cs
           ON cs.context_key_type = 'search_request'
          AND cs.context_key_id = sr.search_request_id
          AND cs.canonical_torrent_id = c.canonical_torrent_id
          AND cs.canonical_torrent_source_id = cs_source.canonical_torrent_source_id
         JOIN search_filter_decision fd
           ON fd.search_request_id = sr.search_request_id
          AND fd.canonical_torrent_id = c.canonical_torrent_id
          AND fd.canonical_torrent_source_id = cs_source.canonical_torrent_source_id
         WHERE sr.search_request_public_id = $1
         ORDER BY fd.search_filter_decision_id ASC
         LIMIT 1",
    )
    .bind(search_request_public_id)
    .bind(source_guid)
    .bind(canonical_torrent_public_id)
    .bind(canonical_torrent_source_public_id)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

fn make_ingest_input<'a>(
    search_request_public_id: Uuid,
    indexer_instance_public_id: Uuid,
    observed_at: chrono::DateTime<Utc>,
) -> SearchResultIngestInput<'a> {
    SearchResultIngestInput {
        search_request_public_id,
        indexer_instance_public_id,
        source_guid: Some("guid-1"),
        details_url: Some("https://example.com/details/1"),
        download_url: Some("https://example.com/download/1"),
        magnet_uri: Some("magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567"),
        title_raw: "Example Release",
        size_bytes: Some(1024_i64 * 1024_i64 * 1024_i64),
        infohash_v1: Some("0123456789abcdef0123456789abcdef01234567"),
        infohash_v2: None,
        magnet_hash: None,
        seeders: Some(10),
        leechers: Some(2),
        published_at: None,
        uploader: Some("uploader-a"),
        observed_at,
        attr_keys: None,
        attr_types: None,
        attr_value_text: None,
        attr_value_int: None,
        attr_value_bigint: None,
        attr_value_numeric: None,
        attr_value_bool: None,
        attr_value_uuid: None,
    }
}

async fn search_result_ingest_on_connection(
    connection: &mut PgConnection,
    input: &SearchResultIngestInput<'_>,
) -> anyhow::Result<SearchResultIngestRow> {
    let row = sqlx::query_as(SEARCH_RESULT_INGEST_CALL)
        .bind(input.search_request_public_id)
        .bind(input.indexer_instance_public_id)
        .bind(input.source_guid)
        .bind(input.details_url)
        .bind(input.download_url)
        .bind(input.magnet_uri)
        .bind(input.title_raw)
        .bind(input.size_bytes)
        .bind(input.infohash_v1)
        .bind(input.infohash_v2)
        .bind(input.magnet_hash)
        .bind(input.seeders)
        .bind(input.leechers)
        .bind(input.published_at)
        .bind(input.uploader)
        .bind(input.observed_at)
        .bind(input.attr_keys)
        .bind(input.attr_types)
        .bind(input.attr_value_text)
        .bind(input.attr_value_int)
        .bind(input.attr_value_bigint)
        .bind(input.attr_value_numeric)
        .bind(input.attr_value_bool)
        .bind(input.attr_value_uuid)
        .fetch_one(connection)
        .await?;
    Ok(row)
}
#[tokio::test]
async fn search_result_ingest_requires_request() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let now = Utc::now();
    let input = SearchResultIngestInput {
        search_request_public_id: Uuid::new_v4(),
        indexer_instance_public_id: Uuid::new_v4(),
        source_guid: None,
        details_url: None,
        download_url: None,
        magnet_uri: None,
        title_raw: "Sample",
        size_bytes: None,
        infohash_v1: None,
        infohash_v2: None,
        magnet_hash: None,
        seeders: None,
        leechers: None,
        published_at: None,
        uploader: None,
        observed_at: now,
        attr_keys: None,
        attr_types: None,
        attr_value_text: None,
        attr_value_int: None,
        attr_value_bigint: None,
        attr_value_numeric: None,
        attr_value_bool: None,
        attr_value_uuid: None,
    };

    let err = search_result_ingest(pool, &input).await.unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("search_request_not_found"));
    Ok(())
}

#[tokio::test]
async fn search_result_ingest_rejects_duplicate_attr_keys() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();
    let (search_request_public_id, indexer_instance_public_id) = setup_ingest_scope(pool).await?;
    let now = test_db.now();

    let keys = vec![
        "tracker_category".to_string(),
        "tracker_category".to_string(),
    ];
    let types = vec!["int".to_string(), "int".to_string()];
    let text_values = vec![None::<String>, None::<String>];
    let int_values = vec![Some(2000_i32), Some(2000_i32)];
    let bigint_values = vec![None::<i64>, None::<i64>];
    let numeric_values = vec![None::<String>, None::<String>];
    let bool_values = vec![None::<bool>, None::<bool>];
    let uuid_values = vec![None::<Uuid>, None::<Uuid>];

    let input = SearchResultIngestInput {
        search_request_public_id,
        indexer_instance_public_id,
        source_guid: Some("guid-dup-attrs"),
        details_url: Some("https://example.com/details/dup"),
        download_url: Some("https://example.com/download/dup"),
        magnet_uri: Some("magnet:?xt=urn:btih:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        title_raw: "Duplicate Attr Release",
        size_bytes: Some(2_000_000_000),
        infohash_v1: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        infohash_v2: None,
        magnet_hash: None,
        seeders: Some(5),
        leechers: Some(1),
        published_at: None,
        uploader: None,
        observed_at: now,
        attr_keys: Some(&keys),
        attr_types: Some(&types),
        attr_value_text: Some(&text_values),
        attr_value_int: Some(&int_values),
        attr_value_bigint: Some(&bigint_values),
        attr_value_numeric: Some(&numeric_values),
        attr_value_bool: Some(&bool_values),
        attr_value_uuid: Some(&uuid_values),
    };

    let err = search_result_ingest(pool, &input).await.unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("duplicate_attr_key"));
    Ok(())
}

#[tokio::test]
async fn search_result_ingest_keeps_last_seen_monotonic() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();
    let (search_request_public_id, indexer_instance_public_id) = setup_ingest_scope(pool).await?;
    let now = test_db.now();

    let newer = make_ingest_input(search_request_public_id, indexer_instance_public_id, now);
    let newer_row = search_result_ingest(pool, &newer).await?;
    assert!(newer_row.observation_created);
    assert!(newer_row.durable_source_created);

    let older = make_ingest_input(
        search_request_public_id,
        indexer_instance_public_id,
        now - Duration::minutes(10),
    );
    let older_row = search_result_ingest(pool, &older).await?;
    assert!(!older_row.durable_source_created);

    let (last_seen_at, last_seen_seeders, last_seen_leechers, last_seen_uploader): (
        chrono::DateTime<Utc>,
        Option<i32>,
        Option<i32>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT
            last_seen_at,
            last_seen_seeders,
            last_seen_leechers,
            last_seen_uploader
         FROM canonical_torrent_source
         WHERE canonical_torrent_source_public_id = $1",
    )
    .bind(newer_row.canonical_torrent_source_public_id)
    .fetch_one(pool)
    .await?;

    assert_eq!(last_seen_at, now);
    assert_eq!(last_seen_seeders, Some(10));
    assert_eq!(last_seen_leechers, Some(2));
    assert_eq!(last_seen_uploader.as_deref(), Some("uploader-a"));
    Ok(())
}

#[tokio::test]
async fn search_result_ingest_logs_hash_conflicts_without_overwriting_source_identity()
-> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();
    let (search_request_public_id, indexer_instance_public_id) = setup_ingest_scope(pool).await?;
    let source_guid = "conflict-guid";

    let first_input = SearchResultIngestInput {
        search_request_public_id,
        indexer_instance_public_id,
        source_guid: Some(source_guid),
        details_url: Some("https://example.com/details/conflict"),
        download_url: Some("https://example.com/download/conflict"),
        magnet_uri: None,
        title_raw: "Conflict Release",
        size_bytes: Some(2_048),
        infohash_v1: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        infohash_v2: None,
        magnet_hash: None,
        seeders: Some(5),
        leechers: Some(1),
        published_at: None,
        uploader: Some("uploader-a"),
        observed_at: test_db.now(),
        attr_keys: None,
        attr_types: None,
        attr_value_text: None,
        attr_value_int: None,
        attr_value_bigint: None,
        attr_value_numeric: None,
        attr_value_bool: None,
        attr_value_uuid: None,
    };
    let first_row = search_result_ingest(pool, &first_input).await?;

    let second_input = SearchResultIngestInput {
        observed_at: test_db.now() + Duration::seconds(1),
        infohash_v1: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        ..first_input
    };
    let _second_row = search_result_ingest(pool, &second_input).await?;

    let durable_hash: Option<String> = sqlx::query_scalar(
        "SELECT infohash_v1
         FROM canonical_torrent_source
         WHERE canonical_torrent_source_public_id = $1",
    )
    .bind(first_row.canonical_torrent_source_public_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(
        durable_hash.as_deref(),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );

    let conflict_rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT
            conflict_type::text,
            existing_value,
            incoming_value
         FROM source_metadata_conflict
         WHERE canonical_torrent_source_id = (
             SELECT canonical_torrent_source_id
             FROM canonical_torrent_source
             WHERE canonical_torrent_source_public_id = $1
         )",
    )
    .bind(first_row.canonical_torrent_source_public_id)
    .fetch_all(pool)
    .await?;
    assert!(conflict_rows.contains(&(
        String::from("hash"),
        String::from("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        String::from("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
    )));

    let health_events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM indexer_health_event
         WHERE event_type = 'identity_conflict'
           AND detail = 'hash'",
    )
    .fetch_one(pool)
    .await?;
    assert!(health_events >= 1);
    Ok(())
}

#[tokio::test]
async fn search_result_ingest_rejects_missing_identity() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();
    let (search_request_public_id, indexer_instance_public_id) = setup_ingest_scope(pool).await?;

    let input = SearchResultIngestInput {
        search_request_public_id,
        indexer_instance_public_id,
        source_guid: None,
        details_url: Some("https://example.com/details/no-identity"),
        download_url: Some("https://example.com/download/no-identity"),
        magnet_uri: None,
        title_raw: "No Identity",
        size_bytes: None,
        infohash_v1: None,
        infohash_v2: None,
        magnet_hash: None,
        seeders: Some(1),
        leechers: Some(0),
        published_at: None,
        uploader: None,
        observed_at: test_db.now(),
        attr_keys: None,
        attr_types: None,
        attr_value_text: None,
        attr_value_int: None,
        attr_value_bigint: None,
        attr_value_numeric: None,
        attr_value_bool: None,
        attr_value_uuid: None,
    };

    let err = search_result_ingest(pool, &input).await.unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("insufficient_identity"));
    Ok(())
}

#[tokio::test]
async fn search_result_ingest_uses_title_size_fallback_without_hashes() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();
    let (search_request_public_id, indexer_instance_public_id) = setup_ingest_scope(pool).await?;

    let input = SearchResultIngestInput {
        search_request_public_id,
        indexer_instance_public_id,
        source_guid: None,
        details_url: Some("https://example.com/details/fallback"),
        download_url: Some("https://example.com/download/fallback"),
        magnet_uri: None,
        title_raw: "Fallback Release",
        size_bytes: Some(1_500_000_000),
        infohash_v1: None,
        infohash_v2: None,
        magnet_hash: None,
        seeders: Some(3),
        leechers: Some(1),
        published_at: None,
        uploader: Some("fallback-uploader"),
        observed_at: test_db.now(),
        attr_keys: None,
        attr_types: None,
        attr_value_text: None,
        attr_value_int: None,
        attr_value_bigint: None,
        attr_value_numeric: None,
        attr_value_bool: None,
        attr_value_uuid: None,
    };

    let row = search_result_ingest(pool, &input).await?;
    let identity_strategy: String = sqlx::query_scalar(
        "SELECT identity_strategy::text
         FROM canonical_torrent
         WHERE canonical_torrent_public_id = $1",
    )
    .bind(row.canonical_torrent_public_id)
    .fetch_one(pool)
    .await?;

    assert_eq!(identity_strategy, "title_size_fallback");
    Ok(())
}

#[tokio::test]
async fn search_result_ingest_updates_size_rollup_median_after_three_samples() -> anyhow::Result<()>
{
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();
    let (search_request_public_id, indexer_instance_public_id) = setup_ingest_scope(pool).await?;
    let now = test_db.now();

    let mut input_one =
        make_ingest_input(search_request_public_id, indexer_instance_public_id, now);
    input_one.size_bytes = Some(1_000);
    let mut connection = pool.acquire().await?;
    search_result_ingest_on_connection(&mut connection, &input_one).await?;
    sqlx::query("DISCARD TEMP")
        .execute(&mut *connection)
        .await?;

    let mut input_two = make_ingest_input(
        search_request_public_id,
        indexer_instance_public_id,
        now + Duration::seconds(1),
    );
    input_two.size_bytes = Some(3_000);
    search_result_ingest_on_connection(&mut connection, &input_two).await?;
    sqlx::query("DISCARD TEMP")
        .execute(&mut *connection)
        .await?;

    let mut input_three = make_ingest_input(
        search_request_public_id,
        indexer_instance_public_id,
        now + Duration::seconds(2),
    );
    input_three.size_bytes = Some(2_000);
    let row_three = search_result_ingest_on_connection(&mut connection, &input_three).await?;

    let (sample_count, size_median, size_min, size_max): (i32, i64, i64, i64) = sqlx::query_as(
        "SELECT sample_count, size_median, size_min, size_max
         FROM canonical_size_rollup r
         JOIN canonical_torrent c ON c.canonical_torrent_id = r.canonical_torrent_id
         WHERE c.canonical_torrent_public_id = $1",
    )
    .bind(row_three.canonical_torrent_public_id)
    .fetch_one(pool)
    .await?;

    assert_eq!(sample_count, 3);
    assert_eq!(size_median, 2_000);
    assert_eq!(size_min, 1_000);
    assert_eq!(size_max, 3_000);

    let canonical_size: i64 = sqlx::query_scalar(
        "SELECT size_bytes
         FROM canonical_torrent
         WHERE canonical_torrent_public_id = $1",
    )
    .bind(row_three.canonical_torrent_public_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(canonical_size, 2_000);
    Ok(())
}

#[derive(Clone, Copy)]
struct StreamInputSpec<'a> {
    source_guid: &'a str,
    title_raw: &'a str,
    infohash_v1: &'a str,
    observed_at: DateTime<Utc>,
    seeders: i32,
    size_bytes: i64,
}

fn make_stream_input(
    search_request_public_id: Uuid,
    indexer_instance_public_id: Uuid,
    spec: StreamInputSpec<'_>,
) -> SearchResultIngestInput<'_> {
    SearchResultIngestInput {
        search_request_public_id,
        indexer_instance_public_id,
        source_guid: Some(spec.source_guid),
        details_url: Some("https://example.com/details/stream"),
        download_url: Some("https://example.com/download/stream"),
        magnet_uri: None,
        title_raw: spec.title_raw,
        size_bytes: Some(spec.size_bytes),
        infohash_v1: Some(spec.infohash_v1),
        infohash_v2: None,
        magnet_hash: None,
        seeders: Some(spec.seeders),
        leechers: Some(1),
        published_at: None,
        uploader: Some("streamer"),
        observed_at: spec.observed_at,
        attr_keys: None,
        attr_types: None,
        attr_value_text: None,
        attr_value_int: None,
        attr_value_bigint: None,
        attr_value_numeric: None,
        attr_value_bool: None,
        attr_value_uuid: None,
    }
}

async fn fetch_stream_page_items(
    pool: &PgPool,
    search_request_public_id: Uuid,
) -> anyhow::Result<Vec<(i32, i32, String)>> {
    sqlx::query_as(
        "SELECT
            sp.page_number,
            spi.position,
            ct.title_display
         FROM search_request sr
         JOIN search_page sp
           ON sp.search_request_id = sr.search_request_id
         JOIN search_page_item spi
           ON spi.search_page_id = sp.search_page_id
         JOIN search_request_canonical src
           ON src.search_request_canonical_id = spi.search_request_canonical_id
         JOIN canonical_torrent ct
           ON ct.canonical_torrent_id = src.canonical_torrent_id
         WHERE sr.search_request_public_id = $1
         ORDER BY sp.page_number, spi.position",
    )
    .bind(search_request_public_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

async fn assert_request_finished_and_pages_sealed(
    pool: &PgPool,
    search_request_public_id: Uuid,
) -> anyhow::Result<()> {
    let request_status: (String, Option<DateTime<Utc>>) = sqlx::query_as(
        "SELECT status::text, finished_at
         FROM search_request
         WHERE search_request_public_id = $1",
    )
    .bind(search_request_public_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(request_status.0.as_str(), "finished");
    assert!(request_status.1.is_some());

    let unsealed_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM search_page sp
         JOIN search_request sr
           ON sr.search_request_id = sp.search_request_id
         WHERE sr.search_request_public_id = $1
           AND sp.sealed_at IS NULL",
    )
    .bind(search_request_public_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(unsealed_count, 0);
    Ok(())
}

#[tokio::test]
async fn search_result_ingest_streaming_pages_remain_append_only_and_seal_deterministically()
-> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();
    let (search_request_public_id, indexer_instance_public_id) =
        setup_ingest_scope_with_page_size(pool, 10).await?;

    search_indexer_run_mark_started(pool, search_request_public_id, indexer_instance_public_id)
        .await?;

    let now = test_db.now();
    let mut connection = pool.acquire().await?;
    let mut expected_items: Vec<(i32, i32, String)> = Vec::new();
    for idx in 1..=12 {
        let source_guid = format!("stream-guid-{idx}");
        let title = if idx == 12 {
            String::from("Late High Seeder Release")
        } else {
            format!("Stream Release {idx}")
        };
        let infohash_v1 = format!("{idx:040x}");
        let seeders = if idx == 12 { 1000 } else { idx };
        let input = make_stream_input(
            search_request_public_id,
            indexer_instance_public_id,
            StreamInputSpec {
                source_guid: &source_guid,
                title_raw: &title,
                infohash_v1: &infohash_v1,
                observed_at: now + Duration::seconds(i64::from(idx)),
                seeders,
                size_bytes: 1_000_000_000 + i64::from(idx),
            },
        );
        let _ = search_result_ingest_on_connection(&mut connection, &input).await?;
        if idx < 12 {
            sqlx::query("DISCARD TEMP")
                .execute(&mut *connection)
                .await?;
        }

        let page_number = ((idx - 1) / 10) + 1;
        let position = ((idx - 1) % 10) + 1;
        expected_items.push((page_number, position, title));
    }

    let page_items = fetch_stream_page_items(pool, search_request_public_id).await?;
    assert_eq!(page_items, expected_items);

    search_indexer_run_mark_finished(
        pool,
        search_request_public_id,
        indexer_instance_public_id,
        3,
        3,
        3,
    )
    .await?;

    assert_request_finished_and_pages_sealed(pool, search_request_public_id).await?;
    Ok(())
}

#[tokio::test]
async fn search_result_ingest_dropped_sources_are_persisted_but_excluded_from_pages()
-> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();
    let (search_request_public_id, indexer_instance_public_id, source_guid) =
        setup_ingest_scope_with_request_drop_policy(pool, "blocked").await?;
    let observed_at = test_db.now();
    let input = SearchResultIngestInput {
        search_request_public_id,
        indexer_instance_public_id,
        source_guid,
        details_url: Some("https://example.com/details/blocked"),
        download_url: Some("https://example.com/download/blocked"),
        magnet_uri: Some("magnet:?xt=urn:btih:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        title_raw: "Blocked Scene Release",
        size_bytes: Some(2_048),
        infohash_v1: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        infohash_v2: None,
        magnet_hash: None,
        seeders: Some(30),
        leechers: Some(2),
        published_at: None,
        uploader: Some("blocked-uploader"),
        observed_at,
        attr_keys: None,
        attr_types: None,
        attr_value_text: None,
        attr_value_int: None,
        attr_value_bigint: None,
        attr_value_numeric: None,
        attr_value_bool: None,
        attr_value_uuid: None,
    };
    let ingested = search_result_ingest(pool, &input).await?;

    let page_item_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM search_page_item spi
         JOIN search_page sp
           ON sp.search_page_id = spi.search_page_id
         JOIN search_request sr
           ON sr.search_request_id = sp.search_request_id
         WHERE sr.search_request_public_id = $1",
    )
    .bind(search_request_public_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(page_item_count, 0);

    let audit_row = fetch_dropped_source_audit_row(
        pool,
        search_request_public_id,
        input.source_guid,
        ingested.canonical_torrent_public_id,
        ingested.canonical_torrent_source_public_id,
    )
    .await?;
    assert!(audit_row.0);
    assert_eq!(audit_row.1.as_str(), "drop_canonical");
    assert!(audit_row.2);
    assert!(audit_row.3);
    Ok(())
}
