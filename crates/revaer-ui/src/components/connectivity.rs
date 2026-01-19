use crate::components::atoms::IconButton;
use crate::components::atoms::icons::{IconCheckCircle2, IconLoader, IconUnplug, IconX};
use crate::components::daisy::DaisySize;
use crate::core::logic::connectivity::{
    IndicatorIcon, format_error, indicator_style, retry_in_seconds,
};
use crate::core::store::{SseConnectionState, SseStatus, SseStatusSummary};
use gloo::console;
use js_sys::Date;
use web_sys::{HtmlElement, KeyboardEvent};
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
    let now_ms = Date::now() as u64;
    let style = indicator_style(&props.summary, now_ms);
    let title = AttrValue::from(style.title);
    let on_open = {
        let on_open = props.on_open.clone();
        Callback::from(move |_| on_open.emit(()))
    };

    html! {
        <button
            type="button"
            class={classes!(
                "btn",
                "btn-ghost",
                "btn-sm",
                "btn-circle",
                "tooltip",
                "tooltip-right",
                "sse-indicator",
                props.class.clone()
            )}
            title={title.clone()}
            aria-label={title.clone()}
            data-tip={title.clone()}
            onclick={on_open}
        >
            <span class="indicator">
                <span class={classes!("indicator-item", "status", style.status_class, "status-sm")}></span>
                {match style.icon {
                    IndicatorIcon::Connected => html! {
                        <IconCheckCircle2 class="text-base-content/80" size={Some(AttrValue::from("4"))} />
                    },
                    IndicatorIcon::Reconnecting => html! {
                        <IconLoader class="text-base-content/80" size={Some(AttrValue::from("4"))} />
                    },
                    IndicatorIcon::Disconnected => html! {
                        <IconUnplug class="text-base-content/80" size={Some(AttrValue::from("4"))} />
                    },
                }}
            </span>
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
    let dialog_ref = use_node_ref();
    let on_dismiss = props.on_dismiss.clone();
    let on_retry = props.on_retry.clone();
    let on_dismiss_click = {
        let on_dismiss = on_dismiss.clone();
        Callback::from(move |_| on_dismiss.emit(()))
    };
    let on_keydown = {
        let on_dismiss = on_dismiss.clone();
        Callback::from(move |event: KeyboardEvent| {
            if event.key() == "Escape" {
                on_dismiss.emit(());
            }
        })
    };
    {
        let dialog_ref = dialog_ref.clone();
        use_effect_with((), move |_| {
            if let Some(dialog) = dialog_ref.cast::<HtmlElement>() {
                if let Err(err) = dialog.focus() {
                    console::error!("connectivity modal focus failed", err);
                }
            }
            || ()
        });
    }

    let status_label = match props.status.state {
        SseConnectionState::Connected => "Live",
        SseConnectionState::Disconnected => "Disconnected",
        SseConnectionState::Reconnecting => "Reconnecting",
    };
    let now_ms = Date::now() as u64;
    let retry_in = retry_in_seconds(props.status.next_retry_at_ms, now_ms);
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
    let style = indicator_style(
        &SseStatusSummary {
            state: props.status.state,
            next_retry_at_ms: props.status.next_retry_at_ms,
            has_error: props.status.last_error.is_some(),
        },
        now_ms,
    );

    html! {
        <dialog
            open={true}
            ref={dialog_ref}
            tabindex={0}
            onkeydown={on_keydown}
            class={classes!(
                "modal",
                "modal-bottom",
                "pointer-events-none",
                props.class.clone()
            )}
            role="dialog"
            aria-modal="false">
            <div class="modal-box pointer-events-auto w-[min(90vw,24rem)] p-0">
                <div class="card bg-base-100 shadow border border-base-200">
                    <div class="card-body gap-3 p-4">
                        <div class="flex items-start justify-between">
                            <div class="flex items-center gap-2">
                                {match style.icon {
                                    IndicatorIcon::Connected => html! {
                                        <IconCheckCircle2 size={Some(AttrValue::from("4"))} />
                                    },
                                    IndicatorIcon::Reconnecting => html! {
                                        <IconLoader size={Some(AttrValue::from("4"))} />
                                    },
                                    IndicatorIcon::Disconnected => html! {
                                        <IconUnplug size={Some(AttrValue::from("4"))} />
                                    },
                                }}
                                <span class={classes!("status", style.status_class, "status-sm")}></span>
                                <h3 class="text-sm font-semibold">{"Connectivity"}</h3>
                            </div>
                            <IconButton
                                icon={html! { <IconX size={Some(AttrValue::from("4"))} /> }}
                                label={AttrValue::from("Dismiss")}
                                size={DaisySize::Xs}
                                circle={true}
                                onclick={on_dismiss_click.clone()}
                            />
                        </div>
                        <div class="grid gap-2 text-sm">
                            <div class="flex items-center justify-between">
                                <span class="text-base-content/70">{"Current status"}</span>
                                <span>{status_label}</span>
                            </div>
                            <div class="flex items-center justify-between">
                                <span class="text-base-content/70">{"Next retry time"}</span>
                                <span>{retry_in.unwrap_or_else(|| "n/a".to_string())}</span>
                            </div>
                            <div class="flex items-center justify-between">
                                <span class="text-base-content/70">{"Last event ID"}</span>
                                <span>{last_event_id}</span>
                            </div>
                            <div class="flex items-center justify-between">
                                <span class="text-base-content/70">{"Last error reason"}</span>
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
        </dialog>
    }
}
