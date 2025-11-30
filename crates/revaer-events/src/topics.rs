//! Event topic identifiers used across transports.

/// Machine-friendly discriminator for SSE consumers.
#[must_use]
pub const fn event_kind(event: &crate::payloads::Event) -> &'static str {
    event.kind()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payloads::Event;
    use uuid::Uuid;

    #[test]
    fn event_kind_matches_payload() {
        let id = Uuid::nil();
        assert_eq!(
            event_kind(&Event::TorrentAdded {
                torrent_id: id,
                name: "n".into()
            }),
            "torrent_added"
        );
        assert_eq!(
            event_kind(&Event::HealthChanged {
                degraded: vec!["x".into()]
            }),
            "health_changed"
        );
    }
}
