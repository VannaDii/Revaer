//! Live log streaming endpoint.
//!
//! # Design
//! - Bridge structured tracing output to SSE without extra formatting layers.
//! - Allow clients to reconnect without holding server state.
//! - Emit keep-alive frames to keep proxies from closing idle streams.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    response::sse::{self, Sse},
};
use futures_util::StreamExt;
use revaer_telemetry::log_stream_receiver;
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};

use crate::app::state::ApiState;
use crate::http::constants::SSE_KEEP_ALIVE_SECS;
use crate::http::errors::ApiError;

pub(crate) async fn stream_logs(
    State(_state): State<Arc<ApiState>>,
) -> Result<Sse<impl futures_core::Stream<Item = Result<sse::Event, Infallible>> + Send>, ApiError>
{
    let receiver = log_stream_receiver();
    let stream = BroadcastStream::new(receiver).filter_map(|result| async move {
        let event = match result {
            Ok(line) => sse::Event::default().event("log").data(line),
            Err(err) => sse::Event::default()
                .event("log_status")
                .data(log_status_message(&err)),
        };
        Some(Ok(event))
    });

    Ok(Sse::new(stream).keep_alive(
        sse::KeepAlive::new()
            .interval(Duration::from_secs(SSE_KEEP_ALIVE_SECS))
            .text("keep-alive"),
    ))
}

fn log_status_message(err: &BroadcastStreamRecvError) -> String {
    match err {
        BroadcastStreamRecvError::Lagged(count) => {
            format!("log stream lagged; dropped {count} lines")
        }
    }
}
