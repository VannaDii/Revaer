use anyhow::{Result, anyhow};
use async_trait::async_trait;
use cxx::UniquePtr;
use uuid::Uuid;

use crate::convert::{map_native_event, map_priority};
use crate::ffi::ffi;
use crate::types::EngineRuntimeConfig;
use ffi::SourceKind;
use revaer_torrent_core::{
    AddTorrent, EngineEvent, FileSelectionUpdate, PeerSnapshot, RemoveTorrent, TorrentRateLimit,
    TorrentSource, model::TrackerAuth,
};
use tracing::warn;

use super::LibTorrentSession;
use super::options::EngineOptionsPlan;

pub(super) struct NativeSession {
    inner: UniquePtr<ffi::Session>,
}

pub(super) fn create_session() -> Result<Box<dyn LibTorrentSession>> {
    let options = base_options();
    let inner = initialize_session(&options)?;
    Ok(Box::new(NativeSession { inner }))
}

impl NativeSession {
    fn map_error(message: String) -> Result<()> {
        if message.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(message))
        }
    }

    #[cfg(all(test, feature = "libtorrent"))]
    fn inspect_storage_state(&self) -> ffi::EngineStorageState {
        self.inner
            .as_ref()
            .expect("native session must be initialized")
            .inspect_storage_state()
    }
}

const fn base_options() -> ffi::SessionOptions {
    ffi::SessionOptions {
        download_root: String::new(),
        resume_dir: String::new(),
        enable_dht: false,
        sequential_default: false,
    }
}

fn initialize_session(options: &ffi::SessionOptions) -> Result<UniquePtr<ffi::Session>> {
    let inner = ffi::new_session(options);
    if inner.is_null() {
        Err(anyhow!("failed to initialize libtorrent session"))
    } else {
        Ok(inner)
    }
}

const fn map_max_connections(limit: Option<i32>) -> (i32, bool) {
    match limit {
        Some(value) if value > 0 => (value, true),
        _ => (-1, false),
    }
}

fn map_tracker_auth(auth: Option<&TrackerAuth>) -> ffi::TrackerAuthOptions {
    let Some(auth) = auth else {
        return ffi::TrackerAuthOptions {
            username: String::new(),
            password: String::new(),
            cookie: String::new(),
            username_secret: String::new(),
            password_secret: String::new(),
            cookie_secret: String::new(),
            has_username: false,
            has_password: false,
            has_cookie: false,
        };
    };

    ffi::TrackerAuthOptions {
        username: auth.username.clone().unwrap_or_default(),
        password: auth.password.clone().unwrap_or_default(),
        cookie: auth.cookie.clone().unwrap_or_default(),
        username_secret: String::new(),
        password_secret: String::new(),
        cookie_secret: String::new(),
        has_username: auth.username.is_some(),
        has_password: auth.password.is_some(),
        has_cookie: auth.cookie.is_some(),
    }
}

fn map_peer_info(peer: ffi::NativePeerInfo) -> PeerSnapshot {
    let download_bps = u64::try_from(peer.download_rate).unwrap_or(0);
    let upload_bps = u64::try_from(peer.upload_rate).unwrap_or(0);
    PeerSnapshot {
        endpoint: peer.endpoint,
        client: (!peer.client.is_empty()).then_some(peer.client),
        progress: peer.progress,
        download_bps,
        upload_bps,
        interest: revaer_torrent_core::model::PeerInterest {
            local: peer.interesting,
            remote: peer.remote_interested,
        },
        choke: revaer_torrent_core::model::PeerChoke {
            local: peer.choked,
            remote: peer.remote_choked,
        },
    }
}

/// Test harness helpers for exercising the native session.
#[cfg(all(test, feature = "libtorrent"))]
pub(super) mod test_support {
    use super::{NativeSession, create_native_session_for_tests};
    use crate::types::{
        ChokingAlgorithm, EncryptionPolicy, EngineRuntimeConfig, Ipv6Mode, SeedChokingAlgorithm,
        TrackerRuntimeConfig,
    };
    use anyhow::Result;
    use std::path::Path;
    use tempfile::TempDir;

    /// Convenience harness for exercising native config application in tests.
    pub(super) struct NativeSessionHarness {
        /// Native session under test.
        pub(super) session: NativeSession,
        download: TempDir,
        resume: TempDir,
    }

