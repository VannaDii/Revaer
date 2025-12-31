//! Logs page view.
//!
//! # Design
//! - Stream logs only while the page is mounted.
//! - Keep the log buffer bounded to avoid unbounded memory growth.
//! - Surface connection state in a lightweight badge.

use std::collections::VecDeque;

use crate::app::logs_sse::{LogStreamHandle, connect_log_stream};
use crate::core::auth::AuthState;
use crate::features::logs::ansi::{AnsiSpan, AnsiStyle, parse_ansi_line};
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

#[derive(Clone, PartialEq, Eq)]
struct LogLine {
    spans: Vec<AnsiSpan>,
}

#[function_component(LogsPage)]
pub(crate) fn logs_page(props: &LogsPageProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let lines_ref = use_mut_ref(VecDeque::new);
    let render_tick = use_state(|| 0u64);
    let status = use_state(|| LogStreamStatus::Connecting);
    let handle_ref = use_mut_ref(|| None as Option<LogStreamHandle>);

    {
        let base_url = props.base_url.clone();
        let auth_state = props.auth_state.clone();
        let on_error_toast = props.on_error_toast.clone();
        let lines_ref = lines_ref.clone();
        let render_tick = render_tick.clone();
        let status = status.clone();
        let handle_ref = handle_ref.clone();
        use_effect_with_deps(
            move |(base_url, auth_state)| {
                status.set(LogStreamStatus::Connecting);
                let on_line = {
                    let lines_ref = lines_ref.clone();
                    let render_tick = render_tick.clone();
                    let status = status.clone();
                    Callback::from(move |line: String| {
                        if line.trim().is_empty() {
                            return;
                        }
                        status.set(LogStreamStatus::Live);
                        let spans = parse_ansi_line(&line);
                        {
                            let mut buffer = lines_ref.borrow_mut();
                            buffer.push_front(LogLine { spans });
                            while buffer.len() > MAX_LOG_LINES {
                                buffer.pop_back();
                            }
                        }
                        let next_tick = (*render_tick).wrapping_add(1);
                        render_tick.set(next_tick);
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

    let lines_snapshot = lines_ref.borrow();
    let log_body = if lines_snapshot.is_empty() {
        html! {
            <p class="text-base-content/60">{bundle.text("logs.empty")}</p>
        }
    } else {
        html! {
            <div class="space-y-1">
                {for lines_snapshot.iter().map(render_log_line)}
            </div>
        }
    };

    html! {
        <section class="flex h-full min-h-0 flex-col gap-4">
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
            <div class="card bg-base-100 shadow grow min-h-0">
                <div class="card-body p-4 flex min-h-0 flex-col">
                    <div class="log-terminal rounded-box border border-base-200 p-3 text-xs font-mono leading-5 overflow-auto min-h-0 flex-1">
                        {log_body}
                    </div>
                </div>
            </div>
        </section>
    }
}

fn render_log_line(line: &LogLine) -> Html {
    html! {
        <div class="whitespace-pre-wrap">
            {for line.spans.iter().map(render_log_span)}
        </div>
    }
}

fn render_log_span(span: &AnsiSpan) -> Html {
    if span.text.is_empty() {
        return html! {};
    }
    let classes = span_classes(&span.style);
    let style = span_style(&span.style);
    html! {
        <span class={classes} style={style}>{span.text.clone()}</span>
    }
}

fn span_classes(style: &AnsiStyle) -> Classes {
    let mut classes = Classes::new();
    if style.is_bold() {
        classes.push("font-semibold");
    }
    if style.is_dim() {
        classes.push("opacity-70");
    }
    if style.is_italic() {
        classes.push("italic");
    }
    if style.is_underline() {
        classes.push("underline");
    }
    classes
}

fn span_style(style: &AnsiStyle) -> AttrValue {
    let (fg, bg) = style.resolved_colors();
    let mut parts = Vec::new();
    if let Some(fg) = fg {
        parts.push(format!("color: var({});", fg.css_var()));
    }
    if let Some(bg) = bg {
        parts.push(format!("background-color: var({});", bg.css_var()));
    }
    AttrValue::from(parts.join(" "))
}
