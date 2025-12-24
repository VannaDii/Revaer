//! SSE connection runner for the app shell.
//!
//! # Design
//! - Use fetch streaming so auth headers can be attached to requests.
//! - Attempt the primary torrents SSE endpoint first, then fall back on 404.
//! - Expose a cancellable handle so callers can stop the stream on unmount.

use crate::app::preferences::{load_last_event_id, persist_last_event_id};
use crate::core::auth::{AuthState, LocalAuth};
use crate::core::events::UiEventEnvelope;
use crate::core::logic::{SseEndpoint, SseQuery, backoff_delay_ms, build_sse_url};
use crate::models::SseState;
use crate::services::sse::{SseDecodeError, SseParser, decode_frame};
use gloo_timers::future::TimeoutFuture;
use js_sys::{Reflect, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AbortController, AbortSignal, Headers, ReadableStream, ReadableStreamDefaultReader, Request,
    RequestInit, Response, TextDecoder,
};
use yew::Callback;

/// Active SSE stream handle for cancellation.
pub(crate) struct SseHandle {
    controller: AbortController,
}

impl SseHandle {
    pub(crate) fn close(&self) {
        self.controller.abort();
    }
}

/// Spawn an SSE loop with auth headers and return a cancellable handle.
pub(crate) fn connect_sse(
    base_url: String,
    auth: Option<AuthState>,
    query: SseQuery,
    on_event: Callback<UiEventEnvelope>,
    on_error: Callback<SseDecodeError>,
    on_state: Callback<SseState>,
) -> Option<SseHandle> {
    let controller = AbortController::new().ok()?;
    let signal = controller.signal();
    yew::platform::spawn_local(async move {
        run_sse_loop(base_url, auth, query, signal, on_event, on_error, on_state).await;
    });
    Some(SseHandle { controller })
}

async fn run_sse_loop(
    base_url: String,
    auth: Option<AuthState>,
    query: SseQuery,
    signal: AbortSignal,
    on_event: Callback<UiEventEnvelope>,
    on_error: Callback<SseDecodeError>,
    on_state: Callback<SseState>,
) {
    let mut attempt = 0u32;
    let mut retry_hint_ms: Option<u64> = None;
    let mut last_event = String::from("none");
    let mut last_event_id = load_last_event_id();

    loop {
        if signal.aborted() {
            break;
        }

        match open_stream(&base_url, &auth, &query, last_event_id, &signal).await {
            Ok(mut reader) => {
                let mut parser = SseParser::default();
                let decoder = match TextDecoder::new() {
                    Ok(decoder) => decoder,
                    Err(err) => {
                        update_reconnecting(
                            &on_state,
                            retry_hint_ms,
                            attempt,
                            &last_event,
                            format!("decoder error: {err:?}"),
                        )
                        .await;
                        attempt = attempt.saturating_add(1);
                        continue;
                    }
                };
                attempt = 0;
                retry_hint_ms = None;

                loop {
                    if signal.aborted() {
                        return;
                    }
                    match read_chunk(&mut reader).await {
                        Ok(Some(bytes)) => {
                            let text = match decoder.decode_with_js_u8_array(&bytes) {
                                Ok(text) => text,
                                Err(err) => {
                                    update_reconnecting(
                                        &on_state,
                                        retry_hint_ms,
                                        attempt,
                                        &last_event,
                                        format!("decode error: {err:?}"),
                                    )
                                    .await;
                                    break;
                                }
                            };
                            for frame in parser.push(&text) {
                                if let Some(retry) = frame.retry {
                                    retry_hint_ms = Some(retry);
                                }
                                match decode_frame(&frame) {
                                    Ok(envelope) => {
                                        if let Some(id) = envelope.id {
                                            last_event_id = Some(id);
                                            persist_last_event_id(id);
                                            last_event = id.to_string();
                                        }
                                        on_state.emit(SseState::Connected);
                                        on_event.emit(envelope);
                                    }
                                    Err(err) => {
                                        on_error.emit(err);
                                    }
                                }
                            }
                        }
                        Ok(None) => {
                            if let Some(frame) = parser.finish() {
                                match decode_frame(&frame) {
                                    Ok(envelope) => {
                                        if let Some(id) = envelope.id {
                                            last_event_id = Some(id);
                                            persist_last_event_id(id);
                                            last_event = id.to_string();
                                        }
                                        on_state.emit(SseState::Connected);
                                        on_event.emit(envelope);
                                    }
                                    Err(err) => on_error.emit(err),
                                }
                            }
                            break;
                        }
                        Err(err) => {
                            update_reconnecting(
                                &on_state,
                                retry_hint_ms,
                                attempt,
                                &last_event,
                                format!("read error: {err}"),
                            )
                            .await;
                            break;
                        }
                    }
                }
            }
            Err(err) => {
                update_reconnecting(&on_state, retry_hint_ms, attempt, &last_event, err).await;
            }
        }

        attempt = attempt.saturating_add(1);
    }
}

