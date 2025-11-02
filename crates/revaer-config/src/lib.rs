#![forbid(unsafe_code)]
#![deny(
    warnings,
    dead_code,
    unused,
    unused_imports,
    unused_must_use,
    unreachable_pub,
    clippy::all,
    clippy::pedantic,
    clippy::cargo,
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]
#![allow(clippy::module_name_repetitions)]
#![allow(unexpected_cfgs)]
#![allow(clippy::multiple_crate_versions)]

//! Database-backed configuration facade built on `PostgreSQL`.
//!
//! This module exposes a `SettingsFacade` trait and a concrete `ConfigService`
//! that coordinates migrations, safe reads, and LISTEN/NOTIFY driven updates
//! for runtime configuration.

use anyhow::{Context, Result, anyhow, ensure};
use argon2::Argon2;
use argon2::password_hash::{
    Error as PasswordHashError, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
    rand_core::OsRng,
};
use async_trait::async_trait;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use rand::Rng;
use rand::distr::Alphanumeric;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::postgres::{PgListener, PgNotification, PgPoolOptions, PgRow};
use sqlx::{Executor, FromRow, Postgres, Row, Transaction};
use std::collections::HashSet;
use std::convert::TryFrom;
use std::net::IpAddr;
use std::str::FromStr;
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;
use tracing::{info, instrument, warn};
use uuid::Uuid;

const APP_PROFILE_ID: &str = "00000000-0000-0000-0000-000000000001";
const ENGINE_PROFILE_ID: &str = "00000000-0000-0000-0000-000000000002";
const FS_POLICY_ID: &str = "00000000-0000-0000-0000-000000000003";
const SETTINGS_CHANNEL: &str = "revaer_settings_changed";

/// High-level view of the application profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppProfile {
    /// Primary key for the application profile row.
    pub id: Uuid,
    /// Friendly identifier displayed in user interfaces.
    pub instance_name: String,
    /// Operating mode (`setup` or `active`).
    pub mode: AppMode,
    /// Monotonic version used to detect concurrent updates.
    pub version: i64,
    /// HTTP port the API server should bind to.
    pub http_port: i32,
    /// IP address (and interface) the API server should bind to.
    pub bind_addr: IpAddr,
    /// Structured telemetry configuration (JSON object).
    pub telemetry: Value,
    /// Feature flags exposed to the application (JSON object).
    pub features: Value,
    /// Immutable keys that must not be edited by clients.
    pub immutable_keys: Value,
}

/// Setup or active mode flag recorded in `app_profile.mode`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AppMode {
    /// Provisioning mode that restricts APIs to setup operations.
    Setup,
    /// Normal operational mode.
    Active,
}

impl FromStr for AppMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "setup" => Ok(Self::Setup),
            "active" => Ok(Self::Active),
            other => Err(anyhow!("invalid app mode '{other}'")),
        }
    }
}

impl AppMode {
    #[must_use]
    /// Render the mode as its lowercase string representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Active => "active",
        }
    }
}

/// Engine configuration surfaced to consumers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineProfile {
    /// Primary key for the engine profile row.
    pub id: Uuid,
    /// Engine implementation identifier (e.g., `stub`, `libtorrent`).
    pub implementation: String,
    /// Optional TCP port the engine should listen on.
    pub listen_port: Option<i32>,
    /// Whether the engine enables the DHT subsystem.
    pub dht: bool,
    /// Encryption policy string forwarded to the engine.
    pub encryption: String,
    /// Maximum number of concurrent active torrents.
    pub max_active: Option<i32>,
    /// Global download cap in bytes per second.
    pub max_download_bps: Option<i64>,
    /// Global upload cap in bytes per second.
    pub max_upload_bps: Option<i64>,
    /// Whether torrents default to sequential download.
    pub sequential_default: bool,
    /// Filesystem path for storing resume data.
    pub resume_dir: String,
    /// Root directory for active downloads.
    pub download_root: String,
    /// Arbitrary tracker configuration payload (JSON object).
    pub tracker: Value,
}

/// Filesystem policy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsPolicy {
    /// Primary key for the filesystem policy row.
    pub id: Uuid,
    /// Destination directory for completed artifacts.
    pub library_root: String,
    /// Whether archives should be extracted automatically.
    pub extract: bool,
    /// PAR2 verification policy (`disabled`, `verify`, etc.).
    pub par2: String,
    /// Whether nested directory structures should be flattened.
    pub flatten: bool,
    /// Move mode (`copy`, `move`, `hardlink`).
    pub move_mode: String,
    /// Cleanup rules describing paths to retain (JSON array).
    pub cleanup_keep: Value,
    /// Cleanup rules describing paths to purge (JSON array).
    pub cleanup_drop: Value,
    /// Optional chmod value applied to files.
    pub chmod_file: Option<String>,
    /// Optional chmod value applied to directories.
    pub chmod_dir: Option<String>,
    /// Optional owner applied to moved files.
    pub owner: Option<String>,
    /// Optional group applied to moved files.
    pub group: Option<String>,
    /// Optional umask enforced during filesystem operations.
    pub umask: Option<String>,
    /// Allow-list of destination paths (JSON array).
    pub allow_paths: Value,
}

#[async_trait]
/// Abstraction over configuration backends used by the application service.
pub trait SettingsFacade: Send + Sync {
    /// Retrieve the current application profile.
    async fn get_app_profile(&self) -> Result<AppProfile>;
    /// Retrieve the current engine profile.
    async fn get_engine_profile(&self) -> Result<EngineProfile>;
    /// Retrieve the current filesystem policy.
    async fn get_fs_policy(&self) -> Result<FsPolicy>;
    /// Subscribe to configuration change notifications.
    async fn subscribe_changes(&self) -> Result<SettingsStream>;
    /// Apply a structured changeset attributed to an actor and reason.
    async fn apply_changeset(
        &self,
        actor: &str,
        reason: &str,
        changeset: SettingsChangeset,
    ) -> Result<AppliedChanges>;
    /// Issue a new setup token with a given TTL.
    async fn issue_setup_token(&self, ttl: Duration, issued_by: &str) -> Result<SetupToken>;
    /// Permanently consume a setup token.
    async fn consume_setup_token(&self, token: &str) -> Result<()>;
}

