use super::*;
use crate::DataError;

const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";

async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
    crate::indexers::setup_indexer_db("indexer tests").await
}

async fn insert_indexer_definition(pool: &PgPool) -> anyhow::Result<String> {
    let upstream_slug = format!("import-snapshot-{}", Uuid::new_v4().simple());
    sqlx::query(
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
            VALUES (
                $1::upstream_source,
                $2,
                $3,
                $4::protocol,
                $5::engine,
                $6,
                $7,
                $8
            )",
    )
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
    Ok(upstream_slug)
}

async fn insert_snapshot_import_result(
    pool: &PgPool,
    job_id: Uuid,
    tag_alpha: Uuid,
    tag_beta: Uuid,
) -> anyhow::Result<()> {
    let import_job_id: i64 =
        sqlx::query_scalar("SELECT import_job_id FROM import_job WHERE import_job_public_id = $1")
            .bind(job_id)
            .fetch_one(pool)
            .await?;
    let upstream_slug = insert_indexer_definition(pool).await?;
    let tag_alpha_id: i64 = sqlx::query_scalar("SELECT tag_id FROM tag WHERE tag_public_id = $1")
        .bind(tag_alpha)
        .fetch_one(pool)
        .await?;
    let tag_beta_id: i64 = sqlx::query_scalar("SELECT tag_id FROM tag WHERE tag_public_id = $1")
        .bind(tag_beta)
        .fetch_one(pool)
        .await?;
    let import_result_id: i64 = sqlx::query_scalar(
        r"
                INSERT INTO import_indexer_result (
                    import_job_id,
                    prowlarr_identifier,
                    upstream_slug,
                    indexer_instance_id,
                    status,
                    detail,
                    resolved_is_enabled,
                    resolved_priority,
                    missing_secret_fields
                )
                VALUES (
                    $1,
                    'prowlarr-snapshot',
                    $2,
                    NULL,
                    'imported_needs_secret',
                    'missing_secret_bindings',
                    FALSE,
                    73,
                    2
                )
                RETURNING import_indexer_result_id
            ",
    )
    .bind(import_job_id)
    .bind(upstream_slug)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        r"
                INSERT INTO import_indexer_result_media_domain (
                    import_indexer_result_id,
                    media_domain_id
                )
                SELECT $1, media_domain_id
                FROM media_domain
                WHERE media_domain_key::TEXT IN ('tv', 'movies')
            ",
    )
    .bind(import_result_id)
    .execute(pool)
    .await?;

    sqlx::query(
            "INSERT INTO import_indexer_result_tag (import_indexer_result_id, tag_id) VALUES ($1, $2), ($1, $3)",
        )
        .bind(import_result_id)
        .bind(tag_alpha_id)
        .bind(tag_beta_id)
        .execute(pool)
        .await?;

    Ok(())
}

#[tokio::test]
async fn import_job_create_and_status_roundtrip() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let job_id = import_job_create(pool, actor, "prowlarr_api", Some(true), None, None).await?;

    let status = import_job_get_status(pool, job_id).await?;
    assert_eq!(status.status, "pending");
    assert_eq!(status.result_total, 0);

    let results = import_job_list_results(pool, job_id).await?;
    assert!(results.is_empty());
    Ok(())
}
#[tokio::test]
async fn import_job_run_prowlarr_api_requires_secret() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let job_id = import_job_create(pool, actor, "prowlarr_api", Some(false), None, None).await?;

    let err = import_job_run_prowlarr_api(pool, job_id, "http://localhost:9696", Uuid::new_v4())
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("secret_not_found"));
    Ok(())
}
#[tokio::test]
async fn import_job_run_prowlarr_backup_requires_job() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let err = import_job_run_prowlarr_backup(pool, Uuid::new_v4(), "backup")
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("import_job_not_found"));
    Ok(())
}

#[tokio::test]
async fn import_job_create_supports_backup_source_and_dry_run() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let job_id = import_job_create(pool, actor, "prowlarr_backup", Some(true), None, None).await?;

    let row: (String, bool) = sqlx::query_as(
        "SELECT source::text, is_dry_run FROM import_job WHERE import_job_public_id = $1",
    )
    .bind(job_id)
    .fetch_one(pool)
    .await?;

    assert_eq!(row.0, "prowlarr_backup");
    assert!(row.1);
    Ok(())
}

