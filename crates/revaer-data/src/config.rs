//! Configuration schema migrations and helpers shared across crates.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{Executor, FromRow, PgPool, Postgres};
use uuid::Uuid;

/// LISTEN/NOTIFY channel for configuration revision broadcasts.
pub const SETTINGS_CHANNEL: &str = "revaer_settings_changed";

/// Apply all configuration-related migrations (shared with runtime).
///
/// # Errors
///
/// Returns an error when migration execution fails.
pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    let migrator = sqlx::migrate!("./migrations");
    migrator
        .run(pool)
        .await
        .context("failed to execute configuration migrations")?;
    Ok(())
}

/// Raw projection of the `app_profile` table.
#[derive(Debug, Clone, FromRow)]
pub struct AppProfileRow {
    /// Primary key for the application profile row.
    pub id: Uuid,
    /// Friendly instance identifier.
    pub instance_name: String,
    /// Operational mode string (`setup` or `active`).
    pub mode: String,
    /// Monotonic revision number.
    pub version: i64,
    /// API HTTP port.
    pub http_port: i32,
    /// Bind address stored in the database.
    pub bind_addr: String,
    /// Telemetry configuration payload.
    pub telemetry: Value,
    /// Feature flags payload.
    pub features: Value,
    /// Immutable configuration keys payload.
    pub immutable_keys: Value,
}

/// Raw projection of the `engine_profile` table.
#[derive(Debug, Clone, FromRow)]
pub struct EngineProfileRow {
    /// Primary key for the engine profile.
    pub id: Uuid,
    /// Engine implementation identifier.
    pub implementation: String,
    /// Optional listening port.
    pub listen_port: Option<i32>,
    /// DHT enablement flag.
    pub dht: bool,
    /// Encryption policy string.
    pub encryption: String,
    /// Optional active torrent limit.
    pub max_active: Option<i32>,
    /// Optional global download cap.
    pub max_download_bps: Option<i64>,
    /// Optional global upload cap.
    pub max_upload_bps: Option<i64>,
    /// Default sequential flag.
    pub sequential_default: bool,
    /// Resume data directory.
    pub resume_dir: String,
    /// Download root directory.
    pub download_root: String,
    /// Tracker configuration payload.
    pub tracker: Value,
}

/// Raw projection of the `fs_policy` table.
#[derive(Debug, Clone, FromRow)]
pub struct FsPolicyRow {
    /// Primary key for the filesystem policy.
    pub id: Uuid,
    /// Root path for completed artifacts.
    pub library_root: String,
    /// Whether archives should be extracted.
    pub extract: bool,
    /// PAR2 verification policy.
    pub par2: String,
    /// Whether nested directory structures should be flattened.
    pub flatten: bool,
    /// Move mode string (`copy`, `move`, `hardlink`).
    pub move_mode: String,
    /// Paths to keep during cleanup (JSON payload).
    pub cleanup_keep: Value,
    /// Paths to drop during cleanup (JSON payload).
    pub cleanup_drop: Value,
    /// Optional chmod value for files.
    pub chmod_file: Option<String>,
    /// Optional chmod value for directories.
    pub chmod_dir: Option<String>,
    /// Optional owner override.
    pub owner: Option<String>,
    /// Optional group override.
    pub group: Option<String>,
    /// Optional umask override.
    pub umask: Option<String>,
    /// Allowed path prefixes payload.
    pub allow_paths: Value,
}

/// Raw projection of an active setup token.
#[derive(Debug, Clone, FromRow)]
pub struct ActiveTokenRow {
    /// Unique identifier for the token.
    pub id: Uuid,
    /// Hashed token string.
    pub token_hash: String,
    /// Expiration timestamp.
    pub expires_at: DateTime<Utc>,
}

/// Raw projection used for API key auth.
#[derive(Debug, Clone, FromRow)]
pub struct ApiKeyAuthRow {
    /// Stored hash of the API key.
    pub hash: String,
    /// Whether the key is currently enabled.
    pub enabled: bool,
    /// Optional human-readable label.
    pub label: Option<String>,
    /// Rate limit configuration payload.
    pub rate_limit: Value,
}