/// Structured change payload emitted by LISTEN/NOTIFY.
#[derive(Debug, Clone)]
pub struct SettingsChange {
    /// Database table that triggered the notification.
    pub table: String,
    /// Revision recorded after applying the change.
    pub revision: i64,
    /// Operation descriptor (`insert`, `update`, `delete`).
    pub operation: String,
    /// Optional payload describing the updated document.
    pub payload: SettingsPayload,
}

/// Optional rich payload associated with a `SettingsChange`.
#[derive(Debug, Clone)]
pub enum SettingsPayload {
    /// Application profile document that changed.
    AppProfile(AppProfile),
    /// Engine profile document that changed.
    EngineProfile(EngineProfile),
    /// Filesystem policy document that changed.
    FsPolicy(FsPolicy),
    /// Notification that did not include a payload.
    None,
}

/// Structured errors emitted during configuration validation/mutation.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Attempted to modify a field marked as immutable.
    #[error("immutable field '{field}' in '{section}' cannot be modified")]
    ImmutableField {
        /// Section containing the immutable field.
        section: String,
        /// Name of the immutable field.
        field: String,
    },

    /// Field contained an invalid value.
    #[error("invalid value for '{field}' in '{section}': {message}")]
    InvalidField {
        /// Section that failed validation.
        section: String,
        /// Field that failed validation.
        field: String,
        /// Human-readable error description.
        message: String,
    },

    /// Field did not exist in the target section.
    #[error("unknown field '{field}' in '{section}' settings")]
    UnknownField {
        /// Section where the unknown field was encountered.
        section: String,
        /// Name of the unexpected field.
        field: String,
    },
}

/// Structured request describing modifications to config documents.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SettingsChangeset {
    /// Optional application profile update payload.
    pub app_profile: Option<Value>,
    /// Optional engine profile update payload.
    pub engine_profile: Option<Value>,
    /// Optional filesystem policy update payload.
    pub fs_policy: Option<Value>,
    /// API key upserts/deletions included in the changeset.
    pub api_keys: Vec<ApiKeyPatch>,
    /// Secret store mutations included in the changeset.
    pub secrets: Vec<SecretPatch>,
}

/// Patch description for API keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum ApiKeyPatch {
    /// Insert or update an API key record.
    Upsert {
        /// Identifier for the API key.
        key_id: String,
        /// Optional human-readable label.
        label: Option<String>,
        /// Optional enabled flag override.
        enabled: Option<bool>,
        /// Optional new secret value.
        secret: Option<String>,
        /// Optional rate limit configuration payload.
        rate_limit: Option<Value>,
    },
    /// Remove an API key record.
    Delete {
        /// Identifier for the API key to remove.
        key_id: String,
    },
}

/// Patch description for secrets stored in `settings_secret`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum SecretPatch {
    /// Insert or update a secret value.
    Set {
        /// Secret key identifier.
        name: String,
        /// Secret value material.
        value: String,
    },
    /// Remove a secret entry.
    Delete {
        /// Secret key identifier to remove.
        name: String,
    },
}

/// Context returned after applying a changeset.
#[derive(Debug, Clone, Serialize)]
pub struct AppliedChanges {
    /// Revision recorded after the changeset was applied.
    pub revision: i64,
    /// New application profile snapshot when relevant.
    pub app_profile: Option<AppProfile>,
    /// Updated engine profile snapshot when relevant.
    pub engine_profile: Option<EngineProfile>,
    /// Updated filesystem policy snapshot when relevant.
    pub fs_policy: Option<FsPolicy>,
}

/// Token representation surfaced to the caller. The plaintext value is only
/// available at issuance time.
#[derive(Debug, Clone)]
pub struct SetupToken {
    /// Clear-text token value (only returned at issuance time).
    pub plaintext: String,
    /// Expiration timestamp for the token.
    pub expires_at: DateTime<Utc>,
}

/// Authentication context returned for a validated API key.
#[derive(Debug, Clone)]
pub struct ApiKeyAuth {
    /// Unique identifier associated with the API key record.
    pub key_id: String,
    /// Optional human-readable label for the key.
    pub label: Option<String>,
    /// Optional token-bucket rate limit applied to requests.
    pub rate_limit: Option<ApiKeyRateLimit>,
}

/// Token-bucket rate limit configuration applied per API key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeyRateLimit {
    /// Maximum number of requests allowed within a replenishment window.
    pub burst: u32,
    /// Duration between token replenishments.
    pub replenish_period: Duration,
}

impl ApiKeyRateLimit {
    /// Serialise the rate limit into a stable JSON representation.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "burst": self.burst,
            "per_seconds": self.replenish_period.as_secs(),
        })
    }
}

/// Stream wrapper around a `PostgreSQL` LISTEN connection.
pub struct SettingsStream {
    pool: sqlx::PgPool,
    listener: PgListener,
}

impl SettingsStream {
    /// Receive the next configuration change notification, falling back to polling if the
    /// LISTEN connection encounters an error.
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

