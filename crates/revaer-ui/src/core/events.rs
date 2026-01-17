//! Event envelope types used by the UI SSE pipeline.
//!
//! # Design
//! - Wrap core event envelopes from `revaer-events` for transport parity.

use revaer_events::{Event as CoreEvent, EventEnvelope as CoreEnvelope, EventId};

/// UI-facing event variants derived from SSE payloads.
#[derive(Clone, Debug, PartialEq)]
pub enum UiEvent {
    /// Core domain event emitted by the backend.
    Core(CoreEvent),
}

/// Normalized SSE envelope for UI reducers.
#[derive(Clone, Debug, PartialEq)]
pub struct UiEventEnvelope {
    /// Monotonic event identifier when available.
    pub id: Option<EventId>,
    /// Timestamp string for diagnostics.
    pub timestamp: String,
    /// Event payload.
    pub event: UiEvent,
}

impl UiEventEnvelope {
    /// Wrap a core envelope from the backend.
    #[must_use]
    pub fn from_core(envelope: CoreEnvelope) -> Self {
        Self {
            id: Some(envelope.id),
            timestamp: envelope.timestamp.to_rfc3339(),
            event: UiEvent::Core(envelope.event),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn from_core_wraps_core_event() {
        let event = CoreEvent::TorrentAdded {
            torrent_id: Uuid::nil(),
            name: "demo".to_string(),
        };
        let envelope = CoreEnvelope {
            id: 42,
            timestamp: Utc::now(),
            event: event.clone(),
        };
        let wrapped = UiEventEnvelope::from_core(envelope.clone());
        assert_eq!(wrapped.id, Some(42));
        assert_eq!(wrapped.timestamp, envelope.timestamp.to_rfc3339());
        assert_eq!(wrapped.event, UiEvent::Core(event));
    }
}
