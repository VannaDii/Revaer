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
    clippy::cargo,
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::multiple_crate_versions)]
#![allow(unexpected_cfgs)]

//! Core event bus for the Revaer platform.
//!
//! The bus provides a typed event enum, sequential identifiers, and support for
//! replaying recent events when subscribers reconnect (e.g. SSE clients that
//! supply `Last-Event-ID`). Internally it uses `tokio::broadcast` with a bounded
//! buffer; when the channel overflows, the oldest events are dropped, matching
//! the desired backpressure behaviour.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::{DateTime, Utc};
use tokio::sync::broadcast;
use tokio::sync::broadcast::{Receiver, Sender};
use tracing::error;
use uuid::Uuid;

/// Identifier assigned to each event emitted by the platform.
pub type EventId = u64;

/// Default buffer size for the in-memory replay ring.
const DEFAULT_REPLAY_CAPACITY: usize = 1_024;

/// Typed domain events surfaced across the system.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// A torrent was registered with the engine.
    TorrentAdded {
        /// Identifier for the torrent that was added.
        torrent_id: Uuid,
        /// Display name associated with the torrent metadata.
        name: String,
    },
    /// Torrent metadata finished scanning and produced a file listing.
    FilesDiscovered {
        /// Identifier for the torrent that produced the listing.
        torrent_id: Uuid,
        /// Collection of files discovered within the torrent payload.
        files: Vec<DiscoveredFile>,
    },
    /// Periodic progress update emitted while a torrent is downloading.
    Progress {
        /// Identifier for the torrent being tracked.
        torrent_id: Uuid,
        /// Aggregate bytes downloaded so far.
        bytes_downloaded: u64,
        /// Total bytes expected for the torrent payload.
        bytes_total: u64,
    },
    /// Torrent transitioned into a new lifecycle state.
    StateChanged {
        /// Identifier for the torrent whose state changed.
        torrent_id: Uuid,
        /// Updated state snapshot.
        state: TorrentState,
    },
    /// Torrent finished processing and the library artifact is ready.
    Completed {
        /// Identifier for the completed torrent.
        torrent_id: Uuid,
        /// Absolute path to the final library artifact.
        library_path: String,
    },
    /// Torrent was removed from the catalog.
    TorrentRemoved {
        /// Identifier for the torrent that was removed.
        torrent_id: Uuid,
    },
    /// Filesystem post-processing pipeline started for a torrent.
    FsopsStarted {
        /// Identifier for the torrent undergoing filesystem processing.
        torrent_id: Uuid,
    },
    /// Filesystem post-processing reported an intermediate step completion.
    FsopsProgress {
        /// Identifier for the torrent undergoing filesystem processing.
        torrent_id: Uuid,
        /// Name of the pipeline step that completed.
        step: String,
    },
    /// Filesystem post-processing completed successfully.
    FsopsCompleted {
        /// Identifier for the torrent whose filesystem processing completed.
        torrent_id: Uuid,
    },
    /// Filesystem post-processing failed with an error message.
    FsopsFailed {
        /// Identifier for the torrent whose filesystem processing failed.
        torrent_id: Uuid,
        /// Human-readable error detail describing the failure.
        message: String,
    },
    /// Configuration update was applied.
    SettingsChanged {
        /// Description of the applied configuration change.
        description: String,
    },
    /// System health status changed (degraded or restored components).
    HealthChanged {
        /// Components currently considered degraded.
        degraded: Vec<String>,
    },
    /// Torrent file selection was reconciled with the configured policy.
    SelectionReconciled {
        /// Identifier for the torrent whose selection changed.
        torrent_id: Uuid,
        /// Explanation for why the selection changed.
        reason: String,
    },
}

impl Event {
    /// Machine-friendly discriminator for SSE consumers.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::TorrentAdded { .. } => "torrent_added",
            Self::FilesDiscovered { .. } => "files_discovered",
            Self::Progress { .. } => "progress",
            Self::StateChanged { .. } => "state_changed",
            Self::Completed { .. } => "completed",
            Self::TorrentRemoved { .. } => "torrent_removed",
            Self::FsopsStarted { .. } => "fsops_started",
            Self::FsopsProgress { .. } => "fsops_progress",
            Self::FsopsCompleted { .. } => "fsops_completed",
            Self::FsopsFailed { .. } => "fsops_failed",
            Self::SettingsChanged { .. } => "settings_changed",
            Self::HealthChanged { .. } => "health_changed",
            Self::SelectionReconciled { .. } => "selection_reconciled",
        }
    }
}

/// Metadata wrapper around events. Each envelope tracks the event id and
/// emission timestamp.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EventEnvelope {
    /// Monotonic identifier assigned to the wrapped event.
    pub id: EventId,
    /// Timestamp recording when the envelope was produced.
    pub timestamp: DateTime<Utc>,
    /// Wrapped event payload.
    pub event: Event,
}

