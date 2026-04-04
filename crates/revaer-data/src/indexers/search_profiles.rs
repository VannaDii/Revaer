//! Stored-procedure access for search profile management.
//!
//! # Design
//! - Encapsulates search profile procedures behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const SEARCH_PROFILE_CREATE_CALL: &str = r"
    SELECT search_profile_create(
        actor_user_public_id => $1,
        display_name_input => $2,
        is_default_input => $3,
        page_size_input => $4,
        default_media_domain_key_input => $5,
        user_public_id_input => $6
    )
";

const SEARCH_PROFILE_UPDATE_CALL: &str = r"
    SELECT search_profile_update(
        actor_user_public_id => $1,
        search_profile_public_id_input => $2,
        display_name_input => $3,
        page_size_input => $4
    )
";

const SEARCH_PROFILE_SET_DEFAULT_CALL: &str = r"
    SELECT search_profile_set_default(
        actor_user_public_id => $1,
        search_profile_public_id_input => $2,
        page_size_input => $3
    )
";

const SEARCH_PROFILE_SET_DEFAULT_DOMAIN_CALL: &str = r"
    SELECT search_profile_set_default_domain(
        actor_user_public_id => $1,
        search_profile_public_id_input => $2,
        default_media_domain_key_input => $3
    )
";

const SEARCH_PROFILE_SET_DOMAIN_ALLOWLIST_CALL: &str = r"
    SELECT search_profile_set_domain_allowlist(
        actor_user_public_id => $1,
        search_profile_public_id_input => $2,
        media_domain_keys_input => $3
    )
";

const SEARCH_PROFILE_ADD_POLICY_SET_CALL: &str = r"
    SELECT search_profile_add_policy_set(
        actor_user_public_id => $1,
        search_profile_public_id_input => $2,
        policy_set_public_id_input => $3
    )
";

const SEARCH_PROFILE_REMOVE_POLICY_SET_CALL: &str = r"
    SELECT search_profile_remove_policy_set(
        actor_user_public_id => $1,
        search_profile_public_id_input => $2,
        policy_set_public_id_input => $3
    )
";

const SEARCH_PROFILE_INDEXER_ALLOW_CALL: &str = r"
    SELECT search_profile_indexer_allow(
        actor_user_public_id => $1,
        search_profile_public_id_input => $2,
        indexer_instance_public_ids_input => $3
    )
";

const SEARCH_PROFILE_INDEXER_BLOCK_CALL: &str = r"
    SELECT search_profile_indexer_block(
        actor_user_public_id => $1,
        search_profile_public_id_input => $2,
        indexer_instance_public_ids_input => $3
    )
";

const SEARCH_PROFILE_TAG_ALLOW_CALL: &str = r"
    SELECT search_profile_tag_allow(
        actor_user_public_id => $1,
        search_profile_public_id_input => $2,
        tag_public_ids_input => $3,
        tag_keys_input => $4
    )
";

const SEARCH_PROFILE_TAG_BLOCK_CALL: &str = r"
    SELECT search_profile_tag_block(
        actor_user_public_id => $1,
        search_profile_public_id_input => $2,
        tag_public_ids_input => $3,
        tag_keys_input => $4
    )
";

const SEARCH_PROFILE_TAG_PREFER_CALL: &str = r"
    SELECT search_profile_tag_prefer(
        actor_user_public_id => $1,
        search_profile_public_id_input => $2,
        tag_public_ids_input => $3,
        tag_keys_input => $4
    )
";

const SEARCH_PROFILE_LIST_CALL: &str = r"
    SELECT
        search_profile_public_id,
        display_name,
        is_default,
        page_size,
        default_media_domain_key::text AS default_media_domain_key,
        media_domain_keys,
        policy_set_public_ids,
        policy_set_display_names,
        allow_indexer_public_ids,
        block_indexer_public_ids,
        allow_tag_keys,
        block_tag_keys,
        prefer_tag_keys
    FROM indexer_search_profile_list(
        actor_user_public_id => $1
    )
";