/// Raw projection of a stored secret.
#[derive(Debug, Clone, FromRow)]
pub struct SecretRow {
    /// Secret name.
    pub name: String,
    /// Encrypted secret bytes.
    pub ciphertext: Vec<u8>,
}

/// Input payload for inserting a history entry.
#[derive(Debug, Clone)]
pub struct HistoryInsert<'a> {
    /// Table or entity name recorded in history.
    pub kind: &'a str,
    /// Previous value recorded before the change.
    pub old: Option<Value>,
    /// New value stored after the change.
    pub new: Option<Value>,
    /// Actor responsible for the change.
    pub actor: &'a str,
    /// Human-readable reason for the change.
    pub reason: &'a str,
    /// Monotonic configuration revision associated with the change.
    pub revision: i64,
}

/// Input payload for inserting a setup token.
#[derive(Debug, Clone)]
pub struct NewSetupToken<'a> {
    /// Pre-hashed secret token.
    pub token_hash: &'a str,
    /// Expiration timestamp.
    pub expires_at: DateTime<Utc>,
    /// Issuer identity.
    pub issued_by: &'a str,
}

/// Persist a change event into `settings_history`.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn insert_history<'e, E>(executor: E, entry: HistoryInsert<'_>) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    let HistoryInsert {
        kind,
        old,
        new,
        actor,
        reason,
        revision,
    } = entry;

    sqlx::query(
        "SELECT revaer_config.insert_history(_kind => $1, _old => $2, _new => $3, _actor => $4, _reason => $5, _revision => $6)",
    )
        .bind(kind)
        .bind(old)
        .bind(new)
        .bind(actor)
        .bind(reason)
    .bind(revision)
    .execute(executor)
    .await
    .context("failed to insert settings history entry")?;

    Ok(())
}

/// Manually bump the configuration revision for a table that lacks triggers.
///
/// # Errors
///
/// Returns an error when the revision update or notification fails.
pub async fn bump_revision<'e, E>(executor: E, source_table: &str) -> Result<i64>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar("SELECT revaer_config.bump_revision(_source_table => $1)")
        .bind(source_table)
        .fetch_one(executor)
        .await
        .context("failed to bump settings revision")
}

/// Remove expired setup tokens that have not been consumed.
///
/// # Errors
///
/// Returns an error if the delete statement fails.
pub async fn cleanup_expired_setup_tokens<'e, E>(executor: E) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.cleanup_expired_setup_tokens()")
        .execute(executor)
        .await
        .context("failed to remove expired setup tokens")?;
    Ok(())
}

/// Mark all active setup tokens as consumed.
///
/// # Errors
///
/// Returns an error if the update statement fails.
pub async fn invalidate_active_setup_tokens<'e, E>(executor: E) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.invalidate_active_setup_tokens()")
        .execute(executor)
        .await
        .context("failed to invalidate active setup tokens")?;
    Ok(())
}

/// Insert a freshly issued setup token.
///
/// # Errors
///
/// Returns an error if the insert fails.
pub async fn insert_setup_token<'e, E>(executor: E, token: &NewSetupToken<'_>) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.insert_setup_token(_token_hash => $1, _expires_at => $2, _issued_by => $3)",
    )
        .bind(token.token_hash)
        .bind(token.expires_at)
        .bind(token.issued_by)
    .execute(executor)
    .await
    .context("failed to persist setup token")?;
    Ok(())
}

/// Mark a setup token as consumed (either expired or actively used).
///
/// # Errors
///
/// Returns an error if the update fails.
pub async fn mark_setup_token_consumed<'e, E>(executor: E, token_id: Uuid) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.consume_setup_token(_token_id => $1)")
        .bind(token_id)
        .execute(executor)
        .await
        .context("failed to consume setup token")?;
    Ok(())
}

/// Load a secret row by name.
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn fetch_secret_by_name<'e, E>(executor: E, name: &str) -> Result<Option<SecretRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, SecretRow>("SELECT * FROM revaer_config.fetch_secret_by_name(_name => $1)")
        .bind(name)
        .fetch_optional(executor)
        .await
        .context("failed to load secret row by name")
}

/// String columns on `fs_policy` that store textual values.
#[derive(Debug, Clone, Copy)]
pub enum FsStringField {
    /// Destination root.
    LibraryRoot,
    /// PAR2 policy.
    Par2,
    /// Move mode.
    MoveMode,
}

