use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use revaer_events::TorrentState;
use revaer_torrent_core::{
    AddTorrent, EngineEvent, FileSelectionUpdate, PeerSnapshot, RemoveTorrent, StorageMode,
    TorrentError, TorrentRateLimit, TorrentResult,
    model::{TorrentAuthorFile, TorrentAuthorRequest, TorrentAuthorResult},
};
use serde_json::json;
use uuid::Uuid;

use super::LibTorrentSession;
use crate::error::{LibtorrentError, op_failed};
use crate::types::{EngineRuntimeConfig, EngineSettingsSnapshot};
use revaer_torrent_core::{FilePriorityOverride, FileSelectionRules};

/// In-memory test double for the libtorrent session interface.
#[derive(Default)]
pub struct StubSession {
    torrents: HashMap<Uuid, StubTorrent>,
    pending_events: Vec<EngineEvent>,
    peer_map: HashMap<Uuid, Vec<PeerSnapshot>>,
    deadlines: HashMap<Uuid, HashMap<u32, Option<u32>>>,
}

#[derive(Clone)]
struct StubTorrent {
    selection: FileSelectionRules,
    priorities: Vec<FilePriorityOverride>,
    rate_limit: TorrentRateLimit,
    sequential: bool,
    state: TorrentState,
    download_dir: Option<String>,
    storage_mode: Option<StorageMode>,
    use_partfile: Option<bool>,
    connections_limit: Option<i32>,
    seed_mode: Option<bool>,
    hash_check_sample_pct: Option<u8>,
    seed_ratio_limit: Option<f64>,
    seed_time_limit: Option<u64>,
    resume_payload: Option<Vec<u8>>,
    auto_managed: Option<bool>,
    queue_position: Option<i32>,
    pex_enabled: Option<bool>,
    super_seeding: Option<bool>,
    trackers: Vec<String>,
    replace_trackers: bool,
    web_seeds: Vec<String>,
    replace_web_seeds: bool,
}

impl StubTorrent {
    fn from_add(request: &AddTorrent) -> Self {
        let seed_mode = request.options.seed_mode.unwrap_or(false);
        let initial_state = if request.options.start_paused.unwrap_or(false) {
            TorrentState::Stopped
        } else if seed_mode {
            TorrentState::Seeding
        } else {
            TorrentState::Queued
        };
        Self {
            selection: request.options.file_rules.clone(),
            priorities: Vec::new(),
            rate_limit: request.options.rate_limit.clone(),
            sequential: request.options.sequential.unwrap_or(false),
            state: initial_state,
            download_dir: request.options.download_dir.clone(),
            storage_mode: request.options.storage_mode,
            use_partfile: None,
            connections_limit: request.options.connections_limit,
            seed_mode: request.options.seed_mode,
            hash_check_sample_pct: request.options.hash_check_sample_pct,
            seed_ratio_limit: None,
            seed_time_limit: None,
            resume_payload: None,
            auto_managed: request.options.auto_managed,
            queue_position: request.options.queue_position,
            pex_enabled: request.options.pex_enabled,
            super_seeding: request.options.super_seeding,
            trackers: request.options.trackers.clone(),
            replace_trackers: request.options.replace_trackers,
            web_seeds: request.options.web_seeds.clone(),
            replace_web_seeds: request.options.replace_web_seeds,
        }
    }
}

impl StubSession {
    fn torrent_mut(&mut self, id: Uuid) -> TorrentResult<&mut StubTorrent> {
        self.torrents
            .get_mut(&id)
            .ok_or(TorrentError::NotFound { torrent_id: id })
    }

    /// Seed peer snapshots for a given torrent.
    #[must_use]
    pub fn with_peers(mut self, id: Uuid, peers: Vec<PeerSnapshot>) -> Self {
        self.peer_map.insert(id, peers);
        self
    }

    fn push_state(&mut self, id: Uuid, state: TorrentState) {
        self.pending_events.push(EngineEvent::StateChanged {
            torrent_id: id,
            state,
        });
    }

    fn refresh_resume(&mut self, id: Uuid) {
        if let Some(torrent) = self.torrents.get_mut(&id) {
            let payload = json!({
                "selection": {
                    "include": torrent.selection.include,
                    "exclude": torrent.selection.exclude,
                    "skip_fluff": torrent.selection.skip_fluff,
                },
                "priorities": torrent.priorities.iter().map(|priority| {
                    json!({
                        "index": priority.index,
                        "priority": format!("{:?}", priority.priority),
                    })
                }).collect::<Vec<_>>(),
                "rate_limit": {
                    "download_bps": torrent.rate_limit.download_bps,
                    "upload_bps": torrent.rate_limit.upload_bps,
                },
                "sequential": torrent.sequential,
                "seed_mode": torrent.seed_mode,
                "hash_check_sample_pct": torrent.hash_check_sample_pct,
                "connections_limit": torrent.connections_limit,
                "download_dir": torrent.download_dir,
                "storage_mode": torrent
                    .storage_mode
                    .as_ref()
                    .map(|mode| format!("{mode:?}")),
                "use_partfile": torrent.use_partfile,
                "seed_ratio_limit": torrent.seed_ratio_limit,
                "seed_time_limit": torrent.seed_time_limit,
                "auto_managed": torrent.auto_managed,
                "queue_position": torrent.queue_position,
                "pex_enabled": torrent.pex_enabled,
                "super_seeding": torrent.super_seeding,
                "trackers": torrent.trackers,
                "replace_trackers": torrent.replace_trackers,
                "web_seeds": torrent.web_seeds,
                "replace_web_seeds": torrent.replace_web_seeds,
            })
            .to_string()
            .into_bytes();
            torrent.resume_payload = Some(payload.clone());
            self.pending_events.push(EngineEvent::ResumeData {
                torrent_id: id,
                payload,
            });
        }
    }
}

