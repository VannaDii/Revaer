//! Stored-procedure access for search request orchestration.
//!
//! # Design
//! - Encapsulates search request procedures behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use sqlx::PgPool;
use uuid::Uuid;

const SEARCH_REQUEST_CREATE_CALL: &str = r"
    SELECT
        search_request_public_id,
        request_policy_set_public_id
    FROM search_request_create(
        actor_user_public_id => $1,
        query_text_input => $2,
        query_type_input => $3::query_type,
        torznab_mode_input => $4::torznab_mode,
        requested_media_domain_key_input => $5,
        page_size_input => $6,
        search_profile_public_id_input => $7,
        request_policy_set_public_id_input => $8,
        season_number_input => $9,
        episode_number_input => $10,
        identifier_types_input => $11::identifier_type[],
        identifier_values_input => $12,
        torznab_cat_ids_input => $13::int[]
    )
";

const SEARCH_REQUEST_CANCEL_CALL: &str = r"
    SELECT search_request_cancel(
        actor_user_public_id => $1,
        search_request_public_id_input => $2
    )
";

const SEARCH_INDEXER_RUN_ENQUEUE_CALL: &str = r"
    SELECT search_indexer_run_enqueue(
        search_request_public_id_input => $1,
        indexer_instance_public_id_input => $2
    )
";

const SEARCH_INDEXER_RUN_MARK_STARTED_CALL: &str = r"
    SELECT search_indexer_run_mark_started(
        search_request_public_id_input => $1,
        indexer_instance_public_id_input => $2
    )
";

const SEARCH_INDEXER_RUN_MARK_FINISHED_CALL: &str = r"
    SELECT search_indexer_run_mark_finished(
        search_request_public_id_input => $1,
        indexer_instance_public_id_input => $2,
        items_seen_delta_input => $3,
        items_emitted_delta_input => $4,
        canonical_added_delta_input => $5
    )
";

const SEARCH_INDEXER_RUN_MARK_FAILED_CALL: &str = r"
    SELECT search_indexer_run_mark_failed(
        search_request_public_id_input => $1,
        indexer_instance_public_id_input => $2,
        error_class_input => $3::error_class,
        error_detail_input => $4,
        retry_after_seconds_input => $5,
        retry_seq_input => $6,
        rate_limit_scope_input => $7::rate_limit_scope
    )
";

const SEARCH_INDEXER_RUN_MARK_CANCELED_CALL: &str = r"
    SELECT search_indexer_run_mark_canceled(
        search_request_public_id_input => $1,
        indexer_instance_public_id_input => $2
    )
";

/// Output from search request creation.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SearchRequestCreateRow {
    /// Public id for the search request.
    pub search_request_public_id: Uuid,
    /// Public id for the policy set snapshot.
    pub request_policy_set_public_id: Uuid,
}

/// Input payload for creating a search request.
#[derive(Debug, Clone)]
pub struct SearchRequestCreateInput<'a> {
    /// Actor user public id for audit.
    pub actor_user_public_id: Option<Uuid>,
    /// Raw query text.
    pub query_text: &'a str,
    /// Query type key.
    pub query_type: &'a str,
    /// Optional Torznab mode.
    pub torznab_mode: Option<&'a str>,
    /// Optional requested media domain key.
    pub requested_media_domain_key: Option<&'a str>,
    /// Optional page size.
    pub page_size: Option<i32>,
    /// Optional search profile public id.
    pub search_profile_public_id: Option<Uuid>,
    /// Optional request policy set public id.
    pub request_policy_set_public_id: Option<Uuid>,
    /// Optional season number.
    pub season_number: Option<i32>,
    /// Optional episode number.
    pub episode_number: Option<i32>,
    /// Optional identifier types.
    pub identifier_types: Option<&'a [String]>,
    /// Optional identifier values.
    pub identifier_values: Option<&'a [String]>,
    /// Optional Torznab category ids.
    pub torznab_cat_ids: Option<&'a [i32]>,
}

/// Create a search request.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_request_create(
    pool: &PgPool,
    input: &SearchRequestCreateInput<'_>,
) -> Result<SearchRequestCreateRow> {
    sqlx::query_as(SEARCH_REQUEST_CREATE_CALL)
        .bind(input.actor_user_public_id)
        .bind(input.query_text)
        .bind(input.query_type)
        .bind(input.torznab_mode)
        .bind(input.requested_media_domain_key)
        .bind(input.page_size)
        .bind(input.search_profile_public_id)
        .bind(input.request_policy_set_public_id)
        .bind(input.season_number)
        .bind(input.episode_number)
        .bind(input.identifier_types)
        .bind(input.identifier_values)
        .bind(input.torznab_cat_ids)
        .fetch_one(pool)
        .await
        .map_err(try_op("search request create"))
}

