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
    SELECT *
    FROM torznab_instance_create(
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
#[path = "torznab/tests.rs"]
mod tests;
