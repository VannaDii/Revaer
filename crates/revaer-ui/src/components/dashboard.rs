use crate::components::atoms::icons::{ArrowDirection, ArrowIcon, ChevronRightIcon, IconVariant};
use crate::core::store::SystemRates;
use crate::features::torrents::state::TorrentRow;
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::{DashboardSnapshot, EventKind};
use crate::{Density, UiMode};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct DashboardProps {
    pub snapshot: DashboardSnapshot,
    pub system_rates: SystemRates,
    pub mode: UiMode,
    pub density: Density,
    pub torrents: Vec<TorrentRow>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(DashboardPanel)]
pub(crate) fn dashboard_panel(props: &DashboardProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
    let tf = |key: &str, default: &str| bundle.text(key, default);
    let density_class = match props.density {
        Density::Compact => "density-compact",
        Density::Normal => "density-normal",
        Density::Comfy => "density-comfy",
    };
    let mode_class = match props.mode {
        UiMode::Simple => "mode-simple",
        UiMode::Advanced => "mode-advanced",
    };
    let disk_used_pct = usage_pct(props.snapshot.disk_used_gb, props.snapshot.disk_total_gb);
    let torrents = props.torrents.iter().take(7).cloned().collect::<Vec<_>>();

    html! {
        <section
            class={classes!("dashboard-canvas", density_class, mode_class, props.class.clone())}
        >
            <div class="dashboard-heading">
                <div>
                    <p class="eyebrow">{tf("nav.dashboard", "Dashboard")}</p>
                    <h1>{tf("dashboard.title", "Dashboard")}</h1>
                </div>
                <div class="chip chip-success">
                    <span class="dot" />
                    <span>{bundle.text("shell.network_connected", "Connected")}</span>
                </div>
            </div>

            <div class="metric-grid">
                <div class="metric-card gradient-blue">
                    <div class="metric-top">
                        <div>
                            <p class="metric-label">{tf("dashboard.up", "Global upload")}</p>
                        <h2 class="metric-value">{format_speed(props.system_rates.upload_bps)}</h2>
                        </div>
                        {arrow_icon(true)}
                    </div>
                    <p class="metric-sub">{tf("dashboard.speeds", "Live throughput")}</p>
                    <div class="progress-wrap">
                        <span class="progress-label">{tf("dashboard.live", "Live")}</span>
                        <div class="progress-track">
                            <div class="progress-bar accent" style={format!("width: {:.1}%", load_pct(props.system_rates.upload_bps))} />
                        </div>
                    </div>
                </div>
                <div class="metric-card gradient-indigo">
                    <div class="metric-top">
                        <div>
                            <p class="metric-label">{tf("dashboard.down", "Global download")}</p>
                        <h2 class="metric-value">{format_speed(props.system_rates.download_bps)}</h2>
                        </div>
                        {arrow_icon(false)}
                    </div>
                    <p class="metric-sub">{tf("dashboard.speeds_subtext", "Session peak")}</p>
                    <div class="progress-wrap">
                        <span class="progress-label">{tf("dashboard.live", "Live")}</span>
                        <div class="progress-track">
                            <div class="progress-bar primary" style={format!("width: {:.1}%", load_pct(props.system_rates.download_bps))} />
                        </div>
                    </div>
                </div>
                <div class="metric-card gradient-slate">
                    <div class="metric-top">
                        <div>
                            <p class="metric-label">{tf("dashboard.torrents", "Torrents")}</p>
                            <h2 class="metric-value">{props.snapshot.active + props.snapshot.paused}</h2>
                        </div>
                        <div class="chip chip-ghost">{tf("dashboard.active", "Active")}</div>
                    </div>
                    <div class="dual-meta">
                        <div>
                            <small class="muted">{tf("dashboard.active", "Active")}</small>
                            <strong>{props.snapshot.active}</strong>
                        </div>
                        <div>
                            <small class="muted">{tf("dashboard.completed", "Completed")}</small>
                            <strong>{props.snapshot.completed}</strong>
                        </div>
                    </div>
                    <div class="dual-meta">
                        <div>
                            <small class="muted">{tf("dashboard.paused", "Paused")}</small>
                            <strong>{props.snapshot.paused}</strong>
                        </div>
                        <div>
                            <small class="muted">{tf("dashboard.queue", "Queued")}</small>
                            <strong>{props.snapshot.queue.queued}</strong>
                        </div>
                    </div>
                </div>
            </div>

            <div class="wide-grid">
                <div class="panel card-bleed">
                    <div class="panel-head">
                        <div>
                            <p class="eyebrow">{tf("dashboard.disk", "Disk usage")}</p>
                            <h3>{format_storage(props.snapshot.disk_used_gb, props.snapshot.disk_total_gb)}</h3>
                        </div>
                        <div class="chip chip-soft">{format!("{:.0}% used", disk_used_pct)}</div>
                    </div>
                    <div class="progress-track thick">
                        <div class="progress-bar primary" style={format!("width: {:.1}%", disk_used_pct)} />
                    </div>
                    <div class="path-grid">
                        {for props.snapshot.paths.iter().map(|path| {
                            let pct = usage_pct(path.used_gb, path.total_gb);
                            html! {
                                <div class="path-item">
                                    <div class="path-label">
                                        <span>{path.label}</span>
                                        <span class="muted">{format!("{} GB / {} GB", path.used_gb, path.total_gb)}</span>
                                    </div>
                                    <div class="progress-track thin">
                                        <div class="progress-bar accent" style={format!("width: {:.1}%", pct)} />
                                    </div>
                                </div>
                            }
                        })}
                    </div>
                </div>
                <div class="panel card-bleed vpn-card">
                    <div class="panel-head">
                        <div class="chip chip-ghost">{"V1"}</div>
                        <ChevronRightIcon class={classes!("chevron")} size={18} />
                    </div>
                    <div class="vpn-state">
                        <p class="eyebrow">{tf("dashboard.vpn", "VPN")}</p>
                        <h3>{props.snapshot.vpn.state.clone()}</h3>
                        <p class="muted">{props.snapshot.vpn.message.clone()}</p>
                        <span class="pill subtle">{props.snapshot.vpn.last_change.clone()}</span>
                    </div>
                </div>
            </div>

            <div class="lower-grid">
                <div class="panel">
                    <div class="panel-head">
                        <h3>{tf("dashboard.events", "Recent Events")}</h3>
                        <span class="muted">{tf("dashboard.events_sub", "Last 24 hours")}</span>
                    </div>
                    <div class="events-feed">
                        {if props.snapshot.recent_events.is_empty() {
                            html! { <p class="muted">{tf("dashboard.no_events", "No recent events")}</p> }
                        } else {
                            html! { for props.snapshot.recent_events.iter().map(|event| {
                                let tone = match event.kind {
                                    EventKind::Info => "pill ghost",
                                    EventKind::Warning => "pill warn",
                                    EventKind::Error => "pill error",
                                };
                                html! {
                                    <div class="event-row">
                                        <span class={tone}>
                                            {match event.kind {
                                                EventKind::Info => t("dashboard.event_info"),
                                                EventKind::Warning => t("dashboard.event_warn"),
                                                EventKind::Error => t("dashboard.event_error"),
                                            }}
                                        </span>
                                        <div>
                                            <strong>{event.label}</strong>
                                            <p class="muted">{event.detail}</p>
                                        </div>
                                    </div>
                                }
                            }) }
                        }}
                    </div>
                </div>
                <div class="panel tracker-panel">
                    <div class="panel-head">
                        <h3>{tf("dashboard.tracker_health", "Tracker Health")}</h3>
                    </div>
                    <div class="tracker-legend">
                        <span class="legend ok"><span class="dot" />{tf("dashboard.ok", "Ok")}</span>
                        <span class="legend warn"><span class="dot" />{tf("dashboard.warn", "Warning")}</span>
                        <span class="legend error"><span class="dot" />{tf("dashboard.error", "Error")}</span>
                    </div>
                    <div class="stat-row">
                        <div>
                            <strong class="ok">{props.snapshot.tracker_health.ok}</strong>
                            <small class="muted">{tf("dashboard.ok", "Ok")}</small>
                        </div>
                        <div>
                            <strong class="warn">{props.snapshot.tracker_health.warn}</strong>
                            <small class="muted">{tf("dashboard.warn", "Warning")}</small>
                        </div>
                        <div>
                            <strong class="error">{props.snapshot.tracker_health.error}</strong>
                            <small class="muted">{tf("dashboard.error", "Error")}</small>
                        </div>
                    </div>
                </div>
                <div class="panel queue-panel">
                    <div class="panel-head">
                        <h3>{tf("dashboard.queue", "Queue Status")}</h3>
                    </div>
                    <div class="queue-bars">
                        {queue_bar(props.snapshot.queue.active, "Active", "primary")}
                        {queue_bar(props.snapshot.queue.paused, "Paused", "warn")}
                        {queue_bar(props.snapshot.queue.queued, "Queued", "accent")}
                        {queue_bar(props.snapshot.queue.depth, "Depth", "ghost")}
                    </div>
                </div>
            </div>

            <div class="panel table-panel">
                <div class="panel-head">
                    <div>
                        <p class="eyebrow">{tf("dashboard.torrents", "Torrents")}</p>
                        <h3>{tf("dashboard.activity", "Live transfers")}</h3>
                    </div>
                    <div class="chip chip-ghost">{format!("{} {}", tf("dashboard.active", "Active"), props.snapshot.active)}</div>
                </div>
                <div class="table-head compact">
                    <span>{tf("table.name", "Name")}</span>
                    <span>{tf("table.status", "Status")}</span>
                    <span>{tf("table.progress", "Progress")}</span>
                    <span>{tf("table.dl", "DL")}</span>
                    <span>{tf("table.ul", "UL")}</span>
                </div>
                <div class="table-body">
                    {if torrents.is_empty() {
                        html! { <p class="muted">{tf("dashboard.no_torrents", "No torrents yet")}</p> }
                    } else {
                        html! { for torrents.iter().map(|row| {
                            let progress_pct = (row.progress * 100.0).min(100.0);
                            html! {
                                <div class="table-row compact">
                                    <div class="cell name">
                                        <strong class="ellipsis">{&row.name}</strong>
                                        <p class="muted ellipsis">{row.tracker.clone()}</p>
                                    </div>
                                    <div class={classes!("cell", status_class(&row.status))}>{&row.status}</div>
                                    <div class="cell progress-cell">
                                        <div class="progress-track thin">
                                            <div class="progress-bar accent" style={format!("width: {:.1}%", progress_pct)} />
                                        </div>
                                        <small class="muted">{format!("{progress_pct:.0}%")}</small>
                                    </div>
                                    <div class="cell rate">{format_speed(row.download_bps)}</div>
                                    <div class="cell rate">{format_speed(row.upload_bps)}</div>
                                </div>
                            }
                        }) }
                    }}
                </div>
            </div>
        </section>
    }
}

