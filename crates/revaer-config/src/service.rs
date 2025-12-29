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
use chrono::{Duration as ChronoDuration, Utc, Weekday};
use rand::Rng;
use rand::distr::Alphanumeric;
use revaer_data::config::{
    self as data_config, AppProfileRow, EngineProfileRow, FsArrayField, FsBooleanField,
    FsOptionalStringField, FsPolicyRow, FsStringField, LabelPolicyRow, NewSetupToken,
    SETTINGS_CHANNEL, SeedingToggleSet,
};
use sqlx::postgres::{PgListener, PgNotification, PgPoolOptions};
use sqlx::{Executor, Postgres, Transaction};
use std::collections::HashSet;
use std::str::FromStr;
use std::time::Duration;
use tokio::fs;
use tokio::time::sleep;
use tracing::{info, instrument, warn};
use uuid::Uuid;

use crate::engine_profile::{
    AltSpeedConfig, AltSpeedSchedule, IpFilterConfig, PeerClassConfig, PeerClassesConfig,
    TrackerAuthConfig, TrackerConfig, TrackerProxyConfig, TrackerProxyType, normalize_engine_profile,
};
use crate::model::{
    ApiKeyAuth, ApiKeyPatch, ApiKeyRateLimit, AppMode, AppProfile, AppliedChanges, ConfigSnapshot,
    EngineProfile, FsPolicy, LabelPolicy, SettingsChange, SettingsChangeset, SettingsPayload,
    SetupToken, TelemetryConfig,
};
use crate::SecretPatch;
use crate::validate::{
    ConfigError, ensure_mutable, parse_bind_addr, parse_uuid, validate_api_key_rate_limit,
    validate_port,
};

const APP_PROFILE_ID: &str = "00000000-0000-0000-0000-000000000001";
const ENGINE_PROFILE_ID: &str = "00000000-0000-0000-0000-000000000002";
const FS_POLICY_ID: &str = "00000000-0000-0000-0000-000000000003";

#[async_trait]
/// Abstraction over configuration backends used by the application service.
pub trait SettingsFacade: Send + Sync {
    /// Retrieve the current application profile.
    async fn get_app_profile(&self) -> Result<AppProfile>;
    /// Retrieve the current engine profile.
    async fn get_engine_profile(&self) -> Result<EngineProfile>;
    /// Retrieve the current filesystem policy.
    async fn get_fs_policy(&self) -> Result<FsPolicy>;
    /// Retrieve a secret value by name if present.
    async fn get_secret(&self, name: &str) -> Result<Option<String>>;
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
    /// Check whether any API keys are configured.
    async fn has_api_keys(&self) -> Result<bool>;
    /// Perform a factory reset of configuration + runtime tables.
    async fn factory_reset(&self) -> Result<()>;
}