/// Search-profile inventory row for operator read/list surfaces.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct SearchProfileListRow {
    /// Search-profile public identifier.
    pub search_profile_public_id: Uuid,
    /// Operator-facing display name.
    pub display_name: String,
    /// Whether the profile is the default.
    pub is_default: bool,
    /// Optional page-size override.
    pub page_size: Option<i32>,
    /// Optional default media-domain key.
    pub default_media_domain_key: Option<String>,
    /// Allowed media-domain keys.
    pub media_domain_keys: Vec<String>,
    /// Attached policy-set public identifiers.
    pub policy_set_public_ids: Vec<Uuid>,
    /// Attached policy-set display names.
    pub policy_set_display_names: Vec<String>,
    /// Allowed indexer-instance public identifiers.
    pub allow_indexer_public_ids: Vec<Uuid>,
    /// Blocked indexer-instance public identifiers.
    pub block_indexer_public_ids: Vec<Uuid>,
    /// Allowed tag keys.
    pub allow_tag_keys: Vec<String>,
    /// Blocked tag keys.
    pub block_tag_keys: Vec<String>,
    /// Preferred tag keys.
    pub prefer_tag_keys: Vec<String>,
}

/// Create a search profile.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_profile_create(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    display_name: &str,
    is_default: Option<bool>,
    page_size: Option<i32>,
    default_media_domain_key: Option<&str>,
    user_public_id: Option<Uuid>,
) -> Result<Uuid> {
    sqlx::query_scalar(SEARCH_PROFILE_CREATE_CALL)
        .bind(actor_user_public_id)
        .bind(display_name)
        .bind(is_default)
        .bind(page_size)
        .bind(default_media_domain_key)
        .bind(user_public_id)
        .fetch_one(pool)
        .await
        .map_err(try_op("search profile create"))
}

/// Update a search profile.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_profile_update(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    search_profile_public_id: Uuid,
    display_name: Option<&str>,
    page_size: Option<i32>,
) -> Result<Uuid> {
    sqlx::query_scalar(SEARCH_PROFILE_UPDATE_CALL)
        .bind(actor_user_public_id)
        .bind(search_profile_public_id)
        .bind(display_name)
        .bind(page_size)
        .fetch_one(pool)
        .await
        .map_err(try_op("search profile update"))
}

/// Set a search profile as default.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_profile_set_default(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    search_profile_public_id: Uuid,
    page_size: Option<i32>,
) -> Result<()> {
    sqlx::query(SEARCH_PROFILE_SET_DEFAULT_CALL)
        .bind(actor_user_public_id)
        .bind(search_profile_public_id)
        .bind(page_size)
        .execute(pool)
        .await
        .map_err(try_op("search profile set default"))?;
    Ok(())
}

/// Set a default media domain for a profile.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_profile_set_default_domain(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    search_profile_public_id: Uuid,
    default_media_domain_key: Option<&str>,
) -> Result<()> {
    sqlx::query(SEARCH_PROFILE_SET_DEFAULT_DOMAIN_CALL)
        .bind(actor_user_public_id)
        .bind(search_profile_public_id)
        .bind(default_media_domain_key)
        .execute(pool)
        .await
        .map_err(try_op("search profile set default domain"))?;
    Ok(())
}

/// Replace the domain allowlist for a profile.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_profile_set_domain_allowlist(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    search_profile_public_id: Uuid,
    media_domain_keys: &[String],
) -> Result<()> {
    sqlx::query(SEARCH_PROFILE_SET_DOMAIN_ALLOWLIST_CALL)
        .bind(actor_user_public_id)
        .bind(search_profile_public_id)
        .bind(media_domain_keys)
        .execute(pool)
        .await
        .map_err(try_op("search profile set domain allowlist"))?;
    Ok(())
}

/// Add a policy set to a search profile.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_profile_add_policy_set(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    search_profile_public_id: Uuid,
    policy_set_public_id: Uuid,
) -> Result<()> {
    sqlx::query(SEARCH_PROFILE_ADD_POLICY_SET_CALL)
        .bind(actor_user_public_id)
        .bind(search_profile_public_id)
        .bind(policy_set_public_id)
        .execute(pool)
        .await
        .map_err(try_op("search profile add policy set"))?;
    Ok(())
}

/// Remove a policy set from a search profile.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_profile_remove_policy_set(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    search_profile_public_id: Uuid,
    policy_set_public_id: Uuid,
) -> Result<()> {
    sqlx::query(SEARCH_PROFILE_REMOVE_POLICY_SET_CALL)
        .bind(actor_user_public_id)
        .bind(search_profile_public_id)
        .bind(policy_set_public_id)
        .execute(pool)
        .await
        .map_err(try_op("search profile remove policy set"))?;
    Ok(())
}