impl FsStringField {
    const fn column(self) -> &'static str {
        match self {
            Self::LibraryRoot => "library_root",
            Self::Par2 => "par2",
            Self::MoveMode => "move_mode",
        }
    }
}

/// Boolean columns on `fs_policy`.
#[derive(Debug, Clone, Copy)]
pub enum FsBooleanField {
    /// Extract flag.
    Extract,
    /// Flatten flag.
    Flatten,
}

impl FsBooleanField {
    const fn column(self) -> &'static str {
        match self {
            Self::Extract => "extract",
            Self::Flatten => "flatten",
        }
    }
}

/// Array/JSON columns on `fs_policy`.
#[derive(Debug, Clone, Copy)]
pub enum FsArrayField {
    /// Cleanup keep patterns.
    CleanupKeep,
    /// Cleanup drop patterns.
    CleanupDrop,
    /// Allowed paths array.
    AllowPaths,
}

impl FsArrayField {
    const fn column(self) -> &'static str {
        match self {
            Self::CleanupKeep => "cleanup_keep",
            Self::CleanupDrop => "cleanup_drop",
            Self::AllowPaths => "allow_paths",
        }
    }
}

/// Optional text columns on `fs_policy`.
#[derive(Debug, Clone, Copy)]
pub enum FsOptionalStringField {
    /// File chmod field.
    ChmodFile,
    /// Directory chmod field.
    ChmodDir,
    /// Owner column.
    Owner,
    /// Group column.
    Group,
    /// Umask column.
    Umask,
}

impl FsOptionalStringField {
    const fn column(self) -> &'static str {
        match self {
            Self::ChmodFile => "chmod_file",
            Self::ChmodDir => "chmod_dir",
            Self::Owner => "owner",
            Self::Group => "group",
            Self::Umask => "umask",
        }
    }
}

/// Load the application profile row for the provided identifier.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_app_profile_row<'e, E>(executor: E, id: Uuid) -> Result<AppProfileRow>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, AppProfileRow>(
        "SELECT * FROM revaer_config.fetch_app_profile_row(_id => $1)",
    )
    .bind(id)
    .fetch_one(executor)
    .await
    .context("failed to load app_profile row")
}

/// Load the engine profile row for the provided identifier.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_engine_profile_row<'e, E>(executor: E, id: Uuid) -> Result<EngineProfileRow>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, EngineProfileRow>(
        "SELECT * FROM revaer_config.fetch_engine_profile_row(_id => $1)",
    )
    .bind(id)
    .fetch_one(executor)
    .await
    .context("failed to load engine_profile row")
}

/// Load the filesystem policy row for the provided identifier.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_fs_policy_row<'e, E>(executor: E, id: Uuid) -> Result<FsPolicyRow>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, FsPolicyRow>("SELECT * FROM revaer_config.fetch_fs_policy_row(_id => $1)")
        .bind(id)
        .fetch_one(executor)
        .await
        .context("failed to load fs_policy row")
}

/// Fetch the monotonic configuration revision.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_revision<'e, E>(executor: E) -> Result<i64>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar("SELECT revaer_config.fetch_revision()")
        .fetch_one(executor)
        .await
        .context("failed to load settings revision")
}

/// Fetch the application profile document as JSON.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_app_profile_json<'e, E>(executor: E, id: Uuid) -> Result<Value>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar("SELECT revaer_config.fetch_app_profile_json(_id => $1)")
        .bind(id)
        .fetch_one(executor)
        .await
        .context("failed to fetch app_profile JSON")
}

/// Fetch the engine profile document as JSON.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_engine_profile_json<'e, E>(executor: E, id: Uuid) -> Result<Value>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar("SELECT revaer_config.fetch_engine_profile_json(_id => $1)")
        .bind(id)
        .fetch_one(executor)
        .await
        .context("failed to fetch engine_profile JSON")
}

/// Fetch the filesystem policy document as JSON.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_fs_policy_json<'e, E>(executor: E, id: Uuid) -> Result<Value>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar("SELECT revaer_config.fetch_fs_policy_json(_id => $1)")
        .bind(id)
        .fetch_one(executor)
        .await
        .context("failed to fetch fs_policy JSON")
}

