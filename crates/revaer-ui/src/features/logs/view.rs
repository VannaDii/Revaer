//! Logs page view.
//!
//! # Design
//! - Stream logs only while the page is mounted.
//! - Keep the log buffer bounded to avoid unbounded memory growth.
//! - Surface connection state in a lightweight badge.

use std::collections::VecDeque;

use crate::app::logs_sse::{LogStreamHandle, connect_log_stream};
use crate::components::atoms::SearchInput;
use crate::components::daisy::{DaisySize, Select};
use crate::core::auth::AuthState;
use crate::features::logs::ansi::{AnsiSpan, AnsiStyle, parse_ansi_line};
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use gloo_timers::callback::Interval;
use js_sys::Date;
use yew::prelude::*;

const LOG_WINDOW_MS: u64 = 120_000;

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LogLevelFilter {
    All,
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevelFilter {
    const fn as_value(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    fn from_value(value: &str) -> Self {
        match value {
            "trace" => Self::Trace,
            "debug" => Self::Debug,
            "info" => Self::Info,
            "warn" => Self::Warn,
            "error" => Self::Error,
            _ => Self::All,
        }
    }

    const fn matches(self, level: LogLevel) -> bool {
        match (self, level) {
            (Self::All, _) => true,
            (Self::Trace, LogLevel::Trace) => true,
            (Self::Debug, LogLevel::Debug) => true,
            (Self::Info, LogLevel::Info) => true,
            (Self::Warn, LogLevel::Warn) => true,
            (Self::Error, LogLevel::Error) => true,
            _ => false,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct LogLine {
    spans: Vec<AnsiSpan>,
    plain_lower: String,
    level: LogLevel,
    received_at_ms: u64,
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
                    let spans = parse_ansi_line(&line);
                    let plain = spans
                        .iter()
                        .map(|span| span.text.as_str())
                        .collect::<String>();
                    let level = detect_level(&plain);
                    let now_ms = now_ms();
                    {
                        let mut buffer = lines_ref.borrow_mut();
                        buffer.push_front(LogLine {
                            spans,
                            plain_lower: plain.to_lowercase(),
                            level,
                            received_at_ms: now_ms,
                        });
                        prune_old_lines(&mut buffer, now_ms);
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

    let level_options = vec![
        (
            AttrValue::from(LogLevelFilter::All.as_value()),
            AttrValue::from(bundle.text("logs.level_all")),
        ),
        (
            AttrValue::from(LogLevelFilter::Error.as_value()),
            AttrValue::from(bundle.text("telemetry.level.error")),
        ),
        (
            AttrValue::from(LogLevelFilter::Warn.as_value()),
            AttrValue::from(bundle.text("telemetry.level.warn")),
        ),
        (
            AttrValue::from(LogLevelFilter::Info.as_value()),
            AttrValue::from(bundle.text("telemetry.level.info")),
        ),
        (
            AttrValue::from(LogLevelFilter::Debug.as_value()),
            AttrValue::from(bundle.text("telemetry.level.debug")),
        ),
        (
            AttrValue::from(LogLevelFilter::Trace.as_value()),
            AttrValue::from(bundle.text("telemetry.level.trace")),
        ),
    ];

    let search_term = (*search_query).trim().to_lowercase();
    let level_filter_value = *level_filter;
    let lines_snapshot = lines_ref.borrow();
    let filtered_lines: Vec<&LogLine> = lines_snapshot
        .iter()
        .filter(|line| {
            level_filter_value.matches(line.level)
                && (search_term.is_empty() || line.plain_lower.contains(&search_term))
        })
        .collect();

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
                <div class="card-body p-4 flex min-h-0 flex-col gap-3">
                    <div class="flex flex-wrap items-center justify-between gap-3">
                        <div class="flex flex-wrap items-center gap-2">
                            <Select
                                aria_label={Some(AttrValue::from(bundle.text("telemetry.level.label")))}
                                value={Some(AttrValue::from(level_filter_value.as_value()))}
                                options={level_options}
                                size={DaisySize::Sm}
                                class="w-40"
                                onchange={{
                                    let level_filter = level_filter.clone();
                                    Callback::from(move |value: AttrValue| {
                                        level_filter.set(LogLevelFilter::from_value(&value));
                                    })
                                }}
                            />
                            <SearchInput
                                aria_label={Some(AttrValue::from(bundle.text("search")))}
                                placeholder={Some(AttrValue::from(bundle.text("search")))}
                                value={AttrValue::from((*search_query).clone())}
                                debounce_ms={200}
                                size={DaisySize::Sm}
                                input_class="w-40 sm:w-64"
                                on_search={{
                                    let search_query = search_query.clone();
                                    Callback::from(move |value: String| {
                                        search_query.set(value);
                                    })
                                }}
                            />
                        </div>
                    </div>
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

fn detect_level(line: &str) -> LogLevel {
    let trimmed = line.trim();
    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(level) = value.get("level").and_then(|value| value.as_str()) {
                if let Some(parsed) = level_from_token(level) {
                    return parsed;
                }
            }
        }
    }
    for token in trimmed.split_whitespace() {
        if let Some(parsed) = level_from_token(token) {
            return parsed;
        }
    }
    LogLevel::Unknown
}

fn level_from_token(token: &str) -> Option<LogLevel> {
    let normalized = token.trim_matches(|ch: char| !ch.is_ascii_alphabetic());
    if normalized.eq_ignore_ascii_case("trace") {
        Some(LogLevel::Trace)
    } else if normalized.eq_ignore_ascii_case("debug") {
        Some(LogLevel::Debug)
    } else if normalized.eq_ignore_ascii_case("info") {
        Some(LogLevel::Info)
    } else if normalized.eq_ignore_ascii_case("warn") || normalized.eq_ignore_ascii_case("warning")
    {
        Some(LogLevel::Warn)
    } else if normalized.eq_ignore_ascii_case("error") {
        Some(LogLevel::Error)
    } else {
        None
    }
}

fn prune_old_lines(lines: &mut VecDeque<LogLine>, now_ms: u64) -> bool {
    let cutoff = now_ms.saturating_sub(LOG_WINDOW_MS);
    let mut removed = false;
    while matches!(lines.back(), Some(line) if line.received_at_ms < cutoff) {
        lines.pop_back();
        removed = true;
    }
    removed
}

fn now_ms() -> u64 {
    Date::now() as u64
}