// Models are defined in model.rs.

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
        let effective_engine = normalize_engine_profile(&engine);
        let fs = fetch_fs_policy(&self.pool).await?;
        let revision = fetch_revision(&self.pool).await?;

        Ok(ConfigSnapshot {
            revision,
            app_profile: app,
            engine_profile: engine,
            engine_profile_effective: effective_engine,
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

        let rate_limit = match (record.rate_limit_burst, record.rate_limit_per_seconds) {
            (Some(burst), Some(per_seconds)) => {
                let burst = u32::try_from(burst)
                    .context("invalid rate_limit burst for API key")?;
                let per_seconds = u64::try_from(per_seconds)
                    .context("invalid rate_limit per_seconds for API key")?;
                let limit = ApiKeyRateLimit {
                    burst,
                    replenish_period: Duration::from_secs(per_seconds),
                };
                validate_api_key_rate_limit(&limit)?;
                Some(limit)
            }
            _ => None,
        };

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

    async fn get_secret(&self, name: &str) -> Result<Option<String>> {
        let record = data_config::fetch_secret_by_name(&self.pool, name).await?;
        record
            .map(|row| {
                String::from_utf8(row.ciphertext).context("secret payload is not valid UTF-8")
            })
            .transpose()
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
        _reason: &str,
        changeset: SettingsChangeset,
    ) -> Result<AppliedChanges> {
        let mut tx = self.pool.begin().await?;

        let current_app = fetch_app_profile_tx(&mut tx).await?;
        let immutable_keys: HashSet<String> = current_app.immutable_keys.iter().cloned().collect();

        let mut applied_app: Option<AppProfile> = None;
        let mut applied_engine: Option<EngineProfile> = None;
        let mut applied_fs: Option<FsPolicy> = None;
        let mut any_change = false;

        if let Some(app_update) = changeset.app_profile {
            if apply_app_profile_update(&mut tx, &current_app, &app_update, &immutable_keys)
                .await?
            {
                applied_app = Some(fetch_app_profile_tx(&mut tx).await?);
                any_change = true;
            }
        }

        if let Some(engine_update) = changeset.engine_profile {
            let current_engine = fetch_engine_profile(tx.as_mut()).await?;
            if apply_engine_profile_update(&mut tx, &current_engine, &engine_update, &immutable_keys)
                .await?
            {
                applied_engine = Some(fetch_engine_profile(tx.as_mut()).await?);
                any_change = true;
            }
        }

        if let Some(fs_update) = changeset.fs_policy {
            let current_fs = fetch_fs_policy(tx.as_mut()).await?;
            if apply_fs_policy_update(&mut tx, &current_fs, &fs_update, &immutable_keys).await? {
                applied_fs = Some(fetch_fs_policy(tx.as_mut()).await?);
                any_change = true;
            }
        }

        let api_keys_changed = if changeset.api_keys.is_empty() {
            false
        } else {
            apply_api_key_patches(&mut tx, &changeset.api_keys, &immutable_keys).await?
        };
        if api_keys_changed {
            any_change = true;
        }

        let secrets_changed = if changeset.secrets.is_empty() {
            false
        } else {
            apply_secret_patches(&mut tx, &changeset.secrets, actor, &immutable_keys).await?
        };
        if secrets_changed {
            any_change = true;
        }

        let mutated_via_triggers = applied_app.is_some()
            || applied_engine.is_some()
            || applied_fs.is_some()
            || api_keys_changed;
        if secrets_changed && !mutated_via_triggers {
            data_config::bump_revision(tx.as_mut(), "settings_secret").await?;
        }

        let revision = fetch_revision(tx.as_mut()).await?;
        if any_change {
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

        tx.commit().await?;
        info!("setup token consumed successfully");
        Ok(())
    }

    async fn has_api_keys(&self) -> Result<bool> {
        let keys = data_config::fetch_api_keys(&self.pool)
            .await
            .context("failed to load API keys")?;
        Ok(!keys.is_empty())
    }

    async fn factory_reset(&self) -> Result<()> {
        data_config::factory_reset(&self.pool).await?;
        info!("factory reset completed");
        Ok(())
    }
}

async fn apply_migrations(pool: &sqlx::PgPool) -> Result<()> {
    data_config::run_migrations(pool)
        .await
        .context("failed to apply configuration migrations")?;
    Ok(())
}

async fn fetch_app_profile(pool: &sqlx::PgPool) -> Result<AppProfile> {
    let id = parse_uuid(APP_PROFILE_ID)?;
    let row = data_config::fetch_app_profile_row(pool, id)
        .await
        .context("failed to load app_profile row")?;
    let labels = data_config::fetch_app_label_policies(pool, id)
        .await
        .context("failed to load app label policies")?;
    map_app_profile_row(row, labels)
}

async fn fetch_app_profile_tx(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<AppProfile> {
    let id = parse_uuid(APP_PROFILE_ID)?;
    let row = data_config::fetch_app_profile_row(tx.as_mut(), id)
        .await
        .context("failed to load app_profile row")?;
    let labels = data_config::fetch_app_label_policies(tx.as_mut(), id)
        .await
        .context("failed to load app label policies")?;
    map_app_profile_row(row, labels)
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
        "engine_profile" => {
            SettingsPayload::EngineProfile(Box::new(fetch_engine_profile(pool).await?))
        }
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

// fetch_app_profile_row removed; pool/tx variants above handle label policies explicitly.

fn map_app_profile_row(row: AppProfileRow, label_rows: Vec<LabelPolicyRow>) -> Result<AppProfile> {
    let mode = AppMode::from_str(&row.mode)?;
    let telemetry = TelemetryConfig {
        level: row.telemetry_level,
        format: row.telemetry_format,
        otel_enabled: row.telemetry_otel_enabled,
        otel_service_name: row.telemetry_otel_service_name,
        otel_endpoint: row.telemetry_otel_endpoint,
    };
    let label_policies = map_label_policies(label_rows)?;
    Ok(AppProfile {
        id: row.id,
        instance_name: row.instance_name,
        mode,
        version: row.version,
        http_port: row.http_port,
        bind_addr: parse_bind_addr(&row.bind_addr)?,
        telemetry,
        label_policies,
        immutable_keys: row.immutable_keys,
    })
}

fn map_engine_profile_row(row: EngineProfileRow) -> EngineProfile {
    let tracker = map_tracker_config(&row);
    let alt_speed = map_alt_speed_config(&row);
    let ip_filter = map_ip_filter_config(&row);
    let peer_classes = map_peer_classes_config(&row);

    EngineProfile {
        id: row.id,
        implementation: row.implementation,
        listen_port: row.listen_port,
        listen_interfaces: row.listen_interfaces,
        ipv6_mode: row.ipv6_mode,
        dht: row.dht,
        encryption: row.encryption,
        max_active: row.max_active,
        max_download_bps: row.max_download_bps,
        max_upload_bps: row.max_upload_bps,
        seed_ratio_limit: row.seed_ratio_limit,
        seed_time_limit: row.seed_time_limit,
        sequential_default: row.seeding.sequential_default(),
        auto_managed: row.queue.auto_managed().into(),
        auto_manage_prefer_seeds: row.queue.prefer_seeds().into(),
        dont_count_slow_torrents: row.queue.dont_count_slow().into(),
        super_seeding: row.seeding.super_seeding().into(),
        choking_algorithm: row.choking_algorithm,
        seed_choking_algorithm: row.seed_choking_algorithm,
        strict_super_seeding: row.seeding.strict_super_seeding().into(),
        optimistic_unchoke_slots: row.optimistic_unchoke_slots,
        max_queued_disk_bytes: row.max_queued_disk_bytes,
        resume_dir: row.resume_dir,
        download_root: row.download_root,
        storage_mode: row.storage_mode,
        use_partfile: row.storage.use_partfile().into(),
        disk_read_mode: row.disk_read_mode,
        disk_write_mode: row.disk_write_mode,
        verify_piece_hashes: row.verify_piece_hashes.into(),
        cache_size: row.cache_size,
        cache_expiry: row.cache_expiry,
        coalesce_reads: row.storage.coalesce_reads().into(),
        coalesce_writes: row.storage.coalesce_writes().into(),
        use_disk_cache_pool: row.storage.use_disk_cache_pool().into(),
        enable_lsd: row.nat.lsd().into(),
        enable_upnp: row.nat.upnp().into(),
        enable_natpmp: row.nat.natpmp().into(),
        enable_pex: row.nat.pex().into(),
        dht_bootstrap_nodes: row.dht_bootstrap_nodes,
        dht_router_nodes: row.dht_router_nodes,
        anonymous_mode: row.privacy.anonymous_mode().into(),
        force_proxy: row.privacy.force_proxy().into(),
        prefer_rc4: row.privacy.prefer_rc4().into(),
        allow_multiple_connections_per_ip: row.privacy.allow_multiple_connections_per_ip().into(),
        enable_outgoing_utp: row.privacy.enable_outgoing_utp().into(),
        enable_incoming_utp: row.privacy.enable_incoming_utp().into(),
        outgoing_port_min: row.outgoing_port_min,
        outgoing_port_max: row.outgoing_port_max,
        peer_dscp: row.peer_dscp,
        connections_limit: row.connections_limit,
        connections_limit_per_torrent: row.connections_limit_per_torrent,
        unchoke_slots: row.unchoke_slots,
        half_open_limit: row.half_open_limit,
        alt_speed,
        stats_interval_ms: row.stats_interval_ms.map(i64::from),
        tracker,
        ip_filter,
        peer_classes,
    }
}

fn map_label_policies(rows: Vec<LabelPolicyRow>) -> Result<Vec<LabelPolicy>> {
    rows.into_iter()
        .map(|row| {
            let kind = row.kind.parse()?;
            Ok(LabelPolicy {
                kind,
                name: row.name,
                download_dir: row.download_dir,
                rate_limit_download_bps: row.rate_limit_download_bps,
                rate_limit_upload_bps: row.rate_limit_upload_bps,
                queue_position: row.queue_position,
                auto_managed: row.auto_managed,
                seed_ratio_limit: row.seed_ratio_limit,
                seed_time_limit: row.seed_time_limit,
                cleanup_seed_ratio_limit: row.cleanup_seed_ratio_limit,
                cleanup_seed_time_limit: row.cleanup_seed_time_limit,
                cleanup_remove_data: row.cleanup_remove_data,
            })
        })
        .collect()
}

fn map_tracker_config(row: &EngineProfileRow) -> TrackerConfig {
    let proxy = match (&row.tracker_proxy_host, row.tracker_proxy_port) {
        (Some(host), Some(port)) => Some(TrackerProxyConfig {
            host: host.clone(),
            port: u16::try_from(port).unwrap_or(0),
            username_secret: row.tracker_proxy_username_secret.clone(),
            password_secret: row.tracker_proxy_password_secret.clone(),
            kind: parse_tracker_proxy_kind(row.tracker_proxy_kind.as_deref()),
            proxy_peers: row.tracker_proxy_peers.unwrap_or(false),
        }),
        _ => None,
    };

    let auth = if row.tracker_auth_username_secret.is_some()
        || row.tracker_auth_password_secret.is_some()
        || row.tracker_auth_cookie_secret.is_some()
    {
        Some(TrackerAuthConfig {
            username_secret: row.tracker_auth_username_secret.clone(),
            password_secret: row.tracker_auth_password_secret.clone(),
            cookie_secret: row.tracker_auth_cookie_secret.clone(),
        })
    } else {
        None
    };

    TrackerConfig {
        default: row.tracker_default_urls.clone(),
        extra: row.tracker_extra_urls.clone(),
        replace: row.tracker_replace_trackers.unwrap_or(false),
        user_agent: row.tracker_user_agent.clone(),
        announce_ip: row.tracker_announce_ip.clone(),
        listen_interface: row.tracker_listen_interface.clone(),
        request_timeout_ms: row.tracker_request_timeout_ms.map(i64::from),
        announce_to_all: row.tracker_announce_to_all.unwrap_or(false),
        ssl_cert: row.tracker_ssl_cert.clone(),
        ssl_private_key: row.tracker_ssl_private_key.clone(),
        ssl_ca_cert: row.tracker_ssl_ca_cert.clone(),
        ssl_tracker_verify: row.tracker_ssl_verify.unwrap_or(true),
        proxy,
        auth,
    }
}

fn map_alt_speed_config(row: &EngineProfileRow) -> AltSpeedConfig {
    let days = row
        .alt_speed_days
        .iter()
        .filter_map(|label| parse_weekday_label(label))
        .collect::<Vec<_>>();
    let schedule = match (
        row.alt_speed_schedule_start_minutes,
        row.alt_speed_schedule_end_minutes,
    ) {
        (Some(start), Some(end)) if !days.is_empty() => {
            let start = u16::try_from(start.max(0)).ok();
            let end = u16::try_from(end.max(0)).ok();
            match (start, end) {
                (Some(start), Some(end)) => Some(AltSpeedSchedule {
                    days,
                    start_minutes: start,
                    end_minutes: end,
                }),
                _ => None,
            }
        }
        _ => None,
    };

    AltSpeedConfig {
        download_bps: row.alt_speed_download_bps,
        upload_bps: row.alt_speed_upload_bps,
        schedule,
    }
}

fn map_ip_filter_config(row: &EngineProfileRow) -> IpFilterConfig {
    IpFilterConfig {
        cidrs: row.ip_filter_cidrs.clone(),
        blocklist_url: row.ip_filter_blocklist_url.clone(),
        etag: row.ip_filter_etag.clone(),
        last_updated_at: row.ip_filter_last_updated_at,
        last_error: row.ip_filter_last_error.clone(),
    }
}

fn map_peer_classes_config(row: &EngineProfileRow) -> PeerClassesConfig {
    let len = row
        .peer_class_ids
        .len()
        .min(row.peer_class_labels.len())
        .min(row.peer_class_download_priorities.len())
        .min(row.peer_class_upload_priorities.len())
        .min(row.peer_class_connection_limit_factors.len())
        .min(row.peer_class_ignore_unchoke_slots.len());

    let mut classes = Vec::new();
    for idx in 0..len {
        let id = u8::try_from(row.peer_class_ids[idx]).unwrap_or(0);
        let label = row.peer_class_labels[idx].clone();
        let download_priority = u8::try_from(row.peer_class_download_priorities[idx]).unwrap_or(1);
        let upload_priority = u8::try_from(row.peer_class_upload_priorities[idx]).unwrap_or(1);
        let connection_limit_factor =
            u16::try_from(row.peer_class_connection_limit_factors[idx]).unwrap_or(100);
        let ignore_unchoke_slots = row.peer_class_ignore_unchoke_slots[idx];

        classes.push(PeerClassConfig {
            id,
            label,
            download_priority,
            upload_priority,
            connection_limit_factor,
            ignore_unchoke_slots,
        });
    }

    let default = row
        .peer_class_default_ids
        .iter()
        .filter_map(|id| u8::try_from(*id).ok())
        .collect::<Vec<_>>();

    PeerClassesConfig { classes, default }
}

fn parse_tracker_proxy_kind(value: Option<&str>) -> TrackerProxyType {
    match value.unwrap_or("http").trim().to_ascii_lowercase().as_str() {
        "https" => TrackerProxyType::Https,
        "socks5" => TrackerProxyType::Socks5,
        _ => TrackerProxyType::Http,
    }
}

fn parse_weekday_label(value: &str) -> Option<Weekday> {
    match value.trim().to_ascii_lowercase().as_str() {
        "mon" | "monday" => Some(Weekday::Mon),
        "tue" | "tues" | "tuesday" => Some(Weekday::Tue),
        "wed" | "wednesday" => Some(Weekday::Wed),
        "thu" | "thur" | "thurs" | "thursday" => Some(Weekday::Thu),
        "fri" | "friday" => Some(Weekday::Fri),
        "sat" | "saturday" => Some(Weekday::Sat),
        "sun" | "sunday" => Some(Weekday::Sun),
        _ => None,
    }
}

async fn validate_directory_path(
    section: &str,
    field: &str,
    path: &str,
) -> Result<(), ConfigError> {
    if path.trim().is_empty() {
        return Err(ConfigError::InvalidField {
            section: section.to_string(),
            field: field.to_string(),
            message: "path must not be empty".to_string(),
        });
    }
    let metadata = fs::metadata(path)
        .await
        .map_err(|err| ConfigError::InvalidField {
            section: section.to_string(),
            field: field.to_string(),
            message: format!("{path}: {err}"),
        })?;
    if !metadata.is_dir() {
        return Err(ConfigError::InvalidField {
            section: section.to_string(),
            field: field.to_string(),
            message: format!("{path}: not a directory"),
        });
    }
    Ok(())
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

async fn apply_app_profile_update(
    tx: &mut Transaction<'_, Postgres>,
    current: &AppProfile,
    update: &AppProfile,
    immutable_keys: &HashSet<String>,
) -> Result<bool> {
    let app_id = parse_uuid(APP_PROFILE_ID)?;
    if update.id != app_id {
        return Err(ConfigError::InvalidField {
            section: "app_profile".to_string(),
            field: "id".to_string(),
            message: "invalid app profile id".to_string(),
        }
        .into());
    }

    let mut mutated = false;
    if update.instance_name != current.instance_name {
        ensure_mutable(immutable_keys, "app_profile", "instance_name")?;
        data_config::update_app_instance_name(tx.as_mut(), app_id, &update.instance_name).await?;
        mutated = true;
    }
    if update.mode != current.mode {
        ensure_mutable(immutable_keys, "app_profile", "mode")?;
        data_config::update_app_mode(tx.as_mut(), app_id, update.mode.as_str()).await?;
        mutated = true;
    }
    if update.http_port != current.http_port {
        ensure_mutable(immutable_keys, "app_profile", "http_port")?;
        validate_port(update.http_port, "app_profile", "http_port")?;
        data_config::update_app_http_port(tx.as_mut(), app_id, update.http_port).await?;
        mutated = true;
    }
    if update.bind_addr != current.bind_addr {
        ensure_mutable(immutable_keys, "app_profile", "bind_addr")?;
        data_config::update_app_bind_addr(tx.as_mut(), app_id, &update.bind_addr.to_string())
            .await?;
        mutated = true;
    }
    if update.telemetry != current.telemetry {
        ensure_mutable(immutable_keys, "app_profile", "telemetry")?;
        data_config::update_app_telemetry(
            tx.as_mut(),
            app_id,
            update.telemetry.level.as_deref(),
            update.telemetry.format.as_deref(),
            update.telemetry.otel_enabled,
            update.telemetry.otel_service_name.as_deref(),
            update.telemetry.otel_endpoint.as_deref(),
        )
        .await?;
        mutated = true;
    }
    if update.label_policies != current.label_policies {
        ensure_mutable(immutable_keys, "app_profile", "features")?;
        validate_label_policy_paths(&update.label_policies).await?;
        apply_label_policies(tx, app_id, &update.label_policies).await?;
        mutated = true;
    }
    if update.immutable_keys != current.immutable_keys {
        ensure_mutable(immutable_keys, "app_profile", "immutable_keys")?;
        data_config::update_app_immutable_keys(tx.as_mut(), app_id, &update.immutable_keys).await?;
        mutated = true;
    }

    if mutated {
        bump_app_profile_version(tx, app_id).await?;
    }

    Ok(mutated)
}

async fn apply_engine_profile_update(
    tx: &mut Transaction<'_, Postgres>,
    current: &EngineProfile,
    update: &EngineProfile,
    immutable_keys: &HashSet<String>,
) -> Result<bool> {
    let engine_id = parse_uuid(ENGINE_PROFILE_ID)?;
    if update.id != engine_id {
        return Err(ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "id".to_string(),
            message: "invalid engine profile id".to_string(),
        }
        .into());
    }

    if update == current {
        return Ok(false);
    }

    ensure_engine_profile_mutable(current, update, immutable_keys)?;
    if update.download_root != current.download_root {
        validate_directory_path("engine_profile", "download_root", &update.download_root).await?;
    }
    if update.resume_dir != current.resume_dir {
        validate_directory_path("engine_profile", "resume_dir", &update.resume_dir).await?;
    }

    let stored = normalize_engine_profile_for_storage(update);
    persist_engine_profile(tx, &stored).await?;
    Ok(true)
}

async fn apply_fs_policy_update(
    tx: &mut Transaction<'_, Postgres>,
    current: &FsPolicy,
    update: &FsPolicy,
    immutable_keys: &HashSet<String>,
) -> Result<bool> {
    let policy_id = parse_uuid(FS_POLICY_ID)?;
    if update.id != policy_id {
        return Err(ConfigError::InvalidField {
            section: "fs_policy".to_string(),
            field: "id".to_string(),
            message: "invalid filesystem policy id".to_string(),
        }
        .into());
    }

    let mut mutated = false;
    if update.library_root != current.library_root {
        ensure_mutable(immutable_keys, "fs_policy", "library_root")?;
        validate_directory_path("fs_policy", "library_root", &update.library_root).await?;
        data_config::update_fs_string_field(
            tx.as_mut(),
            policy_id,
            FsStringField::LibraryRoot,
            &update.library_root,
        )
        .await?;
        mutated = true;
    }
    if update.extract != current.extract {
        ensure_mutable(immutable_keys, "fs_policy", "extract")?;
        data_config::update_fs_boolean_field(
            tx.as_mut(),
            policy_id,
            FsBooleanField::Extract,
            update.extract,
        )
        .await?;
        mutated = true;
    }
    if update.par2 != current.par2 {
        ensure_mutable(immutable_keys, "fs_policy", "par2")?;
        data_config::update_fs_string_field(
            tx.as_mut(),
            policy_id,
            FsStringField::Par2,
            &update.par2,
        )
        .await?;
        mutated = true;
    }
    if update.flatten != current.flatten {
        ensure_mutable(immutable_keys, "fs_policy", "flatten")?;
        data_config::update_fs_boolean_field(
            tx.as_mut(),
            policy_id,
            FsBooleanField::Flatten,
            update.flatten,
        )
        .await?;
        mutated = true;
    }
    if update.move_mode != current.move_mode {
        ensure_mutable(immutable_keys, "fs_policy", "move_mode")?;
        data_config::update_fs_string_field(
            tx.as_mut(),
            policy_id,
            FsStringField::MoveMode,
            &update.move_mode,
        )
        .await?;
        mutated = true;
    }
    if update.cleanup_keep != current.cleanup_keep {
        ensure_mutable(immutable_keys, "fs_policy", "cleanup_keep")?;
        data_config::update_fs_array_field(
            tx.as_mut(),
            policy_id,
            FsArrayField::CleanupKeep,
            &update.cleanup_keep,
        )
        .await?;
        mutated = true;
    }
    if update.cleanup_drop != current.cleanup_drop {
        ensure_mutable(immutable_keys, "fs_policy", "cleanup_drop")?;
        data_config::update_fs_array_field(
            tx.as_mut(),
            policy_id,
            FsArrayField::CleanupDrop,
            &update.cleanup_drop,
        )
        .await?;
        mutated = true;
    }
    if update.chmod_file != current.chmod_file {
        ensure_mutable(immutable_keys, "fs_policy", "chmod_file")?;
        data_config::update_fs_optional_string_field(
            tx.as_mut(),
            policy_id,
            FsOptionalStringField::ChmodFile,
            update.chmod_file.as_deref(),
        )
        .await?;
        mutated = true;
    }
    if update.chmod_dir != current.chmod_dir {
        ensure_mutable(immutable_keys, "fs_policy", "chmod_dir")?;
        data_config::update_fs_optional_string_field(
            tx.as_mut(),
            policy_id,
            FsOptionalStringField::ChmodDir,
            update.chmod_dir.as_deref(),
        )
        .await?;
        mutated = true;
    }
    if update.owner != current.owner {
        ensure_mutable(immutable_keys, "fs_policy", "owner")?;
        data_config::update_fs_optional_string_field(
            tx.as_mut(),
            policy_id,
            FsOptionalStringField::Owner,
            update.owner.as_deref(),
        )
        .await?;
        mutated = true;
    }
    if update.group != current.group {
        ensure_mutable(immutable_keys, "fs_policy", "group")?;
        data_config::update_fs_optional_string_field(
            tx.as_mut(),
            policy_id,
            FsOptionalStringField::Group,
            update.group.as_deref(),
        )
        .await?;
        mutated = true;
    }
    if update.umask != current.umask {
        ensure_mutable(immutable_keys, "fs_policy", "umask")?;
        data_config::update_fs_optional_string_field(
            tx.as_mut(),
            policy_id,
            FsOptionalStringField::Umask,
            update.umask.as_deref(),
        )
        .await?;
        mutated = true;
    }
    if update.allow_paths != current.allow_paths {
        ensure_mutable(immutable_keys, "fs_policy", "allow_paths")?;
        validate_allow_paths(&update.allow_paths).await?;
        data_config::update_fs_array_field(
            tx.as_mut(),
            policy_id,
            FsArrayField::AllowPaths,
            &update.allow_paths,
        )
        .await?;
        mutated = true;
    }

    Ok(mutated)
}

async fn apply_label_policies(
    tx: &mut Transaction<'_, Postgres>,
    app_id: Uuid,
    policies: &[LabelPolicy],
) -> Result<()> {
    let mut kinds = Vec::with_capacity(policies.len());
    let mut names = Vec::with_capacity(policies.len());
    let mut download_dirs = Vec::with_capacity(policies.len());
    let mut rate_limit_download_bps = Vec::with_capacity(policies.len());
    let mut rate_limit_upload_bps = Vec::with_capacity(policies.len());
    let mut queue_positions = Vec::with_capacity(policies.len());
    let mut auto_managed = Vec::with_capacity(policies.len());
    let mut seed_ratio_limits = Vec::with_capacity(policies.len());
    let mut seed_time_limits = Vec::with_capacity(policies.len());
    let mut cleanup_seed_ratio_limits = Vec::with_capacity(policies.len());
    let mut cleanup_seed_time_limits = Vec::with_capacity(policies.len());
    let mut cleanup_remove_data = Vec::with_capacity(policies.len());

    for policy in policies {
        kinds.push(policy.kind.as_str().to_string());
        names.push(policy.name.clone());
        download_dirs.push(policy.download_dir.clone());
        rate_limit_download_bps.push(policy.rate_limit_download_bps);
        rate_limit_upload_bps.push(policy.rate_limit_upload_bps);
        queue_positions.push(policy.queue_position);
        auto_managed.push(policy.auto_managed);
        seed_ratio_limits.push(policy.seed_ratio_limit);
        seed_time_limits.push(policy.seed_time_limit);
        cleanup_seed_ratio_limits.push(policy.cleanup_seed_ratio_limit);
        cleanup_seed_time_limits.push(policy.cleanup_seed_time_limit);
        cleanup_remove_data.push(policy.cleanup_remove_data);
    }

    data_config::replace_app_label_policies(
        tx.as_mut(),
        app_id,
        &kinds,
        &names,
        &download_dirs,
        &rate_limit_download_bps,
        &rate_limit_upload_bps,
        &queue_positions,
        &auto_managed,
        &seed_ratio_limits,
        &seed_time_limits,
        &cleanup_seed_ratio_limits,
        &cleanup_seed_time_limits,
        &cleanup_remove_data,
    )
    .await?;

    Ok(())
}

async fn validate_label_policy_paths(policies: &[LabelPolicy]) -> Result<(), ConfigError> {
    for policy in policies {
        if let Some(path) = policy.download_dir.as_deref() {
            if path.trim().is_empty() {
                continue;
            }
            validate_directory_path(
                "app_profile",
                &format!("features.{}.{}.download_dir", policy.kind.as_str(), policy.name),
                path,
            )
            .await?;
        }
    }
    Ok(())
}

async fn validate_allow_paths(paths: &[String]) -> Result<(), ConfigError> {
    for path in paths {
        if path.trim().is_empty() {
            continue;
        }
        validate_directory_path("fs_policy", "allow_paths", path).await?;
    }
    Ok(())
}

async fn bump_app_profile_version(tx: &mut Transaction<'_, Postgres>, app_id: Uuid) -> Result<()> {
    data_config::bump_app_profile_version(tx.as_mut(), app_id).await?;
    Ok(())
}

async fn persist_engine_profile(
    tx: &mut Transaction<'_, Postgres>,
    profile: &EngineProfile,
) -> Result<()> {
    data_config::update_engine_profile(
        tx.as_mut(),
        &data_config::EngineProfileUpdate {
            id: profile.id,
            implementation: &profile.implementation,
            listen_port: profile.listen_port,
            dht: profile.dht,
            encryption: &profile.encryption,
            max_active: profile.max_active,
            max_download_bps: profile.max_download_bps,
            max_upload_bps: profile.max_upload_bps,
            seed_ratio_limit: profile.seed_ratio_limit,
            seed_time_limit: profile.seed_time_limit,
            seeding: SeedingToggleSet::from_flags([
                profile.sequential_default,
                bool::from(profile.super_seeding),
                bool::from(profile.strict_super_seeding),
            ]),
            queue: data_config::QueuePolicySet::from_flags([
                bool::from(profile.auto_managed),
                bool::from(profile.auto_manage_prefer_seeds),
                bool::from(profile.dont_count_slow_torrents),
            ]),
            choking_algorithm: &profile.choking_algorithm,
            seed_choking_algorithm: &profile.seed_choking_algorithm,
            optimistic_unchoke_slots: profile.optimistic_unchoke_slots,
            max_queued_disk_bytes: profile.max_queued_disk_bytes,
            resume_dir: &profile.resume_dir,
            download_root: &profile.download_root,
            storage_mode: &profile.storage_mode,
            storage: data_config::StorageToggleSet::from_flags([
                bool::from(profile.use_partfile),
                bool::from(profile.coalesce_reads),
                bool::from(profile.coalesce_writes),
                bool::from(profile.use_disk_cache_pool),
            ]),
            disk_read_mode: profile.disk_read_mode.as_deref(),
            disk_write_mode: profile.disk_write_mode.as_deref(),
            verify_piece_hashes: bool::from(profile.verify_piece_hashes),
            cache_size: profile.cache_size,
            cache_expiry: profile.cache_expiry,
            nat: data_config::NatToggleSet::from_flags([
                bool::from(profile.enable_lsd),
                bool::from(profile.enable_upnp),
                bool::from(profile.enable_natpmp),
                bool::from(profile.enable_pex),
            ]),
            ipv6_mode: &profile.ipv6_mode,
            privacy: data_config::PrivacyToggleSet::from_flags([
                bool::from(profile.anonymous_mode),
                bool::from(profile.force_proxy),
                bool::from(profile.prefer_rc4),
                bool::from(profile.allow_multiple_connections_per_ip),
                bool::from(profile.enable_outgoing_utp),
                bool::from(profile.enable_incoming_utp),
            ]),
            outgoing_port_min: profile.outgoing_port_min,
            outgoing_port_max: profile.outgoing_port_max,
            peer_dscp: profile.peer_dscp,
            connections_limit: profile.connections_limit,
            connections_limit_per_torrent: profile.connections_limit_per_torrent,
            unchoke_slots: profile.unchoke_slots,
            half_open_limit: profile.half_open_limit,
            stats_interval_ms: profile
                .stats_interval_ms
                .and_then(|value| i32::try_from(value).ok()),
        },
    )
    .await?;

    data_config::set_engine_list_values(
        tx.as_mut(),
        profile.id,
        "listen_interfaces",
        &profile.listen_interfaces,
    )
    .await?;
    data_config::set_engine_list_values(
        tx.as_mut(),
        profile.id,
        "dht_bootstrap_nodes",
        &profile.dht_bootstrap_nodes,
    )
    .await?;
    data_config::set_engine_list_values(
        tx.as_mut(),
        profile.id,
        "dht_router_nodes",
        &profile.dht_router_nodes,
    )
    .await?;

    let ip_filter = data_config::IpFilterUpdate {
        blocklist_url: profile.ip_filter.blocklist_url.as_deref(),
        etag: profile.ip_filter.etag.as_deref(),
        last_updated_at: profile.ip_filter.last_updated_at,
        last_error: profile.ip_filter.last_error.as_deref(),
        cidrs: &profile.ip_filter.cidrs,
    };
    data_config::set_engine_ip_filter(tx.as_mut(), profile.id, &ip_filter).await?;

    let (schedule_start, schedule_end, days) = match &profile.alt_speed.schedule {
        Some(schedule) => (
            Some(i32::from(schedule.start_minutes)),
            Some(i32::from(schedule.end_minutes)),
            schedule
                .days
                .iter()
                .map(|day| weekday_label(*day).to_string())
                .collect::<Vec<_>>(),
        ),
        None => (None, None, Vec::new()),
    };
    let alt_speed = data_config::AltSpeedUpdate {
        download_bps: profile.alt_speed.download_bps,
        upload_bps: profile.alt_speed.upload_bps,
        schedule_start_minutes: schedule_start,
        schedule_end_minutes: schedule_end,
        days: &days,
    };
    data_config::set_engine_alt_speed(tx.as_mut(), profile.id, &alt_speed).await?;

    let proxy_host = profile.tracker.proxy.as_ref().map(|proxy| proxy.host.as_str());
    let proxy_port = profile.tracker.proxy.as_ref().map(|proxy| i32::from(proxy.port));
    let proxy_kind = profile
        .tracker
        .proxy
        .as_ref()
        .map(|proxy| proxy.kind.as_str());
    let proxy_username_secret = profile
        .tracker
        .proxy
        .as_ref()
        .and_then(|proxy| proxy.username_secret.as_deref());
    let proxy_password_secret = profile
        .tracker
        .proxy
        .as_ref()
        .and_then(|proxy| proxy.password_secret.as_deref());
    let proxy_peers = profile
        .tracker
        .proxy
        .as_ref()
        .map(|proxy| proxy.proxy_peers)
        .unwrap_or(false);

    let auth_username_secret = profile
        .tracker
        .auth
        .as_ref()
        .and_then(|auth| auth.username_secret.as_deref());
    let auth_password_secret = profile
        .tracker
        .auth
        .as_ref()
        .and_then(|auth| auth.password_secret.as_deref());
    let auth_cookie_secret = profile
        .tracker
        .auth
        .as_ref()
        .and_then(|auth| auth.cookie_secret.as_deref());

    let tracker = data_config::TrackerConfigUpdate {
        user_agent: profile.tracker.user_agent.as_deref(),
        announce_ip: profile.tracker.announce_ip.as_deref(),
        listen_interface: profile.tracker.listen_interface.as_deref(),
        request_timeout_ms: profile
            .tracker
            .request_timeout_ms
            .and_then(|value| i32::try_from(value).ok()),
        announce_to_all: profile.tracker.announce_to_all,
        replace_trackers: profile.tracker.replace,
        proxy_host,
        proxy_port,
        proxy_kind,
        proxy_username_secret,
        proxy_password_secret,
        proxy_peers,
        ssl_cert: profile.tracker.ssl_cert.as_deref(),
        ssl_private_key: profile.tracker.ssl_private_key.as_deref(),
        ssl_ca_cert: profile.tracker.ssl_ca_cert.as_deref(),
        ssl_tracker_verify: profile.tracker.ssl_tracker_verify,
        auth_username_secret,
        auth_password_secret,
        auth_cookie_secret,
        default_urls: &profile.tracker.default,
        extra_urls: &profile.tracker.extra,
    };
    data_config::set_tracker_config(tx.as_mut(), profile.id, &tracker).await?;

    let mut class_ids = Vec::new();
    let mut labels = Vec::new();
    let mut download_priorities = Vec::new();
    let mut upload_priorities = Vec::new();
    let mut connection_limit_factors = Vec::new();
    let mut ignore_unchoke_slots = Vec::new();

    for class in &profile.peer_classes.classes {
        class_ids.push(i16::from(class.id));
        labels.push(class.label.clone());
        download_priorities.push(i16::from(class.download_priority));
        upload_priorities.push(i16::from(class.upload_priority));
        let connection_limit_factor =
            i16::try_from(class.connection_limit_factor).unwrap_or(i16::MAX);
        connection_limit_factors.push(connection_limit_factor);
        ignore_unchoke_slots.push(class.ignore_unchoke_slots);
    }

    let default_class_ids = profile
        .peer_classes
        .default
        .iter()
        .map(|id| i16::from(*id))
        .collect::<Vec<_>>();

    let peer_classes = data_config::PeerClassesUpdate {
        class_ids: &class_ids,
        labels: &labels,
        download_priorities: &download_priorities,
        upload_priorities: &upload_priorities,
        connection_limit_factors: &connection_limit_factors,
        ignore_unchoke_slots: &ignore_unchoke_slots,
        default_class_ids: &default_class_ids,
    };
    data_config::set_peer_classes(tx.as_mut(), profile.id, &peer_classes).await?;
    Ok(())
}

fn normalize_engine_profile_for_storage(profile: &EngineProfile) -> EngineProfile {
    let effective = normalize_engine_profile(profile);
    EngineProfile {
        id: profile.id,
        implementation: profile.implementation.clone(),
        listen_port: effective.network.listen_port,
        listen_interfaces: effective.network.listen_interfaces.clone(),
        ipv6_mode: effective.network.ipv6_mode.as_str().to_string(),
        anonymous_mode: effective.network.anonymous_mode,
        force_proxy: effective.network.force_proxy,
        prefer_rc4: effective.network.prefer_rc4,
        allow_multiple_connections_per_ip: effective.network.allow_multiple_connections_per_ip,
        enable_outgoing_utp: effective.network.enable_outgoing_utp,
        enable_incoming_utp: effective.network.enable_incoming_utp,
        outgoing_port_min: effective.network.outgoing_ports.map(|range| i32::from(range.start)),
        outgoing_port_max: effective.network.outgoing_ports.map(|range| i32::from(range.end)),
        peer_dscp: effective.network.peer_dscp.map(i32::from),
        dht: effective.network.enable_dht,
        encryption: effective.network.encryption.as_str().to_string(),
        max_active: effective.limits.max_active,
        max_download_bps: effective.limits.download_rate_limit,
        max_upload_bps: effective.limits.upload_rate_limit,
        seed_ratio_limit: effective.limits.seed_ratio_limit,
        seed_time_limit: effective.limits.seed_time_limit,
        connections_limit: effective.limits.connections_limit,
        connections_limit_per_torrent: effective.limits.connections_limit_per_torrent,
        unchoke_slots: effective.limits.unchoke_slots,
        half_open_limit: effective.limits.half_open_limit,
        alt_speed: effective.alt_speed.clone(),
        stats_interval_ms: effective.limits.stats_interval_ms.map(i64::from),
        sequential_default: effective.behavior.sequential_default,
        auto_managed: effective.behavior.auto_managed,
        auto_manage_prefer_seeds: effective.behavior.auto_manage_prefer_seeds,
        dont_count_slow_torrents: effective.behavior.dont_count_slow_torrents,
        super_seeding: effective.behavior.super_seeding,
        choking_algorithm: effective.limits.choking_algorithm.as_str().to_string(),
        seed_choking_algorithm: effective.limits.seed_choking_algorithm.as_str().to_string(),
        strict_super_seeding: effective.limits.strict_super_seeding,
        optimistic_unchoke_slots: effective.limits.optimistic_unchoke_slots,
        max_queued_disk_bytes: effective.limits.max_queued_disk_bytes,
        resume_dir: effective.storage.resume_dir.clone(),
        download_root: effective.storage.download_root.clone(),
        storage_mode: effective.storage.storage_mode.as_str().to_string(),
        use_partfile: effective.storage.use_partfile,
        disk_read_mode: effective
            .storage
            .disk_read_mode
            .map(|mode| mode.as_str().to_string()),
        disk_write_mode: effective
            .storage
            .disk_write_mode
            .map(|mode| mode.as_str().to_string()),
        verify_piece_hashes: effective.storage.verify_piece_hashes,
        cache_size: effective.storage.cache_size,
        cache_expiry: effective.storage.cache_expiry,
        coalesce_reads: effective.storage.coalesce_reads,
        coalesce_writes: effective.storage.coalesce_writes,
        use_disk_cache_pool: effective.storage.use_disk_cache_pool,
        tracker: effective.tracker.clone(),
        enable_lsd: effective.network.enable_lsd,
        enable_upnp: effective.network.enable_upnp,
        enable_natpmp: effective.network.enable_natpmp,
        enable_pex: effective.network.enable_pex,
        dht_bootstrap_nodes: effective.network.dht_bootstrap_nodes.clone(),
        dht_router_nodes: effective.network.dht_router_nodes.clone(),
        ip_filter: effective.network.ip_filter.clone(),
        peer_classes: effective.peer_classes.clone(),
    }
}

fn ensure_engine_profile_mutable(
    current: &EngineProfile,
    update: &EngineProfile,
    immutable_keys: &HashSet<String>,
) -> Result<(), ConfigError> {
    if update.implementation != current.implementation {
        ensure_mutable(immutable_keys, "engine_profile", "implementation")?;
    }
    if update.listen_port != current.listen_port {
        ensure_mutable(immutable_keys, "engine_profile", "listen_port")?;
    }
    if update.listen_interfaces != current.listen_interfaces {
        ensure_mutable(immutable_keys, "engine_profile", "listen_interfaces")?;
    }
    if update.ipv6_mode != current.ipv6_mode {
        ensure_mutable(immutable_keys, "engine_profile", "ipv6_mode")?;
    }
    if update.anonymous_mode != current.anonymous_mode {
        ensure_mutable(immutable_keys, "engine_profile", "anonymous_mode")?;
    }
    if update.force_proxy != current.force_proxy {
        ensure_mutable(immutable_keys, "engine_profile", "force_proxy")?;
    }
    if update.prefer_rc4 != current.prefer_rc4 {
        ensure_mutable(immutable_keys, "engine_profile", "prefer_rc4")?;
    }
    if update.allow_multiple_connections_per_ip != current.allow_multiple_connections_per_ip {
        ensure_mutable(immutable_keys, "engine_profile", "allow_multiple_connections_per_ip")?;
    }
    if update.enable_outgoing_utp != current.enable_outgoing_utp {
        ensure_mutable(immutable_keys, "engine_profile", "enable_outgoing_utp")?;
    }
    if update.enable_incoming_utp != current.enable_incoming_utp {
        ensure_mutable(immutable_keys, "engine_profile", "enable_incoming_utp")?;
    }
    if update.outgoing_port_min != current.outgoing_port_min {
        ensure_mutable(immutable_keys, "engine_profile", "outgoing_port_min")?;
    }
    if update.outgoing_port_max != current.outgoing_port_max {
        ensure_mutable(immutable_keys, "engine_profile", "outgoing_port_max")?;
    }
    if update.peer_dscp != current.peer_dscp {
        ensure_mutable(immutable_keys, "engine_profile", "peer_dscp")?;
    }
    if update.dht != current.dht {
        ensure_mutable(immutable_keys, "engine_profile", "dht")?;
    }
    if update.encryption != current.encryption {
        ensure_mutable(immutable_keys, "engine_profile", "encryption")?;
    }
    if update.max_active != current.max_active {
        ensure_mutable(immutable_keys, "engine_profile", "max_active")?;
    }
    if update.max_download_bps != current.max_download_bps {
        ensure_mutable(immutable_keys, "engine_profile", "max_download_bps")?;
    }
    if update.max_upload_bps != current.max_upload_bps {
        ensure_mutable(immutable_keys, "engine_profile", "max_upload_bps")?;
    }
    if update.seed_ratio_limit != current.seed_ratio_limit {
        ensure_mutable(immutable_keys, "engine_profile", "seed_ratio_limit")?;
    }
    if update.seed_time_limit != current.seed_time_limit {
        ensure_mutable(immutable_keys, "engine_profile", "seed_time_limit")?;
    }
    if update.connections_limit != current.connections_limit {
        ensure_mutable(immutable_keys, "engine_profile", "connections_limit")?;
    }
    if update.connections_limit_per_torrent != current.connections_limit_per_torrent {
        ensure_mutable(immutable_keys, "engine_profile", "connections_limit_per_torrent")?;
    }
    if update.unchoke_slots != current.unchoke_slots {
        ensure_mutable(immutable_keys, "engine_profile", "unchoke_slots")?;
    }
    if update.half_open_limit != current.half_open_limit {
        ensure_mutable(immutable_keys, "engine_profile", "half_open_limit")?;
    }
    if update.alt_speed != current.alt_speed {
        ensure_mutable(immutable_keys, "engine_profile", "alt_speed")?;
    }
    if update.stats_interval_ms != current.stats_interval_ms {
        ensure_mutable(immutable_keys, "engine_profile", "stats_interval_ms")?;
    }
    if update.sequential_default != current.sequential_default {
        ensure_mutable(immutable_keys, "engine_profile", "sequential_default")?;
    }
    if update.auto_managed != current.auto_managed {
        ensure_mutable(immutable_keys, "engine_profile", "auto_managed")?;
    }
    if update.auto_manage_prefer_seeds != current.auto_manage_prefer_seeds {
        ensure_mutable(immutable_keys, "engine_profile", "auto_manage_prefer_seeds")?;
    }
    if update.dont_count_slow_torrents != current.dont_count_slow_torrents {
        ensure_mutable(immutable_keys, "engine_profile", "dont_count_slow_torrents")?;
    }
    if update.super_seeding != current.super_seeding {
        ensure_mutable(immutable_keys, "engine_profile", "super_seeding")?;
    }
    if update.choking_algorithm != current.choking_algorithm {
        ensure_mutable(immutable_keys, "engine_profile", "choking_algorithm")?;
    }
    if update.seed_choking_algorithm != current.seed_choking_algorithm {
        ensure_mutable(immutable_keys, "engine_profile", "seed_choking_algorithm")?;
    }
    if update.strict_super_seeding != current.strict_super_seeding {
        ensure_mutable(immutable_keys, "engine_profile", "strict_super_seeding")?;
    }
    if update.optimistic_unchoke_slots != current.optimistic_unchoke_slots {
        ensure_mutable(immutable_keys, "engine_profile", "optimistic_unchoke_slots")?;
    }
    if update.max_queued_disk_bytes != current.max_queued_disk_bytes {
        ensure_mutable(immutable_keys, "engine_profile", "max_queued_disk_bytes")?;
    }
    if update.resume_dir != current.resume_dir {
        ensure_mutable(immutable_keys, "engine_profile", "resume_dir")?;
    }
    if update.download_root != current.download_root {
        ensure_mutable(immutable_keys, "engine_profile", "download_root")?;
    }
    if update.storage_mode != current.storage_mode {
        ensure_mutable(immutable_keys, "engine_profile", "storage_mode")?;
    }
    if update.use_partfile != current.use_partfile {
        ensure_mutable(immutable_keys, "engine_profile", "use_partfile")?;
    }
    if update.disk_read_mode != current.disk_read_mode {
        ensure_mutable(immutable_keys, "engine_profile", "disk_read_mode")?;
    }
    if update.disk_write_mode != current.disk_write_mode {
        ensure_mutable(immutable_keys, "engine_profile", "disk_write_mode")?;
    }
    if update.verify_piece_hashes != current.verify_piece_hashes {
        ensure_mutable(immutable_keys, "engine_profile", "verify_piece_hashes")?;
    }
    if update.cache_size != current.cache_size {
        ensure_mutable(immutable_keys, "engine_profile", "cache_size")?;
    }
    if update.cache_expiry != current.cache_expiry {
        ensure_mutable(immutable_keys, "engine_profile", "cache_expiry")?;
    }
    if update.coalesce_reads != current.coalesce_reads {
        ensure_mutable(immutable_keys, "engine_profile", "coalesce_reads")?;
    }
    if update.coalesce_writes != current.coalesce_writes {
        ensure_mutable(immutable_keys, "engine_profile", "coalesce_writes")?;
    }
    if update.use_disk_cache_pool != current.use_disk_cache_pool {
        ensure_mutable(immutable_keys, "engine_profile", "use_disk_cache_pool")?;
    }
    if update.tracker != current.tracker {
        ensure_mutable(immutable_keys, "engine_profile", "tracker")?;
    }
    if update.enable_lsd != current.enable_lsd {
        ensure_mutable(immutable_keys, "engine_profile", "enable_lsd")?;
    }
    if update.enable_upnp != current.enable_upnp {
        ensure_mutable(immutable_keys, "engine_profile", "enable_upnp")?;
    }
    if update.enable_natpmp != current.enable_natpmp {
        ensure_mutable(immutable_keys, "engine_profile", "enable_natpmp")?;
    }
    if update.enable_pex != current.enable_pex {
        ensure_mutable(immutable_keys, "engine_profile", "enable_pex")?;
    }
    if update.dht_bootstrap_nodes != current.dht_bootstrap_nodes {
        ensure_mutable(immutable_keys, "engine_profile", "dht_bootstrap_nodes")?;
    }
    if update.dht_router_nodes != current.dht_router_nodes {
        ensure_mutable(immutable_keys, "engine_profile", "dht_router_nodes")?;
    }
    if update.ip_filter != current.ip_filter {
        ensure_mutable(immutable_keys, "engine_profile", "ip_filter")?;
    }
    if update.peer_classes != current.peer_classes {
        ensure_mutable(immutable_keys, "engine_profile", "peer_classes")?;
    }

    Ok(())
}

fn weekday_label(day: Weekday) -> &'static str {
    match day {
        Weekday::Mon => "mon",
        Weekday::Tue => "tue",
        Weekday::Wed => "wed",
        Weekday::Thu => "thu",
        Weekday::Fri => "fri",
        Weekday::Sat => "sat",
        Weekday::Sun => "sun",
    }
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
    rate_limit: Option<&Option<ApiKeyRateLimit>>,
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
    rate_limit: Option<&Option<ApiKeyRateLimit>>,
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
        match limit_value {
            Some(limit) => {
                validate_api_key_rate_limit(limit)?;
                let burst = i32::try_from(limit.burst).ok();
                let per_seconds = i64::try_from(limit.replenish_period.as_secs()).ok();
                data_config::update_api_key_rate_limit(tx.as_mut(), key_id, burst, per_seconds)
                    .await?;
            }
            None => {
                data_config::update_api_key_rate_limit(tx.as_mut(), key_id, None, None).await?;
            }
        }
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
    rate_limit: Option<&Option<ApiKeyRateLimit>>,
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
    if let Some(Some(limit)) = rate_limit {
        validate_api_key_rate_limit(limit)?;
    }
    data_config::insert_api_key(
        tx.as_mut(),
        key_id,
        &hash,
        label,
        enabled_flag,
        rate_limit
            .and_then(|limit| limit.as_ref())
            .map(|limit| i32::try_from(limit.burst).ok())
            .flatten(),
        rate_limit
            .and_then(|limit| limit.as_ref())
            .map(|limit| i64::try_from(limit.replenish_period.as_secs()).ok())
            .flatten(),
    )
    .await?;

    Ok(true)
}

async fn apply_secret_patches(
    tx: &mut Transaction<'_, Postgres>,
    patches: &[SecretPatch],
    actor: &str,
    immutable_keys: &HashSet<String>,
) -> Result<bool> {
    let mut changed = false;
    for patch in patches {
        match patch {
            SecretPatch::Set { name, value } => {
                ensure_mutable(immutable_keys, "settings_secret", name)?;
                data_config::upsert_secret(tx.as_mut(), name, value.as_bytes(), actor).await?;
                changed = true;
            }
            SecretPatch::Delete { name } => {
                ensure_mutable(immutable_keys, "settings_secret", name)?;
                let affected = data_config::delete_secret(tx.as_mut(), name).await?;
                if affected > 0 {
                    changed = true;
                }
            }
        }
    }
    Ok(changed)
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

#[cfg(test)]
mod tests {
    use super::*;
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
    fn validate_port_accepts_valid_range() {
        validate_port(8080, "app_profile", "http_port").expect("port should validate");
    }

    #[test]
    fn validate_port_rejects_out_of_range() {
        let err = validate_port(0, "app_profile", "http_port").unwrap_err();
        assert!(err.to_string().contains("between 1 and 65535"));
    }
}