/// Fetch the API key projection used by watchers (`[{key_id,...}]`).
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_api_keys_json<'e, E>(executor: E) -> Result<Value>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar("SELECT revaer_config.fetch_api_keys_json()")
        .fetch_one(executor)
        .await
        .context("failed to fetch auth_api_keys JSON")
}

/// Fetch a single active setup token row (if any) for the caller.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_active_setup_token<'e, E>(executor: E) -> Result<Option<ActiveTokenRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, ActiveTokenRow>("SELECT * FROM revaer_config.fetch_active_setup_token()")
        .fetch_optional(executor)
        .await
        .context("failed to fetch active setup token")
}

/// Fetch the API key authentication material for a given key identifier.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_api_key_auth<'e, E>(executor: E, key_id: &str) -> Result<Option<ApiKeyAuthRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, ApiKeyAuthRow>(
        "SELECT * FROM revaer_config.fetch_api_key_auth(_key_id => $1)",
    )
    .bind(key_id)
    .fetch_optional(executor)
    .await
    .context("failed to fetch API key auth material")
}

/// Fetch the hashed secret for a given API key.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn fetch_api_key_hash<'e, E>(executor: E, key_id: &str) -> Result<Option<String>>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT revaer_config.fetch_api_key_hash(_key_id => $1)",
    )
    .bind(key_id)
    .fetch_one(executor)
    .await
    .context("failed to fetch auth_api_keys.hash")
}

/// Delete an API key.
///
/// # Errors
///
/// Returns an error when the deletion fails.
pub async fn delete_api_key<'e, E>(executor: E, key_id: &str) -> Result<u64>
where
    E: Executor<'e, Database = Postgres>,
{
    let removed =
        sqlx::query_scalar::<_, i64>("SELECT revaer_config.delete_api_key(_key_id => $1)")
            .bind(key_id)
            .fetch_one(executor)
            .await
            .context("failed to delete auth_api_keys row")?;
    Ok(u64::try_from(removed).unwrap_or_default())
}

/// Update the hashed secret for an API key.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_api_key_hash<'e, E>(executor: E, key_id: &str, hash: &str) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_api_key_hash(_key_id => $1, _hash => $2)")
        .bind(key_id)
        .bind(hash)
        .execute(executor)
        .await
        .context("failed to update auth_api_keys.hash")?;
    Ok(())
}

/// Update the label for an API key.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_api_key_label<'e, E>(
    executor: E,
    key_id: &str,
    label: Option<&str>,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_api_key_label(_key_id => $1, _label => $2)")
        .bind(key_id)
        .bind(label)
        .execute(executor)
        .await
        .context("failed to update auth_api_keys.label")?;
    Ok(())
}

/// Update the enabled flag for an API key.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_api_key_enabled<'e, E>(executor: E, key_id: &str, enabled: bool) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_api_key_enabled(_key_id => $1, _enabled => $2)")
        .bind(key_id)
        .bind(enabled)
        .execute(executor)
        .await
        .context("failed to update auth_api_keys.enabled")?;
    Ok(())
}

/// Update the `rate_limit` column for an API key.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_api_key_rate_limit<'e, E>(
    executor: E,
    key_id: &str,
    payload: &Value,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_api_key_rate_limit(_key_id => $1, _rate_limit => $2)")
        .bind(key_id)
        .bind(payload)
        .execute(executor)
        .await
        .context("failed to update auth_api_keys.rate_limit")?;
    Ok(())
}

/// Insert a new API key.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn insert_api_key<'e, E>(
    executor: E,
    key_id: &str,
    hash: &str,
    label: Option<&str>,
    enabled: bool,
    rate_limit: &Value,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.insert_api_key(_key_id => $1, _hash => $2, _label => $3, _enabled => $4, _rate_limit => $5)",
    )
    .bind(key_id)
    .bind(hash)
    .bind(label)
    .bind(enabled)
    .bind(rate_limit)
    .execute(executor)
    .await
    .context("failed to insert auth_api_keys row")?;
    Ok(())
}

