//! Event bus routing helpers.

use crate::error::{EventBusError, EventBusResult};
use crate::payloads::{DEFAULT_REPLAY_CAPACITY, Event, EventEnvelope, EventId};
use chrono::Utc;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio_stream::wrappers::BroadcastStream;

/// Stream wrapper used by subscribers.
pub type EventStream = BroadcastStream<EventEnvelope>;

/// Shared event bus built on top of `tokio::broadcast`.
#[derive(Clone)]
pub struct EventBus {
    sender: Sender<EventEnvelope>,
    replay: Arc<Mutex<VecDeque<EventEnvelope>>>,
    replay_capacity: usize,
    next_id: Arc<Mutex<EventId>>,
}

impl EventBus {
    /// Construct a bus with a custom replay capacity.
    #[must_use]
    pub fn with_capacity(replay_capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(replay_capacity);
        Self {
            sender,
            replay: Arc::new(Mutex::new(VecDeque::with_capacity(replay_capacity))),
            replay_capacity,
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    /// Construct a bus with the default replay capacity.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_REPLAY_CAPACITY)
    }

    /// Subscribe to the bus, returning a receiver for new events.
    #[must_use]
    pub fn subscribe(&self, last_event_id: Option<EventId>) -> EventStream {
        let mut rx = self.sender.subscribe();
        if let Some(last) = last_event_id {
            self.replay(last, &mut rx);
        }
        BroadcastStream::new(rx)
    }

    /// Publish a new event to all subscribers.
    ///
    /// # Errors
    ///
    /// Returns an error if the event cannot be delivered to the broadcast channel.
    pub fn send(&self, event: Event) -> EventBusResult<EventId> {
        let kind = event.kind();
        let mut next = self
            .next_id
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = *next;
        *next = next.saturating_add(1);
        drop(next);

        let envelope = EventEnvelope {
            id,
            timestamp: Utc::now(),
            event,
        };
        {
            let mut replay = self.lock_replay();
            if replay.len() == self.replay_capacity {
                replay.pop_front();
            }
            replay.push_back(envelope.clone());
        }
        if self.sender.receiver_count() == 0 {
            return Ok(id);
        }
        if self.sender.send(envelope).is_err() {
            return Err(EventBusError::SendFailed {
                event_id: id,
                event_kind: kind,
            });
        }
        Ok(id)
    }

    /// Publish and return the assigned event id.
    ///
    /// # Errors
    ///
    /// Returns an error if the event cannot be delivered to the broadcast channel.
    pub fn publish(&self, event: Event) -> EventBusResult<EventId> {
        self.send(event)
    }

    /// Last event id observed in the replay buffer.
    #[must_use]
    pub fn last_event_id(&self) -> Option<EventId> {
        self.lock_replay().back().map(|env| env.id)
    }

    /// Collect a backlog of events emitted after the specified id.
    #[must_use]
    pub fn backlog_since(&self, id: EventId) -> Vec<EventEnvelope> {
        let replay = self.lock_replay();
        replay.iter().filter(|env| env.id > id).cloned().collect()
    }

    fn replay(&self, last_event_id: EventId, rx: &mut Receiver<EventEnvelope>) {
        let replay = self.lock_replay();
        let past = replay
            .iter()
            .filter(|env| env.id > last_event_id)
            .cloned()
            .collect::<Vec<_>>();
        drop(replay);
        for env in past {
            match rx.try_recv() {
                Ok(_) | Err(TryRecvError::Empty | TryRecvError::Lagged(_)) => {}
                Err(TryRecvError::Closed) => break,
            }
            if self.sender.send(env).is_err() {
                break;
            }
        }
    }

    fn lock_replay(&self) -> MutexGuard<'_, VecDeque<EventEnvelope>> {
        self.replay
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payloads::Event;
    use std::error::Error;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn publish_and_replay_from_id() -> Result<(), Box<dyn std::error::Error>> {
        let bus = EventBus::with_capacity(4);
        let first = bus.publish(Event::SettingsChanged {
            description: "init".into(),
        })?;
        let second = bus.publish(Event::HealthChanged {
            degraded: vec!["x".into()],
        })?;

        assert_eq!(bus.last_event_id(), Some(second));
        let backlog = bus.backlog_since(first);
        assert_eq!(backlog.len(), 1);
        assert_eq!(backlog[0].id, second);
        Ok(())
    }

    #[tokio::test]
    async fn subscribe_streams_events_and_filters_errors() -> Result<(), Box<dyn Error>> {
        let bus = EventBus::new();
        let mut stream = bus.subscribe(None);
        let id = bus.publish(Event::SelectionReconciled {
            torrent_id: uuid::Uuid::nil(),
            reason: "policy".into(),
        })?;
        let envelope = stream
            .next()
            .await
            .ok_or_else(|| std::io::Error::other("stream item missing"))??;
        assert_eq!(envelope.id, id);
        assert!(matches!(envelope.event, Event::SelectionReconciled { .. }));
        Ok(())
    }
}
