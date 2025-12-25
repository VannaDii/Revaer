use crate::core::store::{SseConnectionState, SseStatus, SseStatusSummary};
use gloo::events::EventListener;
use gloo::utils::window;
use js_sys::Date;
use wasm_bindgen::JsCast;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct ConnectivityIndicatorProps {
    pub summary: SseStatusSummary,
    pub on_open: Callback<()>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(ConnectivityIndicator)]
pub(crate) fn connectivity_indicator(props: &ConnectivityIndicatorProps) -> Html {
    let (icon, status_class, title) = indicator_style(&props.summary);
    let on_open = {
        let on_open = props.on_open.clone();
        Callback::from(move |_| on_open.emit(()))
    };

    html! {
        <button
            class={classes!("btn", "btn-ghost", "btn-xs", "gap-2", props.class.clone())}
            title={title}
            onclick={on_open}
        >
            <span class={classes!("iconify", icon, "size-4")}></span>
            <span class={classes!("status", status_class, "status-sm")}></span>
        </button>
    }
}

#[derive(Properties, PartialEq)]
pub(crate) struct ConnectivityModalProps {
    pub status: SseStatus,
    pub on_retry: Callback<()>,
    pub on_dismiss: Callback<()>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(ConnectivityModal)]
pub(crate) fn connectivity_modal(props: &ConnectivityModalProps) -> Html {
    let on_dismiss = props.on_dismiss.clone();
    let on_retry = props.on_retry.clone();
    let on_dismiss_click = {
        let on_dismiss = on_dismiss.clone();
        Callback::from(move |_| on_dismiss.emit(()))
    };
    use_effect(move || {
        let listener = EventListener::new(&window(), "keydown", move |event| {
            let Some(keyboard) = event.dyn_ref::<web_sys::KeyboardEvent>() else {
                return;
            };
            if keyboard.key() == "Escape" {
                on_dismiss.emit(());
            }
        });
        move || drop(listener)
    });

    let status_label = match props.status.state {
        SseConnectionState::Connected => "Connected",
        SseConnectionState::Disconnected => "Disconnected",
        SseConnectionState::Reconnecting => "Reconnecting",
    };
    let retry_in = retry_in_seconds(props.status.next_retry_at_ms);
    let auth_mode = props
        .status
        .auth_mode
        .clone()
        .unwrap_or_else(|| "Unknown".to_string());
    let last_event_id = props
        .status
        .last_event_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "none".to_string());
    let last_error = props
        .status
        .last_error
        .as_ref()
        .map(|err| format_error(err))
        .unwrap_or_else(|| "none".to_string());
    let (icon, status_class, _) = indicator_style(&SseStatusSummary {
        state: props.status.state,
        next_retry_at_ms: props.status.next_retry_at_ms,
        has_error: props.status.last_error.is_some(),
    });

    html! {
        <div class={classes!("absolute", "bottom-14", "end-4", "z-50", "w-[min(90vw,24rem)]", props.class.clone())}>
            <div class="card bg-base-100 shadow border border-base-200" role="dialog" aria-modal="false">
                <div class="card-body gap-3 p-4">
                    <div class="flex items-start justify-between">
                        <div class="flex items-center gap-2">
                            <span class={classes!("iconify", icon, "size-4")}></span>
                            <span class={classes!("status", status_class, "status-sm")}></span>
                            <h3 class="text-sm font-semibold">{"Connectivity"}</h3>
                        </div>
                        <button
                            class="btn btn-ghost btn-xs btn-circle"
                            aria-label="Dismiss"
                            onclick={on_dismiss_click.clone()}>
                            <span class="iconify lucide--x size-4"></span>
                        </button>
                    </div>
                    <div class="grid gap-2 text-sm">
                        <div class="flex items-center justify-between">
                            <span class="text-base-content/70">{"Status"}</span>
                            <span>{status_label}</span>
                        </div>
                        <div class="flex items-center justify-between">
                            <span class="text-base-content/70">{"Auth mode"}</span>
                            <span>{auth_mode}</span>
                        </div>
                        <div class="flex items-center justify-between">
                            <span class="text-base-content/70">{"Next retry"}</span>
                            <span>{retry_in.unwrap_or_else(|| "n/a".to_string())}</span>
                        </div>
                        <div class="flex items-center justify-between">
                            <span class="text-base-content/70">{"Last event id"}</span>
                            <span>{last_event_id}</span>
                        </div>
                        <div class="flex items-center justify-between">
                            <span class="text-base-content/70">{"Last error"}</span>
                            <span class="text-end">{last_error}</span>
                        </div>
                        <div class="text-xs text-base-content/60">
                            {"Retry strategy: exponential backoff + jitter."}
                        </div>
                    </div>
                    <div class="mt-2 flex items-center justify-end gap-2">
                        <button
                            class="btn btn-sm btn-outline border-base-300"
                            onclick={{
                                let on_retry = on_retry.clone();
                                Callback::from(move |_| on_retry.emit(()))
                            }}>
                            {"Retry now"}
                        </button>
                        <button
                            class="btn btn-sm"
                            onclick={on_dismiss_click}>
                            {"Dismiss"}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    }
}

fn indicator_style(summary: &SseStatusSummary) -> (&'static str, &'static str, String) {
    let retry_label = retry_in_seconds(summary.next_retry_at_ms)
        .map(|value| format!("Reconnecting in {value}"))
        .unwrap_or_default();
    match summary.state {
        SseConnectionState::Connected => (
            "lucide--check-circle-2",
            "status-success",
            "Connected".to_string(),
        ),
        SseConnectionState::Reconnecting => (
            "lucide--loader",
            "status-warning",
            if retry_label.is_empty() {
                "Reconnecting".to_string()
            } else {
                retry_label
            },
        ),
        SseConnectionState::Disconnected => (
            "lucide--unplug",
            if summary.has_error {
                "status-error"
            } else {
                "status-warning"
            },
            "Disconnected".to_string(),
        ),
    }
}

fn retry_in_seconds(next_retry_at_ms: Option<u64>) -> Option<String> {
    let next_retry_at_ms = next_retry_at_ms?;
    let now = Date::now() as u64;
    let remaining_ms = next_retry_at_ms.saturating_sub(now);
    let remaining_secs = (remaining_ms + 999) / 1000;
    Some(format!("{remaining_secs}s"))
}

fn format_error(error: &crate::core::store::SseError) -> String {
    match error.status_code {
        Some(code) => format!("{} ({code})", error.message),
        None => error.message.clone(),
    }
}
