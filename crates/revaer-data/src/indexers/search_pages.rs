//! Stored-procedure access for search page reads.
//!
//! # Design
//! - Encapsulates search page listing and fetch helpers behind stored procedures.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const SEARCH_PAGE_LIST_CALL: &str = r"
    SELECT
        page_number,
        sealed_at,
        item_count
    FROM search_page_list(
        actor_user_public_id => $1,
        search_request_public_id_input => $2
    )
";

const SEARCH_PAGE_FETCH_CALL: &str = r"
    SELECT
        page_number,
        sealed_at,
        item_count,
        item_position,
        canonical_torrent_public_id,
        title_display,
        size_bytes,
        infohash_v1,
        infohash_v2,
        magnet_hash,
        canonical_torrent_source_public_id,
        indexer_instance_public_id,
        indexer_display_name,
        seeders,
        leechers,
        published_at,
        download_url,
        magnet_uri,
        details_url,
        tracker_name,
        tracker_category,
        tracker_subcategory
    FROM search_page_fetch(
        actor_user_public_id => $1,
        search_request_public_id_input => $2,
        page_number_input => $3
    )
";

const SEARCH_REQUEST_EXPLAINABILITY_CALL: &str = r"
    SELECT
        zero_runnable_indexers,
        skipped_canceled_indexers,
        skipped_failed_indexers,
        blocked_results,
        blocked_rule_public_ids,
        rate_limited_indexers,
        retrying_indexers
    FROM search_request_explainability(
        actor_user_public_id => $1,
        search_request_public_id_input => $2
    )
";

/// Summary row for a search request page.
#[derive(Debug, Clone, FromRow)]
pub struct SearchPageSummaryRow {
    /// Page number within the search request.
    pub page_number: i32,
    /// Timestamp when the page was sealed.
    pub sealed_at: Option<DateTime<Utc>>,
    /// Item count for the page.
    pub item_count: i32,
}

/// Row data for a search request page item fetch.
#[derive(Debug, Clone, FromRow)]
pub struct SearchPageFetchRow {
    /// Page number for this row.
    pub page_number: i32,
    /// Timestamp when the page was sealed.
    pub sealed_at: Option<DateTime<Utc>>,
    /// Item count for the page.
    pub item_count: i32,
    /// Position of the item within the page.
    pub item_position: Option<i32>,
    /// Canonical torrent public identifier.
    pub canonical_torrent_public_id: Option<Uuid>,
    /// Display title for the canonical torrent.
    pub title_display: Option<String>,
    /// Canonical size in bytes.
    pub size_bytes: Option<i64>,
    /// Infohash v1 value.
    pub infohash_v1: Option<String>,
    /// Infohash v2 value.
    pub infohash_v2: Option<String>,
    /// Magnet hash value.
    pub magnet_hash: Option<String>,
    /// Canonical torrent source public identifier.
    pub canonical_torrent_source_public_id: Option<Uuid>,
    /// Indexer instance public identifier for the best source.
    pub indexer_instance_public_id: Option<Uuid>,
    /// Indexer instance display name for the best source.
    pub indexer_display_name: Option<String>,
    /// Last seen seeders count.
    pub seeders: Option<i32>,
    /// Last seen leechers count.
    pub leechers: Option<i32>,
    /// Last seen published timestamp.
    pub published_at: Option<DateTime<Utc>>,
    /// Last seen download URL.
    pub download_url: Option<String>,
    /// Last seen magnet URI.
    pub magnet_uri: Option<String>,
    /// Last seen details URL.
    pub details_url: Option<String>,
    /// Tracker name from durable source attributes.
    pub tracker_name: Option<String>,
    /// Tracker category from durable source attributes.
    pub tracker_category: Option<i32>,
    /// Tracker subcategory from durable source attributes.
    pub tracker_subcategory: Option<i32>,
}

/// Explainability summary for a search request.
#[derive(Debug, Clone, FromRow)]
pub struct SearchRequestExplainabilityRow {
    /// Whether no indexers were runnable.
    pub zero_runnable_indexers: bool,
    /// Count of canceled indexer runs.
    pub skipped_canceled_indexers: i32,
    /// Count of failed indexer runs.
    pub skipped_failed_indexers: i32,
    /// Count of blocked decisions.
    pub blocked_results: i32,
    /// Distinct blocking policy rules.
    pub blocked_rule_public_ids: Vec<Uuid>,
    /// Count of runs currently rate-limited.
    pub rate_limited_indexers: i32,
    /// Count of runs currently retrying.
    pub retrying_indexers: i32,
}

/// List pages for a search request.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_page_list(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    search_request_public_id: Uuid,
) -> Result<Vec<SearchPageSummaryRow>> {
    sqlx::query_as(SEARCH_PAGE_LIST_CALL)
        .bind(actor_user_public_id)
        .bind(search_request_public_id)
        .fetch_all(pool)
        .await
        .map_err(try_op("search page list"))
}

/// Fetch items for a specific search request page.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_page_fetch(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    search_request_public_id: Uuid,
    page_number: i32,
) -> Result<Vec<SearchPageFetchRow>> {
    sqlx::query_as(SEARCH_PAGE_FETCH_CALL)
        .bind(actor_user_public_id)
        .bind(search_request_public_id)
        .bind(page_number)
        .fetch_all(pool)
        .await
        .map_err(try_op("search page fetch"))
}

/// Fetch explainability details for a search request.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_request_explainability(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    search_request_public_id: Uuid,
) -> Result<SearchRequestExplainabilityRow> {
    sqlx::query_as(SEARCH_REQUEST_EXPLAINABILITY_CALL)
        .bind(actor_user_public_id)
        .bind(search_request_public_id)
        .fetch_one(pool)
        .await
        .map_err(try_op("search request explainability"))
}

#[cfg(test)]
#[path = "search_pages/tests.rs"]
mod tests;
