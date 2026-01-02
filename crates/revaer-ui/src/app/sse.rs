//! SSE connection runner for the app shell.
//!
//! # Design
//! - Use fetch streaming so auth headers can be attached to requests.
//! - Attempt the primary torrents SSE endpoint first, then fall back on 404.
//! - Expose a cancellable handle so callers can stop the stream on unmount.

use crate::app::preferences::{clear_last_event_id, load_last_event_id, persist_last_event_id};
use crate::core::auth::{AuthState, LocalAuth};
use crate::core::events::UiEventEnvelope;
use crate::core::logic::{SseEndpoint, SseQuery, backoff_delay_ms, build_sse_url};
use crate::core::store::{SseConnectionState, SseError, SseStatus};
use crate::services::sse::{SseDecodeError, SseParser, decode_frame};
use gloo::console;
use gloo_timers::future::TimeoutFuture;
use js_sys::Date;
use js_sys::{Reflect, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AbortController, AbortSignal, Headers, ReadableStream, ReadableStreamDefaultReader, Request,
    RequestInit, RequestMode, Response, TextDecoder,
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
    on_state: Callback<SseStatus>,
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
    on_state: Callback<SseStatus>,
) {
    let auth_label = auth_label(&auth);
    let mut attempt = 0u32;
    let mut retry_hint_ms: Option<u64> = None;
    let mut last_event_id = load_last_event_id();

    loop {
        if signal.aborted() {
            break;
        }

        emit_status(
            &on_state,
            SseConnectionState::Reconnecting,
            &auth_label,
            last_event_id,
            None,
            None,
            Some(SseError {
                message: "connecting".to_string(),
                status_code: None,
            }),
        );

        match open_stream(&base_url, &auth, &query, last_event_id, &signal).await {
            Ok(mut reader) => {
                let mut parser = SseParser::default();
                let decoder = match TextDecoder::new() {
                    Ok(decoder) => decoder,
                    Err(err) => {
                        let error = SseError {
                            message: format!("decoder error: {err:?}"),
                            status_code: None,
                        };
                        emit_status(
                            &on_state,
                            SseConnectionState::Disconnected,
                            &auth_label,
                            last_event_id,
                            None,
                            None,
                            Some(error.clone()),
                        );
                        schedule_reconnect(
                            &on_state,
                            &auth_label,
                            retry_hint_ms,
                            attempt,
                            last_event_id,
                            error,
                        )
                        .await;
                        attempt = attempt.saturating_add(1);
                        continue;
                    }
                };
                attempt = 0;
                retry_hint_ms = None;
                emit_status(
                    &on_state,
                    SseConnectionState::Connected,
                    &auth_label,
                    last_event_id,
                    None,
                    None,
                    None,
                );

                loop {
                    if signal.aborted() {
                        return;
                    }
                    match read_chunk(&mut reader).await {
                        Ok(Some(bytes)) => {
                            let text = match decoder.decode_with_js_u8_array(&bytes) {
                                Ok(text) => text,
                                Err(err) => {
                                    let error = SseError {
                                        message: format!("decode error: {err:?}"),
                                        status_code: None,
                                    };
                                    emit_status(
                                        &on_state,
                                        SseConnectionState::Disconnected,
                                        &auth_label,
                                        last_event_id,
                                        None,
                                        None,
                                        Some(error.clone()),
                                    );
                                    schedule_reconnect(
                                        &on_state,
                                        &auth_label,
                                        retry_hint_ms,
                                        attempt,
                                        last_event_id,
                                        error,
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
                                        }
                                        emit_status(
                                            &on_state,
                                            SseConnectionState::Connected,
                                            &auth_label,
                                            last_event_id,
                                            None,
                                            None,
                                            None,
                                        );
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
                                        }
                                        emit_status(
                                            &on_state,
                                            SseConnectionState::Connected,
                                            &auth_label,
                                            last_event_id,
                                            None,
                                            None,
                                            None,
                                        );
                                        on_event.emit(envelope);
                                    }
                                    Err(err) => on_error.emit(err),
                                }
                            }
                            let error = SseError {
                                message: "stream ended".to_string(),
                                status_code: None,
                            };
                            emit_status(
                                &on_state,
                                SseConnectionState::Disconnected,
                                &auth_label,
                                last_event_id,
                                None,
                                None,
                                Some(error.clone()),
                            );
                            schedule_reconnect(
                                &on_state,
                                &auth_label,
                                retry_hint_ms,
                                attempt,
                                last_event_id,
                                error,
                            )
                            .await;
                            break;
                        }
                        Err(err) => {
                            let error = SseError {
                                message: format!("read error: {err}"),
                                status_code: None,
                            };
                            emit_status(
                                &on_state,
                                SseConnectionState::Disconnected,
                                &auth_label,
                                last_event_id,
                                None,
                                None,
                                Some(error.clone()),
                            );
                            schedule_reconnect(
                                &on_state,
                                &auth_label,
                                retry_hint_ms,
                                attempt,
                                last_event_id,
                                error,
                            )
                            .await;
                            break;
                        }
                    }
                }
            }
            Err(err) => {
                if matches!(err, ConnectError::Conflict) {
                    clear_last_event_id();
                    last_event_id = None;
                    retry_hint_ms = None;
                }
                let error = connect_error_to_sse_error(&err);
                emit_status(
                    &on_state,
                    SseConnectionState::Disconnected,
                    &auth_label,
                    last_event_id,
                    None,
                    None,
                    Some(error.clone()),
                );
                let reconnect_attempt = if matches!(err, ConnectError::Conflict) {
                    0
                } else {
                    attempt
                };
                schedule_reconnect(
                    &on_state,
                    &auth_label,
                    retry_hint_ms,
                    reconnect_attempt,
                    last_event_id,
                    error,
                )
                .await;
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
) -> Result<ReadableStreamDefaultReader, ConnectError> {
    let primary = build_sse_url(base_url, SseEndpoint::Primary, Some(query));
    match fetch_stream(&primary, auth, last_event_id, signal).await {
        Ok(response) => stream_reader(response),
        Err(ConnectError::NotFound) => {
            let fallback = build_sse_url(base_url, SseEndpoint::Fallback, Some(query));
            let response = fetch_stream(&fallback, auth, last_event_id, signal).await?;
            stream_reader(response)
        }
        Err(err) => Err(err),
    }
}

fn stream_reader(response: Response) -> Result<ReadableStreamDefaultReader, ConnectError> {
    let stream: ReadableStream = response.body().ok_or(ConnectError::Stream)?;
    let reader = stream
        .get_reader()
        .dyn_into::<ReadableStreamDefaultReader>()
        .map_err(|_| ConnectError::Reader)?;
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
    init.set_mode(RequestMode::Cors);
    init.set_signal(Some(signal));

    let headers = Headers::new().map_err(|_| ConnectError::Headers)?;
    apply_auth(&headers, auth);
    if let Some(id) = last_event_id {
        set_header(&headers, "Last-Event-ID", &id.to_string());
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
    if response.status() == 409 {
        return Err(ConnectError::Conflict);
    }
    if !response.ok() {
        return Err(ConnectError::Status(response.status()));
    }
    Ok(response)
}

fn apply_auth(headers: &Headers, auth: &Option<AuthState>) {
    match auth {
        Some(AuthState::ApiKey(key)) if !key.trim().is_empty() => {
            set_header(headers, "x-revaer-api-key", key);
        }
        Some(AuthState::Local(auth)) => {
            if let Some(header) = basic_auth_header(auth) {
                set_header(headers, "Authorization", &header);
            } else {
                console::error!("basic auth header unavailable");
            }
        }
        _ => {}
    }
}

fn set_header(headers: &Headers, name: &'static str, value: &str) {
    if let Err(err) = headers.set(name, value) {
        log_header_error(name, err);
    }
}

fn log_header_error(name: &'static str, err: JsValue) {
    console::error!("request header set failed", name, err);
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

fn emit_status(
    on_state: &Callback<SseStatus>,
    state: SseConnectionState,
    auth_label: &Option<String>,
    last_event_id: Option<u64>,
    backoff_ms: Option<u64>,
    next_retry_at_ms: Option<u64>,
    last_error: Option<SseError>,
) {
    on_state.emit(SseStatus {
        state,
        backoff_ms,
        next_retry_at_ms,
        last_event_id,
        last_error,
        auth_mode: auth_label.clone(),
    });
}

fn auth_label(auth: &Option<AuthState>) -> Option<String> {
    match auth {
        Some(AuthState::ApiKey(_)) => Some("API key".to_string()),
        Some(AuthState::Local(_)) => Some("Local auth".to_string()),
        Some(AuthState::Anonymous) => Some("Anonymous".to_string()),
        None => None,
    }
}

fn connect_error_to_sse_error(err: &ConnectError) -> SseError {
    SseError {
        message: err.to_string(),
        status_code: err.status_code(),
    }
}

fn now_ms() -> u64 {
    Date::now() as u64
}

async fn schedule_reconnect(
    on_state: &Callback<SseStatus>,
    auth_label: &Option<String>,
    retry_hint_ms: Option<u64>,
    attempt: u32,
    last_event_id: Option<u64>,
    error: SseError,
) {
    let delay_ms = retry_hint_ms.unwrap_or(u64::from(backoff_delay_ms(attempt)));
    let next_retry_at = now_ms().saturating_add(delay_ms);
    emit_status(
        on_state,
        SseConnectionState::Reconnecting,
        auth_label,
        last_event_id,
        Some(delay_ms),
        Some(next_retry_at),
        Some(error),
    );
    TimeoutFuture::new(delay_ms as u32).await;
}

#[derive(Debug)]
enum ConnectError {
    Window,
    Headers,
    Request,
    Fetch,
    Stream,
    Reader,
    NotFound,
    Conflict,
    Status(u16),
}

impl ConnectError {
    fn status_code(&self) -> Option<u16> {
        match self {
            Self::NotFound => Some(404),
            Self::Conflict => Some(409),
            Self::Status(code) => Some(*code),
            _ => None,
        }
    }
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Window => write!(f, "window unavailable"),
            Self::Headers => write!(f, "headers unavailable"),
            Self::Request => write!(f, "request build failed"),
            Self::Fetch => write!(f, "fetch failed"),
            Self::Stream => write!(f, "SSE response missing body"),
            Self::Reader => write!(f, "SSE stream reader unavailable"),
            Self::NotFound => write!(f, "endpoint not found"),
            Self::Conflict => write!(f, "conflict"),
            Self::Status(code) => write!(f, "http {code}"),
        }
    }
}