/// Allow an indexer instance for a search profile.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_profile_indexer_allow(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    search_profile_public_id: Uuid,
    indexer_instance_public_ids: &[Uuid],
) -> Result<()> {
    sqlx::query(SEARCH_PROFILE_INDEXER_ALLOW_CALL)
        .bind(actor_user_public_id)
        .bind(search_profile_public_id)
        .bind(indexer_instance_public_ids)
        .execute(pool)
        .await
        .map_err(try_op("search profile indexer allow"))?;
    Ok(())
}

/// Block an indexer instance for a search profile.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_profile_indexer_block(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    search_profile_public_id: Uuid,
    indexer_instance_public_ids: &[Uuid],
) -> Result<()> {
    sqlx::query(SEARCH_PROFILE_INDEXER_BLOCK_CALL)
        .bind(actor_user_public_id)
        .bind(search_profile_public_id)
        .bind(indexer_instance_public_ids)
        .execute(pool)
        .await
        .map_err(try_op("search profile indexer block"))?;
    Ok(())
}

/// Allow a tag for a search profile.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_profile_tag_allow(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    search_profile_public_id: Uuid,
    tag_public_ids: Option<&[Uuid]>,
    tag_keys: Option<&[String]>,
) -> Result<()> {
    sqlx::query(SEARCH_PROFILE_TAG_ALLOW_CALL)
        .bind(actor_user_public_id)
        .bind(search_profile_public_id)
        .bind(tag_public_ids)
        .bind(tag_keys)
        .execute(pool)
        .await
        .map_err(try_op("search profile tag allow"))?;
    Ok(())
}

/// Block a tag for a search profile.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_profile_tag_block(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    search_profile_public_id: Uuid,
    tag_public_ids: Option<&[Uuid]>,
    tag_keys: Option<&[String]>,
) -> Result<()> {
    sqlx::query(SEARCH_PROFILE_TAG_BLOCK_CALL)
        .bind(actor_user_public_id)
        .bind(search_profile_public_id)
        .bind(tag_public_ids)
        .bind(tag_keys)
        .execute(pool)
        .await
        .map_err(try_op("search profile tag block"))?;
    Ok(())
}

/// Prefer a tag for a search profile.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_profile_tag_prefer(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    search_profile_public_id: Uuid,
    tag_public_ids: Option<&[Uuid]>,
    tag_keys: Option<&[String]>,
) -> Result<()> {
    sqlx::query(SEARCH_PROFILE_TAG_PREFER_CALL)
        .bind(actor_user_public_id)
        .bind(search_profile_public_id)
        .bind(tag_public_ids)
        .bind(tag_keys)
        .execute(pool)
        .await
        .map_err(try_op("search profile tag prefer"))?;
    Ok(())
}

/// List search profiles for operator inventory flows.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the actor or query.
pub async fn search_profile_list(
    pool: &PgPool,
    actor_user_public_id: Uuid,
) -> Result<Vec<SearchProfileListRow>> {
    sqlx::query_as(SEARCH_PROFILE_LIST_CALL)
        .bind(actor_user_public_id)
        .fetch_all(pool)
        .await
        .map_err(try_op("search profile list"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataError;

    const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";
    async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
        crate::indexers::setup_indexer_db("indexer tests").await
    }
    #[tokio::test]
    async fn search_profile_create_update_roundtrip() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let profile_id =
            search_profile_create(pool, actor, "Default", Some(true), None, None, None).await?;

        let updated =
            search_profile_update(pool, actor, profile_id, Some("Default Updated"), Some(20))
                .await?;
        assert_eq!(profile_id, updated);
        Ok(())
    }
    #[tokio::test]
    async fn search_profile_set_default_requires_profile() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = search_profile_set_default(pool, actor, Uuid::new_v4(), None)
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("search_profile_not_found"));
        Ok(())
    }
    #[tokio::test]
    async fn search_profile_set_default_domain_requires_profile() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = search_profile_set_default_domain(pool, actor, Uuid::new_v4(), Some("movies"))
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("search_profile_not_found"));
        Ok(())
    }
    #[tokio::test]
    async fn search_profile_tag_allow_requires_profile() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = search_profile_tag_allow(pool, actor, Uuid::new_v4(), None, None)
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("search_profile_not_found"));
        Ok(())
    }
}
