//! Libtorrent adapter stub.
//!
//! The adapter is responsible for translating libtorrent state callbacks into the shared
//! workspace event bus so downstream consumers (API/SSE, telemetry) observe real-time changes.

use anyhow::{anyhow, Result};
use revaer_events::{DiscoveredFile, Event, EventBus, TorrentState};
use revaer_torrent_core::{
    AddTorrent, FileSelectionRules, FileSelectionUpdate, RemoveTorrent, TorrentEngine,
    TorrentRateLimit, TorrentSource,
};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

const COMMAND_BUFFER: usize = 128;

/// Thin wrapper around the libtorrent bindings that also emits domain events.
#[derive(Clone)]
pub struct LibtorrentEngine {
    events: EventBus,
    commands: mpsc::Sender<EngineCommand>,
}

impl LibtorrentEngine {
    /// Construct a new engine publisher hooked up to the shared event bus.
    #[must_use]
    pub fn new(events: EventBus) -> Self {
        let (commands, mut rx) = mpsc::channel(COMMAND_BUFFER);
        let worker_events = events.clone();
        tokio::spawn(async move {
            let mut state = WorkerState::new(worker_events);
            while let Some(command) = rx.recv().await {
                if let Err(err) = state.handle(command) {
                    warn!(error = %err, "libtorrent command handling failed");
                }
            }
        });

        Self { events, commands }
    }

    async fn send_command(&self, command: EngineCommand) -> Result<()> {
        self.commands
            .send(command)
            .await
            .map_err(|err| anyhow!("failed to enqueue libtorrent command: {err}"))
    }

    /// Emit a torrent progress update.
    pub fn publish_progress(&self, torrent_id: Uuid, bytes_downloaded: u64, bytes_total: u64) {
        let _ = self.events.publish(Event::Progress {
            torrent_id,
            bytes_downloaded,
            bytes_total,
        });
    }

    /// Emit a torrent state transition.
    pub fn publish_state(&self, torrent_id: Uuid, state: TorrentState) {
        let _ = self
            .events
            .publish(Event::StateChanged { torrent_id, state });
    }

    /// Emit a torrent completion notification and mark the torrent as completed.
    pub fn publish_completed(&self, torrent_id: Uuid, library_path: impl Into<String>) {
        self.publish_state(torrent_id, TorrentState::Completed);
        let _ = self.events.publish(Event::Completed {
            torrent_id,
            library_path: library_path.into(),
        });
    }

    /// Emit the discovered file list when metadata is fetched.
    pub fn publish_files_discovered(&self, torrent_id: Uuid, files: Vec<DiscoveredFile>) {
        if files.is_empty() {
            return;
        }
        let _ = self
            .events
            .publish(Event::FilesDiscovered { torrent_id, files });
    }

    /// Emit a fatal failure state for the torrent.
    pub fn publish_failure(&self, torrent_id: Uuid, message: impl Into<String>) {
        let message = message.into();
        self.publish_state(torrent_id, TorrentState::Failed { message });
    }
}

#[async_trait::async_trait]
impl TorrentEngine for LibtorrentEngine {
    async fn add_torrent(&self, request: AddTorrent) -> Result<()> {
        self.send_command(EngineCommand::Add(request)).await
    }

    async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> Result<()> {
        self.send_command(EngineCommand::Remove { id, options })
            .await
    }

    async fn pause_torrent(&self, id: Uuid) -> Result<()> {
        self.send_command(EngineCommand::Pause { id }).await
    }

    async fn resume_torrent(&self, id: Uuid) -> Result<()> {
        self.send_command(EngineCommand::Resume { id }).await
    }

    async fn set_sequential(&self, id: Uuid, sequential: bool) -> Result<()> {
        self.send_command(EngineCommand::SetSequential { id, sequential })
            .await
    }

    async fn update_limits(&self, id: Option<Uuid>, limits: TorrentRateLimit) -> Result<()> {
        self.send_command(EngineCommand::UpdateLimits { id, limits })
            .await
    }

