//! Core event bus for the Revaer platform.
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
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
}

impl Event {
    /// Machine-friendly discriminator for SSE consumers.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
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
        }
    }
}

/// Metadata wrapper around events. Each envelope tracks the event id and
/// emission timestamp.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct EventEnvelope {
    pub id: EventId,
    pub timestamp: DateTime<Utc>,
    pub event: Event,
}

/// Individual file discovered within a torrent.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct DiscoveredFile {
    pub path: String,
    pub size_bytes: u64,
}

/// High-level torrent states that downstream consumers care about.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
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
}
