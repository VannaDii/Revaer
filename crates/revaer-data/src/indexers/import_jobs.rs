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
#[path = "import_jobs/tests.rs"]
mod tests;
