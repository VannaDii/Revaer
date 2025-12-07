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
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]

//! Database-backed configuration facade built on `PostgreSQL`.
//!
//! Layout: `model.rs` (typed config models and changesets), `validate.rs`
//! (validation/parsing helpers and `ConfigError`), with `lib.rs` hosting the
//! `SettingsFacade`/`ConfigService` implementation and history/persistence
//! glue.

use anyhow::{Context, Result, anyhow, ensure};
use argon2::Argon2;
use argon2::password_hash::{
    Error as PasswordHashError, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
    rand_core::OsRng,
};
use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use rand::Rng;
use rand::distr::Alphanumeric;
use revaer_data::config::{
    self as data_config, AppProfileRow, EngineBooleanField, EngineProfileRow, EngineRateField,
    EngineTextField, FsArrayField, FsBooleanField, FsOptionalStringField, FsPolicyRow,
    FsStringField, HistoryInsert, NewSetupToken, SETTINGS_CHANNEL,
};
use serde_json::{Map, Value, json};
use sqlx::postgres::{PgListener, PgNotification, PgPoolOptions};
use sqlx::{Executor, Postgres, Transaction};
use std::collections::HashSet;
use std::convert::TryFrom;
use std::net::IpAddr;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, instrument, warn};
use uuid::Uuid;

mod model;
pub(crate) mod validate;
use crate::validate::{
    parse_api_key_rate_limit, parse_api_key_rate_limit_for_config, parse_bind_addr, parse_port,
    parse_uuid,
};

pub use model::{
    ApiKeyAuth, ApiKeyPatch, ApiKeyRateLimit, AppMode, AppProfile, AppliedChanges, ConfigSnapshot,
    EngineProfile, FsPolicy, SecretPatch, SettingsChange, SettingsChangeset, SettingsPayload,
    SetupToken,
};
pub use validate::ConfigError;

const APP_PROFILE_ID: &str = "00000000-0000-0000-0000-000000000001";
const ENGINE_PROFILE_ID: &str = "00000000-0000-0000-0000-000000000002";
const FS_POLICY_ID: &str = "00000000-0000-0000-0000-000000000003";
// Models are defined in model.rs.

// Models are defined in model.rs.

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

// Models are defined in model.rs.

/// Pending history entry captured before persistence.
#[derive(Debug, Clone)]
struct PendingHistoryEntry {
    kind: &'static str,
    old: Option<Value>,
    new: Option<Value>,
}

async fn persist_history_entries(
    tx: &mut Transaction<'_, Postgres>,
    entries: Vec<PendingHistoryEntry>,
    actor: &str,
    reason: &str,
    revision: i64,
) -> Result<()> {
    for entry in entries {
        data_config::insert_history(
            tx.as_mut(),
            HistoryInsert {
                kind: entry.kind,
                old: entry.old,
                new: entry.new,
                actor,
                reason,
                revision,
            },
        )
        .await?;
    }
    Ok(())
}