    /// Access the underlying `SQLx` connection pool.
    #[must_use]
    pub const fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }

    /// Produce a strongly typed snapshot of the current configuration revision.
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

    /// Subscribe to configuration changes, falling back to polling if LISTEN fails.
    #[allow(clippy::missing_errors_doc)]
    pub async fn watch_settings(
        &self,
        poll_interval: Duration,
    ) -> Result<(ConfigSnapshot, ConfigWatcher)> {
        let snapshot = self.snapshot().await?;
        let stream = match self.subscribe_changes().await {
            Ok(stream) => Some(stream),
            Err(err) => {
                warn!(error = ?err, "failed to initialize LISTEN stream; polling only");
                None
            }
        };

        let watcher = ConfigWatcher {
            service: self.clone(),
            stream,
            poll_interval,
            last_revision: snapshot.revision,
        };

        Ok((snapshot, watcher))
    }

    /// Verify that a setup token exists and has not expired without consuming it.
    #[allow(clippy::missing_errors_doc)]
    pub async fn validate_setup_token(&self, token: &str) -> Result<()> {
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
            tx.rollback().await?;
            return Err(anyhow!("setup token expired"));
        }

        let matches = match verify_secret(&active.token_hash, token) {
            Ok(result) => result,
            Err(err) => {
                tx.rollback().await?;
                return Err(err);
            }
        };

        tx.rollback().await?;

        if matches {
            Ok(())
        } else {
            Err(anyhow!("invalid setup token"))
        }
    }

    /// Validate an API key/secret combination and return authorisation context.
    #[allow(clippy::missing_errors_doc)]
    pub async fn authenticate_api_key(
        &self,
        key_id: &str,
        secret: &str,
    ) -> Result<Option<ApiKeyAuth>> {
        let record = sqlx::query_as::<_, ApiKeyAuthRow>(
            r"
            SELECT hash, enabled, label, rate_limit
            FROM auth_api_keys
            WHERE key_id = $1
            LIMIT 1
            ",
        )
        .bind(key_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to verify API key")?;

        let Some(record) = record else {
            return Ok(None);
        };

        if !record.enabled {
            return Ok(None);
        }

        let matches = verify_secret(&record.hash, secret)?;
        if !matches {
            return Ok(None);
        }

        let rate_limit = parse_api_key_rate_limit(&record.rate_limit)
            .context("invalid rate_limit payload for API key")?;

        Ok(Some(ApiKeyAuth {
            key_id: key_id.to_string(),
            label: record.label,
            rate_limit,
        }))
    }
}

/// Captures a consistent view of configuration at a given revision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    /// Revision of the configuration snapshot.
    pub revision: i64,
    /// Application profile in effect for this revision.
    pub app_profile: AppProfile,
    /// Engine profile describing torrent runtime behaviour.
    pub engine_profile: EngineProfile,
    /// Filesystem policy applied to completed torrents.
    pub fs_policy: FsPolicy,
}

/// Watches configuration changes, automatically falling back to polling if
/// LISTEN/NOTIFY connectivity is interrupted.
pub struct ConfigWatcher {
    service: ConfigService,
    stream: Option<SettingsStream>,
    poll_interval: Duration,
    last_revision: i64,
}

impl ConfigWatcher {
    /// Await the next configuration snapshot reflecting any applied changes.
    #[allow(clippy::missing_errors_doc)]
    pub async fn next(&mut self) -> Result<ConfigSnapshot> {
        loop {
            if let Some(snapshot) = self.listen_once().await? {
                return Ok(snapshot);
            }

            sleep(self.poll_interval).await;

            if let Some(snapshot) = self.poll_once().await? {
                return Ok(snapshot);
            }
        }
    }

    /// Force the watcher into polling mode, discarding the current LISTEN stream.
    pub fn disable_listen(&mut self) {
        self.stream = None;
    }

    async fn listen_once(&mut self) -> Result<Option<ConfigSnapshot>> {
        if let Some(stream) = &mut self.stream {
            match stream.next().await {
                Some(Ok(change)) => {
                    let snapshot = self.service.snapshot().await?;
                    self.last_revision = change.revision.max(snapshot.revision);
                    return Ok(Some(snapshot));
                }
                Some(Err(err)) => {
                    warn!(
                        error = ?err,
                        "LISTEN connection dropped; switching to polling"
                    );
                    self.stream = None;
                }
                None => {
                    warn!("LISTEN stream closed; switching to polling");
                    self.stream = None;
                }
            }
        }
        Ok(None)
    }

    async fn poll_once(&mut self) -> Result<Option<ConfigSnapshot>> {
        let snapshot = self.service.snapshot().await?;
        if snapshot.revision > self.last_revision {
            self.last_revision = snapshot.revision;
            self.try_reattach_listen().await;
            return Ok(Some(snapshot));
        }
        Ok(None)
    }

    async fn try_reattach_listen(&mut self) {
        if self.stream.is_some() {
            return;
        }

        match self.service.subscribe_changes().await {
            Ok(stream) => {
                self.stream = Some(stream);
            }
            Err(err) => {
                warn!(error = ?err, "failed to re-establish LISTEN connection");
            }
        }
    }
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

        let mut api_keys_changed = false;
        if !changeset.api_keys.is_empty() {
            let before = fetch_api_keys_json(tx.as_mut()).await?;
            if apply_api_key_patches(&mut tx, &changeset.api_keys, &immutable_keys).await? {
                let after = fetch_api_keys_json(tx.as_mut()).await?;
                history_entries.push(("auth_api_keys", Some(before), Some(after)));
                any_change = true;
                api_keys_changed = true;
            }
        }

        let mut secret_events = Vec::new();
        if !changeset.secrets.is_empty() {
            secret_events =
                apply_secret_patches(&mut tx, &changeset.secrets, actor, &immutable_keys).await?;
            if !secret_events.is_empty() {
                history_entries.push((
                    "settings_secret",
                    None,
                    Some(Value::Array(secret_events.clone())),
                ));
                any_change = true;
            }
        }

        let mutated_via_triggers = applied_app.is_some()
            || applied_engine.is_some()
            || applied_fs.is_some()
            || api_keys_changed;
        if !secret_events.is_empty() && !mutated_via_triggers {
            bump_revision(&mut tx, "settings_secret").await?;
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

        info!(
            issued_by,
            expires_at = %expires_at,
            ttl_ms = ttl.as_millis(),
            "setup token issued"
        );

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
            warn!("setup token consumption attempted without an active token");
            return Err(anyhow!("no active setup token"));
        };

