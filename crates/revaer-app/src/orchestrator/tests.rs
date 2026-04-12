use super::*;

#[cfg(test)]
mod orchestrator_tests {
    use super::*;
    use revaer_config::ConfigService;
    use revaer_config::engine_profile::{
        AltSpeedConfig, IpFilterConfig, PeerClassesConfig, TrackerConfig,
    };
    use revaer_test_support::postgres::start_postgres;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;
    use tokio::time::{Duration, timeout};
    use tokio_stream::StreamExt;

    type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

    fn repo_root() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for ancestor in manifest_dir.ancestors() {
            if ancestor.join("AGENT.md").is_file() {
                return ancestor.to_path_buf();
            }
        }
        manifest_dir
    }

    fn server_root() -> TestResult<PathBuf> {
        let root = repo_root().join(".server_root");
        fs::create_dir_all(&root)?;
        Ok(root)
    }

    fn temp_dir() -> TestResult<TempDir> {
        Ok(tempfile::Builder::new()
            .prefix("revaer-app-")
            .tempdir_in(server_root()?)?)
    }

    #[derive(Default)]
    struct StubEngine;

    #[async_trait]
    impl TorrentEngine for StubEngine {
        async fn add_torrent(&self, _request: AddTorrent) -> TorrentResult<()> {
            Ok(())
        }

        async fn remove_torrent(&self, _id: Uuid, _options: RemoveTorrent) -> TorrentResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl EngineConfigurator for StubEngine {
        async fn apply_engine_plan(&self, _plan: &EngineRuntimePlan) -> TorrentResult<()> {
            Ok(())
        }
    }

    fn sample_fs_policy(root: &Path) -> FsPolicy {
        FsPolicy {
            id: Uuid::new_v4(),
            library_root: root.join("library").display().to_string(),
            extract: false,
            par2: "disabled".to_string(),
            flatten: false,
            move_mode: "copy".to_string(),
            cleanup_keep: Vec::new(),
            cleanup_drop: Vec::new(),
            chmod_file: None,
            chmod_dir: None,
            owner: None,
            group: None,
            umask: None,
            allow_paths: vec![root.display().to_string()],
        }
    }

    fn sample_engine_profile() -> EngineProfile {
        EngineProfile {
            id: Uuid::new_v4(),
            implementation: "libtorrent".to_string(),
            listen_port: Some(6_881),
            listen_interfaces: Vec::new(),
            ipv6_mode: "disabled".to_string(),
            anonymous_mode: false.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            dht: true,
            encryption: "prefer".to_string(),
            max_active: Some(4),
            max_download_bps: None,
            max_upload_bps: None,
            seed_ratio_limit: None,
            seed_time_limit: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            stats_interval_ms: None,
            alt_speed: AltSpeedConfig::default(),
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
            resume_dir: ".server_root/resume".to_string(),
            download_root: ".server_root/downloads".to_string(),
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
            tracker: TrackerConfig::default(),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            ip_filter: IpFilterConfig::default(),
            peer_classes: PeerClassesConfig::default(),
            outgoing_port_min: None,
            outgoing_port_max: None,
            peer_dscp: None,
        }
    }

    #[tokio::test]
    async fn completed_event_triggers_fsops_pipeline() -> TestResult<()> {
        let temp = temp_dir()?;
        let policy = sample_fs_policy(temp.path());
        let events = EventBus::with_capacity(16);
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(events.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::new(StubEngine),
            fsops,
            events.clone(),
            policy.clone(),
            sample_engine_profile(),
            None,
            None,
        ));

        let torrent_id = Uuid::new_v4();
        let source_path = temp.path().join("staging").join("title");
        fs::create_dir_all(&source_path)?;
        fs::write(source_path.join("movie.mkv"), b"video-bytes")?;
        let mut stream = events.subscribe(None);

        orchestrator
            .handle_event(&Event::Completed {
                torrent_id,
                library_path: source_path.to_string_lossy().into_owned(),
            })
            .await?;

        timeout(Duration::from_secs(5), async {
            while let Some(result) = stream.next().await {
                let envelope = result.map_err(|err| AppError::InvalidConfig {
                    field: "event_stream",
                    reason: "recv_error",
                    value: Some(err.to_string()),
                })?;
                match envelope.event {
                    Event::FsopsCompleted { torrent_id: id } if id == torrent_id => {
                        return Ok::<(), Box<dyn std::error::Error + Send + Sync>>(());
                    }
                    Event::FsopsFailed {
                        torrent_id: id,
                        ref message,
                    } if id == torrent_id => {
                        return Err(AppError::InvalidConfig {
                            field: "fsops",
                            reason: "unexpected_failure",
                            value: Some(message.clone()),
                        }
                        .into());
                    }
                    _ => {}
                }
            }
            Err(AppError::InvalidConfig {
                field: "fsops",
                reason: "stream_closed",
                value: None,
            }
            .into())
        })
        .await??;

        let meta_path = PathBuf::from(&policy.library_root)
            .join(".revaer")
            .join(format!("{torrent_id}.meta.json"));
        assert!(
            meta_path.exists(),
            "fsops metadata should be written after completion"
        );
        Ok(())
    }

    #[tokio::test]
    async fn spawn_post_processing_consumes_completed_events_from_bus() -> TestResult<()> {
        let temp = temp_dir()?;
        let policy = sample_fs_policy(temp.path());
        let events = EventBus::with_capacity(16);
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(events.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::new(StubEngine),
            fsops,
            events.clone(),
            policy.clone(),
            sample_engine_profile(),
            None,
            None,
        ));

        let torrent_id = Uuid::new_v4();
        let source_path = temp.path().join("staging").join("queued-title");
        fs::create_dir_all(&source_path)?;
        fs::write(source_path.join("movie.mkv"), b"video-bytes")?;
        let mut stream = events.subscribe(None);
        let worker = orchestrator.spawn_post_processing();
        tokio::task::yield_now().await;

        events.publish(Event::Completed {
            torrent_id,
            library_path: source_path.to_string_lossy().into_owned(),
        })?;

        timeout(Duration::from_secs(5), async {
            while let Some(result) = stream.next().await {
                let envelope = result.map_err(|err| AppError::InvalidConfig {
                    field: "event_stream",
                    reason: "recv_error",
                    value: Some(err.to_string()),
                })?;
                if matches!(
                    envelope.event,
                    Event::FsopsCompleted { torrent_id: id } if id == torrent_id
                ) {
                    return Ok::<(), Box<dyn std::error::Error + Send + Sync>>(());
                }
            }
            Err(AppError::InvalidConfig {
                field: "fsops",
                reason: "stream_closed",
                value: None,
            }
            .into())
        })
        .await??;

        worker.abort();
        let meta_path = PathBuf::from(&policy.library_root)
            .join(".revaer")
            .join(format!("{torrent_id}.meta.json"));
        assert!(meta_path.exists(), "spawned worker should apply fsops");
        Ok(())
    }

    #[tokio::test]
    async fn spawn_post_processing_survives_fsops_failures() -> TestResult<()> {
        let temp = temp_dir()?;
        let events = EventBus::with_capacity(16);
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(events.clone(), metrics);
        let mut policy = sample_fs_policy(temp.path());
        policy.library_root = "   ".to_string();
        policy.allow_paths = Vec::new();
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::new(StubEngine),
            fsops,
            events.clone(),
            policy,
            sample_engine_profile(),
            None,
            None,
        ));

        let torrent_id = Uuid::new_v4();
        let source_path = temp.path().join("staging").join("broken-title");
        fs::create_dir_all(&source_path)?;
        fs::write(source_path.join("movie.mkv"), b"video-bytes")?;
        let mut stream = events.subscribe(None);
        let worker = orchestrator.spawn_post_processing();
        tokio::task::yield_now().await;

        events.publish(Event::Completed {
            torrent_id,
            library_path: source_path.to_string_lossy().into_owned(),
        })?;

        timeout(Duration::from_secs(5), async {
            while let Some(result) = stream.next().await {
                let envelope = result.map_err(|err| AppError::InvalidConfig {
                    field: "event_stream",
                    reason: "recv_error",
                    value: Some(err.to_string()),
                })?;
                if matches!(
                    envelope.event,
                    Event::FsopsFailed { torrent_id: id, .. } if id == torrent_id
                ) {
                    return Ok::<(), Box<dyn std::error::Error + Send + Sync>>(());
                }
            }
            Err(AppError::InvalidConfig {
                field: "fsops",
                reason: "stream_closed",
                value: None,
            }
            .into())
        })
        .await??;

        assert!(
            !worker.is_finished(),
            "worker should keep running after a single fsops failure"
        );
        worker.abort();
        Ok(())
    }

    #[tokio::test]
    async fn completed_event_with_invalid_policy_emits_failure() -> TestResult<()> {
        let temp = temp_dir()?;
        let events = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(events.clone(), metrics);
        let mut policy = sample_fs_policy(temp.path());
        policy.library_root = "   ".to_string();
        policy.allow_paths = Vec::new();
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::new(StubEngine),
            fsops,
            events.clone(),
            policy,
            sample_engine_profile(),
            None,
            None,
        ));

        let torrent_id = Uuid::new_v4();
        let staged = temp.path().join("staging").join("title");
        fs::create_dir_all(&staged)?;
        fs::write(staged.join("movie.mkv"), b"video")?;
        let mut stream = events.subscribe(None);

        let result = orchestrator
            .handle_event(&Event::Completed {
                torrent_id,
                library_path: staged.to_string_lossy().into_owned(),
            })
            .await;
        assert!(result.is_err(), "invalid policy should surface an error");

        timeout(Duration::from_secs(3), async {
            while let Some(event) = stream.next().await {
                let envelope = event.map_err(|err| AppError::InvalidConfig {
                    field: "event_stream",
                    reason: "recv_error",
                    value: Some(err.to_string()),
                })?;
                match envelope.event {
                    Event::FsopsFailed { torrent_id: id, .. } if id == torrent_id => {
                        return Ok::<(), Box<dyn std::error::Error + Send + Sync>>(());
                    }
                    Event::FsopsCompleted { torrent_id: id } if id == torrent_id => {
                        return Err(AppError::InvalidConfig {
                            field: "fsops",
                            reason: "unexpected_success",
                            value: None,
                        }
                        .into());
                    }
                    _ => {}
                }
            }
            Err(AppError::InvalidConfig {
                field: "fsops",
                reason: "stream_closed",
                value: None,
            }
            .into())
        })
        .await??;
        Ok(())
    }

    #[tokio::test]
    async fn apply_fsops_requires_catalog_state() -> TestResult<()> {
        let temp = temp_dir()?;
        let events = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(events, metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::new(StubEngine),
            fsops,
            EventBus::new(),
            sample_fs_policy(temp.path()),
            sample_engine_profile(),
            None,
            None,
        ));

        let err = orchestrator
            .apply_fsops(Uuid::new_v4())
            .await
            .expect_err("missing catalog state should fail");
        assert!(matches!(
            err,
            AppError::MissingState {
                field: "torrent_status",
                ..
            }
        ));
        Ok(())
    }

    #[tokio::test]
    async fn apply_fsops_requires_library_path() -> TestResult<()> {
        let temp = temp_dir()?;
        let events = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(events.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::new(StubEngine),
            fsops,
            events,
            sample_fs_policy(temp.path()),
            sample_engine_profile(),
            None,
            None,
        ));
        let torrent_id = Uuid::new_v4();
        orchestrator
            .catalog
            .observe(&Event::TorrentAdded {
                torrent_id,
                name: "demo".to_string(),
            })
            .await;

        let err = orchestrator
            .apply_fsops(torrent_id)
            .await
            .expect_err("missing library path should fail");
        assert!(matches!(
            err,
            AppError::MissingState {
                field: "library_path",
                ..
            }
        ));
        Ok(())
    }

    #[tokio::test]
    async fn create_torrent_rejects_paths_outside_allow_list() -> TestResult<()> {
        let temp = temp_dir()?;
        let policy = sample_fs_policy(temp.path());
        let events = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(events.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::new(StubEngine),
            fsops,
            events,
            policy,
            sample_engine_profile(),
            None,
            None,
        ));

        let request = TorrentAuthorRequest {
            root_path: std::env::temp_dir().join("outside").display().to_string(),
            ..TorrentAuthorRequest::default()
        };
        let err = orchestrator
            .create_torrent(request)
            .await
            .expect_err("disallowed root should fail");
        assert!(matches!(
            err,
            TorrentError::OperationFailed {
                operation: "create_torrent.validate_path",
                ..
            }
        ));
        Ok(())
    }

    #[tokio::test]
    async fn create_torrent_surfaces_engine_errors_after_allow_path_check() -> TestResult<()> {
        let temp = temp_dir()?;
        let policy = sample_fs_policy(temp.path());
        let events = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(events.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::new(StubEngine),
            fsops,
            events,
            policy,
            sample_engine_profile(),
            None,
            None,
        ));

        let request = TorrentAuthorRequest {
            root_path: temp.path().join("inside").display().to_string(),
            ..TorrentAuthorRequest::default()
        };
        let err = orchestrator
            .create_torrent(request)
            .await
            .expect_err("stub engine should report unsupported");
        assert!(matches!(
            err,
            TorrentError::Unsupported {
                operation: "create_torrent"
            }
        ));
        Ok(())
    }

    #[tokio::test]
    async fn handle_event_persists_runtime_status_updates() -> TestResult<()> {
        let postgres = match start_postgres() {
            Ok(database) => database,
            Err(err) => {
                eprintln!("skipping handle_event_persists_runtime_status_updates: {err}");
                return Ok(());
            }
        };
        let temp = temp_dir()?;
        let config = ConfigService::new(postgres.connection_string().to_string()).await?;
        let runtime = RuntimeStore::new(config.pool().clone()).await?;
        let events = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(events.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::new(StubEngine),
            fsops,
            events,
            sample_fs_policy(temp.path()),
            sample_engine_profile(),
            Some(runtime.clone()),
            None,
        ));
        let torrent_id = Uuid::new_v4();

        orchestrator
            .handle_event(&Event::TorrentAdded {
                torrent_id,
                name: "runtime-demo".to_string(),
            })
            .await?;

        let statuses = runtime.load_statuses().await?;
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].id, torrent_id);
        assert_eq!(statuses[0].name.as_deref(), Some("runtime-demo"));
        assert_eq!(statuses[0].state, TorrentState::Queued);
        Ok(())
    }

    #[tokio::test]
    async fn handle_event_removes_runtime_status_after_torrent_removed() -> TestResult<()> {
        let postgres = match start_postgres() {
            Ok(database) => database,
            Err(err) => {
                eprintln!(
                    "skipping handle_event_removes_runtime_status_after_torrent_removed: {err}"
                );
                return Ok(());
            }
        };
        let temp = temp_dir()?;
        let config = ConfigService::new(postgres.connection_string().to_string()).await?;
        let runtime = RuntimeStore::new(config.pool().clone()).await?;
        let events = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(events.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::new(StubEngine),
            fsops,
            events,
            sample_fs_policy(temp.path()),
            sample_engine_profile(),
            Some(runtime.clone()),
            None,
        ));
        let torrent_id = Uuid::new_v4();

        orchestrator
            .handle_event(&Event::TorrentAdded {
                torrent_id,
                name: "runtime-demo".to_string(),
            })
            .await?;
        orchestrator
            .handle_event(&Event::TorrentRemoved { torrent_id })
            .await?;

        let statuses = runtime.load_statuses().await?;
        assert!(
            statuses.iter().all(|status| status.id != torrent_id),
            "removed torrents should not remain in the runtime store"
        );
        Ok(())
    }
}

