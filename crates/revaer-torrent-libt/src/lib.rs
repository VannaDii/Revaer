//! Libtorrent adapter stub.
//!
//! The adapter is responsible for translating libtorrent state callbacks into the shared
//! workspace event bus so downstream consumers (API/SSE, telemetry) observe real-time changes.

use anyhow::Result;
use revaer_events::{DiscoveredFile, Event, EventBus, TorrentState};
use revaer_torrent_core::{TorrentDescriptor, TorrentEngine};
use tracing::info;
use uuid::Uuid;

/// Thin wrapper around the libtorrent bindings that also emits domain events.
#[derive(Clone)]
pub struct LibtorrentEngine {
    events: EventBus,
}

impl LibtorrentEngine {
    /// Construct a new engine publisher hooked up to the shared event bus.
    #[must_use]
    pub fn new(events: EventBus) -> Self {
        Self { events }
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

    fn publish_added(&self, descriptor: &TorrentDescriptor) {
        let _ = self.events.publish(Event::TorrentAdded {
            torrent_id: descriptor.id,
            name: descriptor.name.clone(),
        });
        self.publish_state(descriptor.id, TorrentState::Queued);
    }

    fn publish_stopped(&self, torrent_id: Uuid) {
        self.publish_state(torrent_id, TorrentState::Stopped);
    }
}

#[async_trait::async_trait]
impl TorrentEngine for LibtorrentEngine {
    async fn add_torrent(&self, descriptor: TorrentDescriptor) -> Result<()> {
        info!("Pretend to add torrent {}", descriptor.name);
        self.publish_added(&descriptor);
        Ok(())
    }

    async fn remove_torrent(&self, id: Uuid) -> Result<()> {
        info!("Pretend to remove torrent {}", id);
        self.publish_stopped(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use revaer_events::{Event, EventBus};
    use revaer_torrent_core::TorrentDescriptor;
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
        let descriptor = TorrentDescriptor {
            id: Uuid::new_v4(),
            name: "ubuntu.iso".to_string(),
        };
        let mut stream = bus.subscribe(None);

        engine.add_torrent(descriptor.clone()).await?;

        match next_event(&mut stream).await {
            Event::TorrentAdded { torrent_id, name } => {
                assert_eq!(torrent_id, descriptor.id);
                assert_eq!(name, descriptor.name);
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
}
