//! Stored-procedure access for import job orchestration.
//!
//! # Design
//! - Encapsulates import job procedures behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

const IMPORT_JOB_CREATE_CALL: &str = r"
    SELECT import_job_create(
        actor_user_public_id => $1,
        source_input => $2::import_source,
        is_dry_run_input => $3,
        target_search_profile_public_id_input => $4,
        target_torznab_instance_public_id_input => $5
    )
";

const IMPORT_JOB_RUN_PROWLARR_API_CALL: &str = r"
    SELECT import_job_run_prowlarr_api(
        import_job_public_id_input => $1,
        prowlarr_url_input => $2,
        prowlarr_api_key_secret_public_id_input => $3
    )
";

const IMPORT_JOB_RUN_PROWLARR_BACKUP_CALL: &str = r"
    SELECT import_job_run_prowlarr_backup(
        import_job_public_id_input => $1,
        backup_blob_ref_input => $2
    )
";

const IMPORT_JOB_GET_STATUS_CALL: &str = r"
    SELECT
        status::text AS status,
        result_total,
        result_imported_ready,
        result_imported_needs_secret,
        result_imported_test_failed,
        result_unmapped_definition,
        result_skipped_duplicate
    FROM import_job_get_status(
        import_job_public_id_input => $1
    )
";

const IMPORT_JOB_LIST_RESULTS_CALL: &str = r"
    SELECT
        prowlarr_identifier,
        upstream_slug,
        indexer_instance_public_id,
        status::text AS status,
        detail,
        resolved_is_enabled,
        resolved_priority,
        missing_secret_fields,
        media_domain_keys,
        tag_keys,
        created_at
    FROM import_job_list_results(
        import_job_public_id_input => $1
    )
";

/// Status summary for an import job.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ImportJobStatusRow {
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

/// Row returned by import job result listing.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ImportJobResultRow {
    /// Prowlarr identifier string.
    pub prowlarr_identifier: String,
    /// Upstream slug for the indexer definition.
    pub upstream_slug: Option<String>,
    /// Public id for the created indexer instance, if any.
    pub indexer_instance_public_id: Option<Uuid>,
    /// Result status label.
    pub status: String,
    /// Optional result detail.
    pub detail: Option<String>,
    /// Preserved enabled state from the imported source.
    pub resolved_is_enabled: Option<bool>,
    /// Preserved priority from the imported source.
    pub resolved_priority: Option<i32>,
    /// Count of required secret fields missing from the import.
    pub missing_secret_fields: i32,
    /// Preserved media domain keys derived from imported categories.
    pub media_domain_keys: Vec<String>,
    /// Preserved tag keys derived from imported source tags.
    pub tag_keys: Vec<String>,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
}

/// Create an import job.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn import_job_create(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    source: &str,
    is_dry_run: Option<bool>,
    target_search_profile_public_id: Option<Uuid>,
    target_torznab_instance_public_id: Option<Uuid>,
) -> Result<Uuid> {
    sqlx::query_scalar(IMPORT_JOB_CREATE_CALL)
        .bind(actor_user_public_id)
        .bind(source)
        .bind(is_dry_run)
        .bind(target_search_profile_public_id)
        .bind(target_torznab_instance_public_id)
        .fetch_one(pool)
        .await
        .map_err(try_op("import job create"))
}

/// Mark an import job as running for the Prowlarr API path.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn import_job_run_prowlarr_api(
    pool: &PgPool,
    import_job_public_id: Uuid,
    prowlarr_url: &str,
    prowlarr_api_key_secret_public_id: Uuid,
) -> Result<()> {
    sqlx::query(IMPORT_JOB_RUN_PROWLARR_API_CALL)
        .bind(import_job_public_id)
        .bind(prowlarr_url)
        .bind(prowlarr_api_key_secret_public_id)
        .execute(pool)
        .await
        .map_err(try_op("import job run prowlarr api"))?;
    Ok(())
}

/// Mark an import job as running for the Prowlarr backup path.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn import_job_run_prowlarr_backup(
    pool: &PgPool,
    import_job_public_id: Uuid,
    backup_blob_ref: &str,
) -> Result<()> {
    sqlx::query(IMPORT_JOB_RUN_PROWLARR_BACKUP_CALL)
        .bind(import_job_public_id)
        .bind(backup_blob_ref)
        .execute(pool)
        .await
        .map_err(try_op("import job run prowlarr backup"))?;
    Ok(())
}

/// Fetch status for an import job.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn import_job_get_status(
    pool: &PgPool,
    import_job_public_id: Uuid,
) -> Result<ImportJobStatusRow> {
    sqlx::query_as(IMPORT_JOB_GET_STATUS_CALL)
        .bind(import_job_public_id)
        .fetch_one(pool)
        .await
        .map_err(try_op("import job get status"))
}

/// List results for an import job.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn import_job_list_results(
    pool: &PgPool,
    import_job_public_id: Uuid,
) -> Result<Vec<ImportJobResultRow>> {
    sqlx::query_as(IMPORT_JOB_LIST_RESULTS_CALL)
        .bind(import_job_public_id)
        .fetch_all(pool)
        .await
        .map_err(try_op("import job list results"))
}

#[cfg(test)]
mod tests {
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
        let import_job_id: i64 = sqlx::query_scalar(
            "SELECT import_job_id FROM import_job WHERE import_job_public_id = $1",
        )
        .bind(job_id)
        .fetch_one(pool)
        .await?;
        let upstream_slug = insert_indexer_definition(pool).await?;
        let tag_alpha_id: i64 =
            sqlx::query_scalar("SELECT tag_id FROM tag WHERE tag_public_id = $1")
                .bind(tag_alpha)
                .fetch_one(pool)
                .await?;
        let tag_beta_id: i64 =
            sqlx::query_scalar("SELECT tag_id FROM tag WHERE tag_public_id = $1")
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
        let job_id =
            import_job_create(pool, actor, "prowlarr_api", Some(false), None, None).await?;

        let err =
            import_job_run_prowlarr_api(pool, job_id, "http://localhost:9696", Uuid::new_v4())
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
        let job_id =
            import_job_create(pool, actor, "prowlarr_backup", Some(true), None, None).await?;

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
        let backup_job_api_err = import_job_run_prowlarr_api(
            pool,
            backup_job_id,
            "http://localhost:9696",
            Uuid::new_v4(),
        )
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
        let job_id =
            import_job_create(pool, actor, "prowlarr_api", Some(false), None, None).await?;
        let internal_job_id: i64 = sqlx::query_scalar(
            "SELECT import_job_id FROM import_job WHERE import_job_public_id = $1",
        )
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
        let tag_alpha =
            crate::indexers::tags::tag_create(pool, actor, &tag_alpha_key, "Alpha").await?;
        let tag_beta =
            crate::indexers::tags::tag_create(pool, actor, &tag_beta_key, "Beta").await?;
        let job_id =
            import_job_create(pool, actor, "prowlarr_api", Some(false), None, None).await?;
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
}