        if active.expires_at <= Utc::now() {
            sqlx::query("UPDATE setup_tokens SET consumed_at = now() WHERE id = $1")
                .bind(active.id)
                .execute(tx.as_mut())
                .await
                .context("failed to expire stale token")?;
            tx.commit().await?;
            warn!("setup token expired prior to consumption");
            return Err(anyhow!("setup token expired"));
        }

        let matches = verify_secret(&active.token_hash, token)?;
        if !matches {
            tx.rollback().await?;
            warn!("setup token consumption failed due to invalid secret");
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
        info!("setup token consumed successfully");
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

#[derive(FromRow)]
struct ApiKeyAuthRow {
    hash: String,
    enabled: bool,
    label: Option<String>,
    rate_limit: Value,
}

async fn fetch_app_profile<'e, E>(executor: E) -> Result<AppProfile>
where
    E: Executor<'e, Database = Postgres>,
{
    let row = sqlx::query_as::<_, AppProfileRow>(
        r"
        SELECT id, instance_name, mode, version, http_port, bind_addr::text AS bind_addr, telemetry, features, immutable_keys
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
        bind_addr: parse_bind_addr(&row.bind_addr)?,
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
        mutated |= apply_app_profile_field(tx, app_id, key, value).await?;
    }

    if mutated {
        bump_app_profile_version(tx, app_id).await?;
    }

    Ok(mutated)
}

async fn apply_app_profile_field(
    tx: &mut Transaction<'_, Postgres>,
    app_id: Uuid,
    key: &str,
    value: &Value,
) -> Result<bool> {
    match key {
        "instance_name" => set_app_instance_name(tx, app_id, value).await,
        "mode" => set_app_mode(tx, app_id, value).await,
        "http_port" => set_app_http_port(tx, app_id, value).await,
        "bind_addr" => set_app_bind_addr(tx, app_id, value).await,
        "telemetry" => set_app_telemetry(tx, app_id, value).await,
        "features" => set_app_features(tx, app_id, value).await,
        "immutable_keys" => set_app_immutable_keys(tx, app_id, value).await,
        other => Err(ConfigError::UnknownField {
            section: "app_profile".to_string(),
            field: other.to_string(),
        }
        .into()),
    }
}

async fn set_app_instance_name(
    tx: &mut Transaction<'_, Postgres>,
    app_id: Uuid,
    value: &Value,
) -> Result<bool> {
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
    Ok(true)
}

async fn set_app_mode(
    tx: &mut Transaction<'_, Postgres>,
    app_id: Uuid,
    value: &Value,
) -> Result<bool> {
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
    Ok(true)
}

async fn set_app_http_port(
    tx: &mut Transaction<'_, Postgres>,
    app_id: Uuid,
    value: &Value,
) -> Result<bool> {
    let port = parse_port(value, "app_profile", "http_port")?;
    sqlx::query("UPDATE app_profile SET http_port = $1 WHERE id = $2")
        .bind(port)
        .bind(app_id)
        .execute(tx.as_mut())
        .await?;
    Ok(true)
}

async fn set_app_bind_addr(
    tx: &mut Transaction<'_, Postgres>,
    app_id: Uuid,
    value: &Value,
) -> Result<bool> {
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
    Ok(true)
}

async fn set_app_telemetry(
    tx: &mut Transaction<'_, Postgres>,
    app_id: Uuid,
    value: &Value,
) -> Result<bool> {
    ensure_object(value, "app_profile", "telemetry")?;
    sqlx::query("UPDATE app_profile SET telemetry = $1 WHERE id = $2")
        .bind(value.clone())
        .bind(app_id)
        .execute(tx.as_mut())
        .await?;
    Ok(true)
}

async fn set_app_features(
    tx: &mut Transaction<'_, Postgres>,
    app_id: Uuid,
    value: &Value,
) -> Result<bool> {
    ensure_object(value, "app_profile", "features")?;
    sqlx::query("UPDATE app_profile SET features = $1 WHERE id = $2")
        .bind(value.clone())
        .bind(app_id)
        .execute(tx.as_mut())
        .await?;
    Ok(true)
}

async fn set_app_immutable_keys(
    tx: &mut Transaction<'_, Postgres>,
    app_id: Uuid,
    value: &Value,
) -> Result<bool> {
    ensure_array(value, "app_profile", "immutable_keys")?;
    sqlx::query("UPDATE app_profile SET immutable_keys = $1 WHERE id = $2")
        .bind(value.clone())
        .bind(app_id)
        .execute(tx.as_mut())
        .await?;
    Ok(true)
}

async fn bump_app_profile_version(tx: &mut Transaction<'_, Postgres>, app_id: Uuid) -> Result<()> {
    sqlx::query("UPDATE app_profile SET version = version + 1 WHERE id = $1")
        .bind(app_id)
        .execute(tx.as_mut())
        .await?;
    Ok(())
}

fn parse_port(value: &Value, section: &str, field: &str) -> Result<i32> {
    let Some(port) = value.as_i64() else {
        return Err(ConfigError::InvalidField {
            section: section.to_string(),
            field: field.to_string(),
            message: "must be an integer".to_string(),
        }
        .into());
    };
    ensure!(
        (1..=i64::from(u16::MAX)).contains(&port),
        ConfigError::InvalidField {
            section: section.to_string(),
            field: field.to_string(),
            message: "must be between 1 and 65535".to_string(),
        }
    );
    let port_i32 = i32::try_from(port).map_err(|_| ConfigError::InvalidField {
        section: section.to_string(),
        field: field.to_string(),
        message: "must fit within 32-bit signed integer range".to_string(),
    })?;
    Ok(port_i32)
}

fn ensure_object(value: &Value, section: &str, field: &str) -> Result<()> {
    ensure!(
        value.is_object(),
        ConfigError::InvalidField {
            section: section.to_string(),
            field: field.to_string(),
            message: "must be an object".to_string(),
        }
    );
    Ok(())
}

fn ensure_array(value: &Value, section: &str, field: &str) -> Result<()> {
    ensure!(
        value.is_array(),
        ConfigError::InvalidField {
            section: section.to_string(),
            field: field.to_string(),
            message: "must be an array".to_string(),
        }
    );
    Ok(())
}

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
        mutated |= apply_engine_profile_field(tx, engine_id, key, value).await?;
    }

