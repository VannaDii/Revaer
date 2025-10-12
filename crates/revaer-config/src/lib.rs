//! Database-backed configuration facade built on `PostgreSQL`.
//!
//! This module exposes a `SettingsFacade` trait and a concrete `ConfigService`
//! that coordinates migrations, safe reads, and LISTEN/NOTIFY driven updates
//! for runtime configuration.

use anyhow::{anyhow, ensure, Context, Result};
use argon2::password_hash::{
    rand_core::OsRng, Error as PasswordHashError, PasswordHash, PasswordHasher,
    PasswordVerifier, SaltString,
};
use argon2::Argon2;
use async_trait::async_trait;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use rand::distr::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sqlx::postgres::{PgListener, PgNotification, PgPoolOptions, PgRow};
use sqlx::{Executor, FromRow, Postgres, Row, Transaction};
use std::collections::HashSet;
use std::net::IpAddr;
use std::str::FromStr;
use std::time::Duration;
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

const APP_PROFILE_ID: &str = "00000000-0000-0000-0000-000000000001";
const ENGINE_PROFILE_ID: &str = "00000000-0000-0000-0000-000000000002";
const FS_POLICY_ID: &str = "00000000-0000-0000-0000-000000000003";
const SETTINGS_CHANNEL: &str = "revaer_settings_changed";

/// High-level view of the application profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppProfile {
    pub id: Uuid,
    pub instance_name: String,
    pub mode: AppMode,
    pub version: i64,
    pub http_port: i32,
    pub bind_addr: IpAddr,
    pub telemetry: Value,
    pub features: Value,
    pub immutable_keys: Value,
}

/// Setup or active mode flag recorded in `app_profile.mode`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AppMode {
    Setup,
    Active,
}

impl FromStr for AppMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "setup" => Ok(AppMode::Setup),
            "active" => Ok(AppMode::Active),
            other => Err(anyhow!("invalid app mode '{other}'")),
        }
    }
}

impl AppMode {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            AppMode::Setup => "setup",
            AppMode::Active => "active",
        }
    }
}

/// Engine configuration surfaced to consumers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineProfile {
    pub id: Uuid,
    pub implementation: String,
    pub listen_port: Option<i32>,
    pub dht: bool,
    pub encryption: String,
    pub max_active: Option<i32>,
    pub max_download_bps: Option<i64>,
    pub max_upload_bps: Option<i64>,
    pub sequential_default: bool,
    pub resume_dir: String,
    pub download_root: String,
    pub tracker: Value,
}

/// Filesystem policy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsPolicy {
    pub id: Uuid,
    pub library_root: String,
    pub extract: bool,
    pub par2: String,
    pub flatten: bool,
    pub move_mode: String,
    pub cleanup_keep: Value,
    pub cleanup_drop: Value,
    pub chmod_file: Option<String>,
    pub chmod_dir: Option<String>,
    pub owner: Option<String>,
    pub group: Option<String>,
    pub umask: Option<String>,
    pub allow_paths: Value,
}

#[async_trait]
pub trait SettingsFacade: Send + Sync {
    async fn get_app_profile(&self) -> Result<AppProfile>;
    async fn get_engine_profile(&self) -> Result<EngineProfile>;
    async fn get_fs_policy(&self) -> Result<FsPolicy>;
    async fn subscribe_changes(&self) -> Result<SettingsStream>;
    async fn apply_changeset(
        &self,
        actor: &str,
        reason: &str,
        changeset: SettingsChangeset,
    ) -> Result<AppliedChanges>;
    async fn issue_setup_token(&self, ttl: Duration, issued_by: &str) -> Result<SetupToken>;
    async fn consume_setup_token(&self, token: &str) -> Result<()>;
}

/// Structured change payload emitted by LISTEN/NOTIFY.
#[derive(Debug, Clone)]
pub struct SettingsChange {
    pub table: String,
    pub revision: i64,
    pub operation: String,
    pub payload: SettingsPayload,
}

/// Optional rich payload associated with a `SettingsChange`.
#[derive(Debug, Clone)]
pub enum SettingsPayload {
    AppProfile(AppProfile),
    EngineProfile(EngineProfile),
    FsPolicy(FsPolicy),
    None,
}

/// Structured errors emitted during configuration validation/mutation.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("immutable field '{field}' in '{section}' cannot be modified")]
    ImmutableField { section: String, field: String },

    #[error("invalid value for '{field}' in '{section}': {message}")]
    InvalidField {
        section: String,
        field: String,
        message: String,
    },

    #[error("unknown field '{field}' in '{section}' settings")]
    UnknownField { section: String, field: String },
}

/// Structured request describing modifications to config documents.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SettingsChangeset {
    pub app_profile: Option<Value>,
    pub engine_profile: Option<Value>,
    pub fs_policy: Option<Value>,
    pub api_keys: Vec<ApiKeyPatch>,
}

/// Patch description for API keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum ApiKeyPatch {
    Upsert {
        key_id: String,
        label: Option<String>,
        enabled: Option<bool>,
        secret: Option<String>,
        rate_limit: Option<Value>,
    },
    Delete {
        key_id: String,
    },
}

/// Context returned after applying a changeset.
#[derive(Debug, Clone)]
pub struct AppliedChanges {
    pub revision: i64,
    pub app_profile: Option<AppProfile>,
    pub engine_profile: Option<EngineProfile>,
    pub fs_policy: Option<FsPolicy>,
}

/// Token representation surfaced to the caller. The plaintext value is only
/// available at issuance time.
#[derive(Debug, Clone)]
pub struct SetupToken {
    pub plaintext: String,
    pub expires_at: DateTime<Utc>,
}

/// Stream wrapper around a `PostgreSQL` LISTEN connection.
pub struct SettingsStream {
    pool: sqlx::PgPool,
    listener: PgListener,
}

impl SettingsStream {
    pub async fn next(&mut self) -> Option<Result<SettingsChange>> {
        match self.listener.recv().await {
            Ok(notification) => {
                let result = handle_notification(&self.pool, notification).await;
                Some(result)
            }
            Err(err) => Some(Err(err.into())),
        }
    }
}

/// Concrete implementation backed by `PostgreSQL` + `SQLx`.
#[derive(Clone)]
pub struct ConfigService {
    pool: sqlx::PgPool,
    database_url: String,
}

