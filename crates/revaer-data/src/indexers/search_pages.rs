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
mod tests {
    use super::*;
    use crate::DataError;
    use crate::indexers::search_requests::{SearchRequestCreateInput, search_request_create};
    use chrono::{Duration, Utc};

    const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";

    async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
        crate::indexers::setup_indexer_db("indexer tests").await
    }

    async fn create_search_request(pool: &PgPool) -> anyhow::Result<Uuid> {
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let input = SearchRequestCreateInput {
            actor_user_public_id: Some(actor),
            query_text: "dune",
            query_type: "free_text",
            torznab_mode: None,
            requested_media_domain_key: None,
            page_size: None,
            search_profile_public_id: None,
            request_policy_set_public_id: None,
            season_number: None,
            episode_number: None,
            identifier_types: None,
            identifier_values: None,
            torznab_cat_ids: None,
        };

        let row = search_request_create(pool, &input).await?;
        Ok(row.search_request_public_id)
    }

    async fn fetch_search_request_id(
        pool: &PgPool,
        search_request_public_id: Uuid,
    ) -> anyhow::Result<i64> {
        sqlx::query_scalar(
            "SELECT search_request_id
             FROM search_request
             WHERE search_request_public_id = $1",
        )
        .bind(search_request_public_id)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
    }

    async fn create_indexer_instance(pool: &PgPool) -> anyhow::Result<i64> {
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
        .bind(format!("search-page-{}", Uuid::new_v4().simple()))
        .bind("Search Page Definition")
        .bind("torrent")
        .bind("torznab")
        .bind(1_i32)
        .bind("a".repeat(64))
        .bind(false)
        .fetch_one(pool)
        .await?;

        sqlx::query_scalar(
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
                $5,
                $6::trust_tier_key,
                $7,
                $8
            )
            RETURNING indexer_instance_id",
        )
        .bind(Uuid::new_v4())
        .bind(definition_id)
        .bind(format!("Search Page Instance {}", Uuid::new_v4().simple()))
        .bind("ready")
        .bind(50_i32)
        .bind("public")
        .bind(0_i64)
        .bind(0_i64)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
    }

    async fn create_canonical_source(
        pool: &PgPool,
        indexer_instance_id: i64,
    ) -> anyhow::Result<(i64, i64)> {
        let canonical_torrent_id: i64 = sqlx::query_scalar(
            "INSERT INTO canonical_torrent (
                canonical_torrent_public_id,
                identity_confidence,
                identity_strategy,
                infohash_v1,
                title_display,
                title_normalized,
                size_bytes
            )
            VALUES ($1, $2, $3::identity_strategy, $4, $5, $6, $7)
            RETURNING canonical_torrent_id",
        )
        .bind(Uuid::new_v4())
        .bind(1.0_f64)
        .bind("infohash_v1")
        .bind("b".repeat(40))
        .bind("Blocked Item")
        .bind("blocked item")
        .bind(1234_i64)
        .fetch_one(pool)
        .await?;

        let canonical_torrent_source_id: i64 = sqlx::query_scalar(
            "INSERT INTO canonical_torrent_source (
                indexer_instance_id,
                canonical_torrent_source_public_id,
                source_guid,
                infohash_v1,
                title_normalized,
                size_bytes
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING canonical_torrent_source_id",
        )
        .bind(indexer_instance_id)
        .bind(Uuid::new_v4())
        .bind(format!("source-{}", Uuid::new_v4().simple()))
        .bind("c".repeat(40))
        .bind("blocked item")
        .bind(1234_i64)
        .fetch_one(pool)
        .await?;

        Ok((canonical_torrent_id, canonical_torrent_source_id))
    }

    async fn insert_policy_snapshot(pool: &PgPool) -> anyhow::Result<i64> {
        sqlx::query_scalar(
            "INSERT INTO policy_snapshot (snapshot_hash, ref_count)
             VALUES ($1, $2)
             RETURNING policy_snapshot_id",
        )
        .bind("d".repeat(64))
        .bind(1_i32)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
    }

    async fn insert_rate_limited_retrying_run(
        pool: &PgPool,
        search_request_id: i64,
        indexer_instance_id: i64,
        retry_at: chrono::DateTime<Utc>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO search_request_indexer_run (
                search_request_id,
                indexer_instance_id,
                status,
                next_attempt_at,
                last_error_class,
                last_rate_limit_scope,
                error_class
            )
            VALUES ($1, $2, $3::run_status, $4, $5::error_class, $6::rate_limit_scope, $7::error_class)",
        )
        .bind(search_request_id)
        .bind(indexer_instance_id)
        .bind("queued")
        .bind(retry_at)
        .bind("rate_limited")
        .bind("indexer_instance")
        .bind(None::<String>)
        .execute(pool)
        .await?;
        Ok(())
    }

    async fn insert_terminal_run(
        pool: &PgPool,
        search_request_id: i64,
        status: &str,
        error_class: Option<&str>,
    ) -> anyhow::Result<()> {
        let indexer_instance_id = create_indexer_instance(pool).await?;
        sqlx::query(
            "INSERT INTO search_request_indexer_run (
                search_request_id,
                indexer_instance_id,
                status,
                started_at,
                finished_at,
                error_class
            )
            VALUES ($1, $2, $3::run_status, now() - make_interval(mins => 1), now(), $4::error_class)",
        )
        .bind(search_request_id)
        .bind(indexer_instance_id)
        .bind(status)
        .bind(error_class)
        .execute(pool)
        .await?;
        Ok(())
    }

    async fn insert_blocked_decision(
        pool: &PgPool,
        search_request_id: i64,
        policy_snapshot_id: i64,
        canonical_torrent_id: i64,
        canonical_torrent_source_id: i64,
        block_rule_id: Uuid,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO search_filter_decision (
                search_request_id,
                policy_rule_public_id,
                policy_snapshot_id,
                canonical_torrent_id,
                canonical_torrent_source_id,
                decision,
                decided_at
            )
            VALUES ($1, $2, $3, $4, $5, $6::decision_type, now())",
        )
        .bind(search_request_id)
        .bind(block_rule_id)
        .bind(policy_snapshot_id)
        .bind(canonical_torrent_id)
        .bind(canonical_torrent_source_id)
        .bind("drop_source")
        .execute(pool)
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn search_page_list_requires_request() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;

        let err = search_page_list(pool, actor, Uuid::new_v4())
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("search_request_not_found"));
        Ok(())
    }

    #[tokio::test]
    async fn search_page_list_returns_initial_page() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let request_id = create_search_request(pool).await?;

        let pages = search_page_list(pool, actor, request_id).await?;
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].page_number, 1);
        assert_eq!(pages[0].item_count, 0);
        Ok(())
    }

    #[tokio::test]
    async fn search_page_fetch_rejects_invalid_page_number() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let request_id = create_search_request(pool).await?;

        let err = search_page_fetch(pool, actor, request_id, 0)
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("page_number_invalid"));
        Ok(())
    }

    #[tokio::test]
    async fn search_page_fetch_returns_empty_page() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let request_id = create_search_request(pool).await?;

        let rows = search_page_fetch(pool, actor, request_id, 1).await?;
        let first = rows.first().expect("expected page row");
        assert_eq!(first.page_number, 1);
        assert_eq!(first.item_count, 0);
        assert!(first.canonical_torrent_public_id.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn search_request_explainability_defaults_with_zero_runnable_indexers()
    -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let request_id = create_search_request(pool).await?;

        let row = search_request_explainability(pool, actor, request_id).await?;
        assert!(row.zero_runnable_indexers);
        assert_eq!(row.skipped_canceled_indexers, 0);
        assert_eq!(row.skipped_failed_indexers, 0);
        assert_eq!(row.blocked_results, 0);
        assert!(row.blocked_rule_public_ids.is_empty());
        assert_eq!(row.rate_limited_indexers, 0);
        assert_eq!(row.retrying_indexers, 0);
        Ok(())
    }

    #[tokio::test]
    async fn search_request_explainability_surfaces_blocked_and_retrying_state()
    -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let search_request_public_id = create_search_request(pool).await?;
        let search_request_id = fetch_search_request_id(pool, search_request_public_id).await?;
        let indexer_instance_id = create_indexer_instance(pool).await?;
        let (canonical_torrent_id, canonical_torrent_source_id) =
            create_canonical_source(pool, indexer_instance_id).await?;
        let block_rule_id = Uuid::new_v4();
        let retry_at = Utc::now() + Duration::minutes(5);
        let policy_snapshot_id = insert_policy_snapshot(pool).await?;

        insert_rate_limited_retrying_run(pool, search_request_id, indexer_instance_id, retry_at)
            .await?;
        insert_terminal_run(pool, search_request_id, "failed", Some("timeout")).await?;
        insert_terminal_run(pool, search_request_id, "canceled", None).await?;
        insert_blocked_decision(
            pool,
            search_request_id,
            policy_snapshot_id,
            canonical_torrent_id,
            canonical_torrent_source_id,
            block_rule_id,
        )
        .await?;

        let row = search_request_explainability(pool, actor, search_request_public_id).await?;
        assert!(!row.zero_runnable_indexers);
        assert_eq!(row.skipped_canceled_indexers, 1);
        assert_eq!(row.skipped_failed_indexers, 1);
        assert_eq!(row.blocked_results, 1);
        assert_eq!(row.blocked_rule_public_ids, vec![block_rule_id]);
        assert_eq!(row.rate_limited_indexers, 1);
        assert_eq!(row.retrying_indexers, 1);
        Ok(())
    }
}
