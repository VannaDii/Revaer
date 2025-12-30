//! Event bus error primitives.

use crate::payloads::EventId;
use std::fmt::{self, Display, Formatter};

/// Error emitted when event publishing fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventBusError {
    /// Failed to deliver an event to the broadcast channel.
    SendFailed {
        /// Identifier assigned to the event.
        event_id: EventId,
        /// Event kind string for filtering in logs.
        event_kind: &'static str,
    },
}

impl EventBusError {
    /// Identifier assigned to the event when the failure occurred.
    #[must_use]
    pub const fn event_id(&self) -> EventId {
        match self {
            Self::SendFailed { event_id, .. } => *event_id,
        }
    }

    /// Event kind string associated with the failed delivery.
    #[must_use]
    pub const fn event_kind(&self) -> &'static str {
        match self {
            Self::SendFailed { event_kind, .. } => event_kind,
        }
    }
}

impl Display for EventBusError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("event bus send failed")
    }
}

impl std::error::Error for EventBusError {}

/// Result wrapper for event bus operations.
pub type EventBusResult<T> = Result<T, EventBusError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_bus_error_exposes_fields() {
        let err = EventBusError::SendFailed {
            event_id: 42,
            event_kind: "torrents.updated",
        };

        assert_eq!(err.event_id(), 42);
        assert_eq!(err.event_kind(), "torrents.updated");
        assert_eq!(err.to_string(), "event bus send failed");
    }
}