impl ConfigService {
    /// Establish a connection pool and ensure migrations are applied.
    #[instrument(name = "config_service.new", skip(database_url))]
    #[allow(clippy::missing_errors_doc)]
    pub async fn new(database_url: impl Into<String>) -> Result<Self> {
        let database_url = database_url.into();
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .acquire_timeout(Duration::from_secs(10))
            .connect(&database_url)
            .await
            .with_context(|| "failed to connect to PostgreSQL for configuration service")?;

        apply_migrations(&pool).await?;

        Ok(Self { pool, database_url })
    }

    #[must_use]
    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }

    #[allow(clippy::missing_errors_doc)]
    pub async fn snapshot(&self) -> Result<ConfigSnapshot> {
        let app = fetch_app_profile(&self.pool).await?;
        let engine = fetch_engine_profile(&self.pool).await?;
        let fs = fetch_fs_policy(&self.pool).await?;
        let revision = fetch_revision(&self.pool).await?;

        Ok(ConfigSnapshot {
            revision,
            app_profile: app,
            engine_profile: engine,
            fs_policy: fs,
        })
    }
}

/// Captures a consistent view of configuration at a given revision.
#[derive(Debug, Clone)]
pub struct ConfigSnapshot {
    pub revision: i64,
    pub app_profile: AppProfile,
    pub engine_profile: EngineProfile,
    pub fs_policy: FsPolicy,
}

#[async_trait]
impl SettingsFacade for ConfigService {
    async fn get_app_profile(&self) -> Result<AppProfile> {
        fetch_app_profile(&self.pool).await
    }

    async fn get_engine_profile(&self) -> Result<EngineProfile> {
        fetch_engine_profile(&self.pool).await
    }

    async fn get_fs_policy(&self) -> Result<FsPolicy> {
        fetch_fs_policy(&self.pool).await
    }

    async fn subscribe_changes(&self) -> Result<SettingsStream> {
        let mut listener = PgListener::connect(&self.database_url)
            .await
            .context("failed to open LISTEN connection")?;
        listener
            .listen(SETTINGS_CHANNEL)
            .await
            .context("failed to LISTEN on settings channel")?;

        Ok(SettingsStream {
            pool: self.pool.clone(),
            listener,
        })
    }