    impl NativeSessionHarness {
        /// Spin up a native session backed by temporary storage roots.
        pub(super) fn new() -> Result<Self> {
            Ok(Self {
                session: create_native_session_for_tests()?,
                download: TempDir::new()?,
                resume: TempDir::new()?,
            })
        }

        /// Baseline runtime configuration rooted at the harness directories.
        pub(super) fn runtime_config(&self) -> EngineRuntimeConfig {
            EngineRuntimeConfig {
                download_root: self.download.path().to_string_lossy().into_owned(),
                resume_dir: self.resume.path().to_string_lossy().into_owned(),
                storage_mode: crate::types::StorageMode::Sparse,
                use_partfile: true.into(),
                disk_read_mode: None,
                disk_write_mode: None,
                verify_piece_hashes: true.into(),
                cache_size: None,
                cache_expiry: None,
                coalesce_reads: true.into(),
                coalesce_writes: true.into(),
                use_disk_cache_pool: true.into(),
                listen_interfaces: Vec::new(),
                ipv6_mode: Ipv6Mode::Disabled,
                enable_dht: false,
                dht_bootstrap_nodes: Vec::new(),
                dht_router_nodes: Vec::new(),
                enable_lsd: false.into(),
                enable_upnp: false.into(),
                enable_natpmp: false.into(),
                enable_pex: false.into(),
                outgoing_ports: None,
                peer_dscp: None,
                anonymous_mode: false.into(),
                force_proxy: false.into(),
                prefer_rc4: false.into(),
                allow_multiple_connections_per_ip: false.into(),
                enable_outgoing_utp: false.into(),
                enable_incoming_utp: false.into(),
                sequential_default: false,
                auto_managed: true.into(),
                auto_manage_prefer_seeds: false.into(),
                dont_count_slow_torrents: true.into(),
                listen_port: None,
                max_active: None,
                download_rate_limit: None,
                upload_rate_limit: None,
                seed_ratio_limit: None,
                seed_time_limit: None,
                alt_speed: None,
                stats_interval_ms: None,
                connections_limit: None,
                connections_limit_per_torrent: None,
                unchoke_slots: None,
                half_open_limit: None,
                choking_algorithm: ChokingAlgorithm::FixedSlots,
                seed_choking_algorithm: SeedChokingAlgorithm::RoundRobin,
                strict_super_seeding: false.into(),
                optimistic_unchoke_slots: None,
                max_queued_disk_bytes: None,
                encryption: EncryptionPolicy::Prefer,
                tracker: TrackerRuntimeConfig::default(),
                ip_filter: None,
                super_seeding: false.into(),
            }
        }

        pub(super) fn download_path(&self) -> &Path {
            self.download.path()
        }
    }
}

#[cfg(all(test, feature = "libtorrent"))]
fn create_native_session_for_tests() -> Result<NativeSession> {
    let options = base_options();
    let inner = initialize_session(&options)?;
    Ok(NativeSession { inner })
}

#[async_trait]
impl LibTorrentSession for NativeSession {
    async fn add_torrent(&mut self, request: &AddTorrent) -> Result<()> {
        let mut add_request = ffi::AddTorrentRequest {
            id: request.id.to_string(),
            source_kind: match request.source {
                TorrentSource::Magnet { .. } => SourceKind::Magnet,
                TorrentSource::Metainfo { .. } => SourceKind::Metainfo,
            },
            magnet_uri: String::new(),
            metainfo: Vec::new(),
            download_dir: request.options.download_dir.clone().unwrap_or_default(),
            has_download_dir: request.options.download_dir.is_some(),
            storage_mode: 0,
            has_storage_mode: request.options.storage_mode.is_some(),
            sequential: request.options.sequential.unwrap_or_default(),
            has_sequential_override: request.options.sequential.is_some(),
            start_paused: request.options.start_paused.unwrap_or_default(),
            has_start_paused: request.options.start_paused.is_some(),
            auto_managed: request.options.auto_managed.unwrap_or_default(),
            has_auto_managed: request.options.auto_managed.is_some(),
            queue_position: request.options.queue_position.unwrap_or_default(),
            has_queue_position: request.options.queue_position.is_some(),
            seed_mode: request.options.seed_mode.unwrap_or(false),
            has_seed_mode: request.options.seed_mode.is_some(),
            hash_check_sample_pct: request.options.hash_check_sample_pct.unwrap_or(0),
            has_hash_check_sample: request.options.hash_check_sample_pct.is_some(),
            pex_enabled: request.options.pex_enabled.unwrap_or(true),
            has_pex_enabled: request.options.pex_enabled.is_some(),
            super_seeding: request.options.super_seeding.unwrap_or(false),
            has_super_seeding: request.options.super_seeding.is_some(),
            max_connections: 0,
            has_max_connections: false,
            tags: request.options.tags.clone(),
            trackers: request.options.trackers.clone(),
            replace_trackers: request.options.replace_trackers,
            web_seeds: request.options.web_seeds.clone(),
            replace_web_seeds: request.options.replace_web_seeds,
            tracker_auth: map_tracker_auth(request.options.tracker_auth.as_ref()),
        };
        (add_request.max_connections, add_request.has_max_connections) =
            map_max_connections(request.options.connections_limit);

        match &request.source {
            TorrentSource::Magnet { uri } => add_request.magnet_uri.clone_from(uri),
            TorrentSource::Metainfo { bytes } => add_request.metainfo.clone_from(bytes),
        }

        if let Some(mode) = request.options.storage_mode {
            add_request.storage_mode = crate::types::StorageMode::from(mode).as_i32();
        }

        let session = self.inner.pin_mut();
        let result = session.add_torrent(&add_request);
        Self::map_error(result)
    }

