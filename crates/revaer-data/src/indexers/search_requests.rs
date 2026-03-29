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
mod tests {
    use super::*;
    use crate::DataError;
    use crate::indexers::jobs::job_run_retention_purge;
    use crate::indexers::search_profiles::{
        search_profile_create, search_profile_set_domain_allowlist,
    };
    use chrono::{DateTime, Duration, Utc};
    use sqlx::PgPool;

    const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";
    async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
        crate::indexers::setup_indexer_db("indexer tests").await
    }

    async fn insert_request_policy_set(pool: &sqlx::PgPool) -> anyhow::Result<Uuid> {
        let policy_set_public_id = Uuid::new_v4();
        sqlx::query(
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
            VALUES ($1, NULL, $2, 'request', TRUE, 1000, FALSE, NULL, 0, 0)",
        )
        .bind(policy_set_public_id)
        .bind(format!("Request Policy {}", policy_set_public_id.simple()))
        .execute(pool)
        .await?;
        Ok(policy_set_public_id)
    }

    async fn create_request_with_policy_set(
        pool: &sqlx::PgPool,
        policy_set_public_id: Uuid,
        query_text: &str,
    ) -> anyhow::Result<SearchRequestCreateRow> {
        let input = SearchRequestCreateInput {
            actor_user_public_id: None,
            query_text,
            query_type: "free_text",
            torznab_mode: Some("generic"),
            requested_media_domain_key: Some("movies"),
            page_size: Some(50),
            search_profile_public_id: None,
            request_policy_set_public_id: Some(policy_set_public_id),
            season_number: None,
            episode_number: None,
            identifier_types: None,
            identifier_values: None,
            torznab_cat_ids: None,
        };
        search_request_create(pool, &input)
            .await
            .map_err(Into::into)
    }

    async fn create_search_request_for_runs(
        pool: &PgPool,
        query_text: &str,
    ) -> anyhow::Result<Uuid> {
        let input = SearchRequestCreateInput {
            actor_user_public_id: None,
            query_text,
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
        };
        let row = search_request_create(pool, &input).await?;
        Ok(row.search_request_public_id)
    }

    async fn force_search_request_running(
        pool: &PgPool,
        search_request_public_id: Uuid,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE search_request
             SET status = 'running',
                 finished_at = NULL,
                 canceled_at = NULL,
                 failure_class = NULL,
                 error_detail = NULL
             WHERE search_request_public_id = $1",
        )
        .bind(search_request_public_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    async fn create_indexer_instance_for_runs(pool: &PgPool) -> anyhow::Result<Uuid> {
        let definition_id: Option<i64> = sqlx::query_scalar(
            "SELECT indexer_definition_id
             FROM indexer_definition
             ORDER BY indexer_definition_id
             LIMIT 1",
        )
        .fetch_optional(pool)
        .await?;
        let definition_id = if let Some(existing_id) = definition_id {
            existing_id
        } else {
            let slug = format!("run-def-{}", Uuid::new_v4().simple());
            sqlx::query_scalar(
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
                    'prowlarr_indexers',
                    $1,
                    $2,
                    'torrent',
                    'torznab',
                    1,
                    '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
                    FALSE
                )
                RETURNING indexer_definition_id",
            )
            .bind(&slug)
            .bind(format!("Run Definition {slug}"))
            .fetch_one(pool)
            .await?
        };
        let indexer_instance_public_id = Uuid::new_v4();
        let display_name = format!("Run Instance {}", indexer_instance_public_id.simple());
        sqlx::query(
            "INSERT INTO indexer_instance (
                indexer_instance_public_id,
                indexer_definition_id,
                display_name,
                is_enabled,
                enable_rss,
                enable_automatic_search,
                enable_interactive_search,
                priority,
                created_by_user_id,
                updated_by_user_id
            )
            VALUES ($1, $2, $3, TRUE, TRUE, TRUE, TRUE, 50, 0, 0)",
        )
        .bind(indexer_instance_public_id)
        .bind(definition_id)
        .bind(display_name)
        .execute(pool)
        .await?;
        Ok(indexer_instance_public_id)
    }

    #[derive(Debug, sqlx::FromRow)]
    struct SearchRunState {
        status: String,
        attempt_count: i32,
        rate_limited_attempt_count: i32,
        last_error_class: Option<String>,
        last_rate_limit_scope: Option<String>,
        next_attempt_at: Option<DateTime<Utc>>,
        error_class: Option<String>,
    }

    async fn fetch_search_run_state(
        pool: &PgPool,
        search_request_public_id: Uuid,
        indexer_instance_public_id: Uuid,
    ) -> anyhow::Result<SearchRunState> {
        sqlx::query_as(
            "SELECT
                run.status::TEXT AS status,
                run.attempt_count,
                run.rate_limited_attempt_count,
                run.last_error_class::TEXT AS last_error_class,
                run.last_rate_limit_scope::TEXT AS last_rate_limit_scope,
                run.next_attempt_at,
                run.error_class::TEXT AS error_class
             FROM search_request_indexer_run run
             JOIN search_request sr
               ON sr.search_request_id = run.search_request_id
             JOIN indexer_instance ii
               ON ii.indexer_instance_id = run.indexer_instance_id
             WHERE sr.search_request_public_id = $1
               AND ii.indexer_instance_public_id = $2",
        )
        .bind(search_request_public_id)
        .bind(indexer_instance_public_id)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
    }

    async fn ensure_deployment_config(pool: &sqlx::PgPool) -> anyhow::Result<()> {
        let config_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM deployment_config")
            .fetch_one(pool)
            .await?;
        if config_count == 0 {
            sqlx::query("INSERT INTO deployment_config DEFAULT VALUES")
                .execute(pool)
                .await?;
        }
        Ok(())
    }
    #[tokio::test]
    async fn search_request_create_requires_query() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let input = SearchRequestCreateInput {
            actor_user_public_id: None,
            query_text: "",
            query_type: "free_text",
            torznab_mode: Some("generic"),
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
        let err = search_request_create(pool, &input).await.unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("invalid_query"));
        Ok(())
    }

    #[tokio::test]
    async fn search_request_create_rejects_identifier_mismatch() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let identifier_types = vec!["imdb".to_string()];
        let identifier_values = vec!["tt1234567".to_string()];

        let input = SearchRequestCreateInput {
            actor_user_public_id: Some(actor),
            query_text: "Dune",
            query_type: "tmdb",
            torznab_mode: None,
            requested_media_domain_key: None,
            page_size: None,
            search_profile_public_id: None,
            request_policy_set_public_id: None,
            season_number: None,
            episode_number: None,
            identifier_types: Some(&identifier_types),
            identifier_values: Some(&identifier_values),
            torznab_cat_ids: None,
        };

        let err = search_request_create(pool, &input).await.unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("invalid_identifier_mismatch"));
        Ok(())
    }

    #[tokio::test]
    async fn search_request_create_rejects_tv_episode_without_season() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let input = SearchRequestCreateInput {
            actor_user_public_id: None,
            query_text: "Dune",
            query_type: "free_text",
            torznab_mode: Some("tv"),
            requested_media_domain_key: None,
            page_size: None,
            search_profile_public_id: None,
            request_policy_set_public_id: None,
            season_number: None,
            episode_number: Some(1),
            identifier_types: None,
            identifier_values: None,
            torznab_cat_ids: None,
        };

        let err = search_request_create(pool, &input).await.unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("invalid_season_episode_combo"));
        Ok(())
    }

    #[tokio::test]
    async fn search_request_create_rejects_unknown_category_filter() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let categories = vec![99999];

        let input = SearchRequestCreateInput {
            actor_user_public_id: Some(actor),
            query_text: "Dune",
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
            torznab_cat_ids: Some(&categories),
        };

        let err = search_request_create(pool, &input).await.unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("invalid_category_filter"));
        Ok(())
    }

    #[tokio::test]
    async fn search_request_create_maps_torznab_categories_to_effective_media_domain()
    -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let categories = vec![2000];

        let input = SearchRequestCreateInput {
            actor_user_public_id: Some(actor),
            query_text: "Mapped Categories",
            query_type: "free_text",
            torznab_mode: None,
            requested_media_domain_key: None,
            page_size: Some(50),
            search_profile_public_id: None,
            request_policy_set_public_id: None,
            season_number: None,
            episode_number: None,
            identifier_types: None,
            identifier_values: None,
            torznab_cat_ids: Some(&categories),
        };

        let created = search_request_create(pool, &input).await?;
        let domain_keys: (Option<String>, Option<String>) = sqlx::query_as(
            "SELECT
                req.media_domain_key::TEXT AS requested_key,
                eff.media_domain_key::TEXT AS effective_key
             FROM search_request sr
             LEFT JOIN media_domain req
               ON req.media_domain_id = sr.requested_media_domain_id
             LEFT JOIN media_domain eff
               ON eff.media_domain_id = sr.effective_media_domain_id
             WHERE sr.search_request_public_id = $1",
        )
        .bind(created.search_request_public_id)
        .fetch_one(pool)
        .await?;
        assert_eq!(domain_keys.0, None);
        assert_eq!(domain_keys.1.as_deref(), Some("movies"));

        let effective_category_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM search_request_torznab_category_effective sec
             JOIN torznab_category tc ON tc.torznab_category_id = sec.torznab_category_id
             JOIN search_request sr ON sr.search_request_id = sec.search_request_id
             WHERE sr.search_request_public_id = $1
               AND tc.torznab_cat_id = 2000",
        )
        .bind(created.search_request_public_id)
        .fetch_one(pool)
        .await?;
        assert_eq!(effective_category_count, 1);
        Ok(())
    }

    #[tokio::test]
    async fn search_request_create_rejects_category_filter_outside_profile_allowlist()
    -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let search_profile_public_id = search_profile_create(
            pool,
            actor,
            "TV Allowlist",
            Some(false),
            Some(50),
            None,
            None,
        )
        .await?;
        search_profile_set_domain_allowlist(
            pool,
            actor,
            search_profile_public_id,
            &[String::from("tv")],
        )
        .await?;

        let categories = vec![2000];
        let input = SearchRequestCreateInput {
            actor_user_public_id: Some(actor),
            query_text: "Movies Out Of Allowlist",
            query_type: "free_text",
            torznab_mode: None,
            requested_media_domain_key: None,
            page_size: Some(50),
            search_profile_public_id: Some(search_profile_public_id),
            request_policy_set_public_id: None,
            season_number: None,
            episode_number: None,
            identifier_types: None,
            identifier_values: None,
            torznab_cat_ids: Some(&categories),
        };

        let err = search_request_create(pool, &input).await.unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("invalid_category_filter"));
        Ok(())
    }

    #[tokio::test]
    async fn search_request_create_torznab_other_category_keeps_unrestricted_domain()
    -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let categories = vec![8000];

        let input = SearchRequestCreateInput {
            actor_user_public_id: None,
            query_text: "Other Category",
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
            torznab_cat_ids: Some(&categories),
        };

        let created = search_request_create(pool, &input).await?;
        let effective_domain: Option<String> = sqlx::query_scalar(
            "SELECT eff.media_domain_key::TEXT
             FROM search_request sr
             LEFT JOIN media_domain eff
               ON eff.media_domain_id = sr.effective_media_domain_id
             WHERE sr.search_request_public_id = $1",
        )
        .bind(created.search_request_public_id)
        .fetch_one(pool)
        .await?;
        assert_eq!(effective_domain, None);
        Ok(())
    }

    #[tokio::test]
    async fn search_request_create_torznab_multi_category_yields_multi_domain_scope()
    -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let categories = vec![2000, 5000];

        let input = SearchRequestCreateInput {
            actor_user_public_id: None,
            query_text: "Movies and TV",
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
            torznab_cat_ids: Some(&categories),
        };

        let created = search_request_create(pool, &input).await?;
        let effective_domain: Option<String> = sqlx::query_scalar(
            "SELECT eff.media_domain_key::TEXT
             FROM search_request sr
             LEFT JOIN media_domain eff
               ON eff.media_domain_id = sr.effective_media_domain_id
             WHERE sr.search_request_public_id = $1",
        )
        .bind(created.search_request_public_id)
        .fetch_one(pool)
        .await?;
        assert_eq!(effective_domain, None);

        let effective_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM search_request_torznab_category_effective sec
             JOIN torznab_category tc ON tc.torznab_category_id = sec.torznab_category_id
             JOIN search_request sr ON sr.search_request_id = sec.search_request_id
             WHERE sr.search_request_public_id = $1
               AND tc.torznab_cat_id IN (2000, 5000)",
        )
        .bind(created.search_request_public_id)
        .fetch_one(pool)
        .await?;
        assert_eq!(effective_count, 2);
        Ok(())
    }

    #[tokio::test]
    async fn search_request_cancel_requires_actor() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = search_request_cancel(pool, actor, Uuid::new_v4())
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("search_request_not_found"));
        Ok(())
    }
    #[tokio::test]
    async fn search_indexer_run_enqueue_requires_request() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let err = search_indexer_run_enqueue(pool, Uuid::new_v4(), Uuid::new_v4())
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("search_request_not_found"));
        Ok(())
    }
    #[tokio::test]
    async fn search_indexer_run_mark_failed_requires_request() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let input = SearchIndexerRunFailedInput {
            search_request_public_id: Uuid::new_v4(),
            indexer_instance_public_id: Uuid::new_v4(),
            error_class: "timeout",
            error_detail: None,
            retry_after_seconds: None,
            retry_seq: 0,
            rate_limit_scope: None,
        };
        let err = search_indexer_run_mark_failed(pool, &input)
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("search_request_not_found"));
        Ok(())
    }

    #[tokio::test]
    async fn search_indexer_run_mark_failed_rate_limited_uses_retry_and_scope() -> anyhow::Result<()>
    {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };
        let pool = test_db.pool();
        let search_request_public_id = create_search_request_for_runs(pool, "rate limited").await?;
        force_search_request_running(pool, search_request_public_id).await?;
        let indexer_instance_public_id = create_indexer_instance_for_runs(pool).await?;

        search_indexer_run_enqueue(pool, search_request_public_id, indexer_instance_public_id)
            .await?;

        let input = SearchIndexerRunFailedInput {
            search_request_public_id,
            indexer_instance_public_id,
            error_class: "rate_limited",
            error_detail: None,
            retry_after_seconds: None,
            retry_seq: 0,
            rate_limit_scope: Some("indexer_instance"),
        };
        search_indexer_run_mark_failed(pool, &input).await?;

        let state =
            fetch_search_run_state(pool, search_request_public_id, indexer_instance_public_id)
                .await?;
        assert_eq!(state.status.as_str(), "queued");
        assert_eq!(state.attempt_count, 1);
        assert_eq!(state.rate_limited_attempt_count, 1);
        assert_eq!(state.last_error_class.as_deref(), Some("rate_limited"));
        assert_eq!(
            state.last_rate_limit_scope.as_deref(),
            Some("indexer_instance")
        );
        assert!(state.next_attempt_at.is_some());
        assert_eq!(state.error_class, None);
        Ok(())
    }

    #[tokio::test]
    async fn search_indexer_run_mark_failed_transient_retries_before_limit() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };
        let pool = test_db.pool();
        let search_request_public_id =
            create_search_request_for_runs(pool, "transient retry").await?;
        force_search_request_running(pool, search_request_public_id).await?;
        let indexer_instance_public_id = create_indexer_instance_for_runs(pool).await?;

        search_indexer_run_enqueue(pool, search_request_public_id, indexer_instance_public_id)
            .await?;
        search_indexer_run_mark_started(pool, search_request_public_id, indexer_instance_public_id)
            .await?;

        let input = SearchIndexerRunFailedInput {
            search_request_public_id,
            indexer_instance_public_id,
            error_class: "timeout",
            error_detail: None,
            retry_after_seconds: None,
            retry_seq: 0,
            rate_limit_scope: None,
        };
        search_indexer_run_mark_failed(pool, &input).await?;

        let state =
            fetch_search_run_state(pool, search_request_public_id, indexer_instance_public_id)
                .await?;
        assert_eq!(state.status.as_str(), "queued");
        assert_eq!(state.last_error_class.as_deref(), Some("timeout"));
        assert_eq!(state.last_rate_limit_scope, None);
        assert!(state.next_attempt_at.is_some());
        assert_eq!(state.error_class, None);
        Ok(())
    }

    #[tokio::test]
    async fn search_indexer_run_mark_failed_transient_reaches_retry_limit() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };
        let pool = test_db.pool();
        let search_request_public_id =
            create_search_request_for_runs(pool, "transient terminal").await?;
        force_search_request_running(pool, search_request_public_id).await?;
        let indexer_instance_public_id = create_indexer_instance_for_runs(pool).await?;

        search_indexer_run_enqueue(pool, search_request_public_id, indexer_instance_public_id)
            .await?;
        search_indexer_run_mark_started(pool, search_request_public_id, indexer_instance_public_id)
            .await?;

        let input = SearchIndexerRunFailedInput {
            search_request_public_id,
            indexer_instance_public_id,
            error_class: "timeout",
            error_detail: Some("timeout retry limit reached"),
            retry_after_seconds: None,
            retry_seq: 3,
            rate_limit_scope: None,
        };
        search_indexer_run_mark_failed(pool, &input).await?;

        let state =
            fetch_search_run_state(pool, search_request_public_id, indexer_instance_public_id)
                .await?;
        assert_eq!(state.status.as_str(), "failed");
        assert_eq!(state.last_error_class.as_deref(), Some("timeout"));
        assert_eq!(state.error_class.as_deref(), Some("timeout"));
        assert!(state.next_attempt_at.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn search_request_create_reuses_policy_snapshot_by_hash_and_increments_ref_count()
    -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };
        let pool = test_db.pool();
        let request_policy_set_public_id = insert_request_policy_set(pool).await?;

        let first =
            create_request_with_policy_set(pool, request_policy_set_public_id, "first").await?;
        let second =
            create_request_with_policy_set(pool, request_policy_set_public_id, "second").await?;

        let first_snapshot: (i64, String, i32) = sqlx::query_as(
            "SELECT
                sr.policy_snapshot_id,
                ps.snapshot_hash::text,
                ps.ref_count
             FROM search_request sr
             JOIN policy_snapshot ps
               ON ps.policy_snapshot_id = sr.policy_snapshot_id
             WHERE sr.search_request_public_id = $1",
        )
        .bind(first.search_request_public_id)
        .fetch_one(pool)
        .await?;

        let second_snapshot: (i64, String, i32) = sqlx::query_as(
            "SELECT
                sr.policy_snapshot_id,
                ps.snapshot_hash::text,
                ps.ref_count
             FROM search_request sr
             JOIN policy_snapshot ps
               ON ps.policy_snapshot_id = sr.policy_snapshot_id
             WHERE sr.search_request_public_id = $1",
        )
        .bind(second.search_request_public_id)
        .fetch_one(pool)
        .await?;

        assert_eq!(first_snapshot.0, second_snapshot.0);
        assert_eq!(first_snapshot.1, second_snapshot.1);
        assert_eq!(first_snapshot.2, 2);
        assert_eq!(second_snapshot.2, 2);
        Ok(())
    }

    #[tokio::test]
    async fn retention_purge_decrements_policy_snapshot_ref_count() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };
        let pool = test_db.pool();
        ensure_deployment_config(pool).await?;
        let request_policy_set_public_id = insert_request_policy_set(pool).await?;
        let created =
            create_request_with_policy_set(pool, request_policy_set_public_id, "old").await?;

        let snapshot_id: i64 = sqlx::query_scalar(
            "SELECT policy_snapshot_id
             FROM search_request
             WHERE search_request_public_id = $1",
        )
        .bind(created.search_request_public_id)
        .fetch_one(pool)
        .await?;

        sqlx::query(
            "UPDATE search_request
             SET status = 'finished',
                 finished_at = $2,
                 canceled_at = NULL
             WHERE search_request_public_id = $1",
        )
        .bind(created.search_request_public_id)
        .bind(test_db.now() - Duration::days(40))
        .execute(pool)
        .await?;

        job_run_retention_purge(pool).await?;

        let remaining_request_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM search_request
             WHERE search_request_public_id = $1",
        )
        .bind(created.search_request_public_id)
        .fetch_one(pool)
        .await?;
        assert_eq!(remaining_request_count, 0);

        let ref_count: i32 = sqlx::query_scalar(
            "SELECT ref_count
             FROM policy_snapshot
             WHERE policy_snapshot_id = $1",
        )
        .bind(snapshot_id)
        .fetch_one(pool)
        .await?;
        assert_eq!(ref_count, 0);
        Ok(())
    }
}