/// Cancel a search request.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_request_cancel(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    search_request_public_id: Uuid,
) -> Result<()> {
    sqlx::query(SEARCH_REQUEST_CANCEL_CALL)
        .bind(actor_user_public_id)
        .bind(search_request_public_id)
        .execute(pool)
        .await
        .map_err(try_op("search request cancel"))?;
    Ok(())
}

/// Enqueue a search run for an indexer instance.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_indexer_run_enqueue(
    pool: &PgPool,
    search_request_public_id: Uuid,
    indexer_instance_public_id: Uuid,
) -> Result<()> {
    sqlx::query(SEARCH_INDEXER_RUN_ENQUEUE_CALL)
        .bind(search_request_public_id)
        .bind(indexer_instance_public_id)
        .execute(pool)
        .await
        .map_err(try_op("search indexer run enqueue"))?;
    Ok(())
}

/// Mark an indexer run as started.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_indexer_run_mark_started(
    pool: &PgPool,
    search_request_public_id: Uuid,
    indexer_instance_public_id: Uuid,
) -> Result<()> {
    sqlx::query(SEARCH_INDEXER_RUN_MARK_STARTED_CALL)
        .bind(search_request_public_id)
        .bind(indexer_instance_public_id)
        .execute(pool)
        .await
        .map_err(try_op("search indexer run mark started"))?;
    Ok(())
}

/// Mark an indexer run as finished.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_indexer_run_mark_finished(
    pool: &PgPool,
    search_request_public_id: Uuid,
    indexer_instance_public_id: Uuid,
    items_seen_delta: i32,
    items_emitted_delta: i32,
    canonical_added_delta: i32,
) -> Result<()> {
    sqlx::query(SEARCH_INDEXER_RUN_MARK_FINISHED_CALL)
        .bind(search_request_public_id)
        .bind(indexer_instance_public_id)
        .bind(items_seen_delta)
        .bind(items_emitted_delta)
        .bind(canonical_added_delta)
        .execute(pool)
        .await
        .map_err(try_op("search indexer run mark finished"))?;
    Ok(())
}

/// Input payload for marking an indexer run as failed.
#[derive(Debug, Clone, Copy)]
pub struct SearchIndexerRunFailedInput<'a> {
    /// Search request public id.
    pub search_request_public_id: Uuid,
    /// Indexer instance public id.
    pub indexer_instance_public_id: Uuid,
    /// Error class key.
    pub error_class: &'a str,
    /// Optional error detail.
    pub error_detail: Option<&'a str>,
    /// Optional retry delay.
    pub retry_after_seconds: Option<i32>,
    /// Retry sequence number.
    pub retry_seq: i16,
    /// Optional rate limit scope.
    pub rate_limit_scope: Option<&'a str>,
}

/// Mark an indexer run as failed.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_indexer_run_mark_failed(
    pool: &PgPool,
    input: &SearchIndexerRunFailedInput<'_>,
) -> Result<()> {
    sqlx::query(SEARCH_INDEXER_RUN_MARK_FAILED_CALL)
        .bind(input.search_request_public_id)
        .bind(input.indexer_instance_public_id)
        .bind(input.error_class)
        .bind(input.error_detail)
        .bind(input.retry_after_seconds)
        .bind(input.retry_seq)
        .bind(input.rate_limit_scope)
        .execute(pool)
        .await
        .map_err(try_op("search indexer run mark failed"))?;
    Ok(())
}

/// Mark an indexer run as canceled.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_indexer_run_mark_canceled(
    pool: &PgPool,
    search_request_public_id: Uuid,
    indexer_instance_public_id: Uuid,
) -> Result<()> {
    sqlx::query(SEARCH_INDEXER_RUN_MARK_CANCELED_CALL)
        .bind(search_request_public_id)
        .bind(indexer_instance_public_id)
        .execute(pool)
        .await
        .map_err(try_op("search indexer run mark canceled"))?;
    Ok(())
}

#[cfg(test)]
#[path = "search_requests/tests.rs"]
mod tests;
