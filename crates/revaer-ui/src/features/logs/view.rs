//! Logs page view.
//!
//! # Design
//! - Stream logs only while the page is mounted.
//! - Keep the log buffer bounded to avoid unbounded memory growth.
//! - Surface connection state in a lightweight badge.

use std::collections::VecDeque;

use crate::app::logs_sse::{LogStreamHandle, connect_log_stream};
use crate::components::atoms::SearchInput;
use crate::components::daisy::filter::FilterOption;
use crate::components::daisy::{DaisySize, Filter};
use crate::core::auth::AuthState;
use crate::features::logs::ansi::{AnsiSpan, AnsiStyle};
use crate::features::logs::logic::{
    LogLevelFilter, LogLine, build_log_line, filter_lines, prune_old_lines,
};
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use gloo_timers::callback::Interval;
use js_sys::Date;
use yew::prelude::*;

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
    let lines_ref = use_mut_ref(VecDeque::new);
    let render_tick = use_state(|| 0u64);
    let status = use_state(|| LogStreamStatus::Connecting);
    let handle_ref = use_mut_ref(|| None as Option<LogStreamHandle>);
    let level_filter = use_state(|| LogLevelFilter::All);
    let search_query = use_state(String::new);

    {
        let base_url = props.base_url.clone();
        let auth_state = props.auth_state.clone();
        let on_error_toast = props.on_error_toast.clone();
        let lines_ref = lines_ref.clone();
        let render_tick = render_tick.clone();
        let status = status.clone();
        let handle_ref = handle_ref.clone();
        use_effect_with((base_url, auth_state), move |deps| {
            let (base_url, auth_state) = deps;
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
                    let now_ms = now_ms();
                    if let Some(entry) = build_log_line(&line, now_ms) {
                        {
                            let mut buffer = lines_ref.borrow_mut();
                            buffer.push_front(entry);
                            prune_old_lines(&mut buffer, now_ms);
                        }
                        let next_tick = (*render_tick).wrapping_add(1);
                        render_tick.set(next_tick);
                    }
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
        });
    }

    {
        let lines_ref = lines_ref.clone();
        let render_tick = render_tick.clone();
        use_effect(move || {
            let handle = Interval::new(5_000, move || {
                let now_ms = now_ms();
                let removed = {
                    let mut buffer = lines_ref.borrow_mut();
                    prune_old_lines(&mut buffer, now_ms)
                };
                if removed {
                    let next_tick = (*render_tick).wrapping_add(1);
                    render_tick.set(next_tick);
                }
            });
            move || drop(handle)
        });
    }

    let status_badge = match &*status {
        LogStreamStatus::Connecting => {
            ("badge badge-warning", bundle.text("logs.status_connecting"))
        }
        LogStreamStatus::Live => ("badge badge-success", bundle.text("logs.status_live")),
        LogStreamStatus::Error(_) => ("badge badge-error", bundle.text("logs.status_error")),
    };

    let level_filter_value = *level_filter;
    let level_options = vec![
        FilterOption {
            label: AttrValue::from(bundle.text("logs.level_all")),
            value: AttrValue::from(LogLevelFilter::All.as_value()),
            selected: level_filter_value == LogLevelFilter::All,
            reset: true,
        },
        FilterOption {
            label: AttrValue::from(bundle.text("settings.telemetry.level.error")),
            value: AttrValue::from(LogLevelFilter::Error.as_value()),
            selected: level_filter_value == LogLevelFilter::Error,
            reset: false,
        },
        FilterOption {
            label: AttrValue::from(bundle.text("settings.telemetry.level.warn")),
            value: AttrValue::from(LogLevelFilter::Warn.as_value()),
            selected: level_filter_value == LogLevelFilter::Warn,
            reset: false,
        },
        FilterOption {
            label: AttrValue::from(bundle.text("settings.telemetry.level.info")),
            value: AttrValue::from(LogLevelFilter::Info.as_value()),
            selected: level_filter_value == LogLevelFilter::Info,
            reset: false,
        },
        FilterOption {
            label: AttrValue::from(bundle.text("settings.telemetry.level.debug")),
            value: AttrValue::from(LogLevelFilter::Debug.as_value()),
            selected: level_filter_value == LogLevelFilter::Debug,
            reset: false,
        },
        FilterOption {
            label: AttrValue::from(bundle.text("settings.telemetry.level.trace")),
            value: AttrValue::from(LogLevelFilter::Trace.as_value()),
            selected: level_filter_value == LogLevelFilter::Trace,
            reset: false,
        },
    ];
    let search_term = (*search_query).clone();
    let lines_snapshot = lines_ref.borrow();
    let filtered_lines: Vec<&LogLine> =
        filter_lines(&lines_snapshot, level_filter_value, &search_term);

    let log_body = if filtered_lines.is_empty() {
        html! {
            <p class="text-base-content/60">{bundle.text("logs.empty")}</p>
        }
    } else {
        html! {
            <div class="space-y-1">
                {for filtered_lines.iter().map(|line| render_log_line(*line))}
            </div>
        }
    };

    html! {
        <section class="flex h-full min-h-0 flex-1 flex-col gap-4">
            {match &*status {
                LogStreamStatus::Error(message) => html! {
                    <div role="alert" class="alert alert-error">
                        <span>{message.clone()}</span>
                    </div>
                },
                _ => html! {},
            }}
            <div class="card bg-base-100 shadow grow min-h-0">
                <div class="card-body p-4 flex min-h-0 flex-col gap-3">
                    <div class="flex flex-wrap items-center justify-between gap-3">
                        <div class="flex flex-wrap items-center gap-2">
                            <div data-testid="logs-level-filter">
                                <Filter
                                    class={classes!("gap-1")}
                                    button_class={classes!("btn-sm")}
                                    options={level_options}
                                    on_select={{
                                        let level_filter = level_filter.clone();
                                        Callback::from(move |value: AttrValue| {
                                            level_filter.set(LogLevelFilter::from_value(&value));
                                        })
                                    }}
                                />
                            </div>
                            <SearchInput
                                aria_label={Some(AttrValue::from(bundle.text("nav.search")))}
                                placeholder={Some(AttrValue::from(bundle.text("nav.search")))}
                                value={AttrValue::from((*search_query).clone())}
                                debounce_ms={200}
                                size={DaisySize::Sm}
                                class="input-bordered"
                                input_class="w-40 sm:w-64"
                                on_search={{
                                    let search_query = search_query.clone();
                                    Callback::from(move |value: String| {
                                        search_query.set(value);
                                    })
                                }}
                            />
                        </div>
                        <span class={status_badge.0}>{status_badge.1}</span>
                    </div>
                    <div class="log-terminal rounded-box border border-base-200 p-3 text-xs font-mono leading-5 overflow-auto flex-1">
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

fn now_ms() -> u64 {
    Date::now() as u64
}