/// Individual file discovered within a torrent.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DiscoveredFile {
    /// Relative path to the file inside the torrent contents.
    pub path: String,
    /// Size of the file in bytes.
    pub size_bytes: u64,
}

/// High-level torrent states that downstream consumers care about.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TorrentState {
    /// Torrent has been queued but not yet started.
    Queued,
    /// Torrent is fetching metadata (e.g., magnet resolution).
    FetchingMetadata,
    /// Torrent is actively downloading payload data.
    Downloading,
    /// Torrent is seeding and uploading data to peers.
    Seeding,
    /// Torrent completed downloading and awaits post-processing.
    Completed,
    /// Torrent encountered an unrecoverable error with a description.
    Failed {
        /// Error detail describing why the torrent failed.
        message: String,
    },
    /// Torrent has been stopped manually and is inactive.
    Stopped,
}

/// Shared event bus built on top of `tokio::broadcast`.
#[derive(Clone)]
pub struct EventBus {
    sender: Sender<EventEnvelope>,
    buffer: Arc<Mutex<VecDeque<EventEnvelope>>>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
    replay_capacity: usize,
}

impl EventBus {
    /// Construct a new bus with the provided broadcast capacity.
    ///
    /// The broadcast channel uses the same capacity as the in-memory replay
    /// buffer, ensuring dropped events impact both structures consistently.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is zero.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0, "event bus capacity must be positive");
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            buffer: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            next_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            replay_capacity: capacity,
        }
    }

    /// Construct a bus with the default in-memory buffer size.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_REPLAY_CAPACITY)
    }

    /// Publish a new event to the bus, assigning it a sequential identifier.
    ///
    /// # Panics
    ///
    /// Panics if the replay buffer mutex has been poisoned.
    #[must_use]
    pub fn publish(&self, event: Event) -> EventId {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let envelope = EventEnvelope {
            id,
            timestamp: Utc::now(),
            event,
        };

        {
            let mut buffer = self.lock_buffer();
            if buffer.len() == self.replay_capacity {
                buffer.pop_front();
            }
            buffer.push_back(envelope.clone());
        }

        let _ = self.sender.send(envelope);
        id
    }

    /// Subscribe to the bus, replaying any buffered events newer than `since_id`.
    ///
    /// # Panics
    ///
    /// Panics if the replay buffer mutex has been poisoned.
    #[must_use]
    pub fn subscribe(&self, since_id: Option<EventId>) -> EventStream {
        let mut backlog = VecDeque::new();
        if let Some(since) = since_id {
            let buffer = self.lock_buffer();
            for item in buffer.iter() {
                if item.id > since {
                    backlog.push_back(item.clone());
                }
            }
        }

        let receiver = self.sender.subscribe();
        EventStream { backlog, receiver }
    }

    /// Return a snapshot of buffered events newer than the supplied identifier.
    ///
    /// This is useful for endpoints that need incremental views without
    /// establishing a long-lived subscription.
    ///
    /// # Panics
    ///
    /// Panics if the replay buffer mutex has been poisoned.
    #[must_use]
    pub fn backlog_since(&self, since_id: EventId) -> Vec<EventEnvelope> {
        let buffer = self.lock_buffer();
        buffer
            .iter()
            .filter(|item| item.id > since_id)
            .cloned()
            .collect()
    }

    /// Returns the last assigned identifier, if any events have been published.
    ///
    /// # Panics
    ///
    /// Panics if the replay buffer mutex has been poisoned.
    #[must_use]
    pub fn last_event_id(&self) -> Option<EventId> {
        let buffer = self.lock_buffer();
        buffer.back().map(|event| event.id)
    }

    fn lock_buffer(&self) -> MutexGuard<'_, VecDeque<EventEnvelope>> {
        match self.buffer.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                error!("event buffer mutex poisoned; continuing with recovered guard");
                poisoned.into_inner()
            }
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Stream wrapper that yields events either from the replay backlog or from the
/// live broadcast channel.
pub struct EventStream {
    backlog: VecDeque<EventEnvelope>,
    receiver: Receiver<EventEnvelope>,
}

