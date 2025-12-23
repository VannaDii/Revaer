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

//! HTTP API server and shared routing primitives for the Revaer platform.
//! Layout: bootstrap.rs (wiring), config/, domain/, app/, http/, infra/.

pub mod app;
pub mod config;
pub mod http;
pub mod models;
pub mod openapi;

pub use http::router::ApiServer;
pub use http::torrents::TorrentHandles;
pub use openapi::openapi_document;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::ApiState;
    use crate::config::{ConfigFacade, SharedConfig};
    use crate::http::auth::{AuthContext, map_config_error};
    use crate::http::constants::{
        HEADER_RATE_LIMIT_LIMIT, HEADER_RATE_LIMIT_REMAINING, HEADER_RATE_LIMIT_RESET,
        PROBLEM_BAD_REQUEST, PROBLEM_CONFIG_INVALID, PROBLEM_CONFLICT, PROBLEM_INTERNAL,
        PROBLEM_NOT_FOUND, PROBLEM_RATE_LIMITED, PROBLEM_SERVICE_UNAVAILABLE,
        PROBLEM_SETUP_REQUIRED, PROBLEM_UNAUTHORIZED,
    };
    use crate::http::errors::ApiError;
    use crate::http::health::{health, health_full, metrics};
    use crate::http::rate_limit::RateLimiter;
    use crate::http::settings::{get_config_snapshot, settings_patch};
    use crate::http::setup::{setup_complete, setup_start};
    use crate::http::sse::{
        SseFilter, dummy_payload, event_replay_stream, event_sse_stream, matches_sse_filter,
    };
    use crate::http::torrents::handlers::{
        action_torrent, create_torrent, delete_torrent, dispatch_torrent_add,
        dispatch_torrent_remove, fetch_all_torrents, fetch_torrent_status, get_torrent,
        list_torrents, select_torrent,
    };
    use crate::http::torrents::{
        StatusEntry, TorrentListQuery, TorrentMetadata, TorrentMetadataSeed, decode_cursor_token,
        detail_from_components, encode_cursor_from_entry, normalise_lower, parse_state_filter,
        split_comma_separated, summary_from_components,
    };
    use crate::models::{
        TorrentAction, TorrentCreateRequest, TorrentSelectionRequest, TorrentStateKind,
    };
    use crate::openapi::OpenApiDependencies;
    use anyhow::Result;
    use async_trait::async_trait;
    #[cfg(feature = "compat-qb")]
    use axum::extract::Form;
    use axum::http::header::RETRY_AFTER;
    #[cfg(feature = "compat-qb")]
    use axum::http::{HeaderMap, HeaderValue, header::COOKIE};
    use axum::{
        Extension, Json,
        body::Body,
        extract::{Path as AxumPath, Query, State},
        http::{Request, StatusCode},
        response::IntoResponse,
    };
    use chrono::{DateTime, Duration as ChronoDuration, Utc};
    use futures_util::{StreamExt, future, pin_mut};
    use revaer_config::{
        ApiKeyAuth, ApiKeyRateLimit, AppMode, AppProfile, AppliedChanges, ConfigError,
        ConfigSnapshot, EngineProfile, FsPolicy, SettingsChangeset, SetupToken,
        normalize_engine_profile,
    };
    use revaer_events::{Event as CoreEvent, EventBus, TorrentState};
    use revaer_telemetry::Metrics;
    use revaer_torrent_core::{
        AddTorrent, AddTorrentOptions, FileSelectionUpdate, PeerSnapshot, RemoveTorrent,
        TorrentInspector, TorrentProgress, TorrentRateLimit, TorrentRates, TorrentSource,
        TorrentStatus, TorrentWorkflow,
    };
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::net::IpAddr;
    use std::str::FromStr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};
    use tokio::sync::{Mutex, oneshot};
    use tokio::time::{sleep, timeout};
    use tower::ServiceExt;
    use uuid::Uuid;

    #[cfg(feature = "compat-qb")]
    use crate::http::compat_qb::{
        self, SyncParams, TorrentAddForm, TorrentHashesForm, TorrentsInfoParams, TransferLimitForm,
    };

    struct NoopWorkflow;

    #[async_trait]
    impl TorrentWorkflow for NoopWorkflow {
        async fn add_torrent(&self, _request: AddTorrent) -> anyhow::Result<()> {
            Ok(())
        }

        async fn remove_torrent(&self, _id: Uuid, _options: RemoveTorrent) -> anyhow::Result<()> {
            Ok(())
        }
    }

    struct NoopInspector;

    #[async_trait]
    impl TorrentInspector for NoopInspector {
        async fn list(&self) -> anyhow::Result<Vec<TorrentStatus>> {
            Ok(Vec::new())
        }

        async fn get(&self, _id: Uuid) -> anyhow::Result<Option<TorrentStatus>> {
            Ok(None)
        }

        async fn peers(&self, _id: Uuid) -> anyhow::Result<Vec<PeerSnapshot>> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn torrent_handles_exposes_inner_components() {
        let workflow: Arc<dyn TorrentWorkflow> = Arc::new(NoopWorkflow);
        let inspector: Arc<dyn TorrentInspector> = Arc::new(NoopInspector);

        let handles = TorrentHandles::new(Arc::clone(&workflow), Arc::clone(&inspector));

        handles
            .workflow()
            .add_torrent(AddTorrent {
                id: Uuid::new_v4(),
                source: TorrentSource::magnet("magnet:?xt=urn:btih:demo"),
                options: AddTorrentOptions::default(),
            })
            .await
            .expect("workflow should accept torrent");

        let listed = handles
            .inspector()
            .list()
            .await
            .expect("inspector list should succeed");
        assert!(listed.is_empty());
    }

    #[tokio::test]
    async fn enforce_rate_limit_without_limit_marks_guardrail() {
        let config = MockConfig::new();
        let telemetry = Metrics::new().expect("metrics");
        let state = ApiState::new(
            config.shared(),
            telemetry.clone(),
            Arc::new(Value::Null),
            EventBus::with_capacity(4),
            None,
        );

        state
            .enforce_rate_limit("key-1", None)
            .expect("missing limit should not error");
        assert!(
            state
                .current_health_degraded()
                .contains(&"api_rate_limit_guard".to_string())
        );
        assert_eq!(telemetry.snapshot().guardrail_violations_total, 1);
    }

    #[test]
    fn dummy_payload_covers_all_kinds() {
        let tid = Uuid::nil();
        let tid_other = Uuid::from_u128(1);
        let kinds = (0..10)
            .map(|tick| {
                dummy_payload(tick, tid, tid_other)["kind"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            kinds,
            vec![
                "system_rates",
                "torrent_added",
                "progress",
                "state_changed",
                "completed",
                "torrent_removed",
                "fsops_started",
                "fsops_progress",
                "metadata_updated",
                "fsops_failed"
            ]
        );
    }

    #[test]
    fn dummy_payload_fields_change_with_ticks() {
        let tid = Uuid::nil();
        let tid_other = Uuid::from_u128(1);

        let progress = dummy_payload(2, tid, tid_other);
        assert_eq!(progress["kind"], "progress");
        assert_eq!(
            progress["data"]["id"]
                .as_str()
                .map(|s| s.parse::<Uuid>().ok()),
            Some(Some(tid))
        );
        assert_eq!(progress["data"]["progress"]["bytes_downloaded"], 2 * 1_024);
        assert_eq!(
            progress["data"]["rates"]["download_bps"],
            50_000 + (2 * 2_000)
        );
        assert_eq!(progress["data"]["sequential"], true);

        let state = dummy_payload(3, tid, tid_other);
        assert_eq!(state["kind"], "state_changed");
        assert_eq!(state["data"]["state"], "seeding");

        let metadata = dummy_payload(8, tid, tid_other);
        assert_eq!(metadata["kind"], "metadata_updated");
        assert_eq!(metadata["data"]["download_dir"], "/downloads/relocated-8");

        let jobs = dummy_payload(9, tid, tid_other);
        assert_eq!(jobs["kind"], "fsops_failed");
        assert_eq!(
            jobs["data"]["torrent_id"]
                .as_str()
                .map(|s| s.parse::<Uuid>().ok()),
            Some(Some(tid))
        );
    }

    #[derive(Clone)]
    struct MockConfig {
        inner: Arc<tokio::sync::Mutex<MockConfigState>>,
        fail_snapshot: Arc<AtomicBool>,
    }

    struct MockConfigState {
        snapshot: ConfigSnapshot,
        tokens: HashMap<String, DateTime<Utc>>,
        api_keys: HashMap<String, MockApiKey>,
    }

    struct MockApiKey {
        secret: String,
        auth: ApiKeyAuth,
        enabled: bool,
    }

    impl MockConfig {
        fn new() -> Self {
            let engine_profile = EngineProfile {
                id: Uuid::new_v4(),
                implementation: "stub".to_string(),
                listen_port: Some(6881),
                listen_interfaces: Vec::new(),
                ipv6_mode: "disabled".to_string(),
                anonymous_mode: false.into(),
                force_proxy: false.into(),
                prefer_rc4: false.into(),
                allow_multiple_connections_per_ip: false.into(),
                enable_outgoing_utp: false.into(),
                enable_incoming_utp: false.into(),
                dht: true,
                encryption: "preferred".to_string(),
                max_active: Some(10),
                max_download_bps: None,
                max_upload_bps: None,
                seed_ratio_limit: None,
                seed_time_limit: None,
                connections_limit: None,
                connections_limit_per_torrent: None,
                unchoke_slots: None,
                half_open_limit: None,
                stats_interval_ms: None,
                alt_speed: Value::Null,
                sequential_default: false,
                auto_managed: true.into(),
                auto_manage_prefer_seeds: false.into(),
                dont_count_slow_torrents: true.into(),
                super_seeding: false.into(),
                choking_algorithm: EngineProfile::default_choking_algorithm(),
                seed_choking_algorithm: EngineProfile::default_seed_choking_algorithm(),
                strict_super_seeding: false.into(),
                optimistic_unchoke_slots: None,
                max_queued_disk_bytes: None,
                resume_dir: "/tmp/resume".to_string(),
                download_root: "/tmp/downloads".to_string(),
                storage_mode: EngineProfile::default_storage_mode(),
                use_partfile: EngineProfile::default_use_partfile(),
                disk_read_mode: None,
                disk_write_mode: None,
                verify_piece_hashes: EngineProfile::default_verify_piece_hashes(),
                cache_size: None,
                cache_expiry: None,
                coalesce_reads: EngineProfile::default_coalesce_reads(),
                coalesce_writes: EngineProfile::default_coalesce_writes(),
                use_disk_cache_pool: EngineProfile::default_use_disk_cache_pool(),
                tracker: Value::Null,
                enable_lsd: false.into(),
                enable_upnp: false.into(),
                enable_natpmp: false.into(),
                enable_pex: false.into(),
                dht_bootstrap_nodes: Vec::new(),
                dht_router_nodes: Vec::new(),
                ip_filter: Value::Null,
                peer_classes: Value::Null,
                outgoing_port_min: None,
                outgoing_port_max: None,
                peer_dscp: None,
            };
            let snapshot = ConfigSnapshot {
                revision: 1,
                app_profile: AppProfile {
                    id: Uuid::new_v4(),
                    instance_name: "revaer".to_string(),
                    mode: AppMode::Setup,
                    version: 1,
                    http_port: 7070,
                    bind_addr: IpAddr::from_str("127.0.0.1").expect("ip"),
                    telemetry: Value::Null,
                    features: json!({}),
                    immutable_keys: Value::Null,
                },
                engine_profile: engine_profile.clone(),
                engine_profile_effective: normalize_engine_profile(&engine_profile),
                fs_policy: FsPolicy {
                    id: Uuid::new_v4(),
                    library_root: "/tmp/library".to_string(),
                    extract: false,
                    par2: "disabled".to_string(),
                    flatten: false,
                    move_mode: "copy".to_string(),
                    cleanup_keep: Value::Null,
                    cleanup_drop: Value::Null,
                    chmod_file: None,
                    chmod_dir: None,
                    owner: None,
                    group: None,
                    umask: None,
                    allow_paths: Value::Array(vec![]),
                },
            };
            Self {
                inner: Arc::new(tokio::sync::Mutex::new(MockConfigState {
                    snapshot,
                    tokens: HashMap::new(),
                    api_keys: HashMap::new(),
                })),
                fail_snapshot: Arc::new(AtomicBool::new(false)),
            }
        }

        fn shared(&self) -> SharedConfig {
            Arc::new(self.clone()) as SharedConfig
        }

        async fn set_app_mode(&self, mode: AppMode) {
            let mut guard = self.inner.lock().await;
            guard.snapshot.app_profile.mode = mode;
        }

        async fn insert_api_key(&self, key_id: &str, secret: &str) {
            let mut guard = self.inner.lock().await;
            guard.api_keys.insert(
                key_id.to_string(),
                MockApiKey {
                    secret: secret.to_string(),
                    auth: ApiKeyAuth {
                        key_id: key_id.to_string(),
                        label: Some("test".to_string()),
                        rate_limit: None,
                    },
                    enabled: true,
                },
            );
        }

        fn set_fail_snapshot(&self, flag: bool) {
            self.fail_snapshot.store(flag, Ordering::SeqCst);
        }

        async fn snapshot(&self) -> ConfigSnapshot {
            self.inner.lock().await.snapshot.clone()
        }
    }

    #[async_trait]
    impl ConfigFacade for MockConfig {
        async fn get_app_profile(&self) -> Result<AppProfile> {
            Ok(self.inner.lock().await.snapshot.app_profile.clone())
        }

        async fn issue_setup_token(&self, ttl: Duration, _issued_by: &str) -> Result<SetupToken> {
            let mut guard = self.inner.lock().await;
            let token = format!("token-{}", guard.tokens.len() + 1);
            let expires_at = Utc::now() + ChronoDuration::from_std(ttl).expect("ttl");
            guard.tokens.insert(token.clone(), expires_at);
            drop(guard);
            Ok(SetupToken {
                plaintext: token,
                expires_at,
            })
        }

        async fn validate_setup_token(&self, token: &str) -> Result<()> {
            let expires = {
                let guard = self.inner.lock().await;
                guard
                    .tokens
                    .get(token)
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("unknown token"))?
            };
            if expires > Utc::now() {
                Ok(())
            } else {
                anyhow::bail!("expired")
            }
        }

        async fn consume_setup_token(&self, token: &str) -> Result<()> {
            {
                let mut guard = self.inner.lock().await;
                guard
                    .tokens
                    .remove(token)
                    .ok_or_else(|| anyhow::anyhow!("unknown token"))?;
            }
            Ok(())
        }

        async fn apply_changeset(
            &self,
            _actor: &str,
            _reason: &str,
            mut changeset: SettingsChangeset,
        ) -> Result<AppliedChanges> {
            let mut guard = self.inner.lock().await;
            let mut app_changed = false;
            let mut engine_changed = false;
            let mut fs_changed = false;

            if let Some(patch) = changeset.app_profile.take() {
                apply_app_profile_patch(&mut guard.snapshot.app_profile, &patch)?;
                app_changed = true;
            }

            if let Some(patch) = changeset.engine_profile.take() {
                apply_engine_patch(&mut guard.snapshot.engine_profile, &patch);
                engine_changed = true;
            }

            if let Some(patch) = changeset.fs_policy.take() {
                apply_fs_patch(&mut guard.snapshot.fs_policy, &patch);
                fs_changed = true;
            }

            for patch in changeset.api_keys {
                match patch {
                    revaer_config::ApiKeyPatch::Upsert {
                        key_id,
                        secret,
                        enabled,
                        label,
                        ..
                    } => {
                        let secret = secret.unwrap_or_else(|| "secret".to_string());
                        guard.api_keys.insert(
                            key_id.clone(),
                            MockApiKey {
                                secret,
                                auth: ApiKeyAuth {
                                    key_id,
                                    label,
                                    rate_limit: None,
                                },
                                enabled: enabled.unwrap_or(true),
                            },
                        );
                    }
                    revaer_config::ApiKeyPatch::Delete { key_id } => {
                        guard.api_keys.remove(&key_id);
                    }
                }
            }

            guard.snapshot.revision += 1;
            Ok(AppliedChanges {
                revision: guard.snapshot.revision,
                app_profile: app_changed.then(|| guard.snapshot.app_profile.clone()),
                engine_profile: engine_changed.then(|| guard.snapshot.engine_profile.clone()),
                fs_policy: fs_changed.then(|| guard.snapshot.fs_policy.clone()),
            })
        }

        async fn snapshot(&self) -> Result<ConfigSnapshot> {
            if self.fail_snapshot.load(Ordering::SeqCst) {
                anyhow::bail!("snapshot unavailable")
            }
            Ok(self.inner.lock().await.snapshot.clone())
        }

        async fn authenticate_api_key(
            &self,
            key_id: &str,
            secret: &str,
        ) -> Result<Option<ApiKeyAuth>> {
            let auth = {
                let guard = self.inner.lock().await;
                guard.api_keys.get(key_id).and_then(|entry| {
                    (entry.enabled && entry.secret == secret).then(|| entry.auth.clone())
                })
            };
            Ok(auth)
        }
    }

    fn apply_app_profile_patch(profile: &mut AppProfile, patch: &Value) -> Result<()> {
        if let Some(name) = patch.get("instance_name").and_then(Value::as_str) {
            profile.instance_name = name.to_string();
        }
        if let Some(bind) = patch.get("bind_addr").and_then(Value::as_str) {
            profile.bind_addr = IpAddr::from_str(bind)?;
        }
        if let Some(port) = patch.get("http_port").and_then(Value::as_i64) {
            profile.http_port = i32::try_from(port)
                .map_err(|_| anyhow::anyhow!("http_port {port} exceeds i32 range"))?;
        }
        if let Some(mode) = patch.get("mode").and_then(Value::as_str) {
            profile.mode = mode.parse()?;
        }
        Ok(())
    }

    fn apply_engine_patch(profile: &mut EngineProfile, patch: &Value) {
        if let Some(impl_name) = patch.get("implementation").and_then(Value::as_str) {
            profile.implementation = impl_name.to_string();
        }
        if let Some(resume) = patch.get("resume_dir").and_then(Value::as_str) {
            profile.resume_dir = resume.to_string();
        }
        if let Some(download) = patch.get("download_root").and_then(Value::as_str) {
            profile.download_root = download.to_string();
        }
        if let Some(nodes) = patch.get("dht_bootstrap_nodes").and_then(Value::as_array) {
            profile.dht_bootstrap_nodes = nodes
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
        }
        if let Some(nodes) = patch.get("dht_router_nodes").and_then(Value::as_array) {
            profile.dht_router_nodes = nodes
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
        }
    }

    fn apply_fs_patch(policy: &mut FsPolicy, patch: &Value) {
        if let Some(root) = patch.get("library_root").and_then(Value::as_str) {
            policy.library_root = root.to_string();
        }
        if let Some(allow_paths) = patch.get("allow_paths") {
            policy.allow_paths = allow_paths.clone();
        }
    }

    #[derive(Default)]
    struct StubTorrent {
        statuses: Mutex<Vec<TorrentStatus>>,
        added: Mutex<Vec<AddTorrent>>,
        removed: Mutex<Vec<Uuid>>,
        selections: Mutex<Vec<(Uuid, FileSelectionUpdate)>>,
        actions: Mutex<Vec<(Uuid, String)>>,
    }

    impl StubTorrent {
        async fn push_status(&self, status: TorrentStatus) {
            self.statuses.lock().await.push(status);
        }

        async fn added(&self) -> Vec<AddTorrent> {
            self.added.lock().await.clone()
        }

        async fn selections(&self) -> Vec<(Uuid, FileSelectionUpdate)> {
            self.selections.lock().await.clone()
        }

        async fn actions(&self) -> Vec<(Uuid, String)> {
            self.actions.lock().await.clone()
        }
    }

    #[async_trait]
    impl TorrentWorkflow for StubTorrent {
        async fn add_torrent(&self, request: AddTorrent) -> anyhow::Result<()> {
            self.added.lock().await.push(request.clone());
            let status = TorrentStatus {
                id: request.id,
                name: request.options.name_hint.clone(),
                progress: TorrentProgress {
                    bytes_total: 100,
                    bytes_downloaded: 0,
                    ..TorrentProgress::default()
                },
                last_updated: Utc::now(),
                ..TorrentStatus::default()
            };
            self.statuses.lock().await.push(status);
            Ok(())
        }

        async fn remove_torrent(&self, id: Uuid, _options: RemoveTorrent) -> anyhow::Result<()> {
            self.removed.lock().await.push(id);
            self.statuses.lock().await.retain(|status| status.id != id);
            self.actions.lock().await.push((id, "remove".to_string()));
            Ok(())
        }

        async fn pause_torrent(&self, id: Uuid) -> anyhow::Result<()> {
            self.actions.lock().await.push((id, "pause".to_string()));
            Ok(())
        }

        async fn resume_torrent(&self, id: Uuid) -> anyhow::Result<()> {
            self.actions.lock().await.push((id, "resume".to_string()));
            Ok(())
        }

        async fn update_selection(
            &self,
            id: Uuid,
            rules: FileSelectionUpdate,
        ) -> anyhow::Result<()> {
            self.selections.lock().await.push((id, rules));
            Ok(())
        }

        async fn set_sequential(&self, id: Uuid, enable: bool) -> anyhow::Result<()> {
            self.actions
                .lock()
                .await
                .push((id, format!("sequential:{enable}")));
            Ok(())
        }

        async fn update_limits(
            &self,
            id: Option<Uuid>,
            limits: TorrentRateLimit,
        ) -> anyhow::Result<()> {
            if let Some(id) = id {
                self.actions.lock().await.push((id, "rate".to_string()));
            }
            let _ = limits;
            Ok(())
        }

        async fn reannounce(&self, id: Uuid) -> anyhow::Result<()> {
            self.actions
                .lock()
                .await
                .push((id, "reannounce".to_string()));
            Ok(())
        }

        async fn recheck(&self, id: Uuid) -> anyhow::Result<()> {
            self.actions.lock().await.push((id, "recheck".to_string()));
            Ok(())
        }
    }

    #[async_trait]
    impl TorrentInspector for StubTorrent {
        async fn list(&self) -> anyhow::Result<Vec<TorrentStatus>> {
            Ok(self.statuses.lock().await.clone())
        }

        async fn get(&self, id: Uuid) -> anyhow::Result<Option<TorrentStatus>> {
            Ok(self
                .statuses
                .lock()
                .await
                .iter()
                .find(|status| status.id == id)
                .cloned())
        }

        async fn peers(&self, _id: Uuid) -> anyhow::Result<Vec<PeerSnapshot>> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn setup_flow_promotes_active_mode() -> Result<()> {
        let config = MockConfig::new();
        let events = EventBus::with_capacity(32);
        let mut event_stream = events.subscribe(None);
        let metrics = Metrics::new()?;
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(openapi_document()),
            events.clone(),
            None,
        ));

        let Json(start) = setup_start(State(state.clone()), None)
            .await
            .expect("setup start");
        assert!(!start.token.is_empty());

        let changeset = SettingsChangeset {
            app_profile: Some(json!({
                "instance_name": "demo",
                "bind_addr": "127.0.0.1",
                "http_port": 8080,
                "mode": "active"
            })),
            engine_profile: Some(json!({
                "implementation": "libtorrent",
                "resume_dir": "/var/lib/revaer/resume",
                "download_root": "/var/lib/revaer/downloads"
            })),
            fs_policy: Some(json!({
                "library_root": "/data/library",
                "allow_paths": ["/data"]
            })),
            api_keys: vec![revaer_config::ApiKeyPatch::Upsert {
                key_id: "bootstrap".to_string(),
                label: Some("bootstrap".to_string()),
                enabled: Some(true),
                secret: Some("secret".to_string()),
                rate_limit: None,
            }],
            secrets: vec![],
        };

        let Json(snapshot) = setup_complete(
            State(state.clone()),
            Extension(AuthContext::SetupToken(start.token.clone())),
            Json(changeset),
        )
        .await
        .expect("setup complete");

        assert_eq!(snapshot.app_profile.mode, AppMode::Active);
        let event = timeout(Duration::from_secs(1), event_stream.next())
            .await
            .expect("settings event")
            .expect("event value")
            .expect("stream recv error");
        assert!(matches!(event.event, CoreEvent::SettingsChanged { .. }));
        Ok(())
    }

    #[tokio::test]
    async fn settings_patch_updates_snapshot() -> Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        config.insert_api_key("admin", "secret").await;

        let events = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(openapi_document()),
            events,
            None,
        ));

        let changeset = SettingsChangeset {
            app_profile: Some(json!({"instance_name": "patched"})),
            engine_profile: None,
            fs_policy: None,
            api_keys: Vec::new(),
            secrets: Vec::new(),
        };

        let Json(response) = settings_patch(
            State(state.clone()),
            Extension(AuthContext::ApiKey {
                key_id: "admin".to_string(),
            }),
            Json(changeset),
        )
        .await
        .expect("settings patch");

        assert_eq!(response.app_profile.instance_name, "patched");
        let snapshot = config.snapshot().await;
        assert_eq!(snapshot.app_profile.instance_name, "patched");
        Ok(())
    }

    #[tokio::test]
    async fn config_snapshot_endpoint_returns_snapshot() -> Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        let events = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(openapi_document()),
            events,
            None,
        ));

        let Json(snapshot) = get_config_snapshot(State(state.clone()))
            .await
            .expect("config snapshot");
        assert_eq!(snapshot.revision, config.snapshot().await.revision);
        Ok(())
    }

    #[tokio::test]
    async fn torrent_endpoints_execute_workflow() -> Result<()> {
        let harness = TorrentTestHarness::new().await?;
        harness.assert_create().await?;
        harness.assert_list_contains_existing().await?;
        harness.assert_detail_fetch().await?;
        harness.assert_selection_update().await?;
        harness.assert_actions().await?;
        harness.assert_delete().await?;
        Ok(())
    }

    fn map_api_error(err: &ApiError) -> anyhow::Error {
        anyhow::Error::msg(format!("{err:?}"))
    }

    struct TorrentTestHarness {
        state: Arc<ApiState>,
        stub: Arc<StubTorrent>,
        request: TorrentCreateRequest,
        existing_id: Uuid,
        api_key_id: String,
    }

    impl TorrentTestHarness {
        async fn new() -> Result<Self> {
            let config = MockConfig::new();
            config.set_app_mode(AppMode::Active).await;
            let api_key_id = "operator".to_string();
            config.insert_api_key(&api_key_id, "secret").await;

            let events = EventBus::with_capacity(32);
            let metrics = Metrics::new()?;
            let stub = Arc::new(StubTorrent::default());
            let workflow: Arc<dyn TorrentWorkflow> = stub.clone();
            let inspector: Arc<dyn TorrentInspector> = stub.clone();
            let handles = TorrentHandles::new(workflow, inspector);
            let state = Arc::new(ApiState::new(
                config.shared(),
                metrics,
                Arc::new(openapi_document()),
                events,
                Some(handles),
            ));

            let existing_id = Uuid::new_v4();
            let status = TorrentStatus {
                id: existing_id,
                name: Some("existing".to_string()),
                progress: TorrentProgress {
                    bytes_total: 100,
                    bytes_downloaded: 100,
                    ..TorrentProgress::default()
                },
                state: TorrentState::Completed,
                library_path: Some("/library/existing".to_string()),
                ..TorrentStatus::default()
            };
            stub.push_status(status).await;

            let request = TorrentCreateRequest {
                id: Uuid::new_v4(),
                magnet: Some("magnet:?xt=urn:btih:test".to_string()),
                name: Some("example".to_string()),
                ..TorrentCreateRequest::default()
            };

            Ok(Self {
                state,
                stub,
                request,
                existing_id,
                api_key_id,
            })
        }

        async fn assert_create(&self) -> Result<()> {
            create_torrent(self.state(), self.auth(), Json(self.request.clone()))
                .await
                .map_err(|err| map_api_error(&err))?;
            assert_eq!(self.stub.added().await.len(), 1);
            Ok(())
        }

        async fn assert_list_contains_existing(&self) -> Result<()> {
            let Json(list) = list_torrents(self.state(), Query(TorrentListQuery::default()))
                .await
                .map_err(|err| map_api_error(&err))?;
            assert!(list.torrents.iter().any(|item| item.id == self.existing_id));
            Ok(())
        }

        async fn assert_detail_fetch(&self) -> Result<()> {
            let Json(detail) = get_torrent(self.state(), AxumPath(self.existing_id))
                .await
                .map_err(|err| map_api_error(&err))?;
            assert_eq!(detail.summary.id, self.existing_id);
            Ok(())
        }

        async fn assert_selection_update(&self) -> Result<()> {
            let selection = TorrentSelectionRequest {
                include: vec!["*.mkv".to_string()],
                exclude: vec![],
                skip_fluff: Some(true),
                priorities: Vec::new(),
            };
            select_torrent(
                self.state(),
                self.auth(),
                AxumPath(self.existing_id),
                Json(selection),
            )
            .await
            .map_err(|err| map_api_error(&err))?;
            assert_eq!(self.stub.selections().await.len(), 1);
            Ok(())
        }

        async fn assert_actions(&self) -> Result<()> {
            action_torrent(
                self.state(),
                self.auth(),
                AxumPath(self.existing_id),
                Json(TorrentAction::Sequential { enable: true }),
            )
            .await
            .map_err(|err| map_api_error(&err))?;

            action_torrent(
                self.state(),
                self.auth(),
                AxumPath(self.existing_id),
                Json(TorrentAction::Remove { delete_data: true }),
            )
            .await
            .map_err(|err| map_api_error(&err))?;

            assert!(
                self.stub
                    .actions()
                    .await
                    .iter()
                    .any(|(_, action)| action == "remove")
            );
            Ok(())
        }

        async fn assert_delete(&self) -> Result<()> {
            delete_torrent(self.state(), self.auth(), AxumPath(self.request.id))
                .await
                .map_err(|err| map_api_error(&err))?;
            Ok(())
        }

        fn state(&self) -> State<Arc<ApiState>> {
            State(self.state.clone())
        }

        fn auth(&self) -> Extension<AuthContext> {
            Extension(AuthContext::ApiKey {
                key_id: self.api_key_id.clone(),
            })
        }
    }

    #[tokio::test]
    async fn health_endpoints_reflect_state() -> Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        let events = EventBus::with_capacity(8);
        let telemetry = Metrics::new()?;
        let state = Arc::new(ApiState::new(
            config.shared(),
            telemetry,
            Arc::new(openapi_document()),
            events.clone(),
            None,
        ));

        let Json(health_response) = health(State(state.clone())).await.expect("health");
        assert_eq!(health_response.status, "ok");

        let Json(full) = health_full(State(state.clone()))
            .await
            .expect("health full");
        assert_eq!(full.status, "ok");

        let response = metrics(State(state.clone())).await.expect("metrics");
        assert_eq!(response.status(), StatusCode::OK);

        config.set_fail_snapshot(true);
        let degraded = health(State(state.clone())).await;
        assert!(degraded.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn sse_stream_resumes_from_last_event() {
        let bus = EventBus::with_capacity(32);
        let torrent_id_1 = Uuid::new_v4();
        let torrent_id_2 = Uuid::new_v4();

        let first_id = bus.publish(CoreEvent::Completed {
            torrent_id: torrent_id_1,
            library_path: "/library/a".to_string(),
        });
        let second_id = bus.publish(CoreEvent::Completed {
            torrent_id: torrent_id_2,
            library_path: "/library/b".to_string(),
        });

        let stream = event_replay_stream(bus.clone(), Some(first_id));
        pin_mut!(stream);
        let envelope = stream.next().await.expect("expected event");

        assert_eq!(envelope.id, second_id);
        match envelope.event {
            CoreEvent::Completed { torrent_id, .. } => assert_eq!(torrent_id, torrent_id_2),
            other => panic!("unexpected event {other:?}"),
        }
    }

    #[test]
    fn rate_limiter_blocks_after_burst_exhausted() {
        let limit = ApiKeyRateLimit {
            burst: 2,
            replenish_period: Duration::from_secs(60),
        };
        let mut limiter = RateLimiter::new(limit.clone());
        let start = Instant::now();
        assert!(limiter.evaluate(&limit, start).allowed);
        assert!(limiter.evaluate(&limit, start).allowed);
        assert!(
            !limiter
                .evaluate(&limit, start + Duration::from_secs(1))
                .allowed
        );
    }

    #[test]
    fn rate_limiter_refills_after_period() {
        let limit = ApiKeyRateLimit {
            burst: 1,
            replenish_period: Duration::from_secs(1),
        };
        let mut limiter = RateLimiter::new(limit.clone());
        let start = Instant::now();
        assert!(limiter.evaluate(&limit, start).allowed);
        assert!(
            !limiter
                .evaluate(&limit, start + Duration::from_millis(100))
                .allowed
        );
        let later = start + Duration::from_secs(2);
        assert!(limiter.evaluate(&limit, later).allowed);
    }

    #[test]
    fn rate_limiter_handles_zero_capacity_and_period() {
        let zero_limit = ApiKeyRateLimit {
            burst: 0,
            replenish_period: Duration::from_secs(1),
        };
        let mut zero_cap = RateLimiter::new(zero_limit.clone());
        let now = Instant::now();
        let denied = zero_cap.evaluate(&zero_limit, now);
        assert!(!denied.allowed);
        assert_eq!(denied.remaining, 0);
        assert_eq!(denied.retry_after, Duration::MAX);

        let zero_config = ApiKeyRateLimit {
            burst: 1,
            replenish_period: Duration::ZERO,
        };
        let mut zero_period = RateLimiter::new(zero_config.clone());
        zero_period.tokens = 0;
        zero_period.last_refill = now;
        let blocked = zero_period.evaluate(&zero_config, now);
        assert!(!blocked.allowed);
        assert_eq!(blocked.retry_after, Duration::ZERO);
    }

    #[test]
    fn api_error_builders_cover_all_variants() {
        let cases = vec![
            (
                ApiError::internal("oops"),
                StatusCode::INTERNAL_SERVER_ERROR,
                PROBLEM_INTERNAL,
                false,
            ),
            (
                ApiError::unauthorized("no auth"),
                StatusCode::UNAUTHORIZED,
                PROBLEM_UNAUTHORIZED,
                false,
            ),
            (
                ApiError::bad_request("bad"),
                StatusCode::BAD_REQUEST,
                PROBLEM_BAD_REQUEST,
                false,
            ),
            (
                ApiError::not_found("gone"),
                StatusCode::NOT_FOUND,
                PROBLEM_NOT_FOUND,
                false,
            ),
            (
                ApiError::conflict("clash"),
                StatusCode::CONFLICT,
                PROBLEM_CONFLICT,
                false,
            ),
            (
                ApiError::setup_required("setup"),
                StatusCode::CONFLICT,
                PROBLEM_SETUP_REQUIRED,
                false,
            ),
            (
                ApiError::config_invalid("invalid"),
                StatusCode::UNPROCESSABLE_ENTITY,
                PROBLEM_CONFIG_INVALID,
                false,
            ),
            (
                ApiError::service_unavailable("down"),
                StatusCode::SERVICE_UNAVAILABLE,
                PROBLEM_SERVICE_UNAVAILABLE,
                false,
            ),
            (
                ApiError::too_many_requests("slow").with_rate_limit_headers(
                    10,
                    3,
                    Some(Duration::from_millis(500)),
                ),
                StatusCode::TOO_MANY_REQUESTS,
                PROBLEM_RATE_LIMITED,
                true,
            ),
        ];

        for (error, status, kind, with_rate) in cases {
            assert_eq!(error.status, status);
            assert_eq!(error.kind, kind);
            assert_eq!(error.rate_limit.is_some(), with_rate);
            let response = error.into_response();
            assert_eq!(response.status(), status);
        }
    }

    #[test]
    fn api_error_includes_rate_limit_headers() {
        let response = ApiError::too_many_requests("throttled")
            .with_rate_limit_headers(5, 0, Some(Duration::from_millis(250)))
            .into_response();
        let headers = response.headers();
        assert_eq!(headers.get(HEADER_RATE_LIMIT_LIMIT).unwrap(), "5");
        assert_eq!(headers.get(HEADER_RATE_LIMIT_REMAINING).unwrap(), "0");
        assert_eq!(headers.get(HEADER_RATE_LIMIT_RESET).unwrap(), "1");
        assert_eq!(headers.get(RETRY_AFTER).unwrap(), "1");
    }

    #[cfg(feature = "compat-qb")]
    #[tokio::test]
    async fn api_state_tracks_health_and_sessions() {
        let config: SharedConfig = Arc::new(MockConfig::new());
        let telemetry = Metrics::new().expect("metrics available");
        let state = ApiServer::build_state(
            config,
            telemetry,
            Arc::new(json!({ "openapi": "stub" })),
            EventBus::with_capacity(8),
            None,
        );

        assert!(state.add_degraded_component("storage"));
        assert!(!state.add_degraded_component("storage"));
        assert!(state.remove_degraded_component("storage"));
        assert!(!state.remove_degraded_component("storage"));

        let session = state.issue_qb_session();
        assert!(state.validate_qb_session(&session));
        state.revoke_qb_session(&session);
        assert!(!state.validate_qb_session(&session));

        state.update_torrent_metrics().await;
    }

    #[tokio::test]
    async fn api_server_builds_router_with_mock_config() {
        let config: SharedConfig = Arc::new(MockConfig::new());
        let telemetry = Metrics::new().expect("metrics available");
        let events = EventBus::with_capacity(8);
        let openapi_path = std::env::temp_dir().join("revaer-openapi-test.json");
        let persisted = Arc::new(AtomicBool::new(false));
        let document = Arc::new(json!({ "openapi": "stub" }));
        let openapi = {
            let path_clone = openapi_path.clone();
            let flag = Arc::clone(&persisted);
            let doc_clone = Arc::clone(&document);
            OpenApiDependencies::new(
                document,
                openapi_path,
                Arc::new(move |path, payload| {
                    assert_eq!(path, &path_clone);
                    assert_eq!(payload, doc_clone.as_ref());
                    flag.store(true, Ordering::SeqCst);
                    Ok(())
                }),
            )
        };
        let server = ApiServer::with_config_at(config, events, None, telemetry, &openapi)
            .expect("router should build");

        let response = server
            .router()
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            persisted.load(Ordering::SeqCst),
            "OpenAPI persistence should be invoked"
        );
    }

    #[tokio::test]
    async fn sse_stream_emits_event_for_torrent_added() {
        let bus = EventBus::with_capacity(16);
        let publisher = bus.clone();
        let torrent_id = Uuid::new_v4();
        tokio::spawn(async move {
            sleep(Duration::from_millis(10)).await;
            let _ = publisher.publish(CoreEvent::TorrentAdded {
                torrent_id,
                name: "example".to_string(),
            });
        });
        let stream = event_sse_stream(bus.clone(), None, SseFilter::default());
        pin_mut!(stream);
        match timeout(Duration::from_millis(200), stream.next())
            .await
            .expect("timed out waiting for SSE event")
        {
            Some(Ok(_)) => {}
            other => panic!("expected SSE event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn sse_filter_by_torrent_id() {
        let bus = EventBus::with_capacity(16);
        let target = Uuid::new_v4();
        let other = Uuid::new_v4();
        let publisher = bus.clone();

        tokio::spawn(async move {
            let _ = publisher.publish(CoreEvent::TorrentAdded {
                torrent_id: other,
                name: "other".to_string(),
            });
            let _ = publisher.publish(CoreEvent::TorrentAdded {
                torrent_id: target,
                name: "matching".to_string(),
            });
        });

        let mut filter = SseFilter::default();
        filter.torrent_ids.insert(target);

        let stream = event_replay_stream(bus, None).filter(move |envelope| {
            let filter = filter.clone();
            future::ready(matches_sse_filter(envelope, &filter))
        });
        pin_mut!(stream);
        let envelope = timeout(Duration::from_millis(200), stream.next())
            .await
            .expect("timed out waiting for filtered event")
            .expect("stream terminated");
        match envelope.event {
            CoreEvent::TorrentAdded { torrent_id, .. } => assert_eq!(torrent_id, target),
            other => panic!("unexpected event {other:?}"),
        }
    }

    #[test]
    fn map_config_error_exposes_pointer_for_immutable_field() {
        let err = ConfigError::ImmutableField {
            section: "app_profile".to_string(),
            field: "instance_name".to_string(),
        };
        let api_error = map_config_error(err.into(), "failed");
        assert_eq!(api_error.status, StatusCode::UNPROCESSABLE_ENTITY);
        let params = api_error
            .invalid_params
            .expect("immutable field should set invalid params");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].pointer, "/app_profile/instance_name");
        assert!(
            params[0].message.contains("immutable"),
            "message should mention immutability"
        );
    }

    #[test]
    fn map_config_error_handles_root_pointer() {
        let err = ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "<root>".to_string(),
            message: "changeset must be a JSON object".to_string(),
        };
        let api_error = map_config_error(err.into(), "failed");
        let params = api_error
            .invalid_params
            .expect("invalid field should set invalid params");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].pointer, "/engine_profile");
        assert!(
            params[0].message.contains("must be a JSON object"),
            "message should echo validation failure"
        );
    }

    #[test]
    fn torrent_status_response_formats_state() {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let status = TorrentStatus {
            id,
            name: Some("ubuntu.iso".to_string()),
            state: TorrentState::Failed {
                message: "disk quota exceeded".to_string(),
            },
            progress: TorrentProgress {
                bytes_downloaded: 512,
                bytes_total: 1024,
                eta_seconds: Some(90),
            },
            rates: TorrentRates {
                download_bps: 2_048,
                upload_bps: 512,
                ratio: 0.5,
            },
            files: None,
            library_path: None,
            download_dir: None,
            comment: None,
            source: None,
            private: None,
            sequential: false,
            added_at: now,
            completed_at: None,
            last_updated: now,
        };

        let detail = detail_from_components(status, TorrentMetadata::default());
        assert_eq!(detail.summary.id, id);
        assert_eq!(detail.summary.state.kind, TorrentStateKind::Failed);
        assert_eq!(
            detail.summary.state.failure_message.as_deref(),
            Some("disk quota exceeded")
        );
        assert_eq!(detail.summary.progress.bytes_downloaded, 512);
        assert_eq!(detail.summary.progress.bytes_total, 1024);
        assert!((detail.summary.progress.percent_complete - 50.0).abs() < f64::EPSILON);
        assert_eq!(detail.summary.progress.eta_seconds, Some(90));
        assert_eq!(detail.summary.rates.download_bps, 2_048);
        assert_eq!(detail.summary.rates.upload_bps, 512);
        assert!((detail.summary.rates.ratio - 0.5).abs() < f64::EPSILON);
        assert_eq!(detail.summary.added_at, now);
        assert!(detail.summary.completed_at.is_none());
    }

    #[tokio::test]
    async fn sse_stream_waits_for_new_events_after_reconnect() {
        let bus = EventBus::with_capacity(32);
        let torrent_id = Uuid::new_v4();
        let last_id = bus.publish(CoreEvent::Completed {
            torrent_id,
            library_path: "/library/a".to_string(),
        });

        let stream = event_replay_stream(bus.clone(), Some(last_id));
        pin_mut!(stream);

        let (tx, rx) = oneshot::channel();
        let publisher = bus.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            let next = publisher.publish(CoreEvent::Completed {
                torrent_id: Uuid::new_v4(),
                library_path: "/library/b".to_string(),
            });
            let _ = tx.send(next);
        });

        let envelope = stream.next().await.expect("expected event");
        let next_id = rx.await.expect("publish id");
        assert_eq!(envelope.id, next_id);
    }

    #[derive(Default)]
    struct StubInspector {
        statuses: Mutex<Vec<TorrentStatus>>,
    }

    impl StubInspector {
        fn with_statuses(statuses: Vec<TorrentStatus>) -> Self {
            Self {
                statuses: Mutex::new(statuses),
            }
        }
    }

    #[async_trait]
    impl TorrentInspector for StubInspector {
        async fn list(&self) -> anyhow::Result<Vec<TorrentStatus>> {
            let snapshot = self.statuses.lock().await.clone();
            Ok(snapshot)
        }

        async fn get(&self, id: Uuid) -> anyhow::Result<Option<TorrentStatus>> {
            let snapshot = self.statuses.lock().await.clone();
            Ok(snapshot.into_iter().find(|status| status.id == id))
        }

        async fn peers(&self, _id: Uuid) -> anyhow::Result<Vec<PeerSnapshot>> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn fetch_all_torrents_returns_statuses() {
        let workflow = Arc::new(RecordingWorkflow::default());
        let workflow_trait: Arc<dyn TorrentWorkflow> = workflow.clone();
        let now = Utc::now();
        let sample_status = TorrentStatus {
            id: Uuid::new_v4(),
            name: Some("ubuntu.iso".to_string()),
            state: TorrentState::Downloading,
            progress: TorrentProgress {
                bytes_downloaded: 512,
                bytes_total: 1_024,
                eta_seconds: Some(120),
            },
            rates: TorrentRates {
                download_bps: 4_096,
                upload_bps: 1_024,
                ratio: 0.5,
            },
            files: None,
            library_path: None,
            download_dir: Some("/downloads".to_string()),
            comment: None,
            source: None,
            private: None,
            sequential: true,
            added_at: now,
            completed_at: None,
            last_updated: now,
        };
        let inspector = Arc::new(StubInspector::with_statuses(vec![sample_status.clone()]));
        let inspector_trait: Arc<dyn TorrentInspector> = inspector.clone();
        let handles = TorrentHandles::new(workflow_trait, inspector_trait);

        let statuses = fetch_all_torrents(&handles)
            .await
            .expect("torrent statuses");
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].state, TorrentState::Downloading);
        assert_eq!(statuses[0].name.as_deref(), Some("ubuntu.iso"));
    }

    #[tokio::test]
    async fn fetch_torrent_status_respects_not_found() {
        let workflow = Arc::new(RecordingWorkflow::default());
        let inspector = Arc::new(StubInspector::default());
        let handles = TorrentHandles::new(
            workflow.clone() as Arc<dyn TorrentWorkflow>,
            inspector.clone() as Arc<dyn TorrentInspector>,
        );
        let result = fetch_torrent_status(&handles, Uuid::new_v4()).await;
        match result {
            Err(err) => assert_eq!(err.status, StatusCode::NOT_FOUND),
            Ok(_) => panic!("expected torrent lookup to fail"),
        }
    }

    #[derive(Default)]
    struct RecordingWorkflow {
        added: Mutex<Vec<AddTorrent>>,
        removed: Mutex<Vec<(Uuid, RemoveTorrent)>>,
        should_fail_add: bool,
        should_fail_remove: bool,
    }

    #[async_trait]
    impl TorrentWorkflow for RecordingWorkflow {
        async fn add_torrent(&self, request: AddTorrent) -> anyhow::Result<()> {
            if self.should_fail_add {
                anyhow::bail!("injected failure");
            }
            self.added.lock().await.push(request);
            Ok(())
        }

        async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> anyhow::Result<()> {
            if self.should_fail_remove {
                anyhow::bail!("remove failure");
            }
            self.removed.lock().await.push((id, options));
            Ok(())
        }

        async fn set_piece_deadline(
            &self,
            _: Uuid,
            _: revaer_torrent_core::model::PieceDeadline,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl TorrentInspector for RecordingWorkflow {
        async fn list(&self) -> anyhow::Result<Vec<TorrentStatus>> {
            Ok(Vec::new())
        }

        async fn get(&self, _id: Uuid) -> anyhow::Result<Option<TorrentStatus>> {
            Ok(None)
        }

        async fn peers(&self, _id: Uuid) -> anyhow::Result<Vec<PeerSnapshot>> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn create_torrent_requires_workflow() {
        let request = TorrentCreateRequest {
            id: Uuid::new_v4(),
            magnet: Some("magnet:?xt=urn:btih:example".to_string()),
            name: Some("example".to_string()),
            ..TorrentCreateRequest::default()
        };

        let config = MockConfig::new();
        let state = ApiState::new(
            config.shared(),
            Metrics::new().expect("metrics"),
            Arc::new(json!({})),
            EventBus::with_capacity(4),
            None,
        );
        let err = dispatch_torrent_add(&state, &request, Vec::new(), Vec::new())
            .await
            .expect_err("expected workflow to be unavailable");
        assert_eq!(err.status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn create_torrent_invokes_workflow() -> anyhow::Result<()> {
        let workflow = Arc::new(RecordingWorkflow::default());
        let request = TorrentCreateRequest {
            id: Uuid::new_v4(),
            magnet: Some("magnet:?xt=urn:btih:ubuntu".to_string()),
            name: Some("ubuntu.iso".to_string()),
            sequential: Some(true),
            include: vec!["*/include.mkv".to_string()],
            skip_fluff: true,
            max_download_bps: Some(1_000_000),
            ..TorrentCreateRequest::default()
        };

        let workflow_trait: Arc<dyn TorrentWorkflow> = workflow.clone();
        let inspector_trait: Arc<dyn TorrentInspector> = workflow.clone();
        let handles = TorrentHandles::new(workflow_trait, inspector_trait);

        let config = MockConfig::new();
        let state = ApiState::new(
            config.shared(),
            Metrics::new().expect("metrics"),
            Arc::new(json!({})),
            EventBus::with_capacity(4),
            Some(handles),
        );
        dispatch_torrent_add(&state, &request, Vec::new(), Vec::new())
            .await
            .expect("torrent creation should succeed");
        let recorded_entry = {
            let recorded = workflow.added.lock().await;
            assert_eq!(recorded.len(), 1);
            recorded[0].clone()
        };
        assert_eq!(recorded_entry.id, request.id);
        match &recorded_entry.source {
            TorrentSource::Magnet { uri } => {
                assert!(uri.contains("ubuntu"));
            }
            TorrentSource::Metainfo { .. } => panic!("expected magnet source"),
        }
        assert_eq!(
            recorded_entry.options.name_hint.as_deref(),
            request.name.as_deref()
        );
        assert_eq!(recorded_entry.options.sequential, Some(true));
        assert_eq!(recorded_entry.options.file_rules.include, request.include);
        assert!(recorded_entry.options.file_rules.skip_fluff);
        assert_eq!(
            recorded_entry.options.rate_limit.download_bps,
            request.max_download_bps
        );
        Ok(())
    }

    #[test]
    fn summary_includes_metadata() {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let status = TorrentStatus {
            id,
            name: Some("demo".to_string()),
            state: TorrentState::Completed,
            progress: TorrentProgress {
                bytes_downloaded: 42,
                bytes_total: 42,
                eta_seconds: None,
            },
            rates: TorrentRates::default(),
            files: None,
            library_path: Some("/library/demo".to_string()),
            download_dir: None,
            comment: None,
            source: None,
            private: None,
            sequential: false,
            added_at: now,
            completed_at: Some(now),
            last_updated: now,
        };
        let metadata = TorrentMetadata::new(TorrentMetadataSeed {
            tags: vec!["tagA".to_string(), "tagB".to_string()],
            category: None,
            trackers: vec!["http://tracker".to_string()],
            web_seeds: Vec::new(),
            rate_limit: Some(revaer_torrent_core::TorrentRateLimit {
                download_bps: Some(1_000),
                upload_bps: None,
            }),
            connections_limit: None,
            selection: revaer_torrent_core::FileSelectionUpdate::default(),
            download_dir: status.download_dir.clone(),
            cleanup: None,
        });
        let summary = summary_from_components(status, metadata);
        assert_eq!(summary.tags, vec!["tagA".to_string(), "tagB".to_string()]);
        assert_eq!(summary.trackers, vec!["http://tracker".to_string()]);
        assert_eq!(
            summary.rate_limit.and_then(|limit| limit.download_bps),
            Some(1_000)
        );
    }

    #[tokio::test]
    async fn delete_torrent_requires_workflow() {
        let id = Uuid::new_v4();
        let err = dispatch_torrent_remove(None, id)
            .await
            .expect_err("expected workflow to be unavailable");
        assert_eq!(err.status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn delete_torrent_invokes_workflow() -> anyhow::Result<()> {
        let workflow = Arc::new(RecordingWorkflow::default());
        let id = Uuid::new_v4();

        let workflow_trait: Arc<dyn TorrentWorkflow> = workflow.clone();
        let inspector_trait: Arc<dyn TorrentInspector> = workflow.clone();
        let handles = TorrentHandles::new(workflow_trait, inspector_trait);

        dispatch_torrent_remove(Some(&handles), id)
            .await
            .expect("torrent removal should succeed");

        {
            let recorded = workflow.removed.lock().await;
            assert_eq!(recorded.len(), 1);
            assert_eq!(recorded[0].0, id);
            drop(recorded);
        }
        Ok(())
    }

    #[test]
    fn decode_cursor_token_rejects_invalid_base64() {
        let err = decode_cursor_token("%%%");
        assert!(err.is_err(), "invalid cursor token should error");
        let api_err = err.expect_err("expected error");
        assert_eq!(api_err.status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn cursor_token_round_trip_preserves_identity() {
        let status = TorrentStatus {
            id: Uuid::new_v4(),
            last_updated: Utc::now(),
            ..TorrentStatus::default()
        };
        let entry = StatusEntry {
            status: status.clone(),
            metadata: TorrentMetadata::new(TorrentMetadataSeed {
                tags: vec![],
                category: None,
                trackers: vec![],
                web_seeds: Vec::new(),
                rate_limit: None,
                connections_limit: None,
                selection: revaer_torrent_core::FileSelectionUpdate::default(),
                download_dir: status.download_dir.clone(),
                cleanup: None,
            }),
        };

        let encoded = encode_cursor_from_entry(&entry).expect("cursor encoding should succeed");
        let decoded = decode_cursor_token(&encoded).expect("cursor decoding should succeed");
        assert_eq!(decoded.id, status.id);
        assert_eq!(decoded.last_updated, status.last_updated);
    }

    #[test]
    fn parse_state_filter_rejects_unknown_value() {
        let err =
            parse_state_filter("mystery").expect_err("unexpected success for unknown state filter");
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn comma_splitter_trims_and_lowercases() {
        let values = split_comma_separated(" Alpha , ,BETA ,gamma ");
        assert_eq!(values, vec!["alpha", "beta", "gamma"]);
    }

    #[cfg(feature = "compat-qb")]
    const QB_TEST_MAGNET: &str = "magnet:?xt=urn:btih:revaerqb";

    #[cfg(feature = "compat-qb")]
    #[tokio::test]
    async fn qb_sync_maindata_maps_status() -> anyhow::Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        let events = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let stub = Arc::new(StubTorrent::default());
        let sample_status = TorrentStatus {
            id: Uuid::new_v4(),
            name: Some("sample".to_string()),
            state: TorrentState::Downloading,
            progress: TorrentProgress {
                bytes_downloaded: 256,
                bytes_total: 512,
                eta_seconds: Some(30),
            },
            rates: TorrentRates {
                download_bps: 1_024,
                upload_bps: 256,
                ratio: 0.5,
            },
            download_dir: Some("/downloads".to_string()),
            sequential: false,
            ..TorrentStatus::default()
        };
        stub.push_status(sample_status.clone()).await;
        let handles = TorrentHandles::new(stub.clone(), stub.clone());
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(openapi_document()),
            events,
            Some(handles),
        ));

        let headers = qb_session_headers(&state);
        let Json(response) =
            compat_qb::sync_maindata(State(state.clone()), headers, Query(SyncParams::default()))
                .await
                .expect("sync");

        assert!(response.full_update);
        assert!(
            response
                .torrents
                .contains_key(&sample_status.id.simple().to_string())
        );
        assert!(response.torrents_removed.is_empty());
        Ok(())
    }

    #[cfg(feature = "compat-qb")]
    #[tokio::test]
    async fn qb_sync_maindata_returns_incremental_changes() -> anyhow::Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        let events = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let stub = Arc::new(StubTorrent::default());
        let sample_id = Uuid::new_v4();
        let sample_status = TorrentStatus {
            id: sample_id,
            name: Some("sample".to_string()),
            state: TorrentState::Downloading,
            progress: TorrentProgress {
                bytes_downloaded: 256,
                bytes_total: 512,
                eta_seconds: Some(30),
            },
            rates: TorrentRates {
                download_bps: 1_024,
                upload_bps: 256,
                ratio: 0.5,
            },
            download_dir: Some("/downloads".to_string()),
            sequential: false,
            ..TorrentStatus::default()
        };
        stub.push_status(sample_status.clone()).await;
        let handles = TorrentHandles::new(stub.clone(), stub.clone());
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(openapi_document()),
            events.clone(),
            Some(handles),
        ));

        let _ = events.publish(CoreEvent::TorrentAdded {
            torrent_id: sample_id,
            name: "sample".to_string(),
        });

        let headers = qb_session_headers(&state);
        let Json(initial) =
            compat_qb::sync_maindata(State(state.clone()), headers, Query(SyncParams::default()))
                .await
                .expect("initial sync");
        let previous_rid = initial.rid;
        assert!(initial.full_update);

        {
            let mut statuses = stub.statuses.lock().await;
            if let Some(status) = statuses.get_mut(0) {
                status.progress.bytes_downloaded = 400;
                status.progress.eta_seconds = Some(10);
            }
        }

        let _ = events.publish(CoreEvent::Progress {
            torrent_id: sample_id,
            bytes_downloaded: 400,
            bytes_total: 512,
        });

        let headers = qb_session_headers(&state);
        let Json(delta) = compat_qb::sync_maindata(
            State(state.clone()),
            headers,
            Query(SyncParams {
                rid: Some(previous_rid),
            }),
        )
        .await
        .expect("incremental sync");

        assert!(!delta.full_update);
        assert_eq!(delta.torrents.len(), 1);
        assert!(delta.torrents.contains_key(&sample_id.simple().to_string()));
        assert!(delta.torrents_removed.is_empty());
        assert!(delta.rid > previous_rid);

        stub.statuses.lock().await.clear();
        let latest_rid = delta.rid;
        let _ = events.publish(CoreEvent::TorrentRemoved {
            torrent_id: sample_id,
        });

        let headers = qb_session_headers(&state);
        let Json(removed_delta) = compat_qb::sync_maindata(
            State(state),
            headers,
            Query(SyncParams {
                rid: Some(latest_rid),
            }),
        )
        .await
        .expect("removal sync");

        assert!(!removed_delta.full_update);
        assert!(removed_delta.torrents.is_empty());
        assert_eq!(
            removed_delta.torrents_removed,
            vec![sample_id.simple().to_string()]
        );
        assert!(removed_delta.rid > latest_rid);
        Ok(())
    }

    #[cfg(feature = "compat-qb")]
    #[tokio::test]
    async fn qb_torrents_add_records_submission() -> anyhow::Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        let events = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let stub = Arc::new(StubTorrent::default());
        let handles = TorrentHandles::new(stub.clone(), stub.clone());
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(openapi_document()),
            events,
            Some(handles),
        ));

        let form = TorrentAddForm {
            urls: Some(QB_TEST_MAGNET.to_string()),
            tags: Some("alpha,beta".to_string()),
            ..TorrentAddForm::default()
        };

        let headers = qb_session_headers(&state);
        compat_qb::torrents_add(State(state), headers, Form(form))
            .await
            .expect("add torrent");

        assert_eq!(stub.added().await.len(), 1);
        Ok(())
    }

    #[cfg(feature = "compat-qb")]
    #[test]
    fn qb_parse_limit_handles_unlimited() {
        assert_eq!(compat_qb::parse_limit("0").unwrap(), None);
        assert_eq!(compat_qb::parse_limit("-1").unwrap(), None);
        assert_eq!(compat_qb::parse_limit("1024").unwrap(), Some(1_024));
    }

    #[cfg(feature = "compat-qb")]
    #[tokio::test]
    async fn qb_torrents_info_filters_hashes() -> anyhow::Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        let events = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let stub = Arc::new(StubTorrent::default());
        let sample_status = TorrentStatus {
            id: Uuid::new_v4(),
            name: Some("sample".to_string()),
            state: TorrentState::Seeding,
            progress: TorrentProgress {
                bytes_downloaded: 1_024,
                bytes_total: 1_024,
                eta_seconds: Some(0),
            },
            rates: TorrentRates {
                download_bps: 0,
                upload_bps: 512,
                ratio: 1.0,
            },
            download_dir: Some("/downloads/sample".to_string()),
            sequential: false,
            ..TorrentStatus::default()
        };
        stub.push_status(sample_status.clone()).await;
        let handles = TorrentHandles::new(stub.clone(), stub.clone());
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(openapi_document()),
            events,
            Some(handles),
        ));

        let params = TorrentsInfoParams {
            hashes: Some(sample_status.id.simple().to_string()),
        };

        let headers = qb_session_headers(&state);
        let Json(entries) = compat_qb::torrents_info(State(state), headers, Query(params))
            .await
            .expect("torrents info");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].hash, sample_status.id.simple().to_string());
        Ok(())
    }

    #[cfg(feature = "compat-qb")]
    #[tokio::test]
    async fn qb_torrent_pause_resume_apply_actions() -> anyhow::Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        let events = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let stub = Arc::new(StubTorrent::default());
        let sample_status = TorrentStatus {
            id: Uuid::new_v4(),
            name: Some("pause-me".to_string()),
            state: TorrentState::Downloading,
            download_dir: Some("/downloads".to_string()),
            sequential: false,
            ..TorrentStatus::default()
        };
        stub.push_status(sample_status.clone()).await;
        let handles = TorrentHandles::new(stub.clone(), stub.clone());
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(openapi_document()),
            events,
            Some(handles),
        ));

        let hashes = sample_status.id.simple().to_string();
        let form = TorrentHashesForm { hashes };

        let headers = qb_session_headers(&state);
        compat_qb::torrents_pause(State(state.clone()), headers.clone(), Form(form.clone()))
            .await
            .expect("pause");
        compat_qb::torrents_resume(State(state), headers, Form(form))
            .await
            .expect("resume");

        let actions = stub.actions().await;
        assert!(actions.iter().any(|(_, action)| action == "pause"));
        assert!(actions.iter().any(|(_, action)| action == "resume"));
        Ok(())
    }

    #[cfg(feature = "compat-qb")]
    #[tokio::test]
    async fn qb_transfer_limits_accept_positive_values() -> anyhow::Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        let events = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let stub = Arc::new(StubTorrent::default());
        let handles = TorrentHandles::new(stub.clone(), stub.clone());
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(openapi_document()),
            events,
            Some(handles),
        ));

        let form = TransferLimitForm {
            limit: "2048".to_string(),
        };
        let headers = qb_session_headers(&state);
        compat_qb::transfer_upload_limit(State(state.clone()), headers.clone(), Form(form.clone()))
            .await
            .expect("upload limit");
        compat_qb::transfer_download_limit(State(state), headers, Form(form))
            .await
            .expect("download limit");
        Ok(())
    }

    #[cfg(feature = "compat-qb")]
    fn qb_session_headers(state: &Arc<ApiState>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        let sid = state.issue_qb_session();
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&format!("SID={sid}")).expect("valid cookie header"),
        );
        headers
    }

    #[test]
    fn normalise_lower_trims_whitespace() {
        assert_eq!(normalise_lower("  HeLLo "), "hello");
    }

    #[test]
    fn detail_from_components_embeds_metadata() {
        let now = Utc::now();
        let status = TorrentStatus {
            id: Uuid::new_v4(),
            name: Some("demo".to_string()),
            state: TorrentState::Completed,
            progress: TorrentProgress {
                bytes_downloaded: 100,
                bytes_total: 100,
                eta_seconds: None,
            },
            rates: TorrentRates::default(),
            files: None,
            library_path: Some("/library/demo".to_string()),
            download_dir: None,
            comment: None,
            source: None,
            private: None,
            sequential: false,
            added_at: now,
            completed_at: Some(now),
            last_updated: now,
        };
        let metadata = TorrentMetadata::new(TorrentMetadataSeed {
            tags: vec!["tag".to_string()],
            category: None,
            trackers: vec!["http://tracker".to_string()],
            web_seeds: Vec::new(),
            rate_limit: Some(revaer_torrent_core::TorrentRateLimit {
                download_bps: Some(10),
                upload_bps: None,
            }),
            connections_limit: None,
            selection: revaer_torrent_core::FileSelectionUpdate::default(),
            download_dir: status.download_dir.clone(),
            cleanup: None,
        });

        let detail = detail_from_components(status, metadata);
        assert_eq!(detail.summary.tags, vec!["tag".to_string()]);
        assert_eq!(detail.summary.trackers, vec!["http://tracker".to_string()]);
        assert_eq!(
            detail
                .summary
                .rate_limit
                .and_then(|limit| limit.download_bps),
            Some(10)
        );
        assert_eq!(
            detail.settings.as_ref().expect("settings present").trackers,
            vec!["http://tracker".to_string()]
        );
    }
}
