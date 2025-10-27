//! Core event bus for the Revaer platform.
#![allow(clippy::multiple_crate_versions)]
//!
//! The bus provides a typed event enum, sequential identifiers, and support for
//! replaying recent events when subscribers reconnect (e.g. SSE clients that
//! supply `Last-Event-ID`). Internally it uses `tokio::broadcast` with a bounded
//! buffer; when the channel overflows, the oldest events are dropped, matching
//! the desired backpressure behaviour.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use tokio::sync::broadcast;
use tokio::sync::broadcast::{Receiver, Sender};
use uuid::Uuid;

/// Identifier assigned to each event emitted by the platform.
pub type EventId = u64;

/// Default buffer size for the in-memory replay ring.
const DEFAULT_REPLAY_CAPACITY: usize = 1_024;

/// Typed domain events surfaced across the system.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    TorrentAdded {
        torrent_id: Uuid,
        name: String,
    },
    FilesDiscovered {
        torrent_id: Uuid,
        files: Vec<DiscoveredFile>,
    },
    Progress {
        torrent_id: Uuid,
        bytes_downloaded: u64,
        bytes_total: u64,
    },
    StateChanged {
        torrent_id: Uuid,
        state: TorrentState,
    },
    Completed {
        torrent_id: Uuid,
        library_path: String,
    },
    FsopsStarted {
        torrent_id: Uuid,
    },
    FsopsProgress {
        torrent_id: Uuid,
        step: String,
    },
    FsopsCompleted {
        torrent_id: Uuid,
    },
    FsopsFailed {
        torrent_id: Uuid,
        message: String,
    },
    SettingsChanged {
        description: String,
    },
    HealthChanged {
        degraded: Vec<String>,
    },
    SelectionReconciled {
        torrent_id: Uuid,
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
    pub id: EventId,
    pub timestamp: DateTime<Utc>,
    pub event: Event,
}

/// Individual file discovered within a torrent.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DiscoveredFile {
    pub path: String,
    pub size_bytes: u64,
}

/// High-level torrent states that downstream consumers care about.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TorrentState {
    Queued,
    FetchingMetadata,
    Downloading,
    Seeding,
    Completed,
    Failed { message: String },
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
            let mut buffer = self.buffer.lock().expect("event buffer mutex poisoned");
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
            let buffer = self.buffer.lock().expect("event buffer mutex poisoned");
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
        let buffer = self.buffer.lock().expect("event buffer mutex poisoned");
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
        let buffer = self.buffer.lock().expect("event buffer mutex poisoned");
        buffer.back().map(|event| event.id)
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
