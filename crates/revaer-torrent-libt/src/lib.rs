#![deny(unsafe_code)]
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

//! Libtorrent adapter implementation backed by the native C++ session bridge.

/// Engine command definitions and shared request types used by the adapter.
pub mod command;
#[cfg(feature = "libtorrent")]
#[allow(unsafe_code)]
pub mod ffi;
/// Session abstraction and native/stub implementations.
pub mod session;
mod store;
/// Background worker that drives the libtorrent session.
pub mod worker;

pub use command::{EncryptionPolicy, EngineRuntimeConfig};
pub use store::{FastResumeStore, StoredTorrentMetadata, StoredTorrentState};

use anyhow::{Result, anyhow};
use command::EngineCommand;
use revaer_events::{DiscoveredFile, Event, EventBus, TorrentState};
use revaer_torrent_core::{
    AddTorrent, FileSelectionUpdate, RemoveTorrent, TorrentEngine, TorrentRateLimit,
};
use tokio::sync::mpsc;
use tracing::warn;
use uuid::Uuid;

const COMMAND_BUFFER: usize = 128;

/// Thin wrapper around the libtorrent bindings that also emits domain events.
#[derive(Clone)]
pub struct LibtorrentEngine {
    events: EventBus,
    commands: mpsc::Sender<EngineCommand>,
    resume_store: Option<FastResumeStore>,
}

impl LibtorrentEngine {
    /// Construct a new engine publisher hooked up to the shared event bus.
    ///
    /// # Errors
    ///
    /// Returns an error if the native libtorrent session cannot be initialised.
    pub fn new(events: EventBus) -> Result<Self> {
        Self::build(events, None)
    }

    /// Construct an engine with a configured fast-resume store.
    ///
    /// # Errors
    ///
    /// Returns an error if the native libtorrent session cannot be initialised.
    pub fn with_resume_store(events: EventBus, store: FastResumeStore) -> Result<Self> {
        Self::build(events, Some(store))
    }

    /// Apply the runtime configuration produced from the active engine profile.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration could not be enqueued for the background worker.
    pub async fn apply_runtime_config(&self, config: EngineRuntimeConfig) -> Result<()> {
        self.send_command(EngineCommand::ApplyConfig(config)).await
    }

    fn build(events: EventBus, store: Option<FastResumeStore>) -> Result<Self> {
        let session = session::create_session()?;
        let (commands, rx) = mpsc::channel(COMMAND_BUFFER);
        let worker_store = store.clone();
        worker::spawn(events.clone(), rx, worker_store, session);

        let engine = Self {
            events,
            commands,
            resume_store: store,
        };

        if let Some(store_ref) = engine.resume_store.as_ref()
            && let Err(err) = store_ref.ensure_initialized()
        {
            warn!(
                error = %err,
                "failed to initialise fast resume store"
            );
        }

        Ok(engine)
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
    use crate::store::FastResumeStore;
    use anyhow::Result;
    use revaer_events::{Event, EventBus};
    use tempfile::TempDir;
    use tokio::time::{Duration, timeout};
    use tokio_stream::StreamExt;

    async fn next_event(stream: &mut revaer_events::EventStream) -> Event {
        let envelope = timeout(Duration::from_millis(100), stream.next())
            .await
            .expect("timed out waiting for event")
            .expect("event stream closed unexpectedly")
            .expect("stream recv error");
        envelope.event
    }

    #[tokio::test]
    async fn progress_updates_emit_progress_event() {
        let bus = EventBus::with_capacity(4);
        let engine = LibtorrentEngine::new(bus.clone()).expect("engine initialisation");
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
        let engine = LibtorrentEngine::new(bus.clone()).expect("engine initialisation");
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
        let engine = LibtorrentEngine::new(bus.clone()).expect("engine initialisation");
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
        let engine = LibtorrentEngine::new(bus.clone()).expect("engine initialisation");
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
    async fn engine_with_resume_store_retains_store() -> Result<()> {
        let bus = EventBus::with_capacity(4);
        let temp = TempDir::new()?;
        let store = FastResumeStore::new(temp.path());

        let engine =
            LibtorrentEngine::with_resume_store(bus, store).expect("engine initialisation");

        assert!(engine.resume_store.is_some());
        engine
            .resume_store
            .as_ref()
            .expect("resume store missing")
            .ensure_initialized()?;

        Ok(())
    }
}
