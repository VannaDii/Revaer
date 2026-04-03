//! Stored-procedure access for Torznab instance state.
//!
//! # Design
//! - Encapsulates Torznab instance state procedures behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const TORZNAB_INSTANCE_CREATE_CALL: &str = r"
    SELECT torznab_instance_create(
        actor_user_public_id => $1,
        search_profile_public_id_input => $2,
        display_name_input => $3
    )
";

const TORZNAB_INSTANCE_ROTATE_KEY_CALL: &str = r"
    SELECT torznab_instance_rotate_key(
        actor_user_public_id => $1,
        torznab_instance_public_id_input => $2
    )
";

const TORZNAB_INSTANCE_ENABLE_DISABLE_CALL: &str = r"
    SELECT torznab_instance_enable_disable(
        actor_user_public_id => $1,
        torznab_instance_public_id_input => $2,
        is_enabled_input => $3
    )
";

const TORZNAB_INSTANCE_SOFT_DELETE_CALL: &str = r"
    SELECT torznab_instance_soft_delete(
        actor_user_public_id => $1,
        torznab_instance_public_id_input => $2
    )
";

const TORZNAB_INSTANCE_AUTHENTICATE_CALL: &str = r"
    SELECT
        torznab_instance_id,
        search_profile_id,
        display_name
    FROM torznab_instance_authenticate(
        torznab_instance_public_id_input => $1,
        api_key_plaintext_input => $2
    )
";

const TORZNAB_CATEGORY_LIST_CALL: &str = r"
    SELECT torznab_cat_id, name
    FROM torznab_category_list()
";

const TORZNAB_DOWNLOAD_PREPARE_CALL: &str = r"
    SELECT redirect_url
    FROM torznab_download_prepare(
        torznab_instance_public_id_input => $1,
        canonical_torrent_source_public_id_input => $2
    )
";

const TORZNAB_INSTANCE_LIST_CALL: &str = r"
    SELECT
        torznab_instance_public_id,
        display_name,
        is_enabled,
        search_profile_public_id,
        search_profile_display_name
    FROM indexer_torznab_instance_list(
        actor_user_public_id => $1
    )
";

/// Credentials returned when creating a Torznab instance.
#[derive(Debug, Clone, FromRow)]
pub struct TorznabInstanceCredentials {
    /// Public ID for the new Torznab instance.
    pub torznab_instance_public_id: Uuid,
    /// Plaintext API key for the instance.
    pub api_key_plaintext: String,
}

/// Authentication response for Torznab instance access.
#[derive(Debug, Clone, FromRow)]
pub struct TorznabInstanceAuthRow {
    /// Internal Torznab instance id.
    pub torznab_instance_id: i64,
    /// Internal search profile id.
    pub search_profile_id: i64,
    /// Display name for the instance.
    pub display_name: String,
}

/// Torznab category record for caps responses.
#[derive(Debug, Clone, FromRow)]
pub struct TorznabCategoryRow {
    /// Torznab category id.
    pub torznab_cat_id: i32,
    /// Human-readable category name.
    pub name: String,
}

/// Operator-facing Torznab-instance inventory row.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct TorznabInstanceListRow {
    /// Torznab-instance public identifier.
    pub torznab_instance_public_id: Uuid,
    /// Operator-facing display name.
    pub display_name: String,
    /// Whether the endpoint is enabled.
    pub is_enabled: bool,
    /// Linked search-profile public identifier.
    pub search_profile_public_id: Uuid,
    /// Linked search-profile display name.
    pub search_profile_display_name: String,
}

/// Download target for Torznab redirects.
#[derive(Debug, Clone, FromRow)]
struct TorznabDownloadRow {
    /// Redirect URL for download or magnet.
    pub redirect_url: Option<String>,
}

/// Create a Torznab instance.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn torznab_instance_create(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    search_profile_public_id: Option<Uuid>,
    display_name: &str,
) -> Result<TorznabInstanceCredentials> {
    sqlx::query_as(TORZNAB_INSTANCE_CREATE_CALL)
        .bind(actor_user_public_id)
        .bind(search_profile_public_id)
        .bind(display_name)
        .fetch_one(pool)
        .await
        .map_err(try_op("torznab instance create"))
}