async fn open_stream(
    base_url: &str,
    auth: &Option<AuthState>,
    query: &SseQuery,
    last_event_id: Option<u64>,
    signal: &AbortSignal,
) -> Result<ReadableStreamDefaultReader, String> {
    let primary = build_sse_url(base_url, SseEndpoint::Primary, Some(query));
    match fetch_stream(&primary, auth, last_event_id, signal).await {
        Ok(response) => stream_reader(response),
        Err(ConnectError::NotFound) => {
            let fallback = build_sse_url(base_url, SseEndpoint::Fallback, Some(query));
            let response = fetch_stream(&fallback, auth, last_event_id, signal)
                .await
                .map_err(|err| format!("fallback SSE failed: {}", err.to_string()))?;
            stream_reader(response)
        }
        Err(err) => Err(err.to_string()),
    }
}

fn stream_reader(response: Response) -> Result<ReadableStreamDefaultReader, String> {
    let stream: ReadableStream = response
        .body()
        .ok_or_else(|| "SSE response missing body".to_string())?;
    let reader = stream
        .get_reader()
        .dyn_into::<ReadableStreamDefaultReader>()
        .map_err(|_| "SSE stream reader unavailable".to_string())?;
    Ok(reader)
}

async fn fetch_stream(
    url: &str,
    auth: &Option<AuthState>,
    last_event_id: Option<u64>,
    signal: &AbortSignal,
) -> Result<Response, ConnectError> {
    let window = web_sys::window().ok_or(ConnectError::Window)?;
    let init = RequestInit::new();
    init.set_method("GET");
    init.set_signal(Some(signal));

    let headers = Headers::new().map_err(|_| ConnectError::Headers)?;
    apply_auth(&headers, auth);
    if let Some(id) = last_event_id {
        let _ = headers.set("Last-Event-ID", &id.to_string());
    }
    init.set_headers(&headers);

    let request = Request::new_with_str_and_init(url, &init).map_err(|_| ConnectError::Request)?;
    let resp = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| ConnectError::Fetch)?;
    let response: Response = resp.dyn_into().map_err(|_| ConnectError::Fetch)?;
    if response.status() == 404 {
        return Err(ConnectError::NotFound);
    }
    if !response.ok() {
        return Err(ConnectError::Status(response.status()));
    }
    Ok(response)
}

fn apply_auth(headers: &Headers, auth: &Option<AuthState>) {
    match auth {
        Some(AuthState::ApiKey(key)) if !key.trim().is_empty() => {
            let _ = headers.set("x-revaer-api-key", key);
        }
        Some(AuthState::Local(auth)) => {
            if let Some(header) = basic_auth_header(auth) {
                let _ = headers.set("Authorization", &header);
            }
        }
        _ => {}
    }
}

fn basic_auth_header(auth: &LocalAuth) -> Option<String> {
    let raw = format!("{}:{}", auth.username, auth.password);
    let encoded = web_sys::window()?.btoa(&raw).ok()?;
    Some(format!("Basic {}", encoded))
}

async fn read_chunk(
    reader: &mut ReadableStreamDefaultReader,
) -> Result<Option<Uint8Array>, String> {
    let chunk = JsFuture::from(reader.read())
        .await
        .map_err(|err| format!("read failed: {err:?}"))?;
    let done = Reflect::get(&chunk, &JsValue::from_str("done"))
        .map_err(|err| format!("chunk done lookup failed: {err:?}"))?
        .as_bool()
        .unwrap_or(false);
    if done {
        return Ok(None);
    }
    let value = Reflect::get(&chunk, &JsValue::from_str("value"))
        .map_err(|err| format!("chunk value lookup failed: {err:?}"))?;
    Ok(Some(Uint8Array::new(&value)))
}

async fn update_reconnecting(
    on_state: &Callback<SseState>,
    retry_hint_ms: Option<u64>,
    attempt: u32,
    last_event: &str,
    reason: String,
) {
    let delay_ms = retry_hint_ms.unwrap_or(u64::from(backoff_delay_ms(attempt)));
    let retry_in_secs = (delay_ms / 1000).min(30) as u8;
    on_state.emit(SseState::Reconnecting {
        retry_in_secs,
        last_event: last_event.to_string(),
        reason,
    });
    TimeoutFuture::new(delay_ms as u32).await;
}

#[derive(Debug)]
enum ConnectError {
    Window,
    Headers,
    Request,
    Fetch,
    NotFound,
    Status(u16),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Window => write!(f, "window unavailable"),
            Self::Headers => write!(f, "headers unavailable"),
            Self::Request => write!(f, "request build failed"),
            Self::Fetch => write!(f, "fetch failed"),
            Self::NotFound => write!(f, "endpoint not found"),
            Self::Status(code) => write!(f, "http {code}"),
        }
    }
}