/// Upsert a value in `settings_secret`.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn upsert_secret<'e, E>(
    executor: E,
    name: &str,
    ciphertext: &[u8],
    actor: &str,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.upsert_secret(_name => $1, _ciphertext => $2, _actor => $3)")
        .bind(name)
        .bind(ciphertext)
        .bind(actor)
        .execute(executor)
        .await
        .context("failed to upsert settings_secret row")?;
    Ok(())
}

/// Delete a secret entry.
///
/// # Errors
///
/// Returns an error when the delete fails.
pub async fn delete_secret<'e, E>(executor: E, name: &str) -> Result<u64>
where
    E: Executor<'e, Database = Postgres>,
{
    let removed = sqlx::query_scalar::<_, i64>("SELECT revaer_config.delete_secret(_name => $1)")
        .bind(name)
        .fetch_one(executor)
        .await
        .context("failed to delete settings_secret row")?;
    Ok(u64::try_from(removed).unwrap_or_default())
}

/// Update the application `instance_name` field.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_app_instance_name<'e, E>(
    executor: E,
    id: Uuid,
    instance_name: &str,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_app_instance_name(_id => $1, _instance_name => $2)")
        .bind(id)
        .bind(instance_name)
        .execute(executor)
        .await
        .context("failed to update app_profile.instance_name")?;
    Ok(())
}

/// Update the application mode field.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_app_mode<'e, E>(executor: E, id: Uuid, mode: &str) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_app_mode(_id => $1, _mode => $2)")
        .bind(id)
        .bind(mode)
        .execute(executor)
        .await
        .context("failed to update app_profile.mode")?;
    Ok(())
}

/// Update the application HTTP port field.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_app_http_port<'e, E>(executor: E, id: Uuid, http_port: i32) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_app_http_port(_id => $1, _port => $2)")
        .bind(id)
        .bind(http_port)
        .execute(executor)
        .await
        .context("failed to update app_profile.http_port")?;
    Ok(())
}

/// Update the application bind address field.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_app_bind_addr<'e, E>(executor: E, id: Uuid, bind_addr: &str) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_app_bind_addr(_id => $1, _bind_addr => $2)")
        .bind(id)
        .bind(bind_addr)
        .execute(executor)
        .await
        .context("failed to update app_profile.bind_addr")?;
    Ok(())
}

/// Update the application telemetry JSON field.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_app_telemetry<'e, E>(executor: E, id: Uuid, telemetry: &Value) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_app_telemetry(_id => $1, _telemetry => $2)")
        .bind(id)
        .bind(telemetry)
        .execute(executor)
        .await
        .context("failed to update app_profile.telemetry")?;
    Ok(())
}

/// Update the application features JSON field.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_app_features<'e, E>(executor: E, id: Uuid, features: &Value) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_app_features(_id => $1, _features => $2)")
        .bind(id)
        .bind(features)
        .execute(executor)
        .await
        .context("failed to update app_profile.features")?;
    Ok(())
}

/// Update the application `immutable_keys` JSON field.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_app_immutable_keys<'e, E>(
    executor: E,
    id: Uuid,
    immutable_keys: &Value,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.update_app_immutable_keys(_id => $1, _immutable => $2)")
        .bind(id)
        .bind(immutable_keys)
        .execute(executor)
        .await
        .context("failed to update app_profile.immutable_keys")?;
    Ok(())
}

/// Increment the application profile version for optimistic locking.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn bump_app_profile_version<'e, E>(executor: E, id: Uuid) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT revaer_config.bump_app_profile_version(_id => $1)")
        .bind(id)
        .execute(executor)
        .await
        .context("failed to bump app_profile.version")?;
    Ok(())
}

/// Aggregated engine profile payload used for the unified update path.
#[derive(Debug, Clone)]
pub struct EngineProfileUpdate<'a> {
    /// Primary key for the engine profile row.
    pub id: Uuid,
    /// Engine implementation identifier.
    pub implementation: &'a str,
    /// Optional listen port override.
    pub listen_port: Option<i32>,
    /// DHT enablement flag.
    pub dht: bool,
    /// Encryption policy string.
    pub encryption: &'a str,
    /// Optional maximum active torrent count.
    pub max_active: Option<i32>,
    /// Optional global download cap in bytes per second.
    pub max_download_bps: Option<i64>,
    /// Optional global upload cap in bytes per second.
    pub max_upload_bps: Option<i64>,
    /// Whether sequential download is the default.
    pub sequential_default: bool,
    /// Directory for fast-resume payloads.
    pub resume_dir: &'a str,
    /// Root directory for active downloads.
    pub download_root: &'a str,
    /// Tracker configuration payload.
    pub tracker: &'a Value,
}