fn format_speed(value: u64) -> String {
    const ONE_MB: f64 = 1_000_000.0;
    const ONE_GB: f64 = 1_000_000_000.0;
    if value as f64 >= ONE_GB {
        format!("{:.1} GB/s", value as f64 / ONE_GB)
    } else if value as f64 >= ONE_MB {
        format!("{:.1} MB/s", value as f64 / ONE_MB)
    } else {
        format!("{value} B/s")
    }
}

fn format_storage(used_gb: u32, total_gb: u32) -> String {
    const GB_PER_TB: f64 = 1024.0;
    let used_tb = used_gb as f64 / GB_PER_TB;
    let total_tb = total_gb as f64 / GB_PER_TB;
    format!("{used_tb:.1} / {total_tb:.1} TB")
}

fn usage_pct(used: u32, total: u32) -> f32 {
    if total == 0 {
        0.0
    } else {
        (used as f32 / total as f32).min(1.0) * 100.0
    }
}

fn load_pct(rate: u64) -> f32 {
    const MAX_VIEW: f64 = 200_000_000.0;
    ((rate as f64 / MAX_VIEW).min(1.0) * 100.0) as f32
}

fn status_class(status: &str) -> &'static str {
    match status.to_ascii_lowercase().as_str() {
        "paused" => "status paused",
        "downloading" => "status downloading",
        "seeding" => "status seeding",
        "active" => "status active",
        _ => "status ghost",
    }
}

fn arrow_icon(up: bool) -> Html {
    let direction = if up {
        ArrowDirection::Up
    } else {
        ArrowDirection::Down
    };
    let variant = if up {
        IconVariant::Solid
    } else {
        IconVariant::Outline
    };
    html! {
        <ArrowIcon
            direction={direction}
            variant={variant}
            class={classes!("metric-icon")}
            size={18}
        />
    }
}

fn queue_bar(value: u16, label: &str, tone: &'static str) -> Html {
    let width = ((value as f32 / 40.0).min(1.0) * 100.0).max(6.0);
    html! {
        <div class="queue-bar">
            <div class={classes!("bar", tone)} style={format!("width: {:.1}%", width)} />
            <small class="muted">{format!("{label} ({value})")}</small>
        </div>
    }
}
