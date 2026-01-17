//! Server-sent events filters and streaming helpers.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use async_stream::stream;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::sse::{self, Sse},
};
use futures_util::{StreamExt, future};
use revaer_events::{Event as CoreEvent, EventBus, EventEnvelope, EventId};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};
use uuid::Uuid;

use crate::app::state::ApiState;
use crate::http::constants::{EVENT_KIND_WHITELIST, HEADER_LAST_EVENT_ID, SSE_KEEP_ALIVE_SECS};
use crate::http::errors::ApiError;
use crate::http::torrents::{parse_state_filter, split_comma_separated};
use crate::models::TorrentStateKind;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct SseQuery {
    #[serde(default)]
    pub(crate) torrent: Option<String>,
    #[serde(default)]
    pub(crate) event: Option<String>,
    #[serde(default)]
    pub(crate) state: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct SseFilter {
    pub(crate) torrent_ids: std::collections::HashSet<Uuid>,
    pub(crate) event_kinds: std::collections::HashSet<String>,
    pub(crate) states: std::collections::HashSet<TorrentStateKind>,
}

pub(crate) async fn stream_events(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<SseQuery>,
) -> Result<Sse<impl futures_core::Stream<Item = Result<sse::Event, Infallible>> + Send>, ApiError>
{
    let last_id = headers
        .get(HEADER_LAST_EVENT_ID)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<EventId>().ok());

    let filter = match build_sse_filter(&query) {
        Ok(built) => built,
        Err(err) => {
            warn!(error = ?err, "failed to build SSE filter; using defaults");
            SseFilter::default()
        }
    };

    let stream = event_sse_stream(state.events.clone(), last_id, filter);

    Ok(Sse::new(stream).keep_alive(
        sse::KeepAlive::new()
            .interval(Duration::from_secs(SSE_KEEP_ALIVE_SECS))
            .text("keep-alive"),
    ))
}

pub(crate) fn build_sse_filter(query: &SseQuery) -> Result<SseFilter, ApiError> {
    let mut filter = SseFilter::default();

    if let Some(torrent) = query.torrent.as_deref() {
        for value in split_comma_separated(torrent) {
            let parsed = Uuid::parse_str(&value).map_err(|_| {
                ApiError::bad_request("torrent filter is not a valid UUID")
                    .with_context_field("torrent_filter", value.clone())
            })?;
            filter.torrent_ids.insert(parsed);
        }
    }

    if let Some(events) = query.event.as_deref() {
        for value in split_comma_separated(events) {
            if EVENT_KIND_WHITELIST.contains(&value.as_str()) {
                filter.event_kinds.insert(value);
            }
        }
    }

    if let Some(states) = query.state.as_deref() {
        for value in split_comma_separated(states) {
            filter.states.insert(parse_state_filter(&value)?);
        }
    }

    Ok(filter)
}

pub(crate) fn matches_sse_filter(envelope: &EventEnvelope, filter: &SseFilter) -> bool {
    if !filter.event_kinds.is_empty() && !filter.event_kinds.contains(envelope.event.kind()) {
        return false;
    }

    if !filter.torrent_ids.is_empty() {
        let torrent_id = match &envelope.event {
            CoreEvent::TorrentAdded { torrent_id, .. }
            | CoreEvent::FilesDiscovered { torrent_id, .. }
            | CoreEvent::Progress { torrent_id, .. }
            | CoreEvent::StateChanged { torrent_id, .. }
            | CoreEvent::Completed { torrent_id, .. }
            | CoreEvent::MetadataUpdated { torrent_id, .. }
            | CoreEvent::TorrentRemoved { torrent_id }
            | CoreEvent::FsopsStarted { torrent_id, .. }
            | CoreEvent::FsopsProgress { torrent_id, .. }
            | CoreEvent::FsopsCompleted { torrent_id, .. }
            | CoreEvent::FsopsFailed { torrent_id, .. }
            | CoreEvent::SelectionReconciled { torrent_id, .. } => torrent_id,
            CoreEvent::SettingsChanged { .. } | CoreEvent::HealthChanged { .. } => {
                return false;
            }
        };

        if !filter.torrent_ids.contains(torrent_id) {
            return false;
        }
    }

    if !filter.states.is_empty() {
        match &envelope.event {
            CoreEvent::StateChanged { state, .. } => {
                let mapped = TorrentStateKind::from(state.clone());
                if !filter.states.contains(&mapped) {
                    return false;
                }
            }
            CoreEvent::Completed { .. } => {
                if !filter.states.contains(&TorrentStateKind::Completed) {
                    return false;
                }
            }
            _ => {}
        }
    }

    true
}