#[cfg(test)]
mod engine_refresh_tests {
    use super::*;
    use revaer_config::{
        AppProfile, SetupToken,
        engine_profile::{
            AltSpeedConfig, IpFilterConfig, PeerClassesConfig, TrackerAuthConfig, TrackerConfig,
            TrackerProxyConfig, TrackerProxyType,
        },
    };
    use revaer_events::EventBus;
    use revaer_torrent_core::{
        AddTorrent, FileSelectionUpdate, PeerChoke, PeerInterest, PeerSnapshot, RemoveTorrent,
        TorrentRateLimit, TorrentWorkflow,
        model::{
            TorrentAuthorRequest, TorrentAuthorResult, TorrentOptionsUpdate, TorrentTrackersUpdate,
            TorrentWebSeedsUpdate,
        },
    };
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::{Mutex, RwLock};

    type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

    fn repo_root() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for ancestor in manifest_dir.ancestors() {
            if ancestor.join("AGENT.md").is_file() {
                return ancestor.to_path_buf();
            }
        }
        manifest_dir
    }

    #[derive(Default)]
    struct RecordingEngine {
        added: RwLock<Vec<AddTorrent>>,
        authored: RwLock<Vec<TorrentAuthorRequest>>,
        applied: RwLock<Vec<EngineRuntimePlan>>,
        removed: RwLock<Vec<(Uuid, RemoveTorrent)>>,
        paused: RwLock<Vec<Uuid>>,
        resumed: RwLock<Vec<Uuid>>,
        sequential: RwLock<Vec<(Uuid, bool)>>,
        limits: RwLock<Vec<(Option<Uuid>, TorrentRateLimit)>>,
        selections: RwLock<Vec<(Uuid, FileSelectionUpdate)>>,
        options: RwLock<Vec<(Uuid, TorrentOptionsUpdate)>>,
        trackers: RwLock<Vec<(Uuid, TorrentTrackersUpdate)>>,
        web_seeds: RwLock<Vec<(Uuid, TorrentWebSeedsUpdate)>>,
        moves: RwLock<Vec<(Uuid, String)>>,
        reannounced: RwLock<Vec<Uuid>>,
        rechecked: RwLock<Vec<Uuid>>,
        peers: RwLock<HashMap<Uuid, Vec<PeerSnapshot>>>,
        fail_apply_plan: AtomicBool,
        fail_update_limits: AtomicBool,
    }

    #[async_trait]
    impl TorrentEngine for RecordingEngine {
        async fn add_torrent(&self, request: AddTorrent) -> TorrentResult<()> {
            self.added.write().await.push(request);
            Ok(())
        }

        async fn create_torrent(
            &self,
            request: TorrentAuthorRequest,
        ) -> TorrentResult<TorrentAuthorResult> {
            self.authored.write().await.push(request);
            Ok(TorrentAuthorResult {
                magnet_uri: "magnet:?xt=urn:btih:demo".to_string(),
                info_hash: "demo".to_string(),
                piece_length: 16_384,
                total_size: 1,
                ..TorrentAuthorResult::default()
            })
        }

        async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> TorrentResult<()> {
            self.removed.write().await.push((id, options));
            Ok(())
        }

        async fn pause_torrent(&self, id: Uuid) -> TorrentResult<()> {
            self.paused.write().await.push(id);
            Ok(())
        }

        async fn resume_torrent(&self, id: Uuid) -> TorrentResult<()> {
            self.resumed.write().await.push(id);
            Ok(())
        }

        async fn set_sequential(&self, id: Uuid, sequential: bool) -> TorrentResult<()> {
            self.sequential.write().await.push((id, sequential));
            Ok(())
        }

        async fn update_limits(
            &self,
            id: Option<Uuid>,
            limits: TorrentRateLimit,
        ) -> TorrentResult<()> {
            if self.fail_update_limits.load(AtomicOrdering::Relaxed) {
                return Err(TorrentError::Unsupported {
                    operation: "update_limits",
                });
            }
            self.limits.write().await.push((id, limits));
            Ok(())
        }

        async fn update_selection(
            &self,
            id: Uuid,
            rules: FileSelectionUpdate,
        ) -> TorrentResult<()> {
            self.selections.write().await.push((id, rules));
            Ok(())
        }

        async fn update_options(
            &self,
            id: Uuid,
            options: TorrentOptionsUpdate,
        ) -> TorrentResult<()> {
            self.options.write().await.push((id, options));
            Ok(())
        }

        async fn update_trackers(
            &self,
            id: Uuid,
            trackers: TorrentTrackersUpdate,
        ) -> TorrentResult<()> {
            self.trackers.write().await.push((id, trackers));
            Ok(())
        }

        async fn update_web_seeds(
            &self,
            id: Uuid,
            web_seeds: TorrentWebSeedsUpdate,
        ) -> TorrentResult<()> {
            self.web_seeds.write().await.push((id, web_seeds));
            Ok(())
        }

        async fn reannounce(&self, id: Uuid) -> TorrentResult<()> {
            self.reannounced.write().await.push(id);
            Ok(())
        }

        async fn move_torrent(&self, id: Uuid, download_dir: String) -> TorrentResult<()> {
            self.moves.write().await.push((id, download_dir));
            Ok(())
        }

        async fn recheck(&self, id: Uuid) -> TorrentResult<()> {
            self.rechecked.write().await.push(id);
            Ok(())
        }

        async fn peers(&self, id: Uuid) -> TorrentResult<Vec<PeerSnapshot>> {
            Ok(self
                .peers
                .read()
                .await
                .get(&id)
                .cloned()
                .unwrap_or_default())
        }
    }

    #[async_trait]
    impl EngineConfigurator for RecordingEngine {
        async fn apply_engine_plan(&self, plan: &EngineRuntimePlan) -> TorrentResult<()> {
            if self.fail_apply_plan.load(AtomicOrdering::Relaxed) {
                return Err(TorrentError::Unsupported {
                    operation: "apply_engine_plan",
                });
            }
            self.applied.write().await.push(plan.clone());
            Ok(())
        }
    }

    struct StubConfig {
        secrets: HashMap<String, String>,
        applied: Mutex<Vec<SettingsChangeset>>,
        fail_secret_lookup: bool,
        fail_apply_changeset: bool,
    }

    impl StubConfig {
        async fn applied_len(&self) -> usize {
            self.applied.lock().await.len()
        }
    }

    #[async_trait]
    impl SettingsFacade for StubConfig {
        async fn get_app_profile(&self) -> revaer_config::ConfigResult<AppProfile> {
            Err(revaer_config::ConfigError::NotificationPayloadInvalid)
        }

        async fn get_engine_profile(&self) -> revaer_config::ConfigResult<EngineProfile> {
            Err(revaer_config::ConfigError::NotificationPayloadInvalid)
        }

        async fn get_fs_policy(&self) -> revaer_config::ConfigResult<FsPolicy> {
            Err(revaer_config::ConfigError::NotificationPayloadInvalid)
        }

        async fn get_secret(&self, name: &str) -> revaer_config::ConfigResult<Option<String>> {
            if self.fail_secret_lookup {
                return Err(revaer_config::ConfigError::NotificationPayloadInvalid);
            }
            Ok(self.secrets.get(name).cloned())
        }

        async fn subscribe_changes(
            &self,
        ) -> revaer_config::ConfigResult<revaer_config::SettingsStream> {
            Err(revaer_config::ConfigError::NotificationPayloadInvalid)
        }

        async fn apply_changeset(
            &self,
            _actor: &str,
            _reason: &str,
            changeset: SettingsChangeset,
        ) -> revaer_config::ConfigResult<revaer_config::AppliedChanges> {
            if self.fail_apply_changeset {
                return Err(revaer_config::ConfigError::NotificationPayloadInvalid);
            }
            self.applied.lock().await.push(changeset);
            Ok(revaer_config::AppliedChanges {
                revision: 0,
                app_profile: None,
                engine_profile: None,
                fs_policy: None,
            })
        }

        async fn issue_setup_token(
            &self,
            _ttl: Duration,
            _issued_by: &str,
        ) -> revaer_config::ConfigResult<SetupToken> {
            Err(revaer_config::ConfigError::SetupTokenMissing)
        }

        async fn consume_setup_token(&self, _token: &str) -> revaer_config::ConfigResult<()> {
            Err(revaer_config::ConfigError::SetupTokenInvalid)
        }

        async fn has_api_keys(&self) -> revaer_config::ConfigResult<bool> {
            Ok(false)
        }

        async fn factory_reset(&self) -> revaer_config::ConfigResult<()> {
            Err(revaer_config::ConfigError::NotificationPayloadInvalid)
        }
    }

    fn sample_fs_policy() -> FsPolicy {
        let root = repo_root()
            .join(".server_root")
            .join(format!("revaer-fsops-{}", Uuid::new_v4()));
        FsPolicy {
            id: Uuid::new_v4(),
            library_root: root.display().to_string(),
            extract: false,
            par2: "disabled".to_string(),
            flatten: false,
            move_mode: "copy".to_string(),
            cleanup_keep: Vec::new(),
            cleanup_drop: Vec::new(),
            chmod_file: None,
            chmod_dir: None,
            owner: None,
            group: None,
            umask: None,
            allow_paths: Vec::new(),
        }
    }

    fn engine_profile(label: &str) -> EngineProfile {
        EngineProfile {
            id: Uuid::new_v4(),
            implementation: format!("libtorrent-{label}"),
            listen_port: Some(6_881),
            listen_interfaces: Vec::new(),
            ipv6_mode: "disabled".to_string(),
            anonymous_mode: false.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            dht: true,
            encryption: "prefer".to_string(),
            max_active: Some(4),
            max_download_bps: None,
            max_upload_bps: None,
            seed_ratio_limit: None,
            seed_time_limit: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            stats_interval_ms: None,
            alt_speed: AltSpeedConfig::default(),
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
            resume_dir: ".server_root/resume".to_string(),
            download_root: ".server_root/downloads".to_string(),
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
            tracker: TrackerConfig::default(),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            ip_filter: IpFilterConfig::default(),
            peer_classes: PeerClassesConfig::default(),
            outgoing_port_min: None,
            outgoing_port_max: None,
            peer_dscp: None,
        }
    }

    #[tokio::test]
    async fn update_engine_profile_notifies_engine() -> TestResult<()> {
        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus.clone(),
            sample_fs_policy(),
            engine_profile("initial"),
            None,
            None,
        ));

        let mut updated = engine_profile("updated");
        updated.max_download_bps = Some(1_500_000);
        updated.max_upload_bps = Some(750_000);
        orchestrator.update_engine_profile(updated.clone()).await?;

        let applied_plans = {
            let guard = engine.applied.read().await;
            guard.clone()
        };
        assert_eq!(applied_plans.len(), 1);
        assert_eq!(
            applied_plans[0].effective.implementation,
            updated.implementation
        );
        assert_eq!(applied_plans[0].runtime.listen_port, updated.listen_port);
        assert_eq!(
            applied_plans[0].runtime.download_rate_limit,
            updated.max_download_bps
        );
        assert_eq!(
            applied_plans[0].runtime.upload_rate_limit,
            updated.max_upload_bps
        );

        let recorded_limits = {
            let guard = engine.limits.read().await;
            guard.clone()
        };
        assert_eq!(recorded_limits.len(), 1);
        assert!(
            recorded_limits[0].0.is_none(),
            "expected global rate limit update"
        );
        assert_eq!(recorded_limits[0].1.download_bps, Some(1_500_000));
        assert_eq!(recorded_limits[0].1.upload_bps, Some(750_000));
        Ok(())
    }

    #[tokio::test]
    async fn update_engine_profile_resolves_proxy_secrets() -> TestResult<()> {
        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let secrets = HashMap::from([
            ("PROXY_USER".to_string(), "proxy-user".to_string()),
            ("PROXY_PASS".to_string(), "proxy-pass".to_string()),
        ]);
        let config: Arc<dyn SettingsFacade> = Arc::new(StubConfig {
            secrets,
            applied: Mutex::new(Vec::new()),
            fail_secret_lookup: false,
            fail_apply_changeset: false,
        });

        let mut profile = engine_profile("proxy-auth");
        profile.tracker = TrackerConfig {
            proxy: Some(TrackerProxyConfig {
                host: "proxy.local".to_string(),
                port: 8080,
                username_secret: Some("PROXY_USER".to_string()),
                password_secret: Some("PROXY_PASS".to_string()),
                kind: TrackerProxyType::Socks5,
                proxy_peers: false,
            }),
            ..TrackerConfig::default()
        };

        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus,
            sample_fs_policy(),
            profile.clone(),
            None,
            Some(config),
        ));

        orchestrator.update_engine_profile(profile).await?;

        let applied =
            engine
                .applied
                .read()
                .await
                .last()
                .cloned()
                .ok_or_else(|| AppError::MissingState {
                    field: "runtime_plan",
                    value: None,
                })?;
        let proxy =
            applied
                .runtime
                .tracker
                .proxy
                .as_ref()
                .ok_or_else(|| AppError::MissingState {
                    field: "tracker_proxy",
                    value: None,
                })?;
        assert_eq!(proxy.username.as_deref(), Some("proxy-user"));
        assert_eq!(proxy.password.as_deref(), Some("proxy-pass"));
        Ok(())
    }

    #[tokio::test]
    async fn update_engine_profile_resolves_tracker_auth_secrets() -> TestResult<()> {
        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let secrets = HashMap::from([
            ("TRACKER_USER".to_string(), "tracker-user".to_string()),
            ("TRACKER_PASS".to_string(), "tracker-pass".to_string()),
            ("TRACKER_COOKIE".to_string(), "tracker-cookie".to_string()),
        ]);
        let config: Arc<dyn SettingsFacade> = Arc::new(StubConfig {
            secrets,
            applied: Mutex::new(Vec::new()),
            fail_secret_lookup: false,
            fail_apply_changeset: false,
        });

        let mut profile = engine_profile("tracker-auth");
        profile.tracker = TrackerConfig {
            auth: Some(TrackerAuthConfig {
                username_secret: Some("TRACKER_USER".to_string()),
                password_secret: Some("TRACKER_PASS".to_string()),
                cookie_secret: Some("TRACKER_COOKIE".to_string()),
            }),
            ..TrackerConfig::default()
        };

        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus,
            sample_fs_policy(),
            profile.clone(),
            None,
            Some(config),
        ));

        orchestrator.update_engine_profile(profile).await?;

        let applied =
            engine
                .applied
                .read()
                .await
                .last()
                .cloned()
                .ok_or_else(|| AppError::MissingState {
                    field: "runtime_plan",
                    value: None,
                })?;
        let auth = applied
            .runtime
            .tracker
            .auth
            .as_ref()
            .ok_or_else(|| AppError::MissingState {
                field: "tracker_auth",
                value: None,
            })?;
        assert_eq!(auth.username.as_deref(), Some("tracker-user"));
        assert_eq!(auth.password.as_deref(), Some("tracker-pass"));
        assert_eq!(auth.cookie.as_deref(), Some("tracker-cookie"));
        Ok(())
    }

    #[tokio::test]
    async fn update_engine_profile_records_missing_secret_warnings() -> TestResult<()> {
        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let config: Arc<dyn SettingsFacade> = Arc::new(StubConfig {
            secrets: HashMap::new(),
            applied: Mutex::new(Vec::new()),
            fail_secret_lookup: false,
            fail_apply_changeset: false,
        });

        let mut profile = engine_profile("missing-secrets");
        profile.tracker = TrackerConfig {
            auth: Some(TrackerAuthConfig {
                username_secret: Some("TRACKER_USER".to_string()),
                password_secret: Some("TRACKER_PASS".to_string()),
                cookie_secret: Some("TRACKER_COOKIE".to_string()),
            }),
            proxy: Some(TrackerProxyConfig {
                host: "proxy.local".to_string(),
                port: 8080,
                username_secret: Some("PROXY_USER".to_string()),
                password_secret: Some("PROXY_PASS".to_string()),
                kind: TrackerProxyType::Socks5,
                proxy_peers: false,
            }),
            ..TrackerConfig::default()
        };

        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus,
            sample_fs_policy(),
            profile.clone(),
            None,
            Some(config),
        ));

        orchestrator.update_engine_profile(profile).await?;

        let applied =
            engine
                .applied
                .read()
                .await
                .last()
                .cloned()
                .ok_or_else(|| AppError::MissingState {
                    field: "runtime_plan",
                    value: None,
                })?;
        let warnings = &applied.effective.warnings;
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("tracker.auth.username_secret")),
            "missing tracker username secret should be reported"
        );
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("tracker.proxy.password_secret")),
            "missing proxy password secret should be reported"
        );
        Ok(())
    }

    #[tokio::test]
    async fn update_engine_profile_surfaces_tracker_secret_lookup_errors() -> TestResult<()> {
        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let config: Arc<dyn SettingsFacade> = Arc::new(StubConfig {
            secrets: HashMap::new(),
            applied: Mutex::new(Vec::new()),
            fail_secret_lookup: true,
            fail_apply_changeset: false,
        });

        let mut profile = engine_profile("tracker-secret-error");
        profile.tracker = TrackerConfig {
            auth: Some(TrackerAuthConfig {
                username_secret: Some("TRACKER_USER".to_string()),
                password_secret: None,
                cookie_secret: None,
            }),
            ..TrackerConfig::default()
        };

        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus,
            sample_fs_policy(),
            profile.clone(),
            None,
            Some(config),
        ));

        let err = orchestrator
            .update_engine_profile(profile)
            .await
            .expect_err("secret lookup failure should surface");
        assert!(matches!(
            err,
            AppError::Config {
                operation: "config.get_secret",
                source: revaer_config::ConfigError::NotificationPayloadInvalid,
            }
        ));
        Ok(())
    }

    #[tokio::test]
    async fn update_engine_profile_surfaces_proxy_secret_lookup_errors() -> TestResult<()> {
        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let config: Arc<dyn SettingsFacade> = Arc::new(StubConfig {
            secrets: HashMap::new(),
            applied: Mutex::new(Vec::new()),
            fail_secret_lookup: true,
            fail_apply_changeset: false,
        });

        let mut profile = engine_profile("proxy-secret-error");
        profile.tracker = TrackerConfig {
            proxy: Some(TrackerProxyConfig {
                host: "proxy.local".to_string(),
                port: 8080,
                username_secret: Some("PROXY_USER".to_string()),
                password_secret: None,
                kind: TrackerProxyType::Socks5,
                proxy_peers: false,
            }),
            ..TrackerConfig::default()
        };

        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus,
            sample_fs_policy(),
            profile.clone(),
            None,
            Some(config),
        ));

        let err = orchestrator
            .update_engine_profile(profile)
            .await
            .expect_err("proxy secret lookup failure should surface");
        assert!(matches!(
            err,
            AppError::Config {
                operation: "config.get_secret",
                source: revaer_config::ConfigError::NotificationPayloadInvalid,
            }
        ));
        Ok(())
    }

    #[tokio::test]
    async fn update_engine_profile_surfaces_engine_apply_plan_errors() -> TestResult<()> {
        let engine = Arc::new(RecordingEngine::default());
        engine.fail_apply_plan.store(true, AtomicOrdering::Relaxed);
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus,
            sample_fs_policy(),
            engine_profile("apply-error"),
            None,
            None,
        ));

        let err = orchestrator
            .update_engine_profile(engine_profile("apply-error"))
            .await
            .expect_err("engine apply failures should surface");
        assert!(matches!(
            err,
            AppError::Torrent {
                operation: "engine.apply_plan",
                source: TorrentError::Unsupported {
                    operation: "apply_engine_plan",
                },
            }
        ));
        Ok(())
    }

    #[tokio::test]
    async fn update_engine_profile_surfaces_limit_update_errors() -> TestResult<()> {
        let engine = Arc::new(RecordingEngine::default());
        engine
            .fail_update_limits
            .store(true, AtomicOrdering::Relaxed);
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus,
            sample_fs_policy(),
            engine_profile("limit-error"),
            None,
            None,
        ));

        let err = orchestrator
            .update_engine_profile(engine_profile("limit-error"))
            .await
            .expect_err("limit update failures should surface");
        assert!(matches!(
            err,
            AppError::Torrent {
                operation: "engine.update_limits",
                source: TorrentError::Unsupported {
                    operation: "update_limits",
                },
            }
        ));
        Ok(())
    }

    #[tokio::test]
    async fn update_engine_profile_records_blocklist_fetch_and_metadata_persist_failures()
    -> TestResult<()> {
        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let config: Arc<dyn SettingsFacade> = Arc::new(StubConfig {
            secrets: HashMap::new(),
            applied: Mutex::new(Vec::new()),
            fail_secret_lookup: false,
            fail_apply_changeset: true,
        });

        let mut profile = engine_profile("blocklist-fetch-error");
        profile.ip_filter = IpFilterConfig {
            blocklist_url: Some("http://127.0.0.1:9".to_string()),
            ..IpFilterConfig::default()
        };

        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus,
            sample_fs_policy(),
            profile.clone(),
            None,
            Some(config),
        ));

        orchestrator.update_engine_profile(profile).await?;

        let applied =
            engine
                .applied
                .read()
                .await
                .last()
                .cloned()
                .ok_or_else(|| AppError::MissingState {
                    field: "runtime_plan",
                    value: None,
                })?;
        assert!(
            applied
                .effective
                .warnings
                .iter()
                .any(|warning| warning.contains("blocklist fetch failed")),
            "blocklist fetch failures should be preserved as warnings"
        );
        assert!(
            applied.effective.warnings.iter().any(|warning| warning
                .contains("failed to persist ip_filter metadata; continuing with cached state")),
            "failed metadata persistence should be preserved as warnings"
        );
        assert!(
            applied.effective.network.ip_filter.last_error.is_some(),
            "last blocklist error should be recorded on the effective profile"
        );
        Ok(())
    }

    #[tokio::test]
    async fn update_engine_profile_records_skipped_blocklist_entries() -> TestResult<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buffer = [0_u8; 1024];
                let _ = stream.read(&mut buffer).await;
                let body = "10.0.0.1/32\ninvalid-entry\n";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"v2\"\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });

        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let mut profile = engine_profile("blocklist-skips");
        profile.ip_filter = IpFilterConfig {
            blocklist_url: Some(format!("http://{addr}")),
            ..IpFilterConfig::default()
        };

        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus,
            sample_fs_policy(),
            profile.clone(),
            None,
            None,
        ));

        orchestrator.update_engine_profile(profile).await?;
        let _ = server.await;

        let applied =
            engine
                .applied
                .read()
                .await
                .last()
                .cloned()
                .ok_or_else(|| AppError::MissingState {
                    field: "runtime_plan",
                    value: None,
                })?;
        assert!(
            applied
                .effective
                .warnings
                .iter()
                .any(|warning| warning.contains("skipped 1 invalid blocklist entries")),
            "invalid blocklist entries should be reported to operators"
        );
        Ok(())
    }

    #[tokio::test]
    async fn blocklist_is_fetched_and_cached() -> TestResult<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buffer = [0_u8; 1024];
                let _ = stream.read(&mut buffer).await;
                let body = "10.0.0.1/32\n";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"v1\"\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });

        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let mut profile = engine_profile("blocklist");
        profile.ip_filter = IpFilterConfig {
            blocklist_url: Some(format!("http://{addr}")),
            ..IpFilterConfig::default()
        };
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus,
            sample_fs_policy(),
            profile.clone(),
            None,
            None,
        ));

        orchestrator.update_engine_profile(profile.clone()).await?;
        let _ = server.await;

        let first_plan =
            engine
                .applied
                .read()
                .await
                .last()
                .cloned()
                .ok_or_else(|| AppError::MissingState {
                    field: "runtime_plan",
                    value: None,
                })?;
        let filter =
            first_plan
                .runtime
                .ip_filter
                .as_ref()
                .ok_or_else(|| AppError::MissingState {
                    field: "ip_filter",
                    value: None,
                })?;
        assert_eq!(filter.rules.len(), 1);
        assert_eq!(filter.rules[0].start, "10.0.0.1");

        // Subsequent updates reuse the cached rules even if the server is gone.
        orchestrator.update_engine_profile(profile).await?;
        let cached =
            engine
                .applied
                .read()
                .await
                .last()
                .cloned()
                .ok_or_else(|| AppError::MissingState {
                    field: "cached_runtime_plan",
                    value: None,
                })?;
        let cached_filter =
            cached
                .runtime
                .ip_filter
                .as_ref()
                .ok_or_else(|| AppError::MissingState {
                    field: "cached_ip_filter",
                    value: None,
                })?;
        assert_eq!(cached_filter.rules.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn update_engine_profile_clamps_before_applying() -> TestResult<()> {
        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus.clone(),
            sample_fs_policy(),
            engine_profile("initial"),
            None,
            None,
        ));

        let mut updated = engine_profile("guard");
        updated.max_download_bps = Some(revaer_config::MAX_RATE_LIMIT_BPS + 100);
        updated.download_root = String::new();
        updated.resume_dir = "  ".to_string();
        orchestrator.update_engine_profile(updated).await?;

        let applied_plans = engine.applied.read().await.clone();
        assert_eq!(
            applied_plans[0].runtime.download_rate_limit,
            Some(revaer_config::MAX_RATE_LIMIT_BPS)
        );
        assert_eq!(
            applied_plans[0].runtime.download_root,
            ".server_root/downloads"
        );
        assert_eq!(applied_plans[0].runtime.resume_dir, ".server_root/resume");
        assert!(
            applied_plans[0]
                .effective
                .warnings
                .iter()
                .any(|msg| msg.contains("guard rail")),
            "guard rail warnings should be propagated to the plan"
        );

        let recorded_limits = engine.limits.read().await.clone();
        assert_eq!(
            recorded_limits[0].1.download_bps,
            Some(u64::try_from(revaer_config::MAX_RATE_LIMIT_BPS)?)
        );
        Ok(())
    }

    #[tokio::test]
    async fn workflow_operations_forward_to_engine() -> TestResult<()> {
        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus,
            sample_fs_policy(),
            engine_profile("ops"),
            None,
            None,
        ));

        let torrent_id = Uuid::new_v4();
        let add_request = AddTorrent {
            id: torrent_id,
            source: revaer_torrent_core::model::TorrentSource::magnet("magnet:?xt=urn:btih:demo"),
            options: revaer_torrent_core::AddTorrentOptions::default(),
        };
        let author_request = TorrentAuthorRequest {
            root_path: repo_root().display().to_string(),
            ..TorrentAuthorRequest::default()
        };
        let limit = TorrentRateLimit {
            download_bps: Some(1_000),
            upload_bps: Some(500),
        };
        let selection = FileSelectionUpdate {
            include: vec!["*.mkv".to_string()],
            exclude: vec!["*.tmp".to_string()],
            skip_fluff: true,
            ..FileSelectionUpdate::default()
        };
        let options = TorrentOptionsUpdate {
            connections_limit: Some(33),
            queue_position: Some(2),
            ..TorrentOptionsUpdate::default()
        };
        let trackers = TorrentTrackersUpdate {
            trackers: vec!["https://tracker.example/announce".to_string()],
            replace: true,
        };
        let web_seeds = TorrentWebSeedsUpdate {
            web_seeds: vec!["https://seed.example/file".to_string()],
            replace: true,
        };

        TorrentWorkflow::add_torrent(&*orchestrator, add_request.clone()).await?;
        let authored =
            TorrentWorkflow::create_torrent(&*orchestrator, author_request.clone()).await?;
        assert_eq!(authored.magnet_uri, "magnet:?xt=urn:btih:demo");

        TorrentWorkflow::pause_torrent(&*orchestrator, torrent_id).await?;
        TorrentWorkflow::resume_torrent(&*orchestrator, torrent_id).await?;
        TorrentWorkflow::set_sequential(&*orchestrator, torrent_id, true).await?;
        TorrentWorkflow::update_limits(&*orchestrator, Some(torrent_id), limit.clone()).await?;
        TorrentWorkflow::update_limits(&*orchestrator, None, limit.clone()).await?;
        TorrentWorkflow::update_selection(&*orchestrator, torrent_id, selection.clone()).await?;
        TorrentWorkflow::update_options(&*orchestrator, torrent_id, options.clone()).await?;
        TorrentWorkflow::update_trackers(&*orchestrator, torrent_id, trackers.clone()).await?;
        TorrentWorkflow::update_web_seeds(&*orchestrator, torrent_id, web_seeds.clone()).await?;
        TorrentWorkflow::reannounce(&*orchestrator, torrent_id).await?;
        TorrentWorkflow::move_torrent(
            &*orchestrator,
            torrent_id,
            ".server_root/downloads/custom".to_string(),
        )
        .await?;
        TorrentWorkflow::recheck(&*orchestrator, torrent_id).await?;
        TorrentWorkflow::remove_torrent(
            &*orchestrator,
            torrent_id,
            RemoveTorrent { with_data: true },
        )
        .await?;

        assert_eq!(engine.added.read().await.len(), 1);
        assert_eq!(engine.authored.read().await.len(), 1);
        assert_eq!(engine.paused.read().await.len(), 1);
        assert_eq!(engine.resumed.read().await.len(), 1);
        assert_eq!(engine.sequential.read().await.len(), 1);
        assert_eq!(engine.limits.read().await.len(), 2);
        assert_eq!(engine.selections.read().await.len(), 1);
        assert_eq!(engine.options.read().await.len(), 1);
        assert_eq!(engine.trackers.read().await.len(), 1);
        assert_eq!(engine.web_seeds.read().await.len(), 1);
        assert_eq!(engine.reannounced.read().await.len(), 1);
        assert_eq!(engine.moves.read().await.len(), 1);
        assert_eq!(engine.rechecked.read().await.len(), 1);
        assert_eq!(engine.removed.read().await.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn inspector_methods_read_catalog_and_engine_peers() -> TestResult<()> {
        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus,
            sample_fs_policy(),
            engine_profile("inspect"),
            None,
            None,
        ));

        let torrent_id = Uuid::new_v4();
        orchestrator
            .catalog
            .observe(&Event::TorrentAdded {
                torrent_id,
                name: "alpha".to_string(),
            })
            .await;
        engine.peers.write().await.insert(
            torrent_id,
            vec![PeerSnapshot {
                endpoint: "203.0.113.10:6881".to_string(),
                client: Some("libtorrent".to_string()),
                progress: 0.5,
                download_bps: 128,
                upload_bps: 64,
                interest: PeerInterest {
                    local: true,
                    remote: false,
                },
                choke: PeerChoke {
                    local: false,
                    remote: true,
                },
            }],
        );

        let listed = revaer_torrent_core::TorrentInspector::list(&*orchestrator).await?;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, torrent_id);

        let fetched = revaer_torrent_core::TorrentInspector::get(&*orchestrator, torrent_id)
            .await?
            .ok_or_else(|| AppError::MissingState {
                field: "torrent_status",
                value: Some(torrent_id.to_string()),
            })?;
        assert_eq!(fetched.name.as_deref(), Some("alpha"));

        let peers =
            revaer_torrent_core::TorrentInspector::peers(&*orchestrator, torrent_id).await?;
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].endpoint, "203.0.113.10:6881");
        Ok(())
    }

    #[test]
    fn event_torrent_id_extracts_supported_events() {
        let id = Uuid::new_v4();
        assert_eq!(
            event_torrent_id(&Event::TorrentAdded {
                torrent_id: id,
                name: "demo".into()
            }),
            Some(id)
        );
        assert_eq!(
            event_torrent_id(&Event::SelectionReconciled {
                torrent_id: id,
                reason: "policy".into()
            }),
            Some(id)
        );
        assert_eq!(
            event_torrent_id(&Event::MetadataUpdated {
                torrent_id: id,
                name: None,
                download_dir: Some(".server_root/downloads/demo".into()),
                comment: None,
                source: None,
                private: None,
            }),
            Some(id)
        );
        assert_eq!(
            event_torrent_id(&Event::HealthChanged { degraded: vec![] }),
            None,
            "health events should not carry torrent ids"
        );
    }

    #[tokio::test]
    async fn torrent_catalog_tracks_event_evolution() -> TestResult<()> {
        let catalog = TorrentCatalog::new();
        let id = Uuid::new_v4();
        let other = Uuid::new_v4();

        catalog
            .observe(&Event::TorrentAdded {
                torrent_id: id,
                name: "zeta".into(),
            })
            .await;
        catalog
            .observe(&Event::FilesDiscovered {
                torrent_id: id,
                files: vec![
                    DiscoveredFile {
                        path: "movie.mkv".into(),
                        size_bytes: 1_024,
                    },
                    DiscoveredFile {
                        path: "movie.srt".into(),
                        size_bytes: 512,
                    },
                ],
            })
            .await;
        catalog
            .observe(&Event::Progress {
                torrent_id: id,
                bytes_downloaded: 512,
                bytes_total: 1_024,
                eta_seconds: Some(12),
                download_bps: 256,
                upload_bps: 128,
                ratio: 0.5,
            })
            .await;
        catalog
            .observe(&Event::StateChanged {
                torrent_id: id,
                state: TorrentState::Downloading,
            })
            .await;
        catalog
            .observe(&Event::Completed {
                torrent_id: id,
                library_path: ".server_root/library/title".into(),
            })
            .await;
        catalog
            .observe(&Event::FsopsFailed {
                torrent_id: id,
                message: "oops".into(),
            })
            .await;

        catalog
            .observe(&Event::TorrentAdded {
                torrent_id: other,
                name: "alpha".into(),
            })
            .await;

        let mut statuses = catalog.list().await;
        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses.remove(0).id, other, "sorted by name first");

        let status = catalog
            .get(id)
            .await
            .ok_or_else(|| AppError::MissingState {
                field: "torrent_status",
                value: Some(id.to_string()),
            })?;
        assert_eq!(status.progress.bytes_total, 1_024);
        assert!(matches!(status.state, TorrentState::Failed { .. }));
        let files = status.files.ok_or_else(|| AppError::MissingState {
            field: "torrent_files",
            value: Some(id.to_string()),
        })?;
        assert_eq!(files[0].index, 0);
        assert_eq!(files[1].index, 1);
        Ok(())
    }

    #[tokio::test]
    async fn torrent_catalog_ignores_empty_file_discovery_and_clamps_ratio() -> TestResult<()> {
        let catalog = TorrentCatalog::new();
        let id = Uuid::new_v4();

        catalog
            .observe(&Event::FilesDiscovered {
                torrent_id: id,
                files: Vec::new(),
            })
            .await;
        assert!(catalog.get(id).await.is_none());

        catalog
            .observe(&Event::Progress {
                torrent_id: id,
                bytes_downloaded: 10,
                bytes_total: 20,
                eta_seconds: Some(5),
                download_bps: 100,
                upload_bps: 50,
                ratio: f64::NAN,
            })
            .await;
        let status = catalog
            .get(id)
            .await
            .ok_or_else(|| AppError::MissingState {
                field: "torrent_status",
                value: Some(id.to_string()),
            })?;
        assert!(status.rates.ratio.abs() < f64::EPSILON);
        Ok(())
    }

    #[tokio::test]
    async fn torrent_catalog_metadata_updates_preserve_existing_values() -> TestResult<()> {
        let catalog = TorrentCatalog::new();
        let id = Uuid::new_v4();

        catalog
            .observe(&Event::TorrentAdded {
                torrent_id: id,
                name: "original".into(),
            })
            .await;
        catalog
            .observe(&Event::MetadataUpdated {
                torrent_id: id,
                name: Some("renamed".into()),
                download_dir: Some(".server_root/downloads/renamed".into()),
                comment: Some("seeded from test".into()),
                source: Some("torznab".into()),
                private: Some(true),
            })
            .await;

        let updated = catalog
            .get(id)
            .await
            .ok_or_else(|| AppError::MissingState {
                field: "torrent_status",
                value: Some(id.to_string()),
            })?;
        assert_eq!(updated.name.as_deref(), Some("renamed"));
        assert_eq!(
            updated.download_dir.as_deref(),
            Some(".server_root/downloads/renamed")
        );
        assert_eq!(updated.comment.as_deref(), Some("seeded from test"));
        assert_eq!(updated.source.as_deref(), Some("torznab"));
        assert_eq!(updated.private, Some(true));

        catalog
            .observe(&Event::MetadataUpdated {
                torrent_id: id,
                name: None,
                download_dir: None,
                comment: None,
                source: None,
                private: Some(false),
            })
            .await;

        let preserved = catalog
            .get(id)
            .await
            .ok_or_else(|| AppError::MissingState {
                field: "torrent_status",
                value: Some(id.to_string()),
            })?;
        assert_eq!(preserved.name.as_deref(), Some("renamed"));
        assert_eq!(
            preserved.download_dir.as_deref(),
            Some(".server_root/downloads/renamed")
        );
        assert_eq!(preserved.comment.as_deref(), Some("seeded from test"));
        assert_eq!(preserved.source.as_deref(), Some("torznab"));
        assert_eq!(preserved.private, Some(false));

        catalog
            .observe(&Event::MetadataUpdated {
                torrent_id: id,
                name: None,
                download_dir: None,
                comment: None,
                source: None,
                private: None,
            })
            .await;

        let retained = catalog
            .get(id)
            .await
            .ok_or_else(|| AppError::MissingState {
                field: "torrent_status",
                value: Some(id.to_string()),
            })?;
        assert_eq!(retained.name.as_deref(), Some("renamed"));
        assert_eq!(
            retained.download_dir.as_deref(),
            Some(".server_root/downloads/renamed")
        );
        assert_eq!(retained.comment.as_deref(), Some("seeded from test"));
        assert_eq!(retained.source.as_deref(), Some("torznab"));
        assert_eq!(retained.private, Some(false));
        Ok(())
    }

    #[tokio::test]
    async fn torrent_catalog_touch_remove_and_ignore_events_manage_entries() -> TestResult<()> {
        let catalog = TorrentCatalog::new();
        let id = Uuid::new_v4();

        catalog
            .observe(&Event::SettingsChanged {
                description: "config updated".into(),
            })
            .await;
        catalog
            .observe(&Event::HealthChanged {
                degraded: vec!["config_watcher".into()],
            })
            .await;
        assert!(
            catalog.get(id).await.is_none(),
            "non-torrent events should not create entries"
        );

        catalog
            .observe(&Event::FsopsStarted { torrent_id: id })
            .await;
        let started = catalog
            .get(id)
            .await
            .ok_or_else(|| AppError::MissingState {
                field: "torrent_status",
                value: Some(id.to_string()),
            })?;
        assert_eq!(started.id, id);
        assert!(matches!(started.state, TorrentState::Queued));

        catalog
            .observe(&Event::FsopsProgress {
                torrent_id: id,
                step: "copy".into(),
            })
            .await;
        catalog
            .observe(&Event::FsopsCompleted { torrent_id: id })
            .await;
        assert!(
            catalog.get(id).await.is_some(),
            "fsops touch events should preserve the tracked entry"
        );

        catalog
            .observe(&Event::TorrentRemoved { torrent_id: id })
            .await;
        assert!(
            catalog.get(id).await.is_none(),
            "torrent removal should drop the tracked entry"
        );
        Ok(())
    }

    #[test]
    fn torrent_catalog_compare_status_orders_named_and_unnamed_entries() {
        let id_a = Uuid::from_u128(1);
        let id_b = Uuid::from_u128(2);
        let mut named = TorrentCatalog::blank_status(id_a);
        named.name = Some("alpha".to_string());
        let unnamed = TorrentCatalog::blank_status(id_b);
        assert_eq!(
            TorrentCatalog::compare_status(&named, &unnamed),
            Ordering::Less
        );
        assert_eq!(
            TorrentCatalog::compare_status(&unnamed, &named),
            Ordering::Greater
        );
    }

    #[tokio::test]
    async fn refresh_ip_filter_clears_cache_when_url_missing() -> TestResult<()> {
        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus,
            sample_fs_policy(),
            engine_profile("refresh"),
            None,
            None,
        ));

        orchestrator
            .set_ip_filter_cache(IpFilterCache {
                url: "http://example.invalid".to_string(),
                etag: Some("etag".to_string()),
                rules: vec![RuntimeIpFilterRule {
                    start: "10.0.0.1".to_string(),
                    end: "10.0.0.1".to_string(),
                }],
                fetched_at: Instant::now(),
                last_refreshed: Utc::now(),
            })
            .await;

        let mut plan = EngineRuntimePlan::from_profile(&engine_profile("refresh"));
        orchestrator.refresh_ip_filter(&mut plan).await?;

        assert!(
            orchestrator.ip_filter_cache.read().await.is_none(),
            "expected cache to be cleared"
        );
        let runtime_filter = plan
            .runtime
            .ip_filter
            .ok_or_else(|| AppError::MissingState {
                field: "runtime_ip_filter",
                value: None,
            })?;
        assert!(runtime_filter.blocklist_url.is_none());
        assert!(runtime_filter.etag.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn load_blocklist_returns_missing_state_on_not_modified_without_cache() -> TestResult<()>
    {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buffer = [0_u8; 512];
                let _ = stream.read(&mut buffer).await;
                let response = "HTTP/1.1 304 Not Modified\r\nContent-Length: 0\r\n\r\n";
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });

        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus, metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            EventBus::new(),
            sample_fs_policy(),
            engine_profile("blocklist"),
            None,
            None,
        ));

        let result = orchestrator
            .load_blocklist(&format!("http://{addr}"), Some("etag".to_string()))
            .await;
        let _ = server.await;

        assert!(
            matches!(
                result,
                Err(AppError::MissingState {
                    field: "blocklist_cache",
                    ..
                })
            ),
            "expected missing cache error"
        );
        Ok(())
    }

    #[tokio::test]
    async fn load_blocklist_reuses_cached_rules_on_not_modified() -> TestResult<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let url = format!("http://{addr}");
        let server = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buffer = [0_u8; 512];
                let _ = stream.read(&mut buffer).await;
                let response = "HTTP/1.1 304 Not Modified\r\nContent-Length: 0\r\n\r\n";
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });

        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus, metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            EventBus::new(),
            sample_fs_policy(),
            engine_profile("blocklist"),
            None,
            None,
        ));
        orchestrator
            .set_ip_filter_cache(IpFilterCache {
                url: url.clone(),
                etag: Some("\"v1\"".to_string()),
                rules: vec![RuntimeIpFilterRule {
                    start: "10.0.0.9".to_string(),
                    end: "10.0.0.9".to_string(),
                }],
                fetched_at: Instant::now()
                    .checked_sub(BLOCKLIST_REFRESH_INTERVAL)
                    .expect("blocklist refresh interval should fit within Instant"),
                last_refreshed: Utc::now(),
            })
            .await;

        let resolution = orchestrator.load_blocklist(&url, None).await?;
        let _ = server.await;

        assert_eq!(resolution.rules.len(), 1);
        assert_eq!(resolution.rules[0].start, "10.0.0.9");
        assert_eq!(resolution.etag.as_deref(), Some("\"v1\""));
        Ok(())
    }

    #[tokio::test]
    async fn load_blocklist_returns_http_status_error() -> TestResult<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buffer = [0_u8; 512];
                let _ = stream.read(&mut buffer).await;
                let response = "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n";
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });

        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus, metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            EventBus::new(),
            sample_fs_policy(),
            engine_profile("blocklist"),
            None,
            None,
        ));

        let result = orchestrator
            .load_blocklist(&format!("http://{addr}"), None)
            .await;
        let _ = server.await;

        assert!(matches!(
            result,
            Err(AppError::HttpStatus {
                operation: "blocklist.fetch",
                status: 503,
                ..
            })
        ));
        Ok(())
    }

    #[tokio::test]
    async fn persist_ip_filter_metadata_skips_when_unchanged() -> TestResult<()> {
        let config = Arc::new(StubConfig {
            secrets: HashMap::new(),
            applied: Mutex::new(Vec::new()),
            fail_secret_lookup: false,
            fail_apply_changeset: false,
        });
        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus, metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            EventBus::new(),
            sample_fs_policy(),
            engine_profile("meta"),
            None,
            Some(config.clone()),
        ));

        let previous = IpFilterConfig::default();
        orchestrator
            .persist_ip_filter_metadata(config.as_ref(), &previous, &previous)
            .await?;

        assert_eq!(config.applied_len().await, 0);
        Ok(())
    }

    #[tokio::test]
    async fn persist_ip_filter_metadata_applies_changes() -> TestResult<()> {
        let config = Arc::new(StubConfig {
            secrets: HashMap::new(),
            applied: Mutex::new(Vec::new()),
            fail_secret_lookup: false,
            fail_apply_changeset: false,
        });
        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus, metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            EventBus::new(),
            sample_fs_policy(),
            engine_profile("meta"),
            None,
            Some(config.clone()),
        ));

        let previous = IpFilterConfig::default();
        let updated = IpFilterConfig {
            etag: Some("etag".to_string()),
            ..Default::default()
        };
        orchestrator
            .persist_ip_filter_metadata(config.as_ref(), &previous, &updated)
            .await?;

        assert_eq!(config.applied_len().await, 1);
        Ok(())
    }
}