    async fn remove_torrent(&mut self, id: Uuid, options: &RemoveTorrent) -> Result<()> {
        let key = id.to_string();
        let session = self.inner.pin_mut();
        let result = session.remove_torrent(&key, options.with_data);
        Self::map_error(result)
    }

    async fn pause_torrent(&mut self, id: Uuid) -> Result<()> {
        let key = id.to_string();
        let session = self.inner.pin_mut();
        let result = session.pause_torrent(&key);
        Self::map_error(result)
    }

    async fn resume_torrent(&mut self, id: Uuid) -> Result<()> {
        let key = id.to_string();
        let session = self.inner.pin_mut();
        let result = session.resume_torrent(&key);
        Self::map_error(result)
    }

    async fn set_sequential(&mut self, id: Uuid, sequential: bool) -> Result<()> {
        let key = id.to_string();
        let session = self.inner.pin_mut();
        let result = session.set_sequential(&key, sequential);
        Self::map_error(result)
    }

    async fn load_fastresume(&mut self, id: Uuid, payload: &[u8]) -> Result<()> {
        let key = id.to_string();
        let session = self.inner.pin_mut();
        let result = session.load_fastresume(&key, payload);
        Self::map_error(result)
    }

    async fn update_limits(&mut self, id: Option<Uuid>, limits: &TorrentRateLimit) -> Result<()> {
        let request = ffi::LimitRequest {
            apply_globally: id.is_none(),
            id: id.map_or_else(String::new, |value| value.to_string()),
            download_bps: limits
                .download_bps
                .map_or(-1, |value| i64::try_from(value).unwrap_or(-1)),
            upload_bps: limits
                .upload_bps
                .map_or(-1, |value| i64::try_from(value).unwrap_or(-1)),
        };
        let session = self.inner.pin_mut();
        let result = session.update_limits(&request);
        Self::map_error(result)
    }

    async fn update_selection(&mut self, id: Uuid, rules: &FileSelectionUpdate) -> Result<()> {
        let priorities = rules
            .priorities
            .iter()
            .map(|override_rule| ffi::FilePriorityOverride {
                index: override_rule.index,
                priority: map_priority(override_rule.priority),
            })
            .collect::<Vec<_>>();

        let request = ffi::SelectionRules {
            id: id.to_string(),
            include: rules.include.clone(),
            exclude: rules.exclude.clone(),
            priorities,
            skip_fluff: rules.skip_fluff,
        };
        let session = self.inner.pin_mut();
        let result = session.update_selection(&request);
        Self::map_error(result)
    }