impl EventStream {
    /// Receive the next event, respecting the replay backlog first.
    pub async fn next(&mut self) -> Option<EventEnvelope> {
        if let Some(event) = self.backlog.pop_front() {
            return Some(event);
        }

        match self.receiver.recv().await {
            Ok(event) => Some(event),
            Err(broadcast::error::RecvError::Lagged(_)) => self.receiver.recv().await.ok(),
            Err(broadcast::error::RecvError::Closed) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::time::Duration;
    use tokio::task;
    use tokio::time::timeout;

    const PUBLISH_TIMEOUT: Duration = Duration::from_secs(1);

    #[test]
    fn event_kinds_cover_all_variants() {
        let torrent_id = Uuid::new_v4();
        let files = vec![DiscoveredFile {
            path: "demo.mkv".to_string(),
            size_bytes: 42,
        }];
        let events = [
            Event::TorrentAdded {
                torrent_id,
                name: "demo".to_string(),
            },
            Event::FilesDiscovered { torrent_id, files },
            Event::Progress {
                torrent_id,
                bytes_downloaded: 10,
                bytes_total: 100,
            },
            Event::StateChanged {
                torrent_id,
                state: TorrentState::Downloading,
            },
            Event::Completed {
                torrent_id,
                library_path: "/library/demo".to_string(),
            },
            Event::TorrentRemoved { torrent_id },
            Event::FsopsStarted { torrent_id },
            Event::FsopsProgress {
                torrent_id,
                step: "stage".to_string(),
            },
            Event::FsopsCompleted { torrent_id },
            Event::FsopsFailed {
                torrent_id,
                message: "fail".to_string(),
            },
            Event::SettingsChanged {
                description: "updated".to_string(),
            },
            Event::HealthChanged {
                degraded: vec!["config".to_string()],
            },
            Event::SelectionReconciled {
                torrent_id,
                reason: "metadata".to_string(),
            },
        ];

        for event in events {
            let expected = match &event {
                Event::TorrentAdded { .. } => "torrent_added",
                Event::FilesDiscovered { .. } => "files_discovered",
                Event::Progress { .. } => "progress",
                Event::StateChanged { .. } => "state_changed",
                Event::Completed { .. } => "completed",
                Event::TorrentRemoved { .. } => "torrent_removed",
                Event::FsopsStarted { .. } => "fsops_started",
                Event::FsopsProgress { .. } => "fsops_progress",
                Event::FsopsCompleted { .. } => "fsops_completed",
                Event::FsopsFailed { .. } => "fsops_failed",
                Event::SettingsChanged { .. } => "settings_changed",
                Event::HealthChanged { .. } => "health_changed",
                Event::SelectionReconciled { .. } => "selection_reconciled",
            };
            assert_eq!(event.kind(), expected);
        }
    }

    fn sample_progress_event(id: usize) -> Event {
        Event::Progress {
            torrent_id: Uuid::from_u128(id as u128 + 1),
            bytes_downloaded: (id * 1_000) as u64,
            bytes_total: 500_000,
        }
    }

    #[tokio::test]
    async fn sequential_ids_and_replay() {
        let bus = EventBus::with_capacity(16);

        let mut last_id = 0;
        for i in 0..5 {
            last_id = bus.publish(sample_progress_event(i));
        }
        assert_eq!(last_id, 5);

        let mut stream = bus.subscribe(Some(2));
        let mut received = Vec::new();
        for _ in 0..3 {
            if let Some(event) = stream.next().await {
                received.push(event);
            }
        }

        assert_eq!(received.len(), 3);
        assert_eq!(received.first().unwrap().id, 3);
        assert_eq!(received.last().unwrap().id, 5);
    }

    #[tokio::test]
    async fn load_test_does_not_stall_publishers() {
        let bus = Arc::new(EventBus::with_capacity(512));
        let mut stream = bus.subscribe(None);

        let publisher = {
            let bus = bus.clone();
            task::spawn(async move {
                for i in 0..500 {
                    let publish_bus = bus.clone();
                    timeout(PUBLISH_TIMEOUT, async move {
                        let _ = publish_bus.publish(sample_progress_event(i));
                    })
                    .await
                    .expect("publish timed out");
                }
            })
        };

        let consumer = task::spawn(async move {
            let mut ids = HashSet::new();
            while ids.len() < 500 {
                if let Some(event) = stream.next().await {
                    ids.insert(event.id);
                }
            }
            ids
        });

        publisher.await.expect("publisher task panicked");
        let ids = consumer.await.expect("consumer task panicked");
        assert_eq!(ids.len(), 500);
    }

    #[tokio::test]
    async fn last_event_id_reflects_recent_publish() {
        let bus = EventBus::with_capacity(2);
        assert!(bus.last_event_id().is_none(), "no events published yet");
        let published = bus.publish(sample_progress_event(0));
        assert_eq!(bus.last_event_id(), Some(published));
    }

    #[tokio::test]
    async fn subscribe_without_since_replays_all() {
        let bus = EventBus::with_capacity(4);
        for i in 0..3 {
            let _ = bus.publish(sample_progress_event(i));
        }
        let mut stream = bus.subscribe(Some(0));
        let mut collected = Vec::new();
        for expected_id in 1..=3 {
            collected.push(
                timeout(PUBLISH_TIMEOUT, stream.next())
                    .await
                    .expect("stream stalled")
                    .expect("stream closed"),
            );
            assert_eq!(collected.last().unwrap().id, expected_id);
        }
        assert_eq!(collected.len(), 3);
    }

    #[tokio::test]
    async fn stream_returns_none_after_sender_dropped() {
        let mut stream = {
            let bus = EventBus::with_capacity(1);
            let stream = bus.subscribe(None);
            drop(bus);
            stream
        };
        assert!(
            stream.next().await.is_none(),
            "closing the sender should end the stream"
        );
    }
}