    async fn update_selection(&self, id: Uuid, rules: FileSelectionUpdate) -> Result<()> {
        self.send_command(EngineCommand::UpdateSelection { id, rules })
            .await
    }

    async fn reannounce(&self, id: Uuid) -> Result<()> {
        self.send_command(EngineCommand::Reannounce { id }).await
    }

    async fn recheck(&self, id: Uuid) -> Result<()> {
        self.send_command(EngineCommand::Recheck { id }).await
    }
}

#[derive(Debug)]
enum EngineCommand {
    Add(AddTorrent),
    Remove {
        id: Uuid,
        options: RemoveTorrent,
    },
    Pause {
        id: Uuid,
    },
    Resume {
        id: Uuid,
    },
    SetSequential {
        id: Uuid,
        sequential: bool,
    },
    UpdateLimits {
        id: Option<Uuid>,
        limits: TorrentRateLimit,
    },
    UpdateSelection {
        id: Uuid,
        rules: FileSelectionUpdate,
    },
    Reannounce {
        id: Uuid,
    },
    Recheck {
        id: Uuid,
    },
}

struct WorkerState {
    events: EventBus,
    torrents: HashMap<Uuid, StubTorrent>,
}

impl WorkerState {
    fn new(events: EventBus) -> Self {
        Self {
            events,
            torrents: HashMap::new(),
        }
    }

    fn handle(&mut self, command: EngineCommand) -> Result<()> {
        match command {
            EngineCommand::Add(request) => {
                let stub = StubTorrent::from_add(&request);
                if self.torrents.insert(request.id, stub).is_some() {
                    warn!(torrent_id = %request.id, "replacing existing torrent in stub engine");
                }
                let name = request
                    .options
                    .name_hint
                    .clone()
                    .unwrap_or_else(|| format!("torrent-{}", request.id));
                let source_desc = match &request.source {
                    TorrentSource::Magnet { uri } => {
                        format!("magnet:{}", uri.chars().take(32).collect::<String>())
                    }
                    TorrentSource::Metainfo { .. } => "metainfo-bytes".to_string(),
                };
                info!(
                    torrent_id = %request.id,
                    torrent_name = %name,
                    source = %source_desc,
                    "libtorrent stub add command processed"
                );
                emit_added(&self.events, &request);
            }
            EngineCommand::Remove { id, options } => {
                if self.torrents.remove(&id).is_some() {
                    info!(
                        torrent_id = %id,
                        with_data = options.with_data,
                        "libtorrent stub remove command processed"
                    );
                    emit_state(&self.events, id, TorrentState::Stopped);
                } else {
                    return Err(anyhow!("unknown torrent {id} for remove command"));
                }
            }
            EngineCommand::Pause { id } => {
                let torrent = self.torrent_mut(id)?;
                torrent.state = TorrentState::Stopped;
                emit_state(&self.events, id, TorrentState::Stopped);
            }
            EngineCommand::Resume { id } => {
                let torrent = self.torrent_mut(id)?;
                torrent.state = TorrentState::Downloading;
                emit_state(&self.events, id, TorrentState::Downloading);
            }
            EngineCommand::SetSequential { id, sequential } => {
                let torrent = self.torrent_mut(id)?;
                torrent.sequential = sequential;
                debug!(torrent_id = %id, sequential, "updated sequential flag");
            }
            EngineCommand::UpdateLimits { id, limits } => {
                if let Some(target) = id {
                    let torrent = self.torrent_mut(target)?;
                    torrent.rate_limit = limits.clone();
                } else {
                    for torrent in self.torrents.values_mut() {
                        torrent.rate_limit = limits.clone();
                    }
                }
                debug!(
                    torrent_id = ?id,
                    download_bps = ?limits.download_bps,
                    upload_bps = ?limits.upload_bps,
                    "updated rate limits"
                );
            }
            EngineCommand::UpdateSelection { id, rules } => {
                let torrent = self.torrent_mut(id)?;
                torrent.selection.include.clone_from(&rules.include);
                torrent.selection.exclude.clone_from(&rules.exclude);
                torrent.selection.skip_fluff = rules.skip_fluff;
                debug!(
                    torrent_id = %id,
                    include = torrent.selection.include.len(),
                    exclude = torrent.selection.exclude.len(),
                    priorities = rules.priorities.len(),
                    skip_fluff = torrent.selection.skip_fluff,
                    "updated file selection"
                );
            }
            EngineCommand::Reannounce { id } => {
                if self.torrents.contains_key(&id) {
                    warn!(torrent_id = %id, "reannounce requested (stubbed)");
                } else {
                    return Err(anyhow!("unknown torrent {id} for reannounce"));
                }
            }
            EngineCommand::Recheck { id } => {
                if self.torrents.contains_key(&id) {
                    warn!(torrent_id = %id, "recheck requested (stubbed)");
                } else {
                    return Err(anyhow!("unknown torrent {id} for recheck"));
                }
            }
        }

        Ok(())
    }