    async fn update_options(
        &mut self,
        id: Uuid,
        options: &revaer_torrent_core::model::TorrentOptionsUpdate,
    ) -> Result<()> {
        let mut request = ffi::UpdateOptionsRequest {
            id: id.to_string(),
            max_connections: 0,
            has_max_connections: false,
            pex_enabled: false,
            has_pex_enabled: false,
            super_seeding: false,
            has_super_seeding: false,
            auto_managed: false,
            has_auto_managed: false,
            queue_position: 0,
            has_queue_position: false,
        };

        if let Some(limit) = options.connections_limit
            && limit > 0
        {
            request.max_connections = limit;
            request.has_max_connections = true;
        }
        if let Some(pex_enabled) = options.pex_enabled {
            request.pex_enabled = pex_enabled;
            request.has_pex_enabled = true;
        }
        if let Some(super_seeding) = options.super_seeding {
            request.super_seeding = super_seeding;
            request.has_super_seeding = true;
        }
        if let Some(auto_managed) = options.auto_managed {
            request.auto_managed = auto_managed;
            request.has_auto_managed = true;
        }
        if let Some(queue_position) = options.queue_position {
            request.queue_position = queue_position;
            request.has_queue_position = true;
        }

        let session = self.inner.pin_mut();
        let result = session.update_options(&request);
        Self::map_error(result)
    }

    async fn reannounce(&mut self, id: Uuid) -> Result<()> {
        let key = id.to_string();
        let session = self.inner.pin_mut();
        let result = session.reannounce(&key);
        Self::map_error(result)
    }

    async fn move_torrent(&mut self, id: Uuid, download_dir: &str) -> Result<()> {
        let request = ffi::MoveTorrentRequest {
            id: id.to_string(),
            download_dir: download_dir.to_string(),
        };
        let session = self.inner.pin_mut();
        let result = session.move_torrent(&request);
        Self::map_error(result)
    }

    async fn recheck(&mut self, id: Uuid) -> Result<()> {
        let key = id.to_string();
        let session = self.inner.pin_mut();
        let result = session.recheck(&key);
        Self::map_error(result)
    }

    async fn peers(&mut self, id: Uuid) -> Result<Vec<PeerSnapshot>> {
        let key = id.to_string();
        let session = self.inner.pin_mut();
        let peers = session.list_peers(&key);
        Ok(peers.into_iter().map(map_peer_info).collect())
    }

    async fn update_trackers(
        &mut self,
        id: Uuid,
        trackers: &revaer_torrent_core::model::TorrentTrackersUpdate,
    ) -> Result<()> {
        let request = ffi::UpdateTrackersRequest {
            id: id.to_string(),
            trackers: trackers.trackers.clone(),
            replace: trackers.replace,
        };
        let session = self.inner.pin_mut();
        let result = session.update_trackers(&request);
        Self::map_error(result)
    }

    async fn update_web_seeds(
        &mut self,
        id: Uuid,
        web_seeds: &revaer_torrent_core::model::TorrentWebSeedsUpdate,
    ) -> Result<()> {
        let request = ffi::UpdateWebSeedsRequest {
            id: id.to_string(),
            web_seeds: web_seeds.web_seeds.clone(),
            replace: web_seeds.replace,
        };
        let session = self.inner.pin_mut();
        let result = session.update_web_seeds(&request);
        Self::map_error(result)
    }

    async fn set_piece_deadline(
        &mut self,
        id: Uuid,
        piece: u32,
        deadline_ms: Option<u32>,
    ) -> Result<()> {
        let (deadline, has_deadline) = match deadline_ms {
            Some(value) => {
                let deadline = i32::try_from(value)
                    .map_err(|_| anyhow!("deadline exceeds supported range"))?;
                (deadline, true)
            }
            None => (0, false),
        };
        let session = self.inner.pin_mut();
        let result = session.set_piece_deadline(&id.to_string(), piece, deadline, has_deadline);
        Self::map_error(result)
    }

    async fn apply_config(&mut self, config: &EngineRuntimeConfig) -> Result<()> {
        let plan = EngineOptionsPlan::from_runtime_config(config);
        for warning in &plan.warnings {
            warn!(%warning, "native engine guard rail applied");
        }
        let session = self.inner.pin_mut();
        let result = session.apply_engine_profile(&plan.options);
        Self::map_error(result)
    }

    async fn poll_events(&mut self) -> Result<Vec<EngineEvent>> {
        let session = self.inner.pin_mut();
        let raw_events = session.poll_events();
        let mut events = Vec::with_capacity(raw_events.len());

        for native in raw_events {
            let torrent_id = Uuid::parse_str(&native.id).ok().filter(|id| !id.is_nil());
            events.extend(map_native_event(torrent_id, native));
        }

        Ok(events)
    }
}

