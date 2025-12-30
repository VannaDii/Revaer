//! Log streaming connector.
//!
//! # Design
//! - Use fetch streaming so auth headers can be attached to requests.
//! - Reconnect with exponential backoff when the stream closes.
//! - Keep the API surface minimal: line + error callbacks.

use crate::core::auth::{AuthState, LocalAuth};
use crate::core::logic::backoff_delay_ms;
use crate::services::sse::SseParser;
use gloo::console;
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

/// Active log stream handle for cancellation.
pub(crate) struct LogStreamHandle {
    controller: AbortController,
}

impl LogStreamHandle {
    pub(crate) fn close(&self) {
        self.controller.abort();
    }
}

/// Spawn a log streaming loop and return a cancellable handle.
pub(crate) fn connect_log_stream(
    base_url: String,
    auth: Option<AuthState>,
    on_line: Callback<String>,
    on_error: Callback<String>,
) -> Option<LogStreamHandle> {
    let controller = AbortController::new().ok()?;
    let signal = controller.signal();
    yew::platform::spawn_local(async move {
        run_log_stream_loop(base_url, auth, signal, on_line, on_error).await;
    });
    Some(LogStreamHandle { controller })
}

async fn run_log_stream_loop(
    base_url: String,
    auth: Option<AuthState>,
    signal: AbortSignal,
    on_line: Callback<String>,
    on_error: Callback<String>,
) {
    let mut attempt = 0u32;
    loop {
        if signal.aborted() {
            break;
        }

        match open_stream(&base_url, &auth, &signal).await {
            Ok(mut reader) => {
                let mut parser = SseParser::default();
                let decoder = match TextDecoder::new() {
                    Ok(decoder) => decoder,
                    Err(err) => {
                        on_error.emit(format!("decoder error: {err:?}"));
                        schedule_reconnect(attempt).await;
                        attempt = attempt.saturating_add(1);
                        continue;
                    }
                };
                attempt = 0;

                loop {
                    if signal.aborted() {
                        return;
                    }
                    match read_chunk(&mut reader).await {
                        Ok(Some(bytes)) => {
                            let text = match decoder.decode_with_js_u8_array(&bytes) {
                                Ok(text) => text,
                                Err(err) => {
                                    on_error.emit(format!("decode error: {err:?}"));
                                    break;
                                }
                            };
                            for frame in parser.push(&text) {
                                let data = frame.data.trim();
                                if !data.is_empty() {
                                    on_line.emit(data.to_string());
                                }
                            }
                        }
                        Ok(None) => {
                            if let Some(frame) = parser.finish() {
                                let data = frame.data.trim();
                                if !data.is_empty() {
                                    on_line.emit(data.to_string());
                                }
                            }
                            break;
                        }
                        Err(err) => {
                            on_error.emit(err);
                            break;
                        }
                    }
                }
            }
            Err(err) => {
                on_error.emit(err.to_string());
            }
        }

        attempt = attempt.saturating_add(1);
        schedule_reconnect(attempt).await;
    }
}

async fn schedule_reconnect(attempt: u32) {
    let delay_ms = backoff_delay_ms(attempt);
    TimeoutFuture::new(delay_ms).await;
}

async fn open_stream(
    base_url: &str,
    auth: &Option<AuthState>,
    signal: &AbortSignal,
) -> Result<ReadableStreamDefaultReader, ConnectError> {
    let url = format!("{}/v1/logs/stream", base_url.trim_end_matches('/'));
    let response = fetch_stream(&url, auth, signal).await?;
    stream_reader(response)
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
    signal: &AbortSignal,
) -> Result<Response, ConnectError> {
    let window = web_sys::window().ok_or(ConnectError::Window)?;
    let init = RequestInit::new();
    init.set_method("GET");
    init.set_signal(Some(signal));

    let headers = Headers::new().map_err(|_| ConnectError::Headers)?;
    apply_auth(&headers, auth);
    init.set_headers(&headers);

    let request = Request::new_with_str_and_init(url, &init).map_err(|_| ConnectError::Request)?;
    let resp = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| ConnectError::Fetch)?;
    let response: Response = resp.dyn_into().map_err(|_| ConnectError::Fetch)?;
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

#[derive(Debug)]
enum ConnectError {
    Window,
    Headers,
    Request,
    Fetch,
    Status(u16),
    Stream,
    Reader,
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectError::Window => write!(f, "browser window unavailable"),
            ConnectError::Headers => write!(f, "failed to construct headers"),
            ConnectError::Request => write!(f, "failed to construct request"),
            ConnectError::Fetch => write!(f, "fetch failed"),
            ConnectError::Status(code) => write!(f, "unexpected status {code}"),
            ConnectError::Stream => write!(f, "response stream missing"),
            ConnectError::Reader => write!(f, "failed to read response stream"),
        }
    }
}