#[tokio::test]
async fn import_job_run_procedures_reject_source_mismatch() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;

    let api_job_id =
        import_job_create(pool, actor, "prowlarr_api", Some(false), None, None).await?;
    let api_job_backup_err = import_job_run_prowlarr_backup(pool, api_job_id, "backup")
        .await
        .unwrap_err();
    assert!(matches!(api_job_backup_err, DataError::QueryFailed { .. }));
    assert_eq!(
        api_job_backup_err.database_detail(),
        Some("import_source_mismatch")
    );

    let backup_job_id =
        import_job_create(pool, actor, "prowlarr_backup", Some(false), None, None).await?;
    let backup_job_api_err =
        import_job_run_prowlarr_api(pool, backup_job_id, "http://localhost:9696", Uuid::new_v4())
            .await
            .unwrap_err();
    assert!(matches!(backup_job_api_err, DataError::QueryFailed { .. }));
    assert_eq!(
        backup_job_api_err.database_detail(),
        Some("import_source_mismatch")
    );
    Ok(())
}

#[tokio::test]
async fn import_job_status_and_results_surface_unmapped_definitions() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let job_id = import_job_create(pool, actor, "prowlarr_api", Some(false), None, None).await?;
    let internal_job_id: i64 =
        sqlx::query_scalar("SELECT import_job_id FROM import_job WHERE import_job_public_id = $1")
            .bind(job_id)
            .fetch_one(pool)
            .await?;

    sqlx::query(
        r"
                INSERT INTO import_indexer_result (
                    import_job_id,
                    prowlarr_identifier,
                    upstream_slug,
                    indexer_instance_id,
                    status,
                    detail
                )
                VALUES
                    ($1, 'mapped-1', 'example-indexer', NULL, 'imported_ready', NULL),
                    ($1, 'unmapped-1', NULL, NULL, 'unmapped_definition', 'definition_not_found')
            ",
    )
    .bind(internal_job_id)
    .execute(pool)
    .await?;

    let status = import_job_get_status(pool, job_id).await?;
    assert_eq!(status.result_total, 2);
    assert_eq!(status.result_imported_ready, 1);
    assert_eq!(status.result_unmapped_definition, 1);

    let results = import_job_list_results(pool, job_id).await?;
    assert_eq!(results.len(), 2);
    assert!(
        results
            .iter()
            .any(|row| row.status == "imported_ready" && row.upstream_slug.is_some())
    );
    assert!(results.iter().any(|row| {
        row.status == "unmapped_definition"
            && row.upstream_slug.is_none()
            && row.detail.as_deref() == Some("definition_not_found")
    }));
    Ok(())
}

#[tokio::test]
async fn import_job_results_surface_preserved_configuration_snapshot() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let suffix = Uuid::new_v4().simple().to_string();
    let tag_alpha_key = format!("alpha{}", &suffix[..8]);
    let tag_beta_key = format!("beta{}", &suffix[8..16]);
    let tag_alpha = crate::indexers::tags::tag_create(pool, actor, &tag_alpha_key, "Alpha").await?;
    let tag_beta = crate::indexers::tags::tag_create(pool, actor, &tag_beta_key, "Beta").await?;
    let job_id = import_job_create(pool, actor, "prowlarr_api", Some(false), None, None).await?;
    insert_snapshot_import_result(pool, job_id, tag_alpha, tag_beta).await?;

    let results = import_job_list_results(pool, job_id).await?;
    assert_eq!(results.len(), 1);
    let result = &results[0];
    assert_eq!(result.prowlarr_identifier, "prowlarr-snapshot");
    assert_eq!(result.status, "imported_needs_secret");
    assert_eq!(result.detail.as_deref(), Some("missing_secret_bindings"));
    assert_eq!(result.indexer_instance_public_id, None);
    assert_eq!(result.resolved_is_enabled, Some(false));
    assert_eq!(result.resolved_priority, Some(73));
    assert_eq!(result.missing_secret_fields, 2);
    assert_eq!(result.media_domain_keys, vec!["movies", "tv"]);
    assert_eq!(result.tag_keys, vec![tag_alpha_key, tag_beta_key]);
    Ok(())
}