/// Rotate a Torznab instance API key.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn torznab_instance_rotate_key(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    torznab_instance_public_id: Uuid,
) -> Result<String> {
    sqlx::query_scalar(TORZNAB_INSTANCE_ROTATE_KEY_CALL)
        .bind(actor_user_public_id)
        .bind(torznab_instance_public_id)
        .fetch_one(pool)
        .await
        .map_err(try_op("torznab instance rotate key"))
}

/// Enable or disable a Torznab instance.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn torznab_instance_enable_disable(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    torznab_instance_public_id: Uuid,
    is_enabled: bool,
) -> Result<()> {
    sqlx::query(TORZNAB_INSTANCE_ENABLE_DISABLE_CALL)
        .bind(actor_user_public_id)
        .bind(torznab_instance_public_id)
        .bind(is_enabled)
        .execute(pool)
        .await
        .map_err(try_op("torznab instance enable disable"))?;
    Ok(())
}

/// Soft delete a Torznab instance.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn torznab_instance_soft_delete(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    torznab_instance_public_id: Uuid,
) -> Result<()> {
    sqlx::query(TORZNAB_INSTANCE_SOFT_DELETE_CALL)
        .bind(actor_user_public_id)
        .bind(torznab_instance_public_id)
        .execute(pool)
        .await
        .map_err(try_op("torznab instance soft delete"))?;
    Ok(())
}

/// Authenticate a Torznab instance API key.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn torznab_instance_authenticate(
    pool: &PgPool,
    torznab_instance_public_id: Uuid,
    api_key_plaintext: &str,
) -> Result<TorznabInstanceAuthRow> {
    sqlx::query_as(TORZNAB_INSTANCE_AUTHENTICATE_CALL)
        .bind(torznab_instance_public_id)
        .bind(api_key_plaintext)
        .fetch_one(pool)
        .await
        .map_err(try_op("torznab instance authenticate"))
}

/// List Torznab categories for caps responses.
///
/// # Errors
///
/// Returns an error if the stored procedure fails.
pub async fn torznab_category_list(pool: &PgPool) -> Result<Vec<TorznabCategoryRow>> {
    sqlx::query_as(TORZNAB_CATEGORY_LIST_CALL)
        .fetch_all(pool)
        .await
        .map_err(try_op("torznab category list"))
}

/// List Torznab instances for operator inventory flows.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the actor or query.
pub async fn torznab_instance_list(
    pool: &PgPool,
    actor_user_public_id: Uuid,
) -> Result<Vec<TorznabInstanceListRow>> {
    sqlx::query_as(TORZNAB_INSTANCE_LIST_CALL)
        .bind(actor_user_public_id)
        .fetch_all(pool)
        .await
        .map_err(try_op("torznab instance list"))
}

/// Prepare a Torznab download redirect and record an acquisition attempt.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn torznab_download_prepare(
    pool: &PgPool,
    torznab_instance_public_id: Uuid,
    canonical_torrent_source_public_id: Uuid,
) -> Result<Option<String>> {
    let row: TorznabDownloadRow = sqlx::query_as(TORZNAB_DOWNLOAD_PREPARE_CALL)
        .bind(torznab_instance_public_id)
        .bind(canonical_torrent_source_public_id)
        .fetch_one(pool)
        .await
        .map_err(try_op("torznab download prepare"))?;
    Ok(row.redirect_url)
}

#[cfg(test)]
mod tests {
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
        sqlx::query("UPDATE policy_set SET is_enabled = FALSE")
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
        sqlx::query(
            "INSERT INTO torznab_instance (
                search_profile_id,
                torznab_instance_public_id,
                display_name,
                api_key_hash,
                is_enabled
            )
            VALUES ($1, $2, $3, $4, TRUE)",
        )
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
    async fn torznab_download_prepare_prefers_magnet_and_records_started_attempt()
    -> anyhow::Result<()> {
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
    async fn torznab_download_prepare_uses_download_url_when_magnet_missing() -> anyhow::Result<()>
    {
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
    async fn torznab_download_prepare_records_failure_when_no_redirect_target() -> anyhow::Result<()>
    {
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
}
