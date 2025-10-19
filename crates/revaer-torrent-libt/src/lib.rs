//! Libtorrent adapter scaffold.
//!
//! Once the real libtorrent bindings are wired in, the engine will translate
//! session events into the shared workspace event bus so downstream consumers
//! (API/SSE, telemetry) observe real-time changes.

mod command;
mod session;
mod store;
mod worker;

pub use store::{FastResumeStore, StoredTorrentMetadata, StoredTorrentState};

use anyhow::{Result, anyhow};
use command::EngineCommand;
use revaer_events::{DiscoveredFile, Event, EventBus, TorrentState};
use revaer_torrent_core::{
    AddTorrent, FileSelectionUpdate, RemoveTorrent, TorrentEngine, TorrentRateLimit,
};
use session::{LibtSession, StubSession};
use tokio::sync::mpsc;
use uuid::Uuid;

const COMMAND_BUFFER: usize = 128;

/// Thin wrapper around the libtorrent bindings that also emits domain events.
#[derive(Clone)]
pub struct LibtorrentEngine {
    events: EventBus,
    commands: mpsc::Sender<EngineCommand>,
    #[allow(dead_code)]
    resume_store: Option<FastResumeStore>,
}

impl LibtorrentEngine {
    /// Construct a new engine publisher hooked up to the shared event bus.
    #[must_use]
    pub fn new(events: EventBus) -> Self {
        Self::build(events, None, Box::new(StubSession::default()))
    }

