//! Event envelope types used by the UI SSE pipeline.
//!
//! # Design
//! - Wrap core event envelopes from `revaer-events` for transport parity.
//! - Allow UI-only system rate updates from legacy payloads.

use revaer_events::{Event as CoreEvent, EventEnvelope as CoreEnvelope, EventId};

/// UI-facing event variants derived from SSE payloads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiEvent {
    /// Core domain event emitted by the backend.
    Core(CoreEvent),
    /// Aggregate system rate snapshot (legacy payloads only).
    SystemRates {
        /// Aggregate download rate in bytes per second.
        download_bps: u64,
        /// Aggregate upload rate in bytes per second.
        upload_bps: u64,
    },
}

/// Normalized SSE envelope for UI reducers.
#[derive(Clone, Debug, PartialEq, Eq)]
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

    /// Build a legacy envelope for non-core SSE payloads.
    #[must_use]
    pub fn legacy(event: UiEvent, id: Option<EventId>) -> Self {
        Self {
            id,
            timestamp: "legacy".to_string(),
            event,
        }
    }
}