    async fn apply_changeset(
        &self,
        actor: &str,
        reason: &str,
        changeset: SettingsChangeset,
    ) -> Result<AppliedChanges> {
        let mut tx = self.pool.begin().await?;

        let mut applied_app: Option<AppProfile> = None;
        let mut applied_engine: Option<EngineProfile> = None;
        let mut applied_fs: Option<FsPolicy> = None;
        let mut history_entries: Vec<(&'static str, Option<Value>, Option<Value>)> = Vec::new();
        let mut any_change = false;
        let app_document = fetch_app_profile_json(tx.as_mut()).await?;
        let immutable_keys = extract_immutable_keys(&app_document)?;

        if let Some(app_patch) = changeset.app_profile {
            let before = app_document.clone();
            if apply_app_profile_patch(&mut tx, &app_patch, &immutable_keys).await? {
                let after = fetch_app_profile_json(tx.as_mut()).await?;
                applied_app = Some(fetch_app_profile(tx.as_mut()).await?);
                history_entries.push(("app_profile", Some(before), Some(after)));
                any_change = true;
            }
        }

        if let Some(engine_patch) = changeset.engine_profile {
            let before = fetch_engine_profile_json(tx.as_mut()).await?;
            if apply_engine_profile_patch(&mut tx, &engine_patch, &immutable_keys).await? {
                let after = fetch_engine_profile_json(tx.as_mut()).await?;
                applied_engine = Some(fetch_engine_profile(tx.as_mut()).await?);
                history_entries.push(("engine_profile", Some(before), Some(after)));
                any_change = true;
            }
        }

        if let Some(fs_patch) = changeset.fs_policy {
            let before = fetch_fs_policy_json(tx.as_mut()).await?;
            if apply_fs_policy_patch(&mut tx, &fs_patch, &immutable_keys).await? {
                let after = fetch_fs_policy_json(tx.as_mut()).await?;
                applied_fs = Some(fetch_fs_policy(tx.as_mut()).await?);
                history_entries.push(("fs_policy", Some(before), Some(after)));
                any_change = true;
            }
        }

        if !changeset.api_keys.is_empty() {
            let before = fetch_api_keys_json(tx.as_mut()).await?;
            if apply_api_key_patches(&mut tx, &changeset.api_keys, &immutable_keys).await? {
                let after = fetch_api_keys_json(tx.as_mut()).await?;
                history_entries.push(("auth_api_keys", Some(before), Some(after)));
                any_change = true;
            }
        }

        let revision = fetch_revision(tx.as_mut()).await?;

        if any_change {
            for (kind, old, new) in history_entries {
                insert_history(&mut tx, kind, old, new, actor, reason, revision).await?;
            }
            tx.commit().await?;
        } else {
            tx.rollback().await?;
        }

        Ok(AppliedChanges {
            revision,
            app_profile: applied_app,
            engine_profile: applied_engine,
            fs_policy: applied_fs,
        })
    }

    async fn issue_setup_token(&self, ttl: Duration, issued_by: &str) -> Result<SetupToken> {
        ensure!(
            ttl.as_secs() > 0 || ttl.subsec_nanos() > 0,
            "setup token TTL must be positive"
        );
        let chrono_ttl =
            ChronoDuration::from_std(ttl).context("setup token TTL exceeds supported range")?;

        let mut tx = self.pool.begin().await?;
        cleanup_expired_setup_tokens(&mut tx).await?;
        invalidate_active_setup_tokens(&mut tx).await?;

        let plaintext = generate_token(32);
        let token_hash = hash_secret(&plaintext)?;
        let expires_at = Utc::now() + chrono_ttl;

        sqlx::query(
            r"
            INSERT INTO setup_tokens (token_hash, expires_at, issued_by)
            VALUES ($1, $2, $3)
            ",
        )
        .bind(&token_hash)
        .bind(expires_at)
        .bind(issued_by)
        .execute(tx.as_mut())
        .await
        .context("failed to persist setup token")?;

        let revision = fetch_revision(tx.as_mut()).await?;
        insert_history(
            &mut tx,
            "setup_token",
            None,
            Some(json!({
                "event": "issued",
                "issued_by": issued_by,
                "expires_at": expires_at
            })),
            issued_by,
            "issue_setup_token",
            revision,
        )
        .await?;

        tx.commit().await?;

        Ok(SetupToken {
            plaintext,
            expires_at,
        })
    }

    async fn consume_setup_token(&self, token: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        cleanup_expired_setup_tokens(&mut tx).await?;

        let active = sqlx::query_as::<_, ActiveTokenRow>(
            r"
            SELECT id, token_hash, expires_at
            FROM setup_tokens
            WHERE consumed_at IS NULL
            ORDER BY issued_at DESC
            LIMIT 1
            FOR UPDATE
            ",
        )
        .fetch_optional(tx.as_mut())
        .await
        .context("failed to query setup tokens")?;

        let Some(active) = active else {
            tx.rollback().await?;
            return Err(anyhow!("no active setup token"));
        };

        if active.expires_at <= Utc::now() {
            sqlx::query("UPDATE setup_tokens SET consumed_at = now() WHERE id = $1")
                .bind(active.id)
                .execute(tx.as_mut())
                .await
                .context("failed to expire stale token")?;
            tx.commit().await?;
            return Err(anyhow!("setup token expired"));
        }

        let matches = verify_secret(&active.token_hash, token)?;
        if !matches {
            tx.rollback().await?;
            return Err(anyhow!("invalid setup token"));
        }

        sqlx::query("UPDATE setup_tokens SET consumed_at = now() WHERE id = $1")
            .bind(active.id)
            .execute(tx.as_mut())
            .await
            .context("failed to consume setup token")?;

        let revision = fetch_revision(tx.as_mut()).await?;
        insert_history(
            &mut tx,
            "setup_token",
            None,
            Some(json!({"event": "consumed"})),
            "system",
            "consume_setup_token",
            revision,
        )
        .await?;

        tx.commit().await?;
        Ok(())
    }
}

async fn apply_migrations(pool: &sqlx::PgPool) -> Result<()> {
    // SAFETY: the path is relative to this crate; sqlx embeds migrations at compile time.
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("failed to apply configuration migrations")?;
    Ok(())
}

#[derive(FromRow)]
struct AppProfileRow {
    id: Uuid,
    instance_name: String,
    mode: String,
    version: i64,
    http_port: i32,
    bind_addr: String,
    telemetry: Value,
    features: Value,
    immutable_keys: Value,
}

#[derive(FromRow)]
struct EngineProfileRow {
    id: Uuid,
    implementation: String,
    listen_port: Option<i32>,
    dht: bool,
    encryption: String,
    max_active: Option<i32>,
    max_download_bps: Option<i64>,
    max_upload_bps: Option<i64>,
    sequential_default: bool,
    resume_dir: String,
    download_root: String,
    tracker: Value,
}

#[derive(FromRow)]
struct FsPolicyRow {
    id: Uuid,
    library_root: String,
    extract: bool,
    par2: String,
    flatten: bool,
    move_mode: String,
    cleanup_keep: Value,
    cleanup_drop: Value,
    chmod_file: Option<String>,
    chmod_dir: Option<String>,
    owner: Option<String>,
    group: Option<String>,
    umask: Option<String>,
    allow_paths: Value,
}

#[derive(FromRow)]
struct ActiveTokenRow {
    id: Uuid,
    token_hash: String,
    expires_at: DateTime<Utc>,
}

async fn fetch_app_profile<'e, E>(executor: E) -> Result<AppProfile>
where
    E: Executor<'e, Database = Postgres>,
{
    let row = sqlx::query_as::<_, AppProfileRow>(
        r"
        SELECT id, instance_name, mode, version, http_port, bind_addr, telemetry, features, immutable_keys
        FROM app_profile
        WHERE id = $1
        ",
    )
    .bind(parse_uuid(APP_PROFILE_ID)?)
    .fetch_one(executor)
    .await
    .context("failed to load app_profile")?;

    let mode = AppMode::from_str(&row.mode)?;

    Ok(AppProfile {
        id: row.id,
        instance_name: row.instance_name,
        mode,
        version: row.version,
        http_port: row.http_port,
        bind_addr: row
            .bind_addr
            .parse::<IpAddr>()
            .context("invalid bind_addr stored in app_profile")?,
        telemetry: row.telemetry,
        features: row.features,
        immutable_keys: row.immutable_keys,
    })
}

async fn fetch_engine_profile<'e, E>(executor: E) -> Result<EngineProfile>
where
    E: Executor<'e, Database = Postgres>,
{
    let row = sqlx::query_as::<_, EngineProfileRow>(
        r"
        SELECT id, implementation, listen_port, dht, encryption, max_active,
               max_download_bps, max_upload_bps, sequential_default,
               resume_dir, download_root, tracker
        FROM engine_profile
        WHERE id = $1
        ",
    )
    .bind(parse_uuid(ENGINE_PROFILE_ID)?)
    .fetch_one(executor)
    .await
    .context("failed to load engine_profile")?;

    Ok(EngineProfile {
        id: row.id,
        implementation: row.implementation,
        listen_port: row.listen_port,
        dht: row.dht,
        encryption: row.encryption,
        max_active: row.max_active,
        max_download_bps: row.max_download_bps,
        max_upload_bps: row.max_upload_bps,
        sequential_default: row.sequential_default,
        resume_dir: row.resume_dir,
        download_root: row.download_root,
        tracker: row.tracker,
    })
}

async fn fetch_fs_policy<'e, E>(executor: E) -> Result<FsPolicy>
where
    E: Executor<'e, Database = Postgres>,
{
    let row = sqlx::query_as::<_, FsPolicyRow>(
        r#"
        SELECT id, library_root, extract, par2, flatten, move_mode,
               cleanup_keep, cleanup_drop, chmod_file, chmod_dir,
               owner, "group", umask, allow_paths
        FROM fs_policy
        WHERE id = $1
        "#,
    )
    .bind(parse_uuid(FS_POLICY_ID)?)
    .fetch_one(executor)
    .await
    .context("failed to load fs_policy")?;

    Ok(FsPolicy {
        id: row.id,
        library_root: row.library_root,
        extract: row.extract,
        par2: row.par2,
        flatten: row.flatten,
        move_mode: row.move_mode,
        cleanup_keep: row.cleanup_keep,
        cleanup_drop: row.cleanup_drop,
        chmod_file: row.chmod_file,
        chmod_dir: row.chmod_dir,
        owner: row.owner,
        group: row.group,
        umask: row.umask,
        allow_paths: row.allow_paths,
    })
}

async fn fetch_revision<'e, E>(executor: E) -> Result<i64>
where
    E: Executor<'e, Database = Postgres>,
{
    let revision: i64 = sqlx::query("SELECT revision FROM settings_revision WHERE id = 1")
        .map(|row: PgRow| row.get::<i64, _>("revision"))
        .fetch_one(executor)
        .await
        .context("failed to load settings revision")?;
    Ok(revision)
}

async fn handle_notification(
    pool: &sqlx::PgPool,
    notification: PgNotification,
) -> Result<SettingsChange> {
    let payload = notification.payload();
    let mut parts = payload.split(':');
    let table = parts
        .next()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("invalid notification payload '{payload}'"))?;
    let revision = parts
        .next()
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| anyhow!("missing revision in notification payload '{payload}'"))?;
    let operation = parts.next().unwrap_or("UNKNOWN").to_string();

    let payload = match table.as_str() {
        "app_profile" => SettingsPayload::AppProfile(fetch_app_profile(pool).await?),
        "engine_profile" => SettingsPayload::EngineProfile(fetch_engine_profile(pool).await?),
        "fs_policy" => SettingsPayload::FsPolicy(fetch_fs_policy(pool).await?),
        _ => SettingsPayload::None,
    };

    Ok(SettingsChange {
        table,
        revision,
        operation,
        payload,
    })
}