#[async_trait]
impl LibTorrentSession for StubSession {
    async fn add_torrent(&mut self, request: &AddTorrent) -> TorrentResult<()> {
        let torrent = StubTorrent::from_add(request);
        let download_dir = torrent.download_dir.clone();
        self.torrents.insert(request.id, torrent);
        let initial_state = self
            .torrents
            .get(&request.id)
            .map_or(TorrentState::Queued, |entry| entry.state.clone());
        self.push_state(request.id, initial_state);
        self.pending_events.push(EngineEvent::MetadataUpdated {
            torrent_id: request.id,
            name: request.options.name_hint.clone(),
            download_dir,
            comment: request.options.comment.clone(),
            source: request.options.source.clone(),
            private: request.options.private,
        });
        self.refresh_resume(request.id);
        Ok(())
    }

    async fn create_torrent(
        &mut self,
        request: &TorrentAuthorRequest,
    ) -> TorrentResult<TorrentAuthorResult> {
        if request.root_path.trim().is_empty() {
            return Err(op_failed(
                "create_torrent",
                None,
                LibtorrentError::MissingField { field: "root_path" },
            ));
        }

        let files = vec![TorrentAuthorFile {
            path: request.root_path.clone(),
            size_bytes: 0,
        }];
        Ok(TorrentAuthorResult {
            metainfo: b"stub".to_vec(),
            magnet_uri: "magnet:?xt=urn:btih:stub".to_string(),
            info_hash: "stub".to_string(),
            piece_length: request.piece_length.unwrap_or(16_384),
            total_size: 0,
            files,
            warnings: Vec::new(),
            trackers: request.trackers.clone(),
            web_seeds: request.web_seeds.clone(),
            private: request.private,
            comment: request.comment.clone(),
            source: request.source.clone(),
        })
    }

    async fn remove_torrent(&mut self, id: Uuid, _options: &RemoveTorrent) -> TorrentResult<()> {
        if self.torrents.remove(&id).is_some() {
            self.push_state(id, TorrentState::Stopped);
            self.pending_events.push(EngineEvent::ResumeData {
                torrent_id: id,
                payload: Vec::new(),
            });
            Ok(())
        } else {
            Err(TorrentError::NotFound { torrent_id: id })
        }
    }

    async fn pause_torrent(&mut self, id: Uuid) -> TorrentResult<()> {
        let torrent = self.torrent_mut(id)?;
        torrent.state = TorrentState::Stopped;
        self.push_state(id, TorrentState::Stopped);
        self.refresh_resume(id);
        Ok(())
    }

    async fn resume_torrent(&mut self, id: Uuid) -> TorrentResult<()> {
        let torrent = self.torrent_mut(id)?;
        torrent.state = TorrentState::Downloading;
        self.push_state(id, TorrentState::Downloading);
        self.refresh_resume(id);
        Ok(())
    }

    async fn set_sequential(&mut self, id: Uuid, sequential: bool) -> TorrentResult<()> {
        let torrent = self.torrent_mut(id)?;
        torrent.sequential = sequential;
        self.refresh_resume(id);
        Ok(())
    }

    async fn update_limits(
        &mut self,
        id: Option<Uuid>,
        limits: &TorrentRateLimit,
    ) -> TorrentResult<()> {
        if let Some(target) = id {
            let torrent = self.torrent_mut(target)?;
            torrent.rate_limit = limits.clone();
            self.refresh_resume(target);
        } else {
            let ids: Vec<Uuid> = self.torrents.keys().copied().collect();
            for id in &ids {
                if let Some(entry) = self.torrents.get_mut(id) {
                    entry.rate_limit = limits.clone();
                }
            }
            for id in ids {
                self.refresh_resume(id);
            }
        }
        Ok(())
    }

    async fn update_selection(
        &mut self,
        id: Uuid,
        rules: &FileSelectionUpdate,
    ) -> TorrentResult<()> {
        let torrent = self.torrent_mut(id)?;
        torrent.selection.include.clone_from(&rules.include);
        torrent.selection.exclude.clone_from(&rules.exclude);
        torrent.selection.skip_fluff = rules.skip_fluff;
        torrent.priorities.clone_from(&rules.priorities);
        self.refresh_resume(id);
        Ok(())
    }