#[cfg(all(test, feature = "libtorrent"))]
mod tests {
    use super::test_support::NativeSessionHarness;
    use super::*;
    use crate::ffi::ffi::{NativeEvent, NativeEventKind, NativeTorrentState};
    use crate::types::{IpFilterRule, IpFilterRuntimeConfig, Ipv6Mode};
    use revaer_torrent_core::{AddTorrent, AddTorrentOptions, EngineEvent, TorrentSource};
    use std::{convert::TryFrom, fs, path::Path, time::Duration};
    use tokio::time::sleep;
    use uuid::Uuid;

    const SEED_PIECE_LENGTH: i64 = 16_384;
    const VALID_PIECE_HASH: [u8; 20] = [
        137, 114, 86, 182, 112, 158, 26, 77, 169, 218, 186, 146, 182, 189, 227, 156, 207, 204, 216,
        193,
    ];
    const MISMATCH_PIECE_HASH: [u8; 20] = [0_u8; 20];

    fn seed_mode_metainfo(hash: &[u8; 20]) -> Vec<u8> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(
            b"d8:announce30:http://localhost:6969/announce4:infod6:lengthi16384e4:name6:sample12:piece lengthi16384e6:pieces20:",
        );
        encoded.extend_from_slice(hash);
        encoded.extend_from_slice(b"ee");
        encoded
    }

    fn write_seed_payload(root: &Path) -> std::io::Result<()> {
        let piece_len =
            usize::try_from(SEED_PIECE_LENGTH).expect("seed piece length must fit in usize");
        fs::write(root.join("sample"), vec![0_u8; piece_len])
    }

    #[tokio::test]
    async fn native_session_accepts_configuration_and_add() -> Result<()> {
        let mut harness = NativeSessionHarness::new()?;
        let config = harness.runtime_config();
        harness.session.apply_config(&config).await?;

        let descriptor = AddTorrent {
            id: Uuid::new_v4(),
            source: TorrentSource::magnet(
                "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567",
            ),
            options: AddTorrentOptions {
                connections_limit: Some(32),
                ..AddTorrentOptions::default()
            },
        };

        harness.session.add_torrent(&descriptor).await?;
        // Polling immediately should succeed even if no events are queued yet.
        let _ = harness.session.poll_events().await?;
        Ok(())
    }

    #[tokio::test]
    async fn native_session_accepts_seed_mode_with_metainfo() -> Result<()> {
        let mut harness = NativeSessionHarness::new()?;
        let config = harness.runtime_config();
        harness.session.apply_config(&config).await?;
        write_seed_payload(harness.download_path())?;

        let descriptor = AddTorrent {
            id: Uuid::new_v4(),
            source: TorrentSource::metainfo(seed_mode_metainfo(&VALID_PIECE_HASH)),
            options: AddTorrentOptions {
                seed_mode: Some(true),
                ..AddTorrentOptions::default()
            },
        };

        harness.session.add_torrent(&descriptor).await?;
        Ok(())
    }

    #[tokio::test]
    async fn native_session_moves_storage_and_reports_metadata() -> Result<()> {
        let mut harness = NativeSessionHarness::new()?;
        let config = harness.runtime_config();
        harness.session.apply_config(&config).await?;
        write_seed_payload(harness.download_path())?;

        let descriptor = AddTorrent {
            id: Uuid::new_v4(),
            source: TorrentSource::metainfo(seed_mode_metainfo(&VALID_PIECE_HASH)),
            options: AddTorrentOptions::default(),
        };

        harness.session.add_torrent(&descriptor).await?;
        let target = harness.download_path().join("relocated");
        fs::create_dir_all(&target)?;
        harness
            .session
            .move_torrent(descriptor.id, target.to_string_lossy().as_ref())
            .await?;
        sleep(Duration::from_millis(200)).await;

        let events = harness.session.poll_events().await?;
        let mut saw_metadata = false;
        for event in events {
            if let EngineEvent::MetadataUpdated {
                torrent_id,
                download_dir,
                ..
            } = event
                && torrent_id == descriptor.id
            {
                assert_eq!(
                    download_dir.as_deref(),
                    Some(target.to_string_lossy().as_ref())
                );
                saw_metadata = true;
            }
        }
        assert!(saw_metadata, "expected metadata update after move");
        Ok(())
    }

    #[tokio::test]
    async fn native_session_rejects_hash_sample_on_mismatch() -> Result<()> {
        let mut harness = NativeSessionHarness::new()?;
        let config = harness.runtime_config();
        harness.session.apply_config(&config).await?;
        write_seed_payload(harness.download_path())?;

        let descriptor = AddTorrent {
            id: Uuid::new_v4(),
            source: TorrentSource::metainfo(seed_mode_metainfo(&MISMATCH_PIECE_HASH)),
            options: AddTorrentOptions {
                seed_mode: Some(true),
                hash_check_sample_pct: Some(100),
                ..AddTorrentOptions::default()
            },
        };

        let err = harness
            .session
            .add_torrent(&descriptor)
            .await
            .expect_err("hash sample should fail for mismatched data");
        assert!(
            err.to_string().contains("seed-mode sample failed")
                || err.to_string().contains("hash mismatch")
        );
        Ok(())
    }

    #[tokio::test]
    async fn native_session_rejects_seed_mode_for_magnets() -> Result<()> {
        let mut harness = NativeSessionHarness::new()?;
        let config = harness.runtime_config();
        harness.session.apply_config(&config).await?;

        let descriptor = AddTorrent {
            id: Uuid::new_v4(),
            source: TorrentSource::magnet(
                "magnet:?xt=urn:btih:fedcba98765432100123456789abcdef01234567",
            ),
            options: AddTorrentOptions {
                seed_mode: Some(true),
                ..AddTorrentOptions::default()
            },
        };

        let err = harness
            .session
            .add_torrent(&descriptor)
            .await
            .expect_err("seed mode without metainfo should be rejected");
        assert!(
            err.to_string()
                .contains("seed_mode requires metainfo payload")
        );
        Ok(())
    }

    #[tokio::test]
    async fn native_session_applies_rate_limits() -> Result<()> {
        let mut harness = NativeSessionHarness::new()?;
        let mut config = harness.runtime_config();
        config.listen_port = Some(68_81);
        config.max_active = Some(2);
        config.download_rate_limit = Some(256_000);
        config.upload_rate_limit = Some(128_000);
        config.enable_dht = true;
        config.dht_bootstrap_nodes = vec!["router.bittorrent.com:6881".into()];
        config.dht_router_nodes = vec!["dht.transmissionbt.com:6881".into()];
        config.enable_lsd = true.into();
        config.enable_upnp = true.into();
        config.enable_natpmp = true.into();
        config.enable_pex = true.into();

        harness.session.apply_config(&config).await?;

        let descriptor = AddTorrent {
            id: Uuid::new_v4(),
            source: TorrentSource::magnet(
                "magnet:?xt=urn:btih:fedcba98765432100123456789abcdef01234567",
            ),
            options: AddTorrentOptions::default(),
        };

        harness.session.add_torrent(&descriptor).await?;

        harness
            .session
            .update_limits(
                None,
                &TorrentRateLimit {
                    download_bps: Some(128_000),
                    upload_bps: Some(64_000),
                },
            )
            .await?;

        harness
            .session
            .update_limits(
                Some(descriptor.id),
                &TorrentRateLimit {
                    download_bps: Some(64_000),
                    upload_bps: Some(32_000),
                },
            )
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn native_session_accepts_v2_magnet() -> Result<()> {
        let mut harness = NativeSessionHarness::new()?;
        let config = harness.runtime_config();
        harness.session.apply_config(&config).await?;

        let descriptor = AddTorrent {
            id: Uuid::new_v4(),
            source: TorrentSource::magnet(
                "magnet:?xt=urn:btmh:1220aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
            options: AddTorrentOptions::default(),
        };

        harness.session.add_torrent(&descriptor).await?;
        Ok(())
    }

    #[tokio::test]
    async fn native_session_accepts_explicit_listen_interfaces() -> Result<()> {
        let mut harness = NativeSessionHarness::new()?;
        let mut config = harness.runtime_config();
        config.listen_interfaces = vec!["0.0.0.0:7000".into(), "[::]:7000".into()];
        config.ipv6_mode = Ipv6Mode::Enabled;
        config.listen_port = Some(7_000);

        harness.session.apply_config(&config).await?;
        Ok(())
    }

    #[tokio::test]
    async fn native_session_applies_ip_filter() -> Result<()> {
        let mut harness = NativeSessionHarness::new()?;
        let mut config = harness.runtime_config();
        config.ip_filter = Some(IpFilterRuntimeConfig {
            rules: vec![IpFilterRule {
                start: "203.0.113.1".into(),
                end: "203.0.113.1".into(),
            }],
            blocklist_url: None,
            etag: None,
            last_updated_at: None,
        });

        harness.session.apply_config(&config).await?;
        Ok(())
    }

    #[tokio::test]
    async fn native_session_applies_disk_cache_settings() -> Result<()> {
        #[derive(Copy, Clone)]
        struct StorageFlags(u8);
        impl StorageFlags {
            const USE_PARTFILE: u8 = 0b0001;
            const COALESCE_READS: u8 = 0b0010;
            const COALESCE_WRITES: u8 = 0b0100;
            const USE_DISK_CACHE_POOL: u8 = 0b1000;

            fn use_partfile(self) -> bool {
                self.0 & Self::USE_PARTFILE != 0
            }

            fn coalesce_reads(self) -> bool {
                self.0 & Self::COALESCE_READS != 0
            }

            fn coalesce_writes(self) -> bool {
                self.0 & Self::COALESCE_WRITES != 0
            }

            fn use_disk_cache_pool(self) -> bool {
                self.0 & Self::USE_DISK_CACHE_POOL != 0
            }
        }

        let mut harness = NativeSessionHarness::new()?;
        let mut config = harness.runtime_config();
        config.cache_size = Some(192);
        config.cache_expiry = Some(120);
        config.use_partfile = false.into();
        config.coalesce_reads = false.into();
        config.coalesce_writes = true.into();
        config.use_disk_cache_pool = false.into();
        config.disk_read_mode = Some(crate::types::DiskIoMode::DisableOsCache);
        config.disk_write_mode = Some(crate::types::DiskIoMode::WriteThrough);
        config.verify_piece_hashes = false.into();

        harness.session.apply_config(&config).await?;

        let snapshot = harness.session.inspect_storage_state();
        let flags = StorageFlags(snapshot.flags);

        assert_eq!(snapshot.cache_size, 192);
        assert_eq!(snapshot.cache_expiry, 120);
        assert!(!flags.use_partfile());
        assert!(!flags.coalesce_reads());
        assert!(flags.coalesce_writes());
        assert!(!flags.use_disk_cache_pool());
        assert_eq!(
            snapshot.disk_read_mode,
            crate::types::DiskIoMode::DisableOsCache.as_i32()
        );
        assert_eq!(
            snapshot.disk_write_mode,
            crate::types::DiskIoMode::WriteThrough.as_i32()
        );
        assert!(!snapshot.verify_piece_hashes);
        Ok(())
    }

    #[test]
    fn native_event_translates_progress_and_resume_data() {
        let torrent_id = Uuid::new_v4();
        let events = map_native_event(
            Some(torrent_id),
            NativeEvent {
                id: torrent_id.to_string(),
                kind: NativeEventKind::Progress,
                state: NativeTorrentState::Downloading,
                name: "demo".to_string(),
                download_dir: "/tmp/downloads".to_string(),
                library_path: String::new(),
                bytes_downloaded: 512,
                bytes_total: 1024,
                download_bps: 4096,
                upload_bps: 2048,
                ratio: 0.5,
                files: Vec::new(),
                resume_data: Vec::new(),
                message: String::new(),
                tracker_statuses: Vec::new(),
                component: String::new(),
            },
        );

        assert!(matches!(
            events.first(),
            Some(EngineEvent::Progress {
                progress,
                rates,
                torrent_id: id,
            }) if *id == torrent_id
                && progress.bytes_downloaded == 512
                && progress.bytes_total == 1024
                && rates.download_bps == 4096
                && rates.upload_bps == 2048
                && (rates.ratio - 0.5).abs() < f64::EPSILON
        ));

        let resume = map_native_event(
            Some(torrent_id),
            NativeEvent {
                id: torrent_id.to_string(),
                kind: NativeEventKind::ResumeData,
                state: NativeTorrentState::Downloading,
                name: String::new(),
                download_dir: String::new(),
                library_path: String::new(),
                bytes_downloaded: 0,
                bytes_total: 0,
                download_bps: 0,
                upload_bps: 0,
                ratio: 0.0,
                files: Vec::new(),
                resume_data: vec![1, 2, 3, 4],
                message: String::new(),
                tracker_statuses: Vec::new(),
                component: String::new(),
            },
        );

        assert!(matches!(
            resume.first(),
            Some(EngineEvent::ResumeData {
                torrent_id: id,
                payload,
            }) if *id == torrent_id && payload == &vec![1, 2, 3, 4]
        ));
    }
}