#[cfg(test)]
mod parsing_tests {
    use super::*;
    use std::fmt::Write;
    use tempfile::TempDir;

    type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

    #[test]
    fn parse_blocklist_skips_invalid_lines_and_dedupes() -> TestResult<()> {
        let body = "# header\ninvalid\n10.0.0.1/32\n10.0.0.1/32\n";
        let parsed = parse_blocklist(body)?;
        assert_eq!(parsed.rules.len(), 1);
        assert_eq!(parsed.rules[0].start, "10.0.0.1");
        assert_eq!(parsed.skipped, 1);
        Ok(())
    }

    #[test]
    fn parse_blocklist_rejects_excess_entries() {
        let mut body = String::new();
        for idx in 0..=MAX_BLOCKLIST_RULES {
            let a = (idx / 65_536) % 256;
            let b = (idx / 256) % 256;
            let c = idx % 256;
            writeln!(&mut body, "10.{a}.{b}.{c}/32").expect("append cidr");
        }
        let err = parse_blocklist(&body).expect_err("expected too many entries error");
        assert!(matches!(
            err,
            AppError::InvalidConfig {
                field: "ip_filter.blocklist_url",
                reason: "too_many_entries",
                ..
            }
        ));
    }

    #[test]
    fn merge_rules_dedupes_additions() {
        let mut base = vec![RuntimeIpFilterRule {
            start: "10.0.0.1".to_string(),
            end: "10.0.0.1".to_string(),
        }];
        merge_rules(
            &mut base,
            vec![
                RuntimeIpFilterRule {
                    start: "10.0.0.1".to_string(),
                    end: "10.0.0.1".to_string(),
                },
                RuntimeIpFilterRule {
                    start: "10.0.0.2".to_string(),
                    end: "10.0.0.2".to_string(),
                },
            ],
        );
        assert_eq!(base.len(), 2);
    }