/// Update the engine profile in a single stored procedure call.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_engine_profile<'e, E>(
    executor: E,
    profile: &EngineProfileUpdate<'_>,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.update_engine_profile(_id => $1, _implementation => $2, _listen_port => $3, _dht => $4, _encryption => $5, _max_active => $6, _max_download_bps => $7, _max_upload_bps => $8, _sequential_default => $9, _resume_dir => $10, _download_root => $11, _tracker => $12)",
    )
    .bind(profile.id)
    .bind(profile.implementation)
    .bind(profile.listen_port)
    .bind(profile.dht)
    .bind(profile.encryption)
    .bind(profile.max_active)
    .bind(profile.max_download_bps)
    .bind(profile.max_upload_bps)
    .bind(profile.sequential_default)
    .bind(profile.resume_dir)
    .bind(profile.download_root)
    .bind(profile.tracker)
    .execute(executor)
    .await
    .context("failed to update engine_profile via unified procedure")?;
    Ok(())
}

/// Update a string column on `fs_policy`.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_fs_string_field<'e, E>(
    executor: E,
    id: Uuid,
    field: FsStringField,
    value: &str,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.update_fs_string_field(_id => $1, _column => $2, _value => $3)",
    )
    .bind(id)
    .bind(field.column())
    .bind(value)
    .execute(executor)
    .await
    .context("failed to update fs_policy string field")?;
    Ok(())
}

/// Update a boolean column on `fs_policy`.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_fs_boolean_field<'e, E>(
    executor: E,
    id: Uuid,
    field: FsBooleanField,
    value: bool,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.update_fs_boolean_field(_id => $1, _column => $2, _value => $3)",
    )
    .bind(id)
    .bind(field.column())
    .bind(value)
    .execute(executor)
    .await
    .context("failed to update fs_policy boolean field")?;
    Ok(())
}

/// Update an array/JSON column on `fs_policy`.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_fs_array_field<'e, E>(
    executor: E,
    id: Uuid,
    field: FsArrayField,
    value: &Value,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.update_fs_array_field(_id => $1, _column => $2, _value => $3)",
    )
    .bind(id)
    .bind(field.column())
    .bind(value)
    .execute(executor)
    .await
    .context("failed to update fs_policy array field")?;
    Ok(())
}

/// Update an optional string column on `fs_policy`.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_fs_optional_string_field<'e, E>(
    executor: E,
    id: Uuid,
    field: FsOptionalStringField,
    value: Option<&str>,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "SELECT revaer_config.update_fs_optional_string_field(_id => $1, _column => $2, _value => $3)",
    )
    .bind(id)
    .bind(field.column())
    .bind(value)
    .execute(executor)
    .await
    .context("failed to update fs_policy optional string field")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fs_column_mappings_are_stable() {
        assert_eq!(FsStringField::LibraryRoot.column(), "library_root");
        assert_eq!(FsStringField::Par2.column(), "par2");
        assert_eq!(FsStringField::MoveMode.column(), "move_mode");
        assert_eq!(FsBooleanField::Extract.column(), "extract");
        assert_eq!(FsBooleanField::Flatten.column(), "flatten");
        assert_eq!(FsArrayField::CleanupKeep.column(), "cleanup_keep");
        assert_eq!(FsArrayField::CleanupDrop.column(), "cleanup_drop");
        assert_eq!(FsArrayField::AllowPaths.column(), "allow_paths");
        assert_eq!(FsOptionalStringField::ChmodFile.column(), "chmod_file");
        assert_eq!(FsOptionalStringField::ChmodDir.column(), "chmod_dir");
        assert_eq!(FsOptionalStringField::Owner.column(), "owner");
        assert_eq!(FsOptionalStringField::Group.column(), "group");
        assert_eq!(FsOptionalStringField::Umask.column(), "umask");
    }
}