    Ok(mutated)
}

async fn apply_engine_profile_field(
    tx: &mut Transaction<'_, Postgres>,
    engine_id: Uuid,
    key: &str,
    value: &Value,
) -> Result<bool> {
    match key {
        "implementation" => set_engine_implementation(tx, engine_id, value).await,
        "listen_port" => set_engine_listen_port(tx, engine_id, value).await,
        "dht" => set_engine_boolean_flag(tx, engine_id, value, "dht").await,
        "encryption" => set_engine_encryption(tx, engine_id, value).await,
        "max_active" => set_engine_max_active(tx, engine_id, value).await,
        "max_download_bps" => set_engine_rate_limit(tx, engine_id, value, "max_download_bps").await,
        "max_upload_bps" => set_engine_rate_limit(tx, engine_id, value, "max_upload_bps").await,
        "sequential_default" => {
            set_engine_boolean_flag(tx, engine_id, value, "sequential_default").await
        }
        "resume_dir" => set_engine_text_field(tx, engine_id, value, "resume_dir").await,
        "download_root" => set_engine_text_field(tx, engine_id, value, "download_root").await,
        "tracker" => set_engine_tracker(tx, engine_id, value).await,
        other => Err(ConfigError::UnknownField {
            section: "engine_profile".to_string(),
            field: other.to_string(),
        }
        .into()),
    }
}

async fn set_engine_implementation(
    tx: &mut Transaction<'_, Postgres>,
    engine_id: Uuid,
    value: &Value,
) -> Result<bool> {
    let Some(name) = value.as_str() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "implementation".to_string(),
            message: "must be a string".to_string(),
        }
        .into());
    };
    sqlx::query("UPDATE engine_profile SET implementation = $1 WHERE id = $2")
        .bind(name)
        .bind(engine_id)
        .execute(tx.as_mut())
        .await?;
    Ok(true)
}

async fn set_engine_listen_port(
    tx: &mut Transaction<'_, Postgres>,
    engine_id: Uuid,
    value: &Value,
) -> Result<bool> {
    if value.is_null() {
        sqlx::query("UPDATE engine_profile SET listen_port = NULL WHERE id = $1")
            .bind(engine_id)
            .execute(tx.as_mut())
            .await?;
        return Ok(true);
    }
    let port = parse_port(value, "engine_profile", "listen_port")?;
    sqlx::query("UPDATE engine_profile SET listen_port = $1 WHERE id = $2")
        .bind(port)
        .bind(engine_id)
        .execute(tx.as_mut())
        .await?;
    Ok(true)
}

async fn set_engine_boolean_flag(
    tx: &mut Transaction<'_, Postgres>,
    engine_id: Uuid,
    value: &Value,
    field: &str,
) -> Result<bool> {
    let Some(flag) = value.as_bool() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be a boolean".to_string(),
        }
        .into());
    };
    let query = format!("UPDATE engine_profile SET {field} = $1 WHERE id = $2");
    sqlx::query(&query)
        .bind(flag)
        .bind(engine_id)
        .execute(tx.as_mut())
        .await?;
    Ok(true)
}

async fn set_engine_encryption(
    tx: &mut Transaction<'_, Postgres>,
    engine_id: Uuid,
    value: &Value,
) -> Result<bool> {
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
    Ok(true)
}

async fn set_engine_max_active(
    tx: &mut Transaction<'_, Postgres>,
    engine_id: Uuid,
    value: &Value,
) -> Result<bool> {
    if value.is_null() {
        sqlx::query("UPDATE engine_profile SET max_active = NULL WHERE id = $1")
            .bind(engine_id)
            .execute(tx.as_mut())
            .await?;
        return Ok(true);
    }
    let Some(raw_value) = value.as_i64() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "max_active".to_string(),
            message: "must be an integer".to_string(),
        }
        .into());
    };
    ensure!(
        raw_value >= 0 && raw_value <= i64::from(i32::MAX),
        ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "max_active".to_string(),
            message: "must be within 0..=i32::MAX".to_string(),
        }
    );
    let max_active = i32::try_from(raw_value).map_err(|_| ConfigError::InvalidField {
        section: "engine_profile".to_string(),
        field: "max_active".to_string(),
        message: "must fit within 32-bit signed integer range".to_string(),
    })?;
    sqlx::query("UPDATE engine_profile SET max_active = $1 WHERE id = $2")
        .bind(max_active)
        .bind(engine_id)
        .execute(tx.as_mut())
        .await?;
    Ok(true)
}

async fn set_engine_rate_limit(
    tx: &mut Transaction<'_, Postgres>,
    engine_id: Uuid,
    value: &Value,
    field: &str,
) -> Result<bool> {
    if value.is_null() {
        let query = format!("UPDATE engine_profile SET {field} = NULL WHERE id = $1");
        sqlx::query(&query)
            .bind(engine_id)
            .execute(tx.as_mut())
            .await?;
        return Ok(true);
    }
    let Some(limit) = value.as_i64() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be an integer".to_string(),
        }
        .into());
    };
    ensure!(
        limit >= 0,
        ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be non-negative".to_string(),
        }
    );
    let query = format!("UPDATE engine_profile SET {field} = $1 WHERE id = $2");
    sqlx::query(&query)
        .bind(limit)
        .bind(engine_id)
        .execute(tx.as_mut())
        .await?;
    Ok(true)
}