    async fn update_options(
        &mut self,
        id: Uuid,
        options: &revaer_torrent_core::model::TorrentOptionsUpdate,
    ) -> TorrentResult<()> {
        let torrent = self.torrent_mut(id)?;
        if options.connections_limit.is_some() {
            torrent.connections_limit = options.connections_limit.filter(|value| *value > 0);
        }
        if let Some(pex_enabled) = options.pex_enabled {
            torrent.pex_enabled = Some(pex_enabled);
        }
        if let Some(super_seeding) = options.super_seeding {
            torrent.super_seeding = Some(super_seeding);
        }
        if let Some(auto_managed) = options.auto_managed {
            torrent.auto_managed = Some(auto_managed);
            torrent.state = if auto_managed {
                revaer_events::TorrentState::Queued
            } else {
                torrent.state.clone()
            };
        }
        if options.queue_position.is_some() {
            torrent.queue_position = options.queue_position;
        }
        self.refresh_resume(id);
        Ok(())
    }

    async fn update_trackers(
        &mut self,
        id: Uuid,
        trackers: &revaer_torrent_core::model::TorrentTrackersUpdate,
    ) -> TorrentResult<()> {
        let torrent = self.torrent_mut(id)?;
        let mut seen: HashSet<String> = if trackers.replace {
            HashSet::new()
        } else {
            torrent.trackers.iter().cloned().collect()
        };
        if trackers.replace {
            torrent.trackers.clear();
        }
        for tracker in &trackers.trackers {
            if seen.insert(tracker.clone()) {
                torrent.trackers.push(tracker.clone());
            }
        }
        torrent.replace_trackers = trackers.replace;
        self.refresh_resume(id);
        Ok(())
    }

    async fn update_web_seeds(
        &mut self,
        id: Uuid,
        web_seeds: &revaer_torrent_core::model::TorrentWebSeedsUpdate,
    ) -> TorrentResult<()> {
        let torrent = self.torrent_mut(id)?;
        let mut seeds = if web_seeds.replace {
            Vec::new()
        } else {
            torrent.web_seeds.clone()
        };
        let mut seen: HashSet<String> = seeds.iter().cloned().collect();
        for seed in &web_seeds.web_seeds {
            if seen.insert(seed.clone()) {
                seeds.push(seed.clone());
            }
        }
        torrent.web_seeds = seeds;
        torrent.replace_web_seeds = web_seeds.replace;
        self.refresh_resume(id);
        Ok(())
    }

    async fn set_piece_deadline(
        &mut self,
        id: Uuid,
        piece: u32,
        deadline_ms: Option<u32>,
    ) -> TorrentResult<()> {
        let deadlines = self.deadlines.entry(id).or_default();
        deadlines.insert(piece, deadline_ms);
        Ok(())
    }

    async fn load_fastresume(&mut self, id: Uuid, payload: &[u8]) -> TorrentResult<()> {
        let torrent = self.torrent_mut(id)?;
        torrent.resume_payload = Some(payload.to_vec());
        self.pending_events.push(EngineEvent::ResumeData {
            torrent_id: id,
            payload: payload.to_vec(),
        });
        Ok(())
    }

    async fn reannounce(&mut self, id: Uuid) -> TorrentResult<()> {
        if self.torrents.contains_key(&id) {
            Ok(())
        } else {
            Err(TorrentError::NotFound { torrent_id: id })
        }
    }

    async fn move_torrent(&mut self, id: Uuid, download_dir: &str) -> TorrentResult<()> {
        let torrent = self.torrent_mut(id)?;
        torrent.download_dir = Some(download_dir.to_string());
        self.pending_events.push(EngineEvent::MetadataUpdated {
            torrent_id: id,
            name: None,
            download_dir: Some(download_dir.to_string()),
            comment: None,
            source: None,
            private: None,
        });
        self.refresh_resume(id);
        Ok(())
    }

    async fn recheck(&mut self, id: Uuid) -> TorrentResult<()> {
        if self.torrents.contains_key(&id) {
            Ok(())
        } else {
            Err(TorrentError::NotFound { torrent_id: id })
        }
    }

    async fn peers(&mut self, id: Uuid) -> TorrentResult<Vec<PeerSnapshot>> {
        Ok(self.peer_map.get(&id).cloned().unwrap_or_default())
    }

    async fn poll_events(&mut self) -> TorrentResult<Vec<EngineEvent>> {
        Ok(std::mem::take(&mut self.pending_events))
    }

    async fn apply_config(&mut self, config: &EngineRuntimeConfig) -> TorrentResult<()> {
        let _ = config;
        Ok(())
    }

    async fn inspect_settings(&mut self) -> TorrentResult<EngineSettingsSnapshot> {
        Ok(EngineSettingsSnapshot::default())
    }
}
