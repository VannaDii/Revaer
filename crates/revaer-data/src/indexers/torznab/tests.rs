use super::*;
use crate::DataError;
use crate::indexers::search_profiles::search_profile_create;
use crate::indexers::search_requests::{SearchRequestCreateInput, search_request_create};
use crate::indexers::search_results::{SearchResultIngestInput, search_result_ingest};
use chrono::Utc;
use sqlx::PgPool;

const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";

async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
    crate::indexers::setup_indexer_db("indexer tests").await
}

async fn insert_indexer_instance(pool: &PgPool) -> anyhow::Result<Uuid> {
    let definition_id: i64 = sqlx::query_scalar(include_str!("sql/insert_indexer_definition.sql"))
        .bind("prowlarr_indexers")
        .bind(format!("torznab-download-{}", Uuid::new_v4().simple()))
        .bind("Torznab Download Definition")
        .bind("torrent")
        .bind("torznab")
        .bind(1_i32)
        .bind("e".repeat(64))
        .bind(false)
        .fetch_one(pool)
        .await?;

    let instance_public_id = Uuid::new_v4();
    sqlx::query(include_str!("sql/insert_indexer_instance.sql"))
        .bind(instance_public_id)
        .bind(definition_id)
        .bind("Torznab Download Instance")
        .bind("ready")
        .execute(pool)
        .await?;
    Ok(instance_public_id)
}

async fn setup_download_scope(
    pool: &PgPool,
    magnet_uri: Option<&str>,
    download_url: Option<&str>,
) -> anyhow::Result<(Uuid, Uuid)> {
    sqlx::query(include_str!("sql/disable_policy_sets.sql"))
        .execute(pool)
        .await?;

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let search_profile_public_id = search_profile_create(
        pool,
        actor,
        "Torznab Download Profile",
        Some(false),
        Some(50),
        Some("movies"),
        None,
    )
    .await?;
    let search_profile_id: i64 = sqlx::query_scalar(
        "SELECT search_profile_id
         FROM search_profile
         WHERE search_profile_public_id = $1",
    )
    .bind(search_profile_public_id)
    .fetch_one(pool)
    .await?;

    let torznab_instance_public_id = Uuid::new_v4();
    sqlx::query(include_str!("sql/insert_torznab_instance.sql"))
        .bind(search_profile_id)
        .bind(torznab_instance_public_id)
        .bind(format!(
            "Torznab Feed {}",
            torznab_instance_public_id.simple()
        ))
        .bind("test-hash")
        .execute(pool)
        .await?;

    let indexer_instance_public_id = insert_indexer_instance(pool).await?;
    let search_request = search_request_create(
        pool,
        &SearchRequestCreateInput {
            actor_user_public_id: None,
            query_text: "torznab download",
            query_type: "free_text",
            torznab_mode: Some("generic"),
            requested_media_domain_key: None,
            page_size: Some(50),
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

    let ingest = search_result_ingest(
        pool,
        &SearchResultIngestInput {
            search_request_public_id: search_request.search_request_public_id,
            indexer_instance_public_id,
            source_guid: Some("torznab-download-source"),
            details_url: Some("https://example.com/details/1"),
            download_url,
            magnet_uri,
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
    .await?;

    Ok((
        torznab_instance_public_id,
        ingest.canonical_torrent_source_public_id,
    ))
}

#[tokio::test]
async fn torznab_instance_enable_disable_requires_instance() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let err = torznab_instance_enable_disable(pool, actor, Uuid::new_v4(), true)
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("torznab_instance_not_found"));
    Ok(())
}

#[tokio::test]
async fn torznab_instance_soft_delete_requires_instance() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let err = torznab_instance_soft_delete(pool, actor, Uuid::new_v4())
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("torznab_instance_not_found"));
    Ok(())
}

#[tokio::test]
async fn torznab_instance_create_rotate_require_valid_refs() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let err = torznab_instance_create(pool, actor, None, "test")
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("search_profile_missing"));

    let err = torznab_instance_rotate_key(pool, actor, Uuid::new_v4())
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("torznab_instance_not_found"));
    Ok(())
}

#[tokio::test]
async fn torznab_instance_authenticate_requires_instance() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let err = torznab_instance_authenticate(pool, Uuid::new_v4(), "bad-key")
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("torznab_instance_not_found"));
    Ok(())
}

#[tokio::test]
async fn torznab_download_prepare_requires_instance() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let err = torznab_download_prepare(pool, Uuid::new_v4(), Uuid::new_v4())
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("torznab_instance_not_found"));
    Ok(())
}

#[tokio::test]
async fn torznab_category_list_returns_seeded_categories() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let categories = torznab_category_list(pool).await?;
    assert!(
        categories.iter().any(|cat| cat.torznab_cat_id == 2000),
        "expected seeded Movies category to exist"
    );
    Ok(())
}

#[tokio::test]
async fn torznab_download_prepare_prefers_magnet_and_records_started_attempt() -> anyhow::Result<()>
{
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();

    let (torznab_instance_public_id, canonical_source_public_id) = setup_download_scope(
        pool,
        Some("magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567"),
        Some("https://example.com/download/1"),
    )
    .await?;

    let redirect =
        torznab_download_prepare(pool, torznab_instance_public_id, canonical_source_public_id)
            .await?;
    assert_eq!(
        redirect.as_deref(),
        Some("magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567")
    );

    let status: String = sqlx::query_scalar(
        "SELECT status::TEXT
         FROM acquisition_attempt
         WHERE canonical_torrent_source_id = (
             SELECT canonical_torrent_source_id
             FROM canonical_torrent_source
             WHERE canonical_torrent_source_public_id = $1
         )
         ORDER BY acquisition_attempt_id DESC
         LIMIT 1",
    )
    .bind(canonical_source_public_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(status, "started");
    Ok(())
}

#[tokio::test]
async fn torznab_download_prepare_uses_download_url_when_magnet_missing() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();

    let (torznab_instance_public_id, canonical_source_public_id) =
        setup_download_scope(pool, None, Some("https://example.com/download/2")).await?;

    let redirect =
        torznab_download_prepare(pool, torznab_instance_public_id, canonical_source_public_id)
            .await?;
    assert_eq!(redirect.as_deref(), Some("https://example.com/download/2"));
    Ok(())
}

#[tokio::test]
async fn torznab_download_prepare_records_failure_when_no_redirect_target() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();

    let (torznab_instance_public_id, canonical_source_public_id) =
        setup_download_scope(pool, None, None).await?;

    let redirect =
        torznab_download_prepare(pool, torznab_instance_public_id, canonical_source_public_id)
            .await?;
    assert!(redirect.is_none());

    let (status, failure_class, failure_detail): (String, Option<String>, Option<String>) =
        sqlx::query_as(
            "SELECT
                status::TEXT,
                failure_class::TEXT,
                failure_detail
             FROM acquisition_attempt
             WHERE canonical_torrent_source_id = (
                 SELECT canonical_torrent_source_id
                 FROM canonical_torrent_source
                 WHERE canonical_torrent_source_public_id = $1
             )
             ORDER BY acquisition_attempt_id DESC
             LIMIT 1",
        )
        .bind(canonical_source_public_id)
        .fetch_one(pool)
        .await?;
    assert_eq!(status, "failed");
    assert_eq!(failure_class.as_deref(), Some("client_error"));
    assert_eq!(failure_detail.as_deref(), Some("no_download_target"));
    Ok(())
}