async fn set_engine_text_field(
    tx: &mut Transaction<'_, Postgres>,
    engine_id: Uuid,
    value: &Value,
    field: &str,
) -> Result<bool> {
    let Some(text) = value.as_str() else {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: field.to_string(),
            message: "must be a string".to_string(),
        }
        .into());
    };
    let query = format!("UPDATE engine_profile SET {field} = $1 WHERE id = $2");
    sqlx::query(&query)
        .bind(text)
        .bind(engine_id)
        .execute(tx.as_mut())
        .await?;
    Ok(true)
}

async fn set_engine_tracker(
    tx: &mut Transaction<'_, Postgres>,
    engine_id: Uuid,
    value: &Value,
) -> Result<bool> {
    ensure_object(value, "engine_profile", "tracker")?;
    sqlx::query("UPDATE engine_profile SET tracker = $1 WHERE id = $2")
        .bind(value.clone())
        .bind(engine_id)
        .execute(tx.as_mut())
        .await?;
    Ok(true)
}

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
        mutated |= apply_fs_policy_field(tx, policy_id, key, value).await?;
    }

    Ok(mutated)
}

async fn apply_fs_policy_field(
    tx: &mut Transaction<'_, Postgres>,
    policy_id: Uuid,
    key: &str,
    value: &Value,
) -> Result<bool> {
    match key {
        "library_root" => set_fs_string_field(tx, policy_id, value, "library_root").await,
        "extract" => set_fs_boolean_field(tx, policy_id, value, "extract").await,
        "par2" => set_fs_string_field(tx, policy_id, value, "par2").await,
        "flatten" => set_fs_boolean_field(tx, policy_id, value, "flatten").await,
        "move_mode" => set_fs_string_field(tx, policy_id, value, "move_mode").await,
        "cleanup_keep" => set_fs_array_field(tx, policy_id, value, "cleanup_keep").await,
        "cleanup_drop" => set_fs_array_field(tx, policy_id, value, "cleanup_drop").await,
        "chmod_file" => set_fs_optional_string_field(tx, policy_id, value, "chmod_file").await,
        "chmod_dir" => set_fs_optional_string_field(tx, policy_id, value, "chmod_dir").await,
        "owner" => set_fs_optional_string_field(tx, policy_id, value, "owner").await,
        "group" => set_fs_optional_string_field(tx, policy_id, value, "group").await,
        "umask" => set_fs_optional_string_field(tx, policy_id, value, "umask").await,
        "allow_paths" => set_fs_array_field(tx, policy_id, value, "allow_paths").await,
        other => Err(ConfigError::UnknownField {
            section: "fs_policy".to_string(),
            field: other.to_string(),
        }
        .into()),
    }
}

