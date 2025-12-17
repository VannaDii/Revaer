//! Server-sent events filters and streaming helpers.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_stream::stream;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::sse::{self, Sse},
};
use futures_util::{StreamExt, future, stream};
use revaer_events::{Event as CoreEvent, EventBus, EventEnvelope, EventId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::sleep;
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

    let stream = select_dummy_and_real_streams(last_id, filter, state.events.clone());

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
                ApiError::bad_request(format!("torrent filter '{value}' is not a valid UUID"))
            })?;
            filter.torrent_ids.insert(parsed);
        }
    }

    if let Some(events) = query.event.as_deref() {
        for value in split_comma_separated(events) {
            if !EVENT_KIND_WHITELIST.contains(&value.as_str()) {
                return Err(ApiError::bad_request(format!(
                    "event filter '{value}' is not recognised"
                )));
            }
            filter.event_kinds.insert(value);
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
            _ => return false,
        }
    }

    true
}

fn select_dummy_and_real_streams(
    last_id: Option<EventId>,
    filter: SseFilter,
    events: EventBus,
) -> impl futures_core::Stream<Item = Result<sse::Event, Infallible>> + Send {
    let real_stream = event_sse_stream(events, last_id, filter);
    let dummy_stream = build_dummy_sse_stream();

    stream::select(dummy_stream, real_stream)
}

fn build_dummy_sse_stream()
-> impl futures_core::Stream<Item = Result<sse::Event, Infallible>> + Send {
    let torrent_id = Uuid::nil();
    let torrent_other = Uuid::from_u128(1);
    stream::unfold(0u64, move |tick| {
        let tid = torrent_id;
        let tid_other = torrent_other;
        async move {
            sleep(Duration::from_millis(800)).await;
            let next_tick = tick.saturating_add(1);
            let payload = dummy_payload(tick, tid, tid_other);
            let event = sse::Event::default()
                .id(format!("dummy-{tick}"))
                .data(payload.to_string());
            Some((Ok(event), next_tick))
        }
    })
}

pub(crate) fn dummy_payload(tick: u64, tid: Uuid, tid_other: Uuid) -> Value {
    match tick % 10 {
        0 => json!({
            "kind": "system_rates",
            "data": {
                "download_bps": 50_000 + (tick * 2_000),
                "upload_bps": 5_000 + (tick * 500)
            }
        }),
        1 => json!({
            "kind": "torrent_added",
            "data": {
                "id": tid,
                "name": format!("example-{tick}"),
                "state": "queued"
            }
        }),
        2 => json!({
            "kind": "progress",
            "data": {
                "id": tid,
                "progress": {
                    "bytes_downloaded": tick * 1024,
                    "bytes_total": 20_480,
                    "eta_seconds": 42
                },
                "rates": {
                    "download_bps": 50_000 + (tick * 2_000),
                    "upload_bps": 5_000 + (tick * 500),
                    "ratio": 0.75
                },
                "state": "downloading",
                "sequential": tick & 1 == 0,
                "download_dir": "/downloads/demo"
            }
        }),
        3 => json!({
            "kind": "state_changed",
            "data": {
                "id": tid,
                "state": "seeding",
                "download_dir": "/downloads/demo"
            }
        }),
        4 => json!({
            "kind": "completed",
            "data": {
                "id": tid,
                "library_path": "/library/demo"
            }
        }),
        5 => json!({
            "kind": "torrent_removed",
            "data": {
                "id": tid_other
            }
        }),
        6 => json!({
            "kind": "fsops_started",
            "data": {
                "torrent_id": tid,
                "src_path": "/downloads/demo",
                "dst_path": "/library/demo"
            }
        }),
        7 => json!({
            "kind": "fsops_progress",
            "data": {
                "torrent_id": tid,
                "status": "moving",
                "percent_complete": 42.0
            }
        }),
        8 => json!({
            "kind": "metadata_updated",
            "data": {
                "torrent_id": tid,
                "download_dir": format!("/downloads/relocated-{tick}"),
                "name": format!("demo-{tick}")
            }
        }),
        _ => json!({
            "kind": "fsops_failed",
            "data": {
                "torrent_id": tid,
                "message": "disk full"
            }
        }),
    }
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
    use std::time::Duration;

    #[test]
    fn build_sse_filter_parses_filters() {
        let query = SseQuery {
            torrent: Some(format!("{},{}", Uuid::nil(), Uuid::from_u128(1))),
            event: Some("progress,completed".to_string()),
            state: Some("downloading,completed".to_string()),
        };
        let filter = build_sse_filter(&query).expect("filter builds");
        assert_eq!(filter.torrent_ids.len(), 2);
        assert_eq!(filter.event_kinds.len(), 2);
        assert_eq!(filter.states.len(), 2);
    }

    #[test]
    fn build_sse_filter_rejects_unknown_event_kind() {
        let query = SseQuery {
            torrent: None,
            event: Some("progress,unknown".to_string()),
            state: None,
        };
        let result = build_sse_filter(&query);
        assert!(result.is_err());
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
    fn dummy_payload_covers_all_kinds() {
        let tid = Uuid::nil();
        let tid_other = Uuid::from_u128(1);
        for tick in 0..10 {
            assert!(
                dummy_payload(tick, tid, tid_other)["kind"]
                    .as_str()
                    .is_some(),
                "tick {tick} should emit a kind"
            );
        }
    }

    #[test]
    fn dummy_payload_fields_change_with_ticks() {
        let tid = Uuid::nil();
        let tid_other = Uuid::from_u128(1);
        let progress = dummy_payload(2, tid, tid_other);
        assert_eq!(
            progress["data"]["progress"]["bytes_downloaded"].as_u64(),
            Some(2 * 1024)
        );
        let state = dummy_payload(7, tid, tid_other);
        assert_eq!(state["data"]["status"], "moving");
        let metadata = dummy_payload(8, tid, tid_other);
        assert_eq!(metadata["data"]["download_dir"], "/downloads/relocated-8");
        let jobs = dummy_payload(9, tid, tid_other);
        assert_eq!(jobs["data"]["message"], "disk full");
    }

    #[tokio::test]
    async fn sse_stream_emits_event_for_torrent_added() {
        let bus = EventBus::with_capacity(16);
        let publisher = bus.clone();
        let torrent_id = Uuid::new_v4();
        tokio::spawn(async move {
            sleep(Duration::from_millis(10)).await;
            let _ = publisher.publish(CoreEvent::TorrentAdded {
                torrent_id,
                name: "example".to_string(),
            });
        });
        let stream = event_sse_stream(bus.clone(), None, SseFilter::default());
        futures_util::pin_mut!(stream);
        match tokio::time::timeout(Duration::from_millis(200), stream.next())
            .await
            .expect("timed out waiting for SSE event")
        {
            Some(Ok(_)) => {}
            other => panic!("expected SSE event, got {other:?}"),
        }
    }
}