    /// Construct an engine with a configured fast-resume store.
    #[must_use]
    pub fn with_resume_store(events: EventBus, store: FastResumeStore) -> Self {
        Self::build(events, Some(store), Box::new(StubSession::default()))
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn with_custom_session(
        events: EventBus,
        store: Option<FastResumeStore>,
        session: Box<dyn LibtSession>,
    ) -> Self {
        Self::build(events, store, session)
    }

    fn build(
        events: EventBus,
        store: Option<FastResumeStore>,
        session: Box<dyn LibtSession>,
    ) -> Self {
        let (commands, rx) = mpsc::channel(COMMAND_BUFFER);
        let worker_store = store.clone();
        // TODO: swap the stub implementation with the real libtorrent session wiring.
        worker::spawn(events.clone(), rx, worker_store, session);

        Self {
            events,
            commands,
            resume_store: store,
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{FastResumeStore, StoredTorrentMetadata};
    use anyhow::Result;
    use chrono::Utc;
    use revaer_events::{Event, EventBus};
    use revaer_torrent_core::{
        AddTorrent, AddTorrentOptions, FilePriority, FilePriorityOverride, FileSelectionRules,
        FileSelectionUpdate, RemoveTorrent, TorrentSource,
    };
    use std::fs;
    use tempfile::TempDir;
    use tokio::time::{Duration, sleep, timeout};

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
    async fn resume_reconciliation_emits_selection_event() -> Result<()> {
        let temp = TempDir::new()?;
        let store = FastResumeStore::new(temp.path());
        let torrent_id = Uuid::new_v4();

        let metadata = StoredTorrentMetadata {
            selection: FileSelectionRules {
                include: vec!["movies/**".to_string()],
                exclude: vec!["extras/**".to_string()],
                skip_fluff: true,
            },
            priorities: vec![FilePriorityOverride {
                index: 0,
                priority: FilePriority::High,
            }],
            download_dir: Some("/persisted/downloads".to_string()),
            sequential: true,
            updated_at: Utc::now(),
        };
        store.write_metadata(torrent_id, &metadata)?;
        store.write_fastresume(torrent_id, br#"{"resume":"data"}"#)?;

        let bus = EventBus::with_capacity(64);
        let bus_for_engine = bus.clone();
        let store_for_engine = store.clone();
        let engine = LibtorrentEngine::with_resume_store(bus_for_engine, store_for_engine);
        let mut stream = bus.subscribe(None);

        let descriptor = AddTorrent {
            id: torrent_id,
            source: TorrentSource::magnet("magnet:?xt=urn:btih:demo"),
            options: AddTorrentOptions::default(),
        };

        engine.add_torrent(descriptor).await?;

        let mut reconciled = false;
        for _ in 0..10 {
            let event = next_event(&mut stream).await;
            if let Event::SelectionReconciled { torrent_id: id, .. } = event {
                if id == torrent_id {
                    reconciled = true;
                    break;
                }
            }
        }
        assert!(
            reconciled,
            "expected selection reconciliation when resume metadata differs"
        );

        // Allow the worker to flush metadata writes.
        sleep(Duration::from_millis(50)).await;

        let persisted = store
            .load_all()?
            .into_iter()
            .find(|state| state.torrent_id == torrent_id)
            .expect("metadata persisted");
        let persisted_meta = persisted.metadata.expect("metadata present");
        assert!(persisted_meta.sequential);
        assert_eq!(persisted_meta.selection.include, metadata.selection.include);
        assert_eq!(persisted_meta.priorities.len(), metadata.priorities.len());

        Ok(())
    }

    #[tokio::test]
    async fn resume_data_persisted_on_engine_events() -> Result<()> {
        let temp = TempDir::new()?;
        let store = FastResumeStore::new(temp.path());
        let bus = EventBus::with_capacity(32);
        let engine = {
            let bus_for_engine = bus.clone();
            let store_for_engine = store.clone();
            LibtorrentEngine::with_resume_store(bus_for_engine, store_for_engine)
        };
        let torrent_id = Uuid::new_v4();
        let mut stream = bus.subscribe(None);

        let descriptor = AddTorrent {
            id: torrent_id,
            source: TorrentSource::magnet("magnet:?xt=urn:btih:resume"),
            options: AddTorrentOptions::default(),
        };

        engine.add_torrent(descriptor).await?;
        // Drain the initial torrent_added/state_changed events.
        let _ = next_event(&mut stream).await;
        let _ = next_event(&mut stream).await;

        sleep(Duration::from_millis(50)).await;

        let state = store
            .load_all()?
            .into_iter()
            .find(|entry| entry.torrent_id == torrent_id)
            .expect("resume state persisted");
        let payload = state.fastresume.expect("fastresume payload recorded");
        assert!(!payload.is_empty(), "expected non-empty fastresume payload");

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
    async fn remove_torrent_emits_stopped_event() -> Result<()> {
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

        engine
            .remove_torrent(descriptor.id, RemoveTorrent { with_data: true })
            .await?;

        match next_event(&mut stream).await {
            Event::StateChanged { torrent_id, state } => {
                assert_eq!(torrent_id, descriptor.id);
                assert!(matches!(state, TorrentState::Stopped));
            }
            other => panic!("unexpected event {other:?}"),
        }

        Ok(())
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

    #[tokio::test]
    async fn engine_with_resume_store_retains_store() -> Result<()> {
        let bus = EventBus::with_capacity(4);
        let temp = TempDir::new()?;
        let store = FastResumeStore::new(temp.path());

        let engine = LibtorrentEngine::with_resume_store(bus, store);

        assert!(engine.resume_store.is_some());
        engine
            .resume_store
            .as_ref()
            .expect("resume store missing")
            .ensure_initialized()?;

        Ok(())
    }

    #[tokio::test]
    async fn metadata_persists_on_add_and_selection_updates() -> Result<()> {
        let bus = EventBus::with_capacity(32);
        let temp = TempDir::new()?;
        let store = FastResumeStore::new(temp.path());
        let engine = {
            let bus_for_engine = bus.clone();
            LibtorrentEngine::with_resume_store(bus_for_engine, store)
        };
        let torrent_id = Uuid::new_v4();

        let descriptor = AddTorrent {
            id: torrent_id,
            source: TorrentSource::magnet("magnet:?xt=urn:btih:demo"),
            options: AddTorrentOptions {
                name_hint: Some("demo.torrent".into()),
                download_dir: Some("/downloads".into()),
                sequential: Some(true),
                file_rules: FileSelectionRules {
                    include: vec!["**/*.mkv".into()],
                    exclude: vec![],
                    skip_fluff: true,
                },
                ..AddTorrentOptions::default()
            },
        };
        let metadata_path = temp.path().join(format!("{torrent_id}.meta.json"));
        let mut stream = bus.subscribe(None);

        engine.add_torrent(descriptor.clone()).await?;
        // Wait for add + initial state to ensure worker processed command.
        let _ = next_event(&mut stream).await;
        let _ = next_event(&mut stream).await;
        sleep(Duration::from_millis(50)).await;

        let initial = fs::read_to_string(&metadata_path)?;
        let mut metadata: StoredTorrentMetadata = serde_json::from_str(&initial)?;
        assert_eq!(
            metadata.selection.include,
            descriptor.options.file_rules.include
        );
        assert_eq!(metadata.download_dir.as_deref(), Some("/downloads"));
        assert!(metadata.sequential);

        let update = FileSelectionUpdate {
            include: vec!["Season1/**".into()],
            exclude: vec!["**/extras/**".into()],
            skip_fluff: false,
            priorities: vec![FilePriorityOverride {
                index: 0,
                priority: FilePriority::High,
            }],
        };

        engine.update_selection(torrent_id, update.clone()).await?;
        sleep(Duration::from_millis(50)).await;

        let after = fs::read_to_string(&metadata_path)?;
        metadata = serde_json::from_str(&after)?;
        assert_eq!(metadata.selection.include, update.include);
        assert_eq!(metadata.selection.exclude, update.exclude);
        assert_eq!(metadata.priorities.len(), 1);
        assert!(!metadata.selection.skip_fluff);

        engine.set_sequential(torrent_id, false).await?;
        sleep(Duration::from_millis(50)).await;
        let after_seq = fs::read_to_string(&metadata_path)?;
        metadata = serde_json::from_str(&after_seq)?;
        assert!(!metadata.sequential);

        Ok(())
    }
}