    #[test]
    fn dedupe_rules_collapses_case_insensitive_duplicates() {
        let rules = dedupe_rules(vec![
            RuntimeIpFilterRule {
                start: "10.0.0.1".to_string(),
                end: "10.0.0.1".to_string(),
            },
            RuntimeIpFilterRule {
                start: "10.0.0.1".to_uppercase(),
                end: "10.0.0.1".to_uppercase(),
            },
        ]);
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn runtime_rule_from_config_copies_start_and_end() -> TestResult<()> {
        let (_, rule) = canonicalize_ip_filter_entry("10.0.0.1/32", "ip_filter.rule")?;
        let runtime = runtime_rule_from_config(&rule);
        assert_eq!(runtime.start, "10.0.0.1");
        assert_eq!(runtime.end, "10.0.0.1");
        Ok(())
    }

    #[test]
    fn parse_path_list_rejects_empty_entry() {
        let entries = vec![String::new()];
        let err = parse_path_list(&entries).expect_err("expected error for empty entry");
        assert!(matches!(
            err,
            AppError::InvalidConfig {
                field: "allow_paths",
                reason: "empty_entry",
                ..
            }
        ));
    }

    #[test]
    fn parse_path_list_accepts_non_empty_entries() -> TestResult<()> {
        let paths = parse_path_list(&["/tmp".to_string(), "/var/tmp".to_string()])?;
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], PathBuf::from("/tmp"));
        Ok(())
    }

    #[test]
    fn enforce_allow_paths_accepts_nested_path() -> TestResult<()> {
        let temp = TempDir::new()?;
        let root = temp.path().join("root");
        std::fs::create_dir_all(&root)?;
        let allow_paths = vec![temp.path().to_string_lossy().to_string()];
        enforce_allow_paths(&root, &allow_paths)?;
        Ok(())
    }

    #[test]
    fn enforce_allow_paths_rejects_unlisted_path() -> TestResult<()> {
        let temp = TempDir::new()?;
        let allowed = temp.path().join("allowed");
        let root = temp.path().join("root");
        std::fs::create_dir_all(&allowed)?;
        std::fs::create_dir_all(&root)?;
        let allow_paths = vec![allowed.to_string_lossy().to_string()];
        let err = enforce_allow_paths(&root, &allow_paths).expect_err("expected rejection");
        assert!(matches!(
            err,
            AppError::InvalidConfig {
                field: "allow_paths",
                reason: "root_not_permitted",
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn enforce_allow_paths_allows_any_path_when_list_empty() -> TestResult<()> {
        let temp = TempDir::new()?;
        let root = temp.path().join("root");
        std::fs::create_dir_all(&root)?;
        enforce_allow_paths(&root, &[])?;
        Ok(())
    }

    #[test]
    fn app_error_to_torrent_preserves_operation_and_identifier() {
        let torrent_id = Uuid::new_v4();
        let wrapped = app_error_to_torrent(
            "engine.update_limits",
            Some(torrent_id),
            AppError::MissingEnv {
                name: "DATABASE_URL",
            },
        );
        assert!(matches!(
            wrapped,
            TorrentError::OperationFailed {
                operation: "engine.update_limits",
                torrent_id: Some(id),
                ..
            } if id == torrent_id
        ));
    }
}