async fn fetch_app_profile_json<'e, E>(executor: E) -> Result<Value>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar(
        r"
        SELECT to_jsonb(app_profile.*)
        FROM app_profile
        WHERE id = $1
        ",
    )
    .bind(parse_uuid(APP_PROFILE_ID)?)
    .fetch_one(executor)
    .await
    .context("failed to load app_profile document")
}

async fn fetch_engine_profile_json<'e, E>(executor: E) -> Result<Value>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar(
        r"
        SELECT to_jsonb(engine_profile.*)
        FROM engine_profile
        WHERE id = $1
        ",
    )
    .bind(parse_uuid(ENGINE_PROFILE_ID)?)
    .fetch_one(executor)
    .await
    .context("failed to load engine_profile document")
}

async fn fetch_fs_policy_json<'e, E>(executor: E) -> Result<Value>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar(
        r"
        SELECT to_jsonb(fs_policy.*)
        FROM fs_policy
        WHERE id = $1
        ",
    )
    .bind(parse_uuid(FS_POLICY_ID)?)
    .fetch_one(executor)
    .await
    .context("failed to load fs_policy document")
}

async fn fetch_api_keys_json<'e, E>(executor: E) -> Result<Value>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar(
        r"
        SELECT COALESCE(
            json_agg(
                json_build_object(
                    'key_id', key_id,
                    'label', label,
                    'enabled', enabled,
                    'rate_limit', rate_limit
                )
                ORDER BY created_at
            ),
            '[]'::jsonb
        )
        FROM auth_api_keys
        ",
    )
    .fetch_one(executor)
    .await
    .context("failed to load auth_api_keys document")
}

async fn insert_history(
    tx: &mut Transaction<'_, Postgres>,
    kind: &str,
    old: Option<Value>,
    new: Option<Value>,
    actor: &str,
    reason: &str,
    revision: i64,
) -> Result<()> {
    sqlx::query(
        r"
        INSERT INTO settings_history (kind, old, new, actor, reason, revision)
        VALUES ($1, $2, $3, $4, $5, $6)
        ",
    )
    .bind(kind)
    .bind(old)
    .bind(new)
    .bind(actor)
    .bind(reason)
    .bind(revision)
    .execute(tx.as_mut())
    .await
    .context("failed to record settings history")?;
    Ok(())
}

