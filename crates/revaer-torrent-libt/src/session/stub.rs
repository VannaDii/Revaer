use std::collections::HashMap;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use revaer_events::TorrentState;
use revaer_torrent_core::{
    AddTorrent, EngineEvent, FileSelectionUpdate, RemoveTorrent, TorrentRateLimit,
};
use serde_json::json;
use uuid::Uuid;

use super::LibTorrentSession;
use crate::types::EngineRuntimeConfig;
use revaer_torrent_core::{FilePriorityOverride, FileSelectionRules};

/// In-memory test double for the libtorrent session interface.
#[derive(Default)]
pub struct StubSession {
    torrents: HashMap<Uuid, StubTorrent>,
    pending_events: Vec<EngineEvent>,
}

#[derive(Clone)]
struct StubTorrent {
    selection: FileSelectionRules,
    priorities: Vec<FilePriorityOverride>,
    rate_limit: TorrentRateLimit,
    sequential: bool,
    state: TorrentState,
    download_dir: Option<String>,
    connections_limit: Option<i32>,
    resume_payload: Option<Vec<u8>>,
}

impl StubTorrent {
    fn from_add(request: &AddTorrent) -> Self {
        Self {
            selection: request.options.file_rules.clone(),
            priorities: Vec::new(),
            rate_limit: request.options.rate_limit.clone(),
            sequential: request.options.sequential.unwrap_or(false),
            state: if request.options.start_paused.unwrap_or(false) {
                TorrentState::Stopped
            } else {
                TorrentState::Queued
            },
            download_dir: request.options.download_dir.clone(),
            connections_limit: request.options.connections_limit,
            resume_payload: None,
        }
    }
}

impl StubSession {
    fn torrent_mut(&mut self, id: Uuid) -> Result<&mut StubTorrent> {
        self.torrents
            .get_mut(&id)
            .ok_or_else(|| anyhow!("unknown torrent {id}"))
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
                "connections_limit": torrent.connections_limit,
                "download_dir": torrent.download_dir,
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
    async fn add_torrent(&mut self, request: &AddTorrent) -> Result<()> {
        let torrent = StubTorrent::from_add(request);
        let download_dir = torrent.download_dir.clone();
        self.torrents.insert(request.id, torrent);
        let initial_state = if request.options.start_paused.unwrap_or(false) {
            TorrentState::Stopped
        } else {
            TorrentState::Queued
        };
        self.push_state(request.id, initial_state);
        self.pending_events.push(EngineEvent::MetadataUpdated {
            torrent_id: request.id,
            name: request.options.name_hint.clone(),
            download_dir,
        });
        self.refresh_resume(request.id);
        Ok(())
    }

    async fn remove_torrent(&mut self, id: Uuid, _options: &RemoveTorrent) -> Result<()> {
        if self.torrents.remove(&id).is_some() {
            self.push_state(id, TorrentState::Stopped);
            self.pending_events.push(EngineEvent::ResumeData {
                torrent_id: id,
                payload: Vec::new(),
            });
            Ok(())
        } else {
            Err(anyhow!("unknown torrent {id} for remove command"))
        }
    }

    async fn pause_torrent(&mut self, id: Uuid) -> Result<()> {
        let torrent = self.torrent_mut(id)?;
        torrent.state = TorrentState::Stopped;
        self.push_state(id, TorrentState::Stopped);
        self.refresh_resume(id);
        Ok(())
    }

    async fn resume_torrent(&mut self, id: Uuid) -> Result<()> {
        let torrent = self.torrent_mut(id)?;
        torrent.state = TorrentState::Downloading;
        self.push_state(id, TorrentState::Downloading);
        self.refresh_resume(id);
        Ok(())
    }

    async fn set_sequential(&mut self, id: Uuid, sequential: bool) -> Result<()> {
        let torrent = self.torrent_mut(id)?;
        torrent.sequential = sequential;
        self.refresh_resume(id);
        Ok(())
    }

    async fn update_limits(&mut self, id: Option<Uuid>, limits: &TorrentRateLimit) -> Result<()> {
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

    async fn update_selection(&mut self, id: Uuid, rules: &FileSelectionUpdate) -> Result<()> {
        let torrent = self.torrent_mut(id)?;
        torrent.selection.include.clone_from(&rules.include);
        torrent.selection.exclude.clone_from(&rules.exclude);
        torrent.selection.skip_fluff = rules.skip_fluff;
        torrent.priorities.clone_from(&rules.priorities);
        self.refresh_resume(id);
        Ok(())
    }

    async fn load_fastresume(&mut self, id: Uuid, payload: &[u8]) -> Result<()> {
        let torrent = self.torrent_mut(id)?;
        torrent.resume_payload = Some(payload.to_vec());
        self.pending_events.push(EngineEvent::ResumeData {
            torrent_id: id,
            payload: payload.to_vec(),
        });
        Ok(())
    }

    async fn reannounce(&mut self, id: Uuid) -> Result<()> {
        if self.torrents.contains_key(&id) {
            Ok(())
        } else {
            Err(anyhow!("unknown torrent {id} for reannounce"))
        }
    }

    async fn recheck(&mut self, id: Uuid) -> Result<()> {
        if self.torrents.contains_key(&id) {
            Ok(())
        } else {
            Err(anyhow!("unknown torrent {id} for recheck"))
        }
    }

    async fn poll_events(&mut self) -> Result<Vec<EngineEvent>> {
        Ok(std::mem::take(&mut self.pending_events))
    }

    async fn apply_config(&mut self, _config: &EngineRuntimeConfig) -> Result<()> {
        Ok(())
    }
}