    fn torrent_mut(&mut self, id: Uuid) -> Result<&mut StubTorrent> {
        self.torrents
            .get_mut(&id)
            .ok_or_else(|| anyhow!("unknown torrent {id}"))
    }
}

#[derive(Clone)]
struct StubTorrent {
    selection: FileSelectionRules,
    rate_limit: TorrentRateLimit,
    sequential: bool,
    state: TorrentState,
}

impl StubTorrent {
    fn from_add(request: &AddTorrent) -> Self {
        Self {
            selection: request.options.file_rules.clone(),
            rate_limit: request.options.rate_limit.clone(),
            sequential: request.options.sequential.unwrap_or(false),
            state: TorrentState::Queued,
        }
    }
}

fn emit_state(events: &EventBus, torrent_id: Uuid, state: TorrentState) {
    let _ = events.publish(Event::StateChanged { torrent_id, state });
}

fn emit_added(events: &EventBus, request: &AddTorrent) {
    let name = request
        .options
        .name_hint
        .clone()
        .unwrap_or_else(|| format!("torrent-{}", request.id));
    let _ = events.publish(Event::TorrentAdded {
        torrent_id: request.id,
        name,
    });
    emit_state(events, request.id, TorrentState::Queued);
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use revaer_events::{Event, EventBus};
    use revaer_torrent_core::{AddTorrent, AddTorrentOptions, TorrentSource};
    use tokio::time::{timeout, Duration};

    async fn next_event(stream: &mut revaer_events::EventStream) -> Event {
        timeout(Duration::from_millis(100), stream.next())
            .await
            .expect("timed out waiting for event")
            .expect("event stream closed unexpectedly")
            .event
    }

    #[tokio::test]
    async fn add_torrent_emits_added_and_state_events() -> Result<()> {
        let bus = EventBus::with_capacity(8);
        let engine = LibtorrentEngine::new(bus.clone());
        let descriptor = AddTorrent {
            id: Uuid::new_v4(),
            source: TorrentSource::magnet("magnet:?xt=urn:btih:demo"),
            options: AddTorrentOptions {
                name_hint: Some("ubuntu.iso".to_string()),
                ..AddTorrentOptions::default()
            },
        };
        let mut stream = bus.subscribe(None);

        engine.add_torrent(descriptor.clone()).await?;

        match next_event(&mut stream).await {
            Event::TorrentAdded { torrent_id, name } => {
                assert_eq!(torrent_id, descriptor.id);
                assert_eq!(name, descriptor.options.name_hint.clone().unwrap());
            }
            other => panic!("unexpected event {other:?}"),
        }

        match next_event(&mut stream).await {
            Event::StateChanged { torrent_id, state } => {
                assert_eq!(torrent_id, descriptor.id);
                assert!(matches!(state, TorrentState::Queued));
            }
            other => panic!("unexpected event {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn progress_updates_emit_progress_event() {
        let bus = EventBus::with_capacity(4);
        let engine = LibtorrentEngine::new(bus.clone());
        let torrent_id = Uuid::new_v4();
        let mut stream = bus.subscribe(None);

        engine.publish_progress(torrent_id, 512, 1024);

        match next_event(&mut stream).await {
            Event::Progress {
                torrent_id: id,
                bytes_downloaded,
                bytes_total,
            } => {
                assert_eq!(id, torrent_id);
                assert_eq!(bytes_downloaded, 512);
                assert_eq!(bytes_total, 1024);
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    #[tokio::test]
    async fn completion_emits_state_and_completed_events() {
        let bus = EventBus::with_capacity(8);
        let engine = LibtorrentEngine::new(bus.clone());
        let torrent_id = Uuid::new_v4();
        let mut stream = bus.subscribe(None);

        engine.publish_completed(torrent_id, "/library/path");

        match next_event(&mut stream).await {
            Event::StateChanged {
                torrent_id: id,
                state,
            } => {
                assert_eq!(id, torrent_id);
                assert!(matches!(state, TorrentState::Completed));
            }
            other => panic!("unexpected event {other:?}"),
        }

        match next_event(&mut stream).await {
            Event::Completed {
                torrent_id: id,
                library_path,
            } => {
                assert_eq!(id, torrent_id);
                assert_eq!(library_path, "/library/path");
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    #[tokio::test]
    async fn file_discovery_only_emits_for_non_empty_list() {
        let bus = EventBus::with_capacity(4);
        let engine = LibtorrentEngine::new(bus.clone());
        let torrent_id = Uuid::new_v4();
        let mut stream = bus.subscribe(None);

        engine.publish_files_discovered(torrent_id, Vec::new());
        assert!(
            timeout(Duration::from_millis(50), stream.next())
                .await
                .is_err(),
            "expected no event for empty discovery list"
        );

        engine.publish_files_discovered(
            torrent_id,
            vec![DiscoveredFile {
                path: "movie.mkv".to_string(),
                size_bytes: 42,
            }],
        );

        match next_event(&mut stream).await {
            Event::FilesDiscovered {
                torrent_id: id,
                files,
            } => {
                assert_eq!(id, torrent_id);
                assert_eq!(files.len(), 1);
                assert_eq!(files[0].path, "movie.mkv");
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    #[tokio::test]
    async fn failures_emit_state_change() {
        let bus = EventBus::with_capacity(4);
        let engine = LibtorrentEngine::new(bus.clone());
        let torrent_id = Uuid::new_v4();
        let mut stream = bus.subscribe(None);

        engine.publish_failure(torrent_id, "tracker unreachable");

        match next_event(&mut stream).await {
            Event::StateChanged {
                torrent_id: id,
                state,
            } => {
                assert_eq!(id, torrent_id);
                match state {
                    TorrentState::Failed { message } => {
                        assert!(message.contains("tracker unreachable"));
                    }
                    other => panic!("expected failed state, got {other:?}"),
                }
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    #[tokio::test]
    async fn pause_and_resume_emit_state_changes() -> Result<()> {
        let bus = EventBus::with_capacity(16);
        let engine = LibtorrentEngine::new(bus.clone());
        let mut stream = bus.subscribe(None);

        let descriptor = AddTorrent {
            id: Uuid::new_v4(),
            source: TorrentSource::magnet("magnet:?xt=urn:btih:demo"),
            options: AddTorrentOptions::default(),
        };

        engine.add_torrent(descriptor.clone()).await?;

        let _ = next_event(&mut stream).await; // TorrentAdded
        let _ = next_event(&mut stream).await; // Initial state change

        engine.pause_torrent(descriptor.id).await?;
        match next_event(&mut stream).await {
            Event::StateChanged { torrent_id, state } => {
                assert_eq!(torrent_id, descriptor.id);
                assert!(matches!(state, TorrentState::Stopped));
            }
            other => panic!("unexpected event {other:?}"),
        }

        engine.resume_torrent(descriptor.id).await?;
        match next_event(&mut stream).await {
            Event::StateChanged { torrent_id, state } => {
                assert_eq!(torrent_id, descriptor.id);
                assert!(matches!(state, TorrentState::Downloading));
            }
            other => panic!("unexpected event {other:?}"),
        }

        Ok(())
    }
}
