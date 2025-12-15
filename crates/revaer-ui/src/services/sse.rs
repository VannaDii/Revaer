//! SSE client helper.

use crate::core::logic::build_sse_url;
use crate::models::SseEvent;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{EventSource, EventSourceInit, MessageEvent};

/// Handle SSE events pushed from the backend using `EventSource`.
pub(crate) fn connect_sse(
    base_url: &str,
    api_key: Option<String>,
    on_event: impl Fn(SseEvent) + 'static,
) -> Option<EventSource> {
    let url = build_sse_url(base_url, &api_key);
    let init = EventSourceInit::new();
    init.set_with_credentials(false);
    let source = EventSource::new_with_event_source_init_dict(&url, &init).ok()?;
    let handler = Closure::<dyn FnMut(_)>::wrap(Box::new(move |event: web_sys::Event| {
        if let Ok(msg) = event.dyn_into::<MessageEvent>() {
            if let Ok(text) = msg.data().dyn_into::<js_sys::JsString>() {
                if let Ok(parsed) = serde_json::from_str::<SseEvent>(&String::from(text)) {
                    on_event(parsed);
                }
            }
        }
    }) as Box<dyn FnMut(_)>);
    let _ = source.add_event_listener_with_callback("message", handler.as_ref().unchecked_ref());
    handler.forget();
    Some(source)
}
