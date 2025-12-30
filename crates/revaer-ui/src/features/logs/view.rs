//! Logs page view.
//!
//! # Design
//! - Stream logs only while the page is mounted.
//! - Keep the log buffer bounded to avoid unbounded memory growth.
//! - Surface connection state in a lightweight badge.

use std::collections::VecDeque;

use crate::app::logs_sse::{LogStreamHandle, connect_log_stream};
use crate::core::auth::AuthState;
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use yew::prelude::*;

const MAX_LOG_LINES: usize = 600;

#[derive(Properties, PartialEq)]
pub(crate) struct LogsPageProps {
    pub base_url: String,
    pub auth_state: Option<AuthState>,
    pub on_error_toast: Callback<String>,
}

#[derive(Clone, PartialEq, Eq)]
enum LogStreamStatus {
    Connecting,
    Live,
    Error(String),
}

#[function_component(LogsPage)]
pub(crate) fn logs_page(props: &LogsPageProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let lines = use_state(VecDeque::new);
    let status = use_state(|| LogStreamStatus::Connecting);
    let handle_ref = use_mut_ref(|| None as Option<LogStreamHandle>);

    {
        let base_url = props.base_url.clone();
        let auth_state = props.auth_state.clone();
        let on_error_toast = props.on_error_toast.clone();
        let lines = lines.clone();
        let status = status.clone();
        let handle_ref = handle_ref.clone();
        use_effect_with_deps(
            move |(base_url, auth_state)| {
                status.set(LogStreamStatus::Connecting);
                let on_line = {
                    let lines = lines.clone();
                    let status = status.clone();
                    Callback::from(move |line: String| {
                        if line.trim().is_empty() {
                            return;
                        }
                        status.set(LogStreamStatus::Live);
                        lines.set(push_line(lines.clone(), line));
                    })
                };
                let on_error = {
                    let status = status.clone();
                    let on_error_toast = on_error_toast.clone();
                    Callback::from(move |message: String| {
                        let toast_message = message.clone();
                        status.set(LogStreamStatus::Error(message));
                        on_error_toast.emit(toast_message);
                    })
                };
                let handle =
                    connect_log_stream(base_url.clone(), auth_state.clone(), on_line, on_error);
                *handle_ref.borrow_mut() = handle;
                move || {
                    if let Some(handle) = handle_ref.borrow_mut().take() {
                        handle.close();
                    }
                }
            },
            (base_url, auth_state),
        );
    }

    let status_badge = match &*status {
        LogStreamStatus::Connecting => {
            ("badge badge-warning", bundle.text("logs.status_connecting"))
        }
        LogStreamStatus::Live => ("badge badge-success", bundle.text("logs.status_live")),
        LogStreamStatus::Error(_) => ("badge badge-error", bundle.text("logs.status_error")),
    };

    html! {
        <section class="space-y-4">
            <div class="flex flex-wrap items-center justify-between gap-3">
                <div>
                    <p class="text-lg font-medium">{bundle.text("logs.title")}</p>
                    <p class="text-sm text-base-content/60">{bundle.text("logs.subtitle")}</p>
                </div>
                <span class={status_badge.0}>{status_badge.1}</span>
            </div>
            {match &*status {
                LogStreamStatus::Error(message) => html! {
                    <div role="alert" class="alert alert-error">
                        <span>{message.clone()}</span>
                    </div>
                },
                _ => html! {},
            }}
            <div class="card bg-base-100 shadow">
                <div class="card-body p-4">
                    <div class="rounded-box border border-base-200 bg-base-200/40 p-3 text-xs font-mono leading-5 max-h-[65vh] overflow-auto">
                        {if lines.is_empty() {
                            html! {
                                <p class="text-base-content/60">{bundle.text("logs.empty")}</p>
                            }
                        } else {
                            html! {
                                <div class="space-y-1">
                                    {for lines.iter().map(|line| html! {
                                        <div class="whitespace-pre-wrap">{line.clone()}</div>
                                    })}
                                </div>
                            }
                        }}
                    </div>
                </div>
            </div>
        </section>
    }
}

fn push_line(lines: UseStateHandle<VecDeque<String>>, line: String) -> VecDeque<String> {
    let mut next = (*lines).clone();
    next.push_back(line);
    while next.len() > MAX_LOG_LINES {
        next.pop_front();
    }
    next
}