pub(crate) fn event_replay_stream(
    bus: EventBus,
    since: Option<EventId>,
) -> impl futures_core::Stream<Item = EventEnvelope> + Send {
    stream! {
        let mut stream = bus.subscribe(since);
        while let Some(result) = stream.next().await {
            if let Ok(envelope) = result {
                yield envelope;
            }
        }
    }
}

pub(crate) fn event_sse_stream(
    bus: EventBus,
    since: Option<EventId>,
    filter: SseFilter,
) -> impl futures_core::Stream<Item = Result<sse::Event, Infallible>> + Send {
    let filter = Arc::new(filter);
    event_replay_stream(bus, since)
        .filter({
            let filter = Arc::clone(&filter);
            move |envelope| future::ready(matches_sse_filter(envelope, &filter))
        })
        .scan(None, move |last_id: &mut Option<EventId>, envelope| {
            if last_id.is_some_and(|prev| prev == envelope.id) {
                future::ready(Some(None))
            } else {
                *last_id = Some(envelope.id);
                future::ready(Some(Some(envelope)))
            }
        })
        .filter_map(|maybe| async move { maybe })
        .filter_map(|envelope| async move {
            match serde_json::to_string(&envelope) {
                Ok(payload) => Some(Ok(sse::Event::default()
                    .id(envelope.id.to_string())
                    .event(envelope.event.kind())
                    .data(payload))),
                Err(err) => {
                    error!(error = %err, "failed to serialise SSE event payload");
                    None
                }
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Result, anyhow};
    use std::time::Duration;
    use tokio::time::sleep;

    #[test]
    fn build_sse_filter_parses_filters() -> Result<()> {
        let query = SseQuery {
            torrent: Some(format!("{},{}", Uuid::nil(), Uuid::from_u128(1))),
            event: Some("progress,completed".to_string()),
            state: Some("downloading,completed".to_string()),
        };
        let filter = build_sse_filter(&query)?;
        assert_eq!(filter.torrent_ids.len(), 2);
        assert_eq!(filter.event_kinds.len(), 2);
        assert_eq!(filter.states.len(), 2);
        Ok(())
    }

    #[test]
    fn build_sse_filter_ignores_unknown_event_kind() -> Result<()> {
        let query = SseQuery {
            torrent: None,
            event: Some("progress,unknown".to_string()),
            state: None,
        };
        let filter = build_sse_filter(&query)?;
        assert_eq!(filter.event_kinds.len(), 1);
        assert!(filter.event_kinds.contains("progress"));
        Ok(())
    }

    #[test]
    fn matches_sse_filter_respects_state_and_ids() {
        let filter = SseFilter {
            torrent_ids: std::iter::once(Uuid::nil()).collect(),
            event_kinds: std::iter::once("state_changed".to_string()).collect(),
            states: std::iter::once(TorrentStateKind::Queued).collect(),
        };
        let envelope = EventEnvelope {
            id: 1u64,
            event: CoreEvent::StateChanged {
                torrent_id: Uuid::nil(),
                state: revaer_events::TorrentState::Queued,
            },
            timestamp: chrono::Utc::now(),
        };
        assert!(matches_sse_filter(&envelope, &filter));
    }

    #[test]
    fn matches_sse_filter_allows_progress_with_state_filter() {
        let filter = SseFilter {
            torrent_ids: std::iter::once(Uuid::nil()).collect(),
            event_kinds: std::iter::once("progress".to_string()).collect(),
            states: std::iter::once(TorrentStateKind::Downloading).collect(),
        };
        let envelope = EventEnvelope {
            id: 2u64,
            event: CoreEvent::Progress {
                torrent_id: Uuid::nil(),
                bytes_downloaded: 10,
                bytes_total: 100,
                eta_seconds: Some(9),
                download_bps: 512,
                upload_bps: 128,
                ratio: 0.5,
            },
            timestamp: chrono::Utc::now(),
        };
        assert!(matches_sse_filter(&envelope, &filter));
    }

    #[tokio::test]
    async fn sse_stream_emits_event_for_torrent_added() -> Result<()> {
        let bus = EventBus::with_capacity(16);
        let publisher = bus.clone();
        let torrent_id = Uuid::new_v4();
        tokio::spawn(async move {
            sleep(Duration::from_millis(10)).await;
            if let Err(error) = publisher.publish(CoreEvent::TorrentAdded {
                torrent_id,
                name: "example".to_string(),
            }) {
                tracing::warn!(
                    event_id = error.event_id(),
                    event_kind = error.event_kind(),
                    error = %error,
                    "failed to publish event"
                );
            }
        });
        let stream = event_sse_stream(bus.clone(), None, SseFilter::default());
        futures_util::pin_mut!(stream);
        match tokio::time::timeout(Duration::from_millis(200), stream.next()).await? {
            Some(Ok(_)) => Ok(()),
            Some(Err(err)) => Err(anyhow!(err)),
            None => Err(anyhow!("expected SSE event")),
        }
    }
}