// Rate limit/authentication models are defined in model.rs.

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
    ///
    /// # Errors
    ///
    /// Returns an error if the `PostgreSQL` connection cannot be established or
    /// migrations fail to run.
    #[instrument(name = "config_service.new", skip(database_url))]
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
    ///
    /// # Errors
    ///
    /// Returns an error if any underlying configuration query fails.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the initial snapshot or listener attachment fails.
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
    ///
    /// # Errors
    ///
    /// Returns an error if database access fails, token verification fails, or
    /// the token is expired or missing.
    pub async fn validate_setup_token(&self, token: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        data_config::cleanup_expired_setup_tokens(tx.as_mut()).await?;

        let active = data_config::fetch_active_setup_token(tx.as_mut())
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
    ///
    /// # Errors
    ///
    /// Returns an error if the API key lookup, hashing, or rate limit parsing
    /// fails.
    pub async fn authenticate_api_key(
        &self,
        key_id: &str,
        secret: &str,
    ) -> Result<Option<ApiKeyAuth>> {
        let record = data_config::fetch_api_key_auth(&self.pool, key_id)
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
    ///
    /// # Errors
    ///
    /// Returns an error if polling or LISTEN handling fails while fetching the
    /// next configuration snapshot.
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
        let mut history_entries: Vec<PendingHistoryEntry> = Vec::new();
        let mut any_change = false;
        let app_document = fetch_app_profile_json(tx.as_mut()).await?;
        let immutable_keys = extract_immutable_keys(&app_document)?;

        if let Some(app_patch) = changeset.app_profile {
            let before = app_document.clone();
            if apply_app_profile_patch(&mut tx, &app_patch, &immutable_keys).await? {
                let after = fetch_app_profile_json(tx.as_mut()).await?;
                applied_app = Some(fetch_app_profile(tx.as_mut()).await?);
                history_entries.push(PendingHistoryEntry {
                    kind: "app_profile",
                    old: Some(before),
                    new: Some(after),
                });
                any_change = true;
            }
        }

        if let Some(engine_patch) = changeset.engine_profile {
            let before = fetch_engine_profile_json(tx.as_mut()).await?;
            if apply_engine_profile_patch(&mut tx, &engine_patch, &immutable_keys).await? {
                let after = fetch_engine_profile_json(tx.as_mut()).await?;
                applied_engine = Some(fetch_engine_profile(tx.as_mut()).await?);
                history_entries.push(PendingHistoryEntry {
                    kind: "engine_profile",
                    old: Some(before),
                    new: Some(after),
                });
                any_change = true;
            }
        }

        if let Some(fs_patch) = changeset.fs_policy {
            let before = fetch_fs_policy_json(tx.as_mut()).await?;
            if apply_fs_policy_patch(&mut tx, &fs_patch, &immutable_keys).await? {
                let after = fetch_fs_policy_json(tx.as_mut()).await?;
                applied_fs = Some(fetch_fs_policy(tx.as_mut()).await?);
                history_entries.push(PendingHistoryEntry {
                    kind: "fs_policy",
                    old: Some(before),
                    new: Some(after),
                });
                any_change = true;
            }
        }

        let mut api_keys_changed = false;
        if !changeset.api_keys.is_empty() {
            let before = fetch_api_keys_json(tx.as_mut()).await?;
            if apply_api_key_patches(&mut tx, &changeset.api_keys, &immutable_keys).await? {
                let after = fetch_api_keys_json(tx.as_mut()).await?;
                history_entries.push(PendingHistoryEntry {
                    kind: "auth_api_keys",
                    old: Some(before),
                    new: Some(after),
                });
                any_change = true;
                api_keys_changed = true;
            }
        }

        let mut secret_events = Vec::new();
        if !changeset.secrets.is_empty() {
            secret_events =
                apply_secret_patches(&mut tx, &changeset.secrets, actor, &immutable_keys).await?;
            if !secret_events.is_empty() {
                history_entries.push(PendingHistoryEntry {
                    kind: "settings_secret",
                    old: None,
                    new: Some(Value::Array(secret_events.clone())),
                });
                any_change = true;
            }
        }

        let mutated_via_triggers = applied_app.is_some()
            || applied_engine.is_some()
            || applied_fs.is_some()
            || api_keys_changed;
        if !secret_events.is_empty() && !mutated_via_triggers {
            data_config::bump_revision(tx.as_mut(), "settings_secret").await?;
        }

        let revision = fetch_revision(tx.as_mut()).await?;

        if any_change {
            persist_history_entries(&mut tx, history_entries, actor, reason, revision).await?;
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
        data_config::cleanup_expired_setup_tokens(tx.as_mut()).await?;
        data_config::invalidate_active_setup_tokens(tx.as_mut()).await?;

        let plaintext = generate_token(32);
        let token_hash = hash_secret(&plaintext)?;
        let expires_at = Utc::now() + chrono_ttl;

        let insert = NewSetupToken {
            token_hash: &token_hash,
            expires_at,
            issued_by,
        };
        data_config::insert_setup_token(tx.as_mut(), &insert)
            .await
            .context("failed to persist setup token")?;

        let revision = fetch_revision(tx.as_mut()).await?;
        data_config::insert_history(
            tx.as_mut(),
            HistoryInsert {
                kind: "setup_token",
                old: None,
                new: Some(json!({
                    "event": "issued",
                    "issued_by": issued_by,
                    "expires_at": expires_at
                })),
                actor: issued_by,
                reason: "issue_setup_token",
                revision,
            },
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
        data_config::cleanup_expired_setup_tokens(tx.as_mut()).await?;

        let active = data_config::fetch_active_setup_token(tx.as_mut())
            .await
            .context("failed to query setup tokens")?;

        let Some(active) = active else {
            tx.rollback().await?;
            warn!("setup token consumption attempted without an active token");
            return Err(anyhow!("no active setup token"));
        };

        if active.expires_at <= Utc::now() {
            data_config::mark_setup_token_consumed(tx.as_mut(), active.id)
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

        data_config::mark_setup_token_consumed(tx.as_mut(), active.id)
            .await
            .context("failed to consume setup token")?;

        let revision = fetch_revision(tx.as_mut()).await?;
        data_config::insert_history(
            tx.as_mut(),
            HistoryInsert {
                kind: "setup_token",
                old: None,
                new: Some(json!({"event": "consumed"})),
                actor: "system",
                reason: "consume_setup_token",
                revision,
            },
        )
        .await?;

        tx.commit().await?;
        info!("setup token consumed successfully");
        Ok(())
    }
}

async fn apply_migrations(pool: &sqlx::PgPool) -> Result<()> {
    data_config::run_migrations(pool)
        .await
        .context("failed to apply configuration migrations")?;
    Ok(())
}

async fn fetch_app_profile<'e, E>(executor: E) -> Result<AppProfile>
where
    E: Executor<'e, Database = Postgres>,
{
    let id = parse_uuid(APP_PROFILE_ID)?;
    let row = data_config::fetch_app_profile_row(executor, id)
        .await
        .context("failed to load app_profile")?;
    map_app_profile_row(row)
}

async fn fetch_engine_profile<'e, E>(executor: E) -> Result<EngineProfile>
where
    E: Executor<'e, Database = Postgres>,
{
    let id = parse_uuid(ENGINE_PROFILE_ID)?;
    let row = data_config::fetch_engine_profile_row(executor, id)
        .await
        .context("failed to load engine_profile")?;
    Ok(map_engine_profile_row(row))
}

async fn fetch_fs_policy<'e, E>(executor: E) -> Result<FsPolicy>
where
    E: Executor<'e, Database = Postgres>,
{
    let id = parse_uuid(FS_POLICY_ID)?;
    let row = data_config::fetch_fs_policy_row(executor, id)
        .await
        .context("failed to load fs_policy")?;
    Ok(map_fs_policy_row(row))
}

async fn fetch_revision<'e, E>(executor: E) -> Result<i64>
where
    E: Executor<'e, Database = Postgres>,
{
    data_config::fetch_revision(executor)
        .await
        .context("failed to load settings revision")
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
    let id = parse_uuid(APP_PROFILE_ID)?;
    data_config::fetch_app_profile_json(executor, id)
        .await
        .context("failed to load app_profile document")
}

async fn fetch_engine_profile_json<'e, E>(executor: E) -> Result<Value>
where
    E: Executor<'e, Database = Postgres>,
{
    let id = parse_uuid(ENGINE_PROFILE_ID)?;
    data_config::fetch_engine_profile_json(executor, id)
        .await
        .context("failed to load engine_profile document")
}

async fn fetch_fs_policy_json<'e, E>(executor: E) -> Result<Value>
where
    E: Executor<'e, Database = Postgres>,
{
    let id = parse_uuid(FS_POLICY_ID)?;
    data_config::fetch_fs_policy_json(executor, id)
        .await
        .context("failed to load fs_policy document")
}

async fn fetch_api_keys_json<'e, E>(executor: E) -> Result<Value>
where
    E: Executor<'e, Database = Postgres>,
{
    data_config::fetch_api_keys_json(executor)
        .await
        .context("failed to load auth_api_keys document")
}

fn map_app_profile_row(row: AppProfileRow) -> Result<AppProfile> {
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

fn map_engine_profile_row(row: EngineProfileRow) -> EngineProfile {
    EngineProfile {
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
    }
}

fn map_fs_policy_row(row: FsPolicyRow) -> FsPolicy {
    FsPolicy {
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
    }
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
    data_config::update_app_instance_name(tx.as_mut(), app_id, new_value).await?;
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
    data_config::update_app_mode(tx.as_mut(), app_id, mode.as_str()).await?;
    Ok(true)
}

async fn set_app_http_port(
    tx: &mut Transaction<'_, Postgres>,
    app_id: Uuid,
    value: &Value,
) -> Result<bool> {
    let port = parse_port(value, "app_profile", "http_port")?;
    data_config::update_app_http_port(tx.as_mut(), app_id, port).await?;
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
    addr.parse::<IpAddr>()
        .map_err(|_| ConfigError::InvalidField {
            section: "app_profile".to_string(),
            field: "bind_addr".to_string(),
            message: "must be a valid IP address".to_string(),
        })?;
    data_config::update_app_bind_addr(tx.as_mut(), app_id, addr).await?;
    Ok(true)
}

async fn set_app_telemetry(
    tx: &mut Transaction<'_, Postgres>,
    app_id: Uuid,
    value: &Value,
) -> Result<bool> {
    ensure_object(value, "app_profile", "telemetry")?;
    data_config::update_app_telemetry(tx.as_mut(), app_id, value).await?;
    Ok(true)
}

async fn set_app_features(
    tx: &mut Transaction<'_, Postgres>,
    app_id: Uuid,
    value: &Value,
) -> Result<bool> {
    ensure_object(value, "app_profile", "features")?;
    data_config::update_app_features(tx.as_mut(), app_id, value).await?;
    Ok(true)
}

async fn set_app_immutable_keys(
    tx: &mut Transaction<'_, Postgres>,
    app_id: Uuid,
    value: &Value,
) -> Result<bool> {
    ensure_array(value, "app_profile", "immutable_keys")?;
    data_config::update_app_immutable_keys(tx.as_mut(), app_id, value).await?;
    Ok(true)
}

async fn bump_app_profile_version(tx: &mut Transaction<'_, Postgres>, app_id: Uuid) -> Result<()> {
    data_config::bump_app_profile_version(tx.as_mut(), app_id).await?;
    Ok(())
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
    data_config::update_engine_implementation(tx.as_mut(), engine_id, name).await?;
    Ok(true)
}

async fn set_engine_listen_port(
    tx: &mut Transaction<'_, Postgres>,
    engine_id: Uuid,
    value: &Value,
) -> Result<bool> {
    if value.is_null() {
        data_config::update_engine_listen_port(tx.as_mut(), engine_id, None).await?;
        return Ok(true);
    }
    let port = parse_port(value, "engine_profile", "listen_port")?;
    data_config::update_engine_listen_port(tx.as_mut(), engine_id, Some(port)).await?;
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
    let column = match field {
        "dht" => EngineBooleanField::Dht,
        "sequential_default" => EngineBooleanField::SequentialDefault,
        _ => {
            return Err(ConfigError::UnknownField {
                section: "engine_profile".to_string(),
                field: field.to_string(),
            }
            .into());
        }
    };
    data_config::update_engine_boolean_field(tx.as_mut(), engine_id, column, flag).await?;
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
    data_config::update_engine_encryption(tx.as_mut(), engine_id, mode).await?;
    Ok(true)
}

async fn set_engine_max_active(
    tx: &mut Transaction<'_, Postgres>,
    engine_id: Uuid,
    value: &Value,
) -> Result<bool> {
    if value.is_null() {
        data_config::update_engine_max_active(tx.as_mut(), engine_id, None).await?;
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
    data_config::update_engine_max_active(tx.as_mut(), engine_id, Some(max_active)).await?;
    Ok(true)
}

async fn set_engine_rate_limit(
    tx: &mut Transaction<'_, Postgres>,
    engine_id: Uuid,
    value: &Value,
    field: &str,
) -> Result<bool> {
    let column = match field {
        "max_download_bps" => EngineRateField::MaxDownloadBps,
        "max_upload_bps" => EngineRateField::MaxUploadBps,
        _ => {
            return Err(ConfigError::UnknownField {
                section: "engine_profile".to_string(),
                field: field.to_string(),
            }
            .into());
        }
    };
    if value.is_null() {
        data_config::update_engine_rate_field(tx.as_mut(), engine_id, column, None).await?;
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
    data_config::update_engine_rate_field(tx.as_mut(), engine_id, column, Some(limit)).await?;
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
    let column = match field {
        "resume_dir" => EngineTextField::ResumeDir,
        "download_root" => EngineTextField::DownloadRoot,
        _ => {
            return Err(ConfigError::UnknownField {
                section: "engine_profile".to_string(),
                field: field.to_string(),
            }
            .into());
        }
    };
    data_config::update_engine_text_field(tx.as_mut(), engine_id, column, text).await?;
    Ok(true)
}

async fn set_engine_tracker(
    tx: &mut Transaction<'_, Postgres>,
    engine_id: Uuid,
    value: &Value,
) -> Result<bool> {
    ensure_object(value, "engine_profile", "tracker")?;
    data_config::update_engine_tracker(tx.as_mut(), engine_id, value).await?;
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
    let column = match field {
        "library_root" => FsStringField::LibraryRoot,
        "par2" => FsStringField::Par2,
        "move_mode" => FsStringField::MoveMode,
        _ => {
            return Err(ConfigError::UnknownField {
                section: "fs_policy".to_string(),
                field: field.to_string(),
            }
            .into());
        }
    };
    data_config::update_fs_string_field(tx.as_mut(), policy_id, column, text).await?;
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
    let column = match field {
        "extract" => FsBooleanField::Extract,
        "flatten" => FsBooleanField::Flatten,
        _ => {
            return Err(ConfigError::UnknownField {
                section: "fs_policy".to_string(),
                field: field.to_string(),
            }
            .into());
        }
    };
    data_config::update_fs_boolean_field(tx.as_mut(), policy_id, column, flag).await?;
    Ok(true)
}

async fn set_fs_array_field(
    tx: &mut Transaction<'_, Postgres>,
    policy_id: Uuid,
    value: &Value,
    field: &str,
) -> Result<bool> {
    ensure_array(value, "fs_policy", field)?;
    let column = match field {
        "cleanup_keep" => FsArrayField::CleanupKeep,
        "cleanup_drop" => FsArrayField::CleanupDrop,
        "allow_paths" => FsArrayField::AllowPaths,
        _ => {
            return Err(ConfigError::UnknownField {
                section: "fs_policy".to_string(),
                field: field.to_string(),
            }
            .into());
        }
    };
    data_config::update_fs_array_field(tx.as_mut(), policy_id, column, value).await?;
    Ok(true)
}

async fn set_fs_optional_string_field(
    tx: &mut Transaction<'_, Postgres>,
    policy_id: Uuid,
    value: &Value,
    field: &str,
) -> Result<bool> {
    let column = match field {
        "chmod_file" => FsOptionalStringField::ChmodFile,
        "chmod_dir" => FsOptionalStringField::ChmodDir,
        "owner" => FsOptionalStringField::Owner,
        "group" => FsOptionalStringField::Group,
        "umask" => FsOptionalStringField::Umask,
        _ => {
            return Err(ConfigError::UnknownField {
                section: "fs_policy".to_string(),
                field: field.to_string(),
            }
            .into());
        }
    };
    if value.is_null() {
        data_config::update_fs_optional_string_field(tx.as_mut(), policy_id, column, None).await?;
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
    data_config::update_fs_optional_string_field(tx.as_mut(), policy_id, column, Some(text))
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
    let affected = data_config::delete_api_key(tx.as_mut(), key_id).await?;
    Ok(affected > 0)
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
    let existing = data_config::fetch_api_key_hash(tx.as_mut(), key_id).await?;

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
        data_config::update_api_key_hash(tx.as_mut(), key_id, &hash).await?;
        true
    } else {
        false
    };

    let changed_label = if let Some(text) = label {
        ensure_mutable(immutable_keys, "auth_api_keys", "label")?;
        data_config::update_api_key_label(tx.as_mut(), key_id, Some(text)).await?;
        true
    } else {
        false
    };

    let changed_enabled = if let Some(flag) = enabled {
        ensure_mutable(immutable_keys, "auth_api_keys", "enabled")?;
        data_config::update_api_key_enabled(tx.as_mut(), key_id, flag).await?;
        true
    } else {
        false
    };

    let changed_rate_limit = if let Some(limit_value) = rate_limit {
        ensure_mutable(immutable_keys, "auth_api_keys", "rate_limit")?;
        let parsed = parse_api_key_rate_limit_for_config(limit_value)?;
        let stored = serialise_rate_limit(parsed.as_ref());
        data_config::update_api_key_rate_limit(tx.as_mut(), key_id, &stored).await?;
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

    data_config::insert_api_key(
        tx.as_mut(),
        key_id,
        &hash,
        label,
        enabled_flag,
        &stored_limit,
    )
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
                data_config::upsert_secret(tx.as_mut(), name, ciphertext.as_bytes(), actor).await?;
                events.push(json!({ "op": "set", "name": name }));
            }
            SecretPatch::Delete { name } => {
                ensure_mutable(immutable_keys, "settings_secret", name)?;
                let affected = data_config::delete_secret(tx.as_mut(), name).await?;
                if affected > 0 {
                    events.push(json!({ "op": "delete", "name": name }));
                }
            }
        }
    }
    Ok(events)
}

fn serialise_rate_limit(limit: Option<&ApiKeyRateLimit>) -> Value {
    limit.map_or_else(|| Value::Object(Map::new()), ApiKeyRateLimit::to_json)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::str::FromStr;

    #[test]
    fn app_mode_parses_and_formats() {
        assert_eq!(AppMode::from_str("setup").unwrap(), AppMode::Setup);
        assert_eq!(AppMode::from_str("active").unwrap(), AppMode::Active);
        assert!(AppMode::from_str("invalid").is_err());
        assert_eq!(AppMode::Setup.as_str(), "setup");
        assert_eq!(AppMode::Active.as_str(), "active");
    }

    #[test]
    fn parse_port_accepts_valid_range() {
        let value = json!(8080);
        let port = parse_port(&value, "app_profile", "http_port").expect("port should parse");
        assert_eq!(port, 8080);
    }

    #[test]
    fn parse_port_rejects_out_of_range_and_non_numeric() {
        let value = json!(0);
        let err = parse_port(&value, "app_profile", "http_port").unwrap_err();
        assert!(err.to_string().contains("between 1 and 65535"));

        let non_numeric = json!("not-a-port");
        let err = parse_port(&non_numeric, "app_profile", "http_port").unwrap_err();
        assert!(err.to_string().contains("must be an integer"));
    }
}