async fn set_fs_string_field(
    tx: &mut Transaction<'_, Postgres>,
    policy_id: Uuid,
    value: &Value,
    field: &str,
) -> Result<bool> {
    let Some(text) = value.as_str() else {
        return Err(ConfigError::InvalidField {
            section: "fs_policy".to_string(),
            field: field.to_string(),
            message: "must be a string".to_string(),
        }
        .into());
    };
    let query = format!(r#"UPDATE fs_policy SET "{field}" = $1 WHERE id = $2"#);
    sqlx::query(&query)
        .bind(text)
        .bind(policy_id)
        .execute(tx.as_mut())
        .await?;
    Ok(true)
}

async fn set_fs_boolean_field(
    tx: &mut Transaction<'_, Postgres>,
    policy_id: Uuid,
    value: &Value,
    field: &str,
) -> Result<bool> {
    let Some(flag) = value.as_bool() else {
        return Err(ConfigError::InvalidField {
            section: "fs_policy".to_string(),
            field: field.to_string(),
            message: "must be a boolean".to_string(),
        }
        .into());
    };
    let query = format!(r#"UPDATE fs_policy SET "{field}" = $1 WHERE id = $2"#);
    sqlx::query(&query)
        .bind(flag)
        .bind(policy_id)
        .execute(tx.as_mut())
        .await?;
    Ok(true)
}

async fn set_fs_array_field(
    tx: &mut Transaction<'_, Postgres>,
    policy_id: Uuid,
    value: &Value,
    field: &str,
) -> Result<bool> {
    ensure_array(value, "fs_policy", field)?;
    let query = format!(r#"UPDATE fs_policy SET "{field}" = $1 WHERE id = $2"#);
    sqlx::query(&query)
        .bind(value.clone())
        .bind(policy_id)
        .execute(tx.as_mut())
        .await?;
    Ok(true)
}

async fn set_fs_optional_string_field(
    tx: &mut Transaction<'_, Postgres>,
    policy_id: Uuid,
    value: &Value,
    field: &str,
) -> Result<bool> {
    if value.is_null() {
        let query = format!(r#"UPDATE fs_policy SET "{field}" = NULL WHERE id = $1"#);
        sqlx::query(&query)
            .bind(policy_id)
            .execute(tx.as_mut())
            .await?;
        return Ok(true);
    }
    let Some(text) = value.as_str() else {
        return Err(ConfigError::InvalidField {
            section: "fs_policy".to_string(),
            field: field.to_string(),
            message: "must be a string".to_string(),
        }
        .into());
    };
    let query = format!(r#"UPDATE fs_policy SET "{field}" = $2 WHERE id = $1"#);
    sqlx::query(&query)
        .bind(policy_id)
        .bind(text)
        .execute(tx.as_mut())
        .await?;
    Ok(true)
}
async fn apply_api_key_patches(
    tx: &mut Transaction<'_, Postgres>,
    patches: &[ApiKeyPatch],
    immutable_keys: &HashSet<String>,
) -> Result<bool> {
    if patches.is_empty() {
        return Ok(false);
    }

    let mut changed = false;

    for patch in patches {
        match patch {
            ApiKeyPatch::Delete { key_id } => {
                changed |= delete_api_key(tx, immutable_keys, key_id).await?;
            }
            ApiKeyPatch::Upsert {
                key_id,
                label,
                enabled,
                secret,
                rate_limit,
            } => {
                changed |= upsert_api_key(
                    tx,
                    immutable_keys,
                    key_id,
                    label.as_deref(),
                    *enabled,
                    secret.as_deref(),
                    rate_limit.as_ref(),
                )
                .await?;
            }
        }
    }

    Ok(changed)
}

async fn delete_api_key(
    tx: &mut Transaction<'_, Postgres>,
    immutable_keys: &HashSet<String>,
    key_id: &str,
) -> Result<bool> {
    ensure_mutable(immutable_keys, "auth_api_keys", "key_id")?;
    let result = sqlx::query("DELETE FROM auth_api_keys WHERE key_id = $1")
        .bind(key_id)
        .execute(tx.as_mut())
        .await?;
    Ok(result.rows_affected() > 0)
}

async fn upsert_api_key(
    tx: &mut Transaction<'_, Postgres>,
    immutable_keys: &HashSet<String>,
    key_id: &str,
    label: Option<&str>,
    enabled: Option<bool>,
    secret: Option<&str>,
    rate_limit: Option<&Value>,
) -> Result<bool> {
    ensure_mutable(immutable_keys, "auth_api_keys", "key_id")?;
    let existing: Option<String> =
        sqlx::query_scalar("SELECT hash FROM auth_api_keys WHERE key_id = $1")
            .bind(key_id)
            .fetch_optional(tx.as_mut())
            .await?;

    if existing.is_some() {
        update_api_key(
            tx,
            immutable_keys,
            key_id,
            label,
            enabled,
            secret,
            rate_limit,
        )
        .await
    } else {
        insert_api_key(
            tx,
            immutable_keys,
            key_id,
            label,
            enabled,
            secret,
            rate_limit,
        )
        .await
    }
}

async fn update_api_key(
    tx: &mut Transaction<'_, Postgres>,
    immutable_keys: &HashSet<String>,
    key_id: &str,
    label: Option<&str>,
    enabled: Option<bool>,
    secret: Option<&str>,
    rate_limit: Option<&Value>,
) -> Result<bool> {
    let changed_secret = if let Some(value) = secret {
        ensure_mutable(immutable_keys, "auth_api_keys", "secret")?;
        let hash = hash_secret(value)?;
        sqlx::query("UPDATE auth_api_keys SET hash = $1, updated_at = now() WHERE key_id = $2")
            .bind(hash)
            .bind(key_id)
            .execute(tx.as_mut())
            .await?;
        true
    } else {
        false
    };

    let changed_label = if let Some(text) = label {
        ensure_mutable(immutable_keys, "auth_api_keys", "label")?;
        sqlx::query("UPDATE auth_api_keys SET label = $1, updated_at = now() WHERE key_id = $2")
            .bind(text)
            .bind(key_id)
            .execute(tx.as_mut())
            .await?;
        true
    } else {
        false
    };

    let changed_enabled = if let Some(flag) = enabled {
        ensure_mutable(immutable_keys, "auth_api_keys", "enabled")?;
        sqlx::query("UPDATE auth_api_keys SET enabled = $1, updated_at = now() WHERE key_id = $2")
            .bind(flag)
            .bind(key_id)
            .execute(tx.as_mut())
            .await?;
        true
    } else {
        false
    };

    let changed_rate_limit = if let Some(limit_value) = rate_limit {
        ensure_mutable(immutable_keys, "auth_api_keys", "rate_limit")?;
        let parsed = parse_api_key_rate_limit_for_config(limit_value)?;
        let stored = serialise_rate_limit(parsed.as_ref());
        sqlx::query(
            "UPDATE auth_api_keys SET rate_limit = $1, updated_at = now() WHERE key_id = $2",
        )
        .bind(stored)
        .bind(key_id)
        .execute(tx.as_mut())
        .await?;
        true
    } else {
        false
    };

    Ok(changed_secret || changed_label || changed_enabled || changed_rate_limit)
}

async fn insert_api_key(
    tx: &mut Transaction<'_, Postgres>,
    immutable_keys: &HashSet<String>,
    key_id: &str,
    label: Option<&str>,
    enabled: Option<bool>,
    secret: Option<&str>,
    rate_limit: Option<&Value>,
) -> Result<bool> {
    let Some(secret_value) = secret else {
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

    let hash = hash_secret(secret_value)?;
    let enabled_flag = enabled.unwrap_or(true);
    let parsed_limit = rate_limit
        .map(parse_api_key_rate_limit_for_config)
        .transpose()?
        .flatten();
    let stored_limit = serialise_rate_limit(parsed_limit.as_ref());

    sqlx::query(
        r"
        INSERT INTO auth_api_keys (key_id, hash, label, enabled, rate_limit)
        VALUES ($1, $2, $3, $4, $5)
        ",
    )
    .bind(key_id)
    .bind(hash)
    .bind(label.map(str::to_owned))
    .bind(enabled_flag)
    .bind(stored_limit)
    .execute(tx.as_mut())
    .await?;

    Ok(true)
}

async fn apply_secret_patches(
    tx: &mut Transaction<'_, Postgres>,
    patches: &[SecretPatch],
    actor: &str,
    immutable_keys: &HashSet<String>,
) -> Result<Vec<Value>> {
    let mut events = Vec::new();
    for patch in patches {
        match patch {
            SecretPatch::Set { name, value } => {
                ensure_mutable(immutable_keys, "settings_secret", name)?;
                let ciphertext = hash_secret(value)?;
                sqlx::query(
                    r"
                    INSERT INTO settings_secret (name, ciphertext, created_by, created_at)
                    VALUES ($1, $2, $3, now())
                    ON CONFLICT (name)
                    DO UPDATE SET ciphertext = EXCLUDED.ciphertext,
                                   created_by = EXCLUDED.created_by,
                                   created_at = now()
                    ",
                )
                .bind(name)
                .bind(ciphertext.into_bytes())
                .bind(actor)
                .execute(tx.as_mut())
                .await?;
                events.push(json!({ "op": "set", "name": name }));
            }
            SecretPatch::Delete { name } => {
                ensure_mutable(immutable_keys, "settings_secret", name)?;
                let result = sqlx::query("DELETE FROM settings_secret WHERE name = $1")
                    .bind(name)
                    .execute(tx.as_mut())
                    .await?;
                if result.rows_affected() > 0 {
                    events.push(json!({ "op": "delete", "name": name }));
                }
            }
        }
    }
    Ok(events)
}

fn parse_api_key_rate_limit(value: &Value) -> Result<Option<ApiKeyRateLimit>> {
    parse_api_key_rate_limit_for_config(value).map_err(Into::into)
}

fn parse_api_key_rate_limit_for_config(
    value: &Value,
) -> Result<Option<ApiKeyRateLimit>, ConfigError> {
    let Value::Object(map) = value else {
        if value.is_null() {
            return Ok(None);
        }
        return Err(ConfigError::InvalidField {
            section: "auth_api_keys".to_string(),
            field: "rate_limit".to_string(),
            message: "must be a JSON object with burst and per_seconds fields".to_string(),
        });
    };

    if map.is_empty() {
        return Ok(None);
    }

    let burst = map
        .get("burst")
        .ok_or_else(|| ConfigError::InvalidField {
            section: "auth_api_keys".to_string(),
            field: "rate_limit".to_string(),
            message: "missing 'burst' field".to_string(),
        })?
        .as_u64()
        .ok_or_else(|| ConfigError::InvalidField {
            section: "auth_api_keys".to_string(),
            field: "rate_limit".to_string(),
            message: "'burst' must be a positive integer".to_string(),
        })?;

    if burst == 0 {
        return Err(ConfigError::InvalidField {
            section: "auth_api_keys".to_string(),
            field: "rate_limit".to_string(),
            message: "'burst' must be between 1 and 4_294_967_295".to_string(),
        });
    }

    let burst = u32::try_from(burst).map_err(|_| ConfigError::InvalidField {
        section: "auth_api_keys".to_string(),
        field: "rate_limit".to_string(),
        message: "'burst' must be between 1 and 4_294_967_295".to_string(),
    })?;

    let per_seconds = map
        .get("per_seconds")
        .ok_or_else(|| ConfigError::InvalidField {
            section: "auth_api_keys".to_string(),
            field: "rate_limit".to_string(),
            message: "missing 'per_seconds' field".to_string(),
        })?
        .as_u64()
        .ok_or_else(|| ConfigError::InvalidField {
            section: "auth_api_keys".to_string(),
            field: "rate_limit".to_string(),
            message: "'per_seconds' must be a positive integer".to_string(),
        })?;

    if per_seconds == 0 {
        return Err(ConfigError::InvalidField {
            section: "auth_api_keys".to_string(),
            field: "rate_limit".to_string(),
            message: "'per_seconds' must be greater than zero".to_string(),
        });
    }

    Ok(Some(ApiKeyRateLimit {
        burst,
        replenish_period: Duration::from_secs(per_seconds),
    }))
}

fn serialise_rate_limit(limit: Option<&ApiKeyRateLimit>) -> Value {
    limit.map_or_else(|| Value::Object(Map::new()), ApiKeyRateLimit::to_json)
}

async fn bump_revision(tx: &mut Transaction<'_, Postgres>, source_table: &str) -> Result<i64> {
    let revision: i64 = sqlx::query(
        r"
        UPDATE settings_revision
        SET revision = revision + 1,
            updated_at = now()
        WHERE id = 1
        RETURNING revision
        ",
    )
    .map(|row: PgRow| row.get::<i64, _>("revision"))
    .fetch_one(tx.as_mut())
    .await?;

    let payload = format!("{source_table}:{revision}:UPDATE");
    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(SETTINGS_CHANNEL)
        .bind(&payload)
        .execute(tx.as_mut())
        .await?;

    Ok(revision)
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

fn parse_bind_addr(value: &str) -> Result<IpAddr> {
    if let Ok(addr) = value.parse::<IpAddr>() {
        return Ok(addr);
    }

    let Some(host) = value.split('/').next() else {
        return Err(anyhow!("invalid bind address '{value}'"));
    };

    host.parse::<IpAddr>()
        .with_context(|| format!("invalid bind address '{value}'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn parse_rate_limit_accepts_empty_object() {
        let value = Value::Object(Map::new());
        let parsed =
            parse_api_key_rate_limit_for_config(&value).expect("empty object should be accepted");
        assert!(parsed.is_none());
    }

    #[test]
    fn parse_rate_limit_valid_configuration() {
        let value = json!({ "burst": 10, "per_seconds": 60 });
        let parsed =
            parse_api_key_rate_limit_for_config(&value).expect("valid configuration should parse");
        let limit = parsed.expect("limit should be present");
        assert_eq!(limit.burst, 10);
        assert_eq!(limit.replenish_period, Duration::from_secs(60));
    }

    #[test]
    fn parse_rate_limit_rejects_zero_burst() {
        let value = json!({ "burst": 0, "per_seconds": 30 });
        let err = parse_api_key_rate_limit_for_config(&value).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidField { .. }));
    }

    #[test]
    fn parse_rate_limit_rejects_missing_fields() {
        let value = json!({ "burst": 5 });
        let err = parse_api_key_rate_limit_for_config(&value).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidField { .. }));
    }
}