#[allow(clippy::too_many_lines, clippy::similar_names)]
async fn apply_app_profile_patch(
    tx: &mut Transaction<'_, Postgres>,
    patch: &Value,
    immutable_keys: &HashSet<String>,
) -> Result<bool> {
    let Some(map) = patch.as_object() else {
        return Err(ConfigError::InvalidField {
            section: "app_profile".to_string(),
            field: "<root>".to_string(),
            message: "changeset must be a JSON object".to_string(),
        }
        .into());
    };
    if map.is_empty() {
        return Ok(false);
    }

    let app_id = parse_uuid(APP_PROFILE_ID)?;
    let mut mutated = false;

    for (key, value) in map {
        ensure_mutable(immutable_keys, "app_profile", key)?;
        match key.as_str() {
            "instance_name" => {
                let Some(new_value) = value.as_str() else {
                    return Err(ConfigError::InvalidField {
                        section: "app_profile".to_string(),
                        field: "instance_name".to_string(),
                        message: "must be a string".to_string(),
                    }
                    .into());
                };
                sqlx::query("UPDATE app_profile SET instance_name = $1 WHERE id = $2")
                    .bind(new_value)
                    .bind(app_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "mode" => {
                let Some(mode_str) = value.as_str() else {
                    return Err(ConfigError::InvalidField {
                        section: "app_profile".to_string(),
                        field: "mode".to_string(),
                        message: "must be a string".to_string(),
                    }
                    .into());
                };
                let mode = AppMode::from_str(mode_str).map_err(|_| ConfigError::InvalidField {
                    section: "app_profile".to_string(),
                    field: "mode".to_string(),
                    message: format!("unsupported value '{mode_str}'"),
                })?;
                sqlx::query("UPDATE app_profile SET mode = $1 WHERE id = $2")
                    .bind(mode.as_str())
                    .bind(app_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "http_port" => {
                let Some(port) = value.as_i64() else {
                    return Err(ConfigError::InvalidField {
                        section: "app_profile".to_string(),
                        field: "http_port".to_string(),
                        message: "must be an integer".to_string(),
                    }
                    .into());
                };
                ensure!(
                    (1..=i64::from(u16::MAX)).contains(&port),
                    ConfigError::InvalidField {
                        section: "app_profile".to_string(),
                        field: "http_port".to_string(),
                        message: "must be between 1 and 65535".to_string(),
                    }
                );
                let port_i32 = i32::try_from(port).map_err(|_| ConfigError::InvalidField {
                    section: "app_profile".to_string(),
                    field: "http_port".to_string(),
                    message: "must fit within 32-bit signed integer range".to_string(),
                })?;
                sqlx::query("UPDATE app_profile SET http_port = $1 WHERE id = $2")
                    .bind(port_i32)
                    .bind(app_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "bind_addr" => {
                let Some(addr) = value.as_str() else {
                    return Err(ConfigError::InvalidField {
                        section: "app_profile".to_string(),
                        field: "bind_addr".to_string(),
                        message: "must be a string".to_string(),
                    }
                    .into());
                };
                if addr.parse::<IpAddr>().is_err() {
                    return Err(ConfigError::InvalidField {
                        section: "app_profile".to_string(),
                        field: "bind_addr".to_string(),
                        message: "must be a valid IP address".to_string(),
                    }
                    .into());
                }
                sqlx::query("UPDATE app_profile SET bind_addr = $1 WHERE id = $2")
                    .bind(addr)
                    .bind(app_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "telemetry" => {
                ensure!(
                    value.is_object(),
                    ConfigError::InvalidField {
                        section: "app_profile".to_string(),
                        field: "telemetry".to_string(),
                        message: "must be an object".to_string(),
                    }
                );
                sqlx::query("UPDATE app_profile SET telemetry = $1 WHERE id = $2")
                    .bind(value.clone())
                    .bind(app_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "features" => {
                ensure!(
                    value.is_object(),
                    ConfigError::InvalidField {
                        section: "app_profile".to_string(),
                        field: "features".to_string(),
                        message: "must be an object".to_string(),
                    }
                );
                sqlx::query("UPDATE app_profile SET features = $1 WHERE id = $2")
                    .bind(value.clone())
                    .bind(app_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "immutable_keys" => {
                ensure!(
                    value.is_array(),
                    ConfigError::InvalidField {
                        section: "app_profile".to_string(),
                        field: "immutable_keys".to_string(),
                        message: "must be an array".to_string(),
                    }
                );
                sqlx::query("UPDATE app_profile SET immutable_keys = $1 WHERE id = $2")
                    .bind(value.clone())
                    .bind(app_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            other => {
                return Err(ConfigError::UnknownField {
                    section: "app_profile".to_string(),
                    field: other.to_string(),
                }
                .into());
            }
        }
    }

    if mutated {
        sqlx::query("UPDATE app_profile SET version = version + 1 WHERE id = $1")
            .bind(app_id)
            .execute(tx.as_mut())
            .await?;
    }

    Ok(mutated)
}

#[allow(clippy::too_many_lines, clippy::similar_names)]
async fn apply_engine_profile_patch(
    tx: &mut Transaction<'_, Postgres>,
    patch: &Value,
    immutable_keys: &HashSet<String>,
) -> Result<bool> {
    let Some(map) = patch.as_object() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "<root>".to_string(),
            message: "changeset must be a JSON object".to_string(),
        }
        .into());
    };
    if map.is_empty() {
        return Ok(false);
    }

    let engine_id = parse_uuid(ENGINE_PROFILE_ID)?;
    let mut mutated = false;

    for (key, value) in map {
        ensure_mutable(immutable_keys, "engine_profile", key)?;
        match key.as_str() {
            "implementation" => {
                let Some(impl_name) = value.as_str() else {
                    return Err(ConfigError::InvalidField {
                        section: "engine_profile".to_string(),
                        field: "implementation".to_string(),
                        message: "must be a string".to_string(),
                    }
                    .into());
                };
                sqlx::query("UPDATE engine_profile SET implementation = $1 WHERE id = $2")
                    .bind(impl_name)
                    .bind(engine_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "listen_port" => {
                if value.is_null() {
                    sqlx::query("UPDATE engine_profile SET listen_port = NULL WHERE id = $1")
                        .bind(engine_id)
                        .execute(tx.as_mut())
                        .await?;
                } else {
                    let Some(port) = value.as_i64() else {
                        return Err(ConfigError::InvalidField {
                            section: "engine_profile".to_string(),
                            field: "listen_port".to_string(),
                            message: "must be an integer".to_string(),
                        }
                        .into());
                    };
                    ensure!(
                        (1..=i64::from(u16::MAX)).contains(&port),
                        ConfigError::InvalidField {
                            section: "engine_profile".to_string(),
                            field: "listen_port".to_string(),
                            message: "must be between 1 and 65535".to_string(),
                        }
                    );
                    let port_i32 = i32::try_from(port).map_err(|_| ConfigError::InvalidField {
                        section: "engine_profile".to_string(),
                        field: "listen_port".to_string(),
                        message: "must fit within 32-bit signed integer range".to_string(),
                    })?;
                    sqlx::query("UPDATE engine_profile SET listen_port = $1 WHERE id = $2")
                        .bind(port_i32)
                        .bind(engine_id)
                        .execute(tx.as_mut())
                        .await?;
                }
                mutated = true;
            }
            "dht" => {
                let Some(flag) = value.as_bool() else {
                    return Err(ConfigError::InvalidField {
                        section: "engine_profile".to_string(),
                        field: "dht".to_string(),
                        message: "must be a boolean".to_string(),
                    }
                    .into());
                };
                sqlx::query("UPDATE engine_profile SET dht = $1 WHERE id = $2")
                    .bind(flag)
                    .bind(engine_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "encryption" => {
                let Some(mode) = value.as_str() else {
                    return Err(ConfigError::InvalidField {
                        section: "engine_profile".to_string(),
                        field: "encryption".to_string(),
                        message: "must be a string".to_string(),
                    }
                    .into());
                };
                sqlx::query("UPDATE engine_profile SET encryption = $1 WHERE id = $2")
                    .bind(mode)
                    .bind(engine_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "max_active" => {
                if value.is_null() {
                    sqlx::query("UPDATE engine_profile SET max_active = NULL WHERE id = $1")
                        .bind(engine_id)
                        .execute(tx.as_mut())
                        .await?;
                } else {
                    let Some(max_active) = value.as_i64() else {
                        return Err(ConfigError::InvalidField {
                            section: "engine_profile".to_string(),
                            field: "max_active".to_string(),
                            message: "must be an integer".to_string(),
                        }
                        .into());
                    };
                    ensure!(
                        max_active >= 0 && max_active <= i64::from(i32::MAX),
                        ConfigError::InvalidField {
                            section: "engine_profile".to_string(),
                            field: "max_active".to_string(),
                            message: "must be within 0..=i32::MAX".to_string(),
                        }
                    );
                    let max_active_i32 =
                        i32::try_from(max_active).map_err(|_| ConfigError::InvalidField {
                            section: "engine_profile".to_string(),
                            field: "max_active".to_string(),
                            message: "must fit within 32-bit signed integer range".to_string(),
                        })?;
                    sqlx::query("UPDATE engine_profile SET max_active = $1 WHERE id = $2")
                        .bind(max_active_i32)
                        .bind(engine_id)
                        .execute(tx.as_mut())
                        .await?;
                }
                mutated = true;
            }
            "max_download_bps" => {
                if value.is_null() {
                    sqlx::query("UPDATE engine_profile SET max_download_bps = NULL WHERE id = $1")
                        .bind(engine_id)
                        .execute(tx.as_mut())
                        .await?;
                } else {
                    let Some(limit) = value.as_i64() else {
                        return Err(ConfigError::InvalidField {
                            section: "engine_profile".to_string(),
                            field: "max_download_bps".to_string(),
                            message: "must be an integer".to_string(),
                        }
                        .into());
                    };
                    ensure!(
                        limit >= 0,
                        ConfigError::InvalidField {
                            section: "engine_profile".to_string(),
                            field: "max_download_bps".to_string(),
                            message: "must be non-negative".to_string(),
                        }
                    );
                    sqlx::query("UPDATE engine_profile SET max_download_bps = $1 WHERE id = $2")
                        .bind(limit)
                        .bind(engine_id)
                        .execute(tx.as_mut())
                        .await?;
                }
                mutated = true;
            }
            "max_upload_bps" => {
                if value.is_null() {
                    sqlx::query("UPDATE engine_profile SET max_upload_bps = NULL WHERE id = $1")
                        .bind(engine_id)
                        .execute(tx.as_mut())
                        .await?;
                } else {
                    let Some(limit) = value.as_i64() else {
                        return Err(ConfigError::InvalidField {
                            section: "engine_profile".to_string(),
                            field: "max_upload_bps".to_string(),
                            message: "must be an integer".to_string(),
                        }
                        .into());
                    };
                    ensure!(
                        limit >= 0,
                        ConfigError::InvalidField {
                            section: "engine_profile".to_string(),
                            field: "max_upload_bps".to_string(),
                            message: "must be non-negative".to_string(),
                        }
                    );
                    sqlx::query("UPDATE engine_profile SET max_upload_bps = $1 WHERE id = $2")
                        .bind(limit)
                        .bind(engine_id)
                        .execute(tx.as_mut())
                        .await?;
                }
                mutated = true;
            }
            "sequential_default" => {
                let Some(flag) = value.as_bool() else {
                    return Err(ConfigError::InvalidField {
                        section: "engine_profile".to_string(),
                        field: "sequential_default".to_string(),
                        message: "must be a boolean".to_string(),
                    }
                    .into());
                };
                sqlx::query("UPDATE engine_profile SET sequential_default = $1 WHERE id = $2")
                    .bind(flag)
                    .bind(engine_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "resume_dir" => {
                let Some(resume_path) = value.as_str() else {
                    return Err(ConfigError::InvalidField {
                        section: "engine_profile".to_string(),
                        field: "resume_dir".to_string(),
                        message: "must be a string".to_string(),
                    }
                    .into());
                };
                sqlx::query("UPDATE engine_profile SET resume_dir = $1 WHERE id = $2")
                    .bind(resume_path)
                    .bind(engine_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "download_root" => {
                let Some(download_root) = value.as_str() else {
                    return Err(ConfigError::InvalidField {
                        section: "engine_profile".to_string(),
                        field: "download_root".to_string(),
                        message: "must be a string".to_string(),
                    }
                    .into());
                };
                sqlx::query("UPDATE engine_profile SET download_root = $1 WHERE id = $2")
                    .bind(download_root)
                    .bind(engine_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "tracker" => {
                ensure!(
                    value.is_object(),
                    ConfigError::InvalidField {
                        section: "engine_profile".to_string(),
                        field: "tracker".to_string(),
                        message: "must be an object".to_string(),
                    }
                );
                sqlx::query("UPDATE engine_profile SET tracker = $1 WHERE id = $2")
                    .bind(value.clone())
                    .bind(engine_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            other => {
                return Err(ConfigError::UnknownField {
                    section: "engine_profile".to_string(),
                    field: other.to_string(),
                }
                .into());
            }
        }
    }

    Ok(mutated)
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_lines, clippy::similar_names)]
async fn apply_fs_policy_patch(
    tx: &mut Transaction<'_, Postgres>,
    patch: &Value,
    immutable_keys: &HashSet<String>,
) -> Result<bool> {
    let Some(map) = patch.as_object() else {
        return Err(ConfigError::InvalidField {
            section: "fs_policy".to_string(),
            field: "<root>".to_string(),
            message: "changeset must be a JSON object".to_string(),
        }
        .into());
    };
    if map.is_empty() {
        return Ok(false);
    }

    let policy_id = parse_uuid(FS_POLICY_ID)?;
    let mut mutated = false;

    for (key, value) in map {
        ensure_mutable(immutable_keys, "fs_policy", key)?;
        match key.as_str() {
            "library_root" => {
                let Some(path) = value.as_str() else {
                    return Err(ConfigError::InvalidField {
                        section: "fs_policy".to_string(),
                        field: "library_root".to_string(),
                        message: "must be a string".to_string(),
                    }
                    .into());
                };
                sqlx::query("UPDATE fs_policy SET library_root = $1 WHERE id = $2")
                    .bind(path)
                    .bind(policy_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "extract" => {
                let Some(flag) = value.as_bool() else {
                    return Err(ConfigError::InvalidField {
                        section: "fs_policy".to_string(),
                        field: "extract".to_string(),
                        message: "must be a boolean".to_string(),
                    }
                    .into());
                };
                sqlx::query("UPDATE fs_policy SET extract = $1 WHERE id = $2")
                    .bind(flag)
                    .bind(policy_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "par2" => {
                let Some(mode) = value.as_str() else {
                    return Err(ConfigError::InvalidField {
                        section: "fs_policy".to_string(),
                        field: "par2".to_string(),
                        message: "must be a string".to_string(),
                    }
                    .into());
                };
                sqlx::query("UPDATE fs_policy SET par2 = $1 WHERE id = $2")
                    .bind(mode)
                    .bind(policy_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "flatten" => {
                let Some(flag) = value.as_bool() else {
                    return Err(ConfigError::InvalidField {
                        section: "fs_policy".to_string(),
                        field: "flatten".to_string(),
                        message: "must be a boolean".to_string(),
                    }
                    .into());
                };
                sqlx::query("UPDATE fs_policy SET flatten = $1 WHERE id = $2")
                    .bind(flag)
                    .bind(policy_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "move_mode" => {
                let Some(mode) = value.as_str() else {
                    return Err(ConfigError::InvalidField {
                        section: "fs_policy".to_string(),
                        field: "move_mode".to_string(),
                        message: "must be a string".to_string(),
                    }
                    .into());
                };
                sqlx::query("UPDATE fs_policy SET move_mode = $1 WHERE id = $2")
                    .bind(mode)
                    .bind(policy_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "cleanup_keep" => {
                ensure!(
                    value.is_array(),
                    ConfigError::InvalidField {
                        section: "fs_policy".to_string(),
                        field: "cleanup_keep".to_string(),
                        message: "must be an array".to_string(),
                    }
                );
                sqlx::query("UPDATE fs_policy SET cleanup_keep = $1 WHERE id = $2")
                    .bind(value.clone())
                    .bind(policy_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "cleanup_drop" => {
                ensure!(
                    value.is_array(),
                    ConfigError::InvalidField {
                        section: "fs_policy".to_string(),
                        field: "cleanup_drop".to_string(),
                        message: "must be an array".to_string(),
                    }
                );
                sqlx::query("UPDATE fs_policy SET cleanup_drop = $1 WHERE id = $2")
                    .bind(value.clone())
                    .bind(policy_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            "chmod_file" => {
                if value.is_null() {
                    sqlx::query("UPDATE fs_policy SET chmod_file = NULL WHERE id = $1")
                        .bind(policy_id)
                        .execute(tx.as_mut())
                        .await?;
                } else {
                    let Some(mode) = value.as_str() else {
                        return Err(ConfigError::InvalidField {
                            section: "fs_policy".to_string(),
                            field: "chmod_file".to_string(),
                            message: "must be a string".to_string(),
                        }
                        .into());
                    };
                    sqlx::query("UPDATE fs_policy SET chmod_file = $1 WHERE id = $2")
                        .bind(mode)
                        .bind(policy_id)
                        .execute(tx.as_mut())
                        .await?;
                }
                mutated = true;
            }
            "chmod_dir" => {
                if value.is_null() {
                    sqlx::query("UPDATE fs_policy SET chmod_dir = NULL WHERE id = $1")
                        .bind(policy_id)
                        .execute(tx.as_mut())
                        .await?;
                } else {
                    let Some(mode) = value.as_str() else {
                        return Err(ConfigError::InvalidField {
                            section: "fs_policy".to_string(),
                            field: "chmod_dir".to_string(),
                            message: "must be a string".to_string(),
                        }
                        .into());
                    };
                    sqlx::query("UPDATE fs_policy SET chmod_dir = $1 WHERE id = $2")
                        .bind(mode)
                        .bind(policy_id)
                        .execute(tx.as_mut())
                        .await?;
                }
                mutated = true;
            }
            "owner" => {
                if value.is_null() {
                    sqlx::query("UPDATE fs_policy SET owner = NULL WHERE id = $1")
                        .bind(policy_id)
                        .execute(tx.as_mut())
                        .await?;
                } else {
                    let Some(owner) = value.as_str() else {
                        return Err(ConfigError::InvalidField {
                            section: "fs_policy".to_string(),
                            field: "owner".to_string(),
                            message: "must be a string".to_string(),
                        }
                        .into());
                    };
                    sqlx::query("UPDATE fs_policy SET owner = $1 WHERE id = $2")
                        .bind(owner)
                        .bind(policy_id)
                        .execute(tx.as_mut())
                        .await?;
                }
                mutated = true;
            }
            "group" => {
                if value.is_null() {
                    sqlx::query(r#"UPDATE fs_policy SET "group" = NULL WHERE id = $1"#)
                        .bind(policy_id)
                        .execute(tx.as_mut())
                        .await?;
                } else {
                    let Some(group) = value.as_str() else {
                        return Err(ConfigError::InvalidField {
                            section: "fs_policy".to_string(),
                            field: "group".to_string(),
                            message: "must be a string".to_string(),
                        }
                        .into());
                    };
                    sqlx::query(r#"UPDATE fs_policy SET "group" = $1 WHERE id = $2"#)
                        .bind(group)
                        .bind(policy_id)
                        .execute(tx.as_mut())
                        .await?;
                }
                mutated = true;
            }
            "umask" => {
                if value.is_null() {
                    sqlx::query("UPDATE fs_policy SET umask = NULL WHERE id = $1")
                        .bind(policy_id)
                        .execute(tx.as_mut())
                        .await?;
                } else {
                    let Some(umask) = value.as_str() else {
                        return Err(ConfigError::InvalidField {
                            section: "fs_policy".to_string(),
                            field: "umask".to_string(),
                            message: "must be a string".to_string(),
                        }
                        .into());
                    };
                    sqlx::query("UPDATE fs_policy SET umask = $1 WHERE id = $2")
                        .bind(umask)
                        .bind(policy_id)
                        .execute(tx.as_mut())
                        .await?;
                }
                mutated = true;
            }
            "allow_paths" => {
                ensure!(
                    value.is_array(),
                    ConfigError::InvalidField {
                        section: "fs_policy".to_string(),
                        field: "allow_paths".to_string(),
                        message: "must be an array".to_string(),
                    }
                );
                sqlx::query("UPDATE fs_policy SET allow_paths = $1 WHERE id = $2")
                    .bind(value.clone())
                    .bind(policy_id)
                    .execute(tx.as_mut())
                    .await?;
                mutated = true;
            }
            other => {
                return Err(ConfigError::UnknownField {
                    section: "fs_policy".to_string(),
                    field: other.to_string(),
                }
                .into());
            }
        }
    }

    Ok(mutated)
}
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_lines, clippy::similar_names)]
async fn apply_api_key_patches(
    tx: &mut Transaction<'_, Postgres>,
    patches: &[ApiKeyPatch],
    immutable_keys: &HashSet<String>,
) -> Result<bool> {
    if patches.is_empty() {
        return Ok(false);
    }

    let mut changed = false;

    for patch in patches.iter().cloned() {
        match patch {
            ApiKeyPatch::Delete { key_id } => {
                ensure_mutable(immutable_keys, "auth_api_keys", "key_id")?;
                let result = sqlx::query("DELETE FROM auth_api_keys WHERE key_id = $1")
                    .bind(&key_id)
                    .execute(tx.as_mut())
                    .await?;
                if result.rows_affected() > 0 {
                    changed = true;
                }
            }
            ApiKeyPatch::Upsert {
                key_id,
                label,
                enabled,
                secret,
                rate_limit,
            } => {
                ensure_mutable(immutable_keys, "auth_api_keys", "key_id")?;

                let existing_hash: Option<String> =
                    sqlx::query_scalar("SELECT hash FROM auth_api_keys WHERE key_id = $1")
                        .bind(&key_id)
                        .fetch_optional(tx.as_mut())
                        .await?;

                if let Some(_hash) = existing_hash {
                    let mut touched = false;

                    if let Some(secret) = secret {
                        ensure_mutable(immutable_keys, "auth_api_keys", "secret")?;
                        let hash = hash_secret(&secret)?;
                        sqlx::query(
                            "UPDATE auth_api_keys SET hash = $1, updated_at = now() WHERE key_id = $2",
                        )
                        .bind(hash)
                        .bind(&key_id)
                        .execute(tx.as_mut())
                        .await?;
                        touched = true;
                    }

                    if let Some(label) = label {
                        ensure_mutable(immutable_keys, "auth_api_keys", "label")?;
                        sqlx::query(
                            "UPDATE auth_api_keys SET label = $1, updated_at = now() WHERE key_id = $2",
                        )
                        .bind(label)
                        .bind(&key_id)
                        .execute(tx.as_mut())
                        .await?;
                        touched = true;
                    }

                    if let Some(enabled) = enabled {
                        ensure_mutable(immutable_keys, "auth_api_keys", "enabled")?;
                        sqlx::query(
                            "UPDATE auth_api_keys SET enabled = $1, updated_at = now() WHERE key_id = $2",
                        )
                        .bind(enabled)
                        .bind(&key_id)
                        .execute(tx.as_mut())
                        .await?;
                        touched = true;
                    }

                    if let Some(rate_limit) = rate_limit {
                        ensure_mutable(immutable_keys, "auth_api_keys", "rate_limit")?;
                        sqlx::query(
                            "UPDATE auth_api_keys SET rate_limit = $1, updated_at = now() WHERE key_id = $2",
                        )
                        .bind(rate_limit)
                        .bind(&key_id)
                        .execute(tx.as_mut())
                        .await?;
                        touched = true;
                    }

                    if touched {
                        changed = true;
                    }
                } else {
                    let Some(secret) = secret else {
                        return Err(ConfigError::InvalidField {
                            section: "auth_api_keys".to_string(),
                            field: "secret".to_string(),
                            message: "required when creating a new API key".to_string(),
                        }
                        .into());
                    };

                    ensure_mutable(immutable_keys, "auth_api_keys", "secret")?;
                    if label.is_some() {
                        ensure_mutable(immutable_keys, "auth_api_keys", "label")?;
                    }
                    if enabled.is_some() {
                        ensure_mutable(immutable_keys, "auth_api_keys", "enabled")?;
                    }
                    if rate_limit.is_some() {
                        ensure_mutable(immutable_keys, "auth_api_keys", "rate_limit")?;
                    }

                    let hash = hash_secret(&secret)?;
                    let enabled = enabled.unwrap_or(true);
                    let rate_limit_value = rate_limit.unwrap_or_else(|| Value::Object(Map::new()));

                    sqlx::query(
                        r"
                        INSERT INTO auth_api_keys (key_id, hash, label, enabled, rate_limit)
                        VALUES ($1, $2, $3, $4, $5)
                        ",
                    )
                    .bind(&key_id)
                    .bind(hash)
                    .bind(label)
                    .bind(enabled)
                    .bind(rate_limit_value)
                    .execute(tx.as_mut())
                    .await?;
                    changed = true;
                }
            }
        }
    }

    Ok(changed)
}

async fn cleanup_expired_setup_tokens(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    sqlx::query("DELETE FROM setup_tokens WHERE consumed_at IS NULL AND expires_at <= now()")
        .execute(tx.as_mut())
        .await?;
    Ok(())
}

async fn invalidate_active_setup_tokens(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    sqlx::query("UPDATE setup_tokens SET consumed_at = now() WHERE consumed_at IS NULL")
        .execute(tx.as_mut())
        .await?;
    Ok(())
}

fn generate_token(length: usize) -> String {
    let mut rng = rand::rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric) as char)
        .take(length)
        .collect()
}

fn hash_secret(input: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    let hash = argon
        .hash_password(input.as_bytes(), &salt)
        .map_err(|err| anyhow!("failed to hash secret material: {err}"))?;
    Ok(hash.to_string())
}

fn verify_secret(expected_hash: &str, candidate: &str) -> Result<bool> {
    let parsed =
        PasswordHash::new(expected_hash).map_err(|err| anyhow!("invalid stored hash: {err}"))?;
    match Argon2::default().verify_password(candidate.as_bytes(), &parsed) {
        Ok(()) => Ok(true),
        Err(PasswordHashError::Password) => Ok(false),
        Err(err) => Err(anyhow!("failed to verify secret: {err}")),
    }
}

fn extract_immutable_keys(doc: &Value) -> Result<HashSet<String>> {
    let mut keys = HashSet::new();
    match doc.get("immutable_keys") {
        Some(Value::Array(items)) => {
            for item in items {
                let Some(key) = item.as_str() else {
                    return Err(ConfigError::InvalidField {
                        section: "app_profile".to_string(),
                        field: "immutable_keys".to_string(),
                        message: "entries must be strings".to_string(),
                    }
                    .into());
                };
                keys.insert(key.to_string());
            }
        }
        Some(_) => {
            return Err(ConfigError::InvalidField {
                section: "app_profile".to_string(),
                field: "immutable_keys".to_string(),
                message: "must be an array".to_string(),
            }
            .into());
        }
        None => {}
    }
    Ok(keys)
}

fn ensure_mutable(
    immutable_keys: &HashSet<String>,
    section: &str,
    field: &str,
) -> Result<(), ConfigError> {
    if field != "immutable_keys" {
        let scoped = format!("{section}.{field}");
        let scoped_wildcard = format!("{section}.*");
        if immutable_keys.contains(section)
            || immutable_keys.contains(field)
            || immutable_keys.contains(&scoped)
            || immutable_keys.contains(&scoped_wildcard)
        {
            return Err(ConfigError::ImmutableField {
                section: section.to_string(),
                field: field.to_string(),
            });
        }
    }
    Ok(())
}

fn parse_uuid(value: &str) -> Result<Uuid> {
    Uuid::parse_str(value).with_context(|| format!("invalid UUID literal '{value}'"))
}
