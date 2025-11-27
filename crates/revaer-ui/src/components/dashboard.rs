use crate::{Density, UiMode};
use yew::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub struct DashboardSnapshot {
    pub download_bps: u64,
    pub upload_bps: u64,
    pub active: u32,
    pub paused: u32,
    pub completed: u32,
    pub disk_total_gb: u32,
    pub disk_used_gb: u32,
    pub paths: Vec<PathUsage>,
    pub recent_events: Vec<DashboardEvent>,
    pub tracker_health: TrackerHealth,
    pub queue: QueueStatus,
    pub vpn: VpnState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PathUsage {
    pub label: &'static str,
    pub used_gb: u32,
    pub total_gb: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DashboardEvent {
    pub label: &'static str,
    pub detail: &'static str,
    pub kind: EventKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EventKind {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrackerHealth {
    pub ok: u16,
    pub warn: u16,
    pub error: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueueStatus {
    pub active: u16,
    pub paused: u16,
    pub queued: u16,
    pub depth: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VpnState {
    pub state: &'static str,
    pub message: &'static str,
    pub last_change: &'static str,
}

#[derive(Properties, PartialEq)]
pub struct DashboardProps {
    pub snapshot: DashboardSnapshot,
    pub mode: UiMode,
    pub density: Density,
}

#[function_component(DashboardPanel)]
pub fn dashboard_panel(props: &DashboardProps) -> Html {
    let density_class = match props.density {
        Density::Compact => "density-compact",
        Density::Normal => "density-normal",
        Density::Comfy => "density-comfy",
    };
    let mode_class = match props.mode {
        UiMode::Simple => "mode-simple",
        UiMode::Advanced => "mode-advanced",
    };

    html! {
        <section class={classes!("dashboard-grid", density_class, mode_class)}>
            <div class="tile metric">
                <header><span>{"Global Speeds"}</span><span class="pill live">{"SSE live"}</span></header>
                <div class="metric-row">
                    <div><small>{"Down"}</small><strong>{format_rate(props.snapshot.download_bps)}</strong></div>
                    <div><small>{"Up"}</small><strong>{format_rate(props.snapshot.upload_bps)}</strong></div>
                </div>
                <p class="muted">{"Sub-second updates with backoff on mobile and constrained networks."}</p>
            </div>
            <div class="tile stats">
                <header><span>{"Torrents"}</span><span class="muted">{"Active / Paused / Completed"}</span></header>
                <div class="stat-row">
                    <div><strong>{props.snapshot.active}</strong><small>{"Active"}</small></div>
                    <div><strong>{props.snapshot.paused}</strong><small>{"Paused"}</small></div>
                    <div><strong>{props.snapshot.completed}</strong><small>{"Completed"}</small></div>
                </div>
            </div>
            <div class="tile stats">
                <header><span>{"Disk Usage"}</span><span class="muted">{"Global + per path"}</span></header>
                <div class="stat-row">
                    <div>
                        <strong>{format!("{} / {} GB", props.snapshot.disk_used_gb, props.snapshot.disk_total_gb)}</strong>
                        <small>{"Total"}</small>
                    </div>
                </div>
                <ul class="path-usage">
                    {for props.snapshot.paths.iter().map(|path| html! {
                        <li>
                            <span>{path.label}</span>
                            <span class="muted">{format!("{} / {} GB", path.used_gb, path.total_gb)}</span>
                        </li>
                    })}
                </ul>
            </div>
            <div class="tile stats">
                <header><span>{"Tracker Health"}</span><span class="muted">{"Aggregated"}</span></header>
                <div class="stat-row">
                    <div><strong class="ok">{props.snapshot.tracker_health.ok}</strong><small>{"OK"}</small></div>
                    <div><strong class="warn">{props.snapshot.tracker_health.warn}</strong><small>{"Warn"}</small></div>
                    <div><strong class="error">{props.snapshot.tracker_health.error}</strong><small>{"Error"}</small></div>
                </div>
                <p class="muted">{"Failures surface in the event log with next announce timestamps."}</p>
            </div>
            <div class="tile stats">
                <header><span>{"Queue"}</span><span class="muted">{"Depth and paused counts"}</span></header>
                <div class="stat-row">
                    <div><strong>{props.snapshot.queue.active}</strong><small>{"Active"}</small></div>
                    <div><strong>{props.snapshot.queue.queued}</strong><small>{"Queued"}</small></div>
                    <div><strong>{props.snapshot.queue.depth}</strong><small>{"Depth"}</small></div>
                </div>
            </div>
            <div class="tile stats">
                <header><span>{"VPN Status"}</span><span class="muted">{props.snapshot.vpn.last_change}</span></header>
                <div class="stat-row">
                    <div><strong>{props.snapshot.vpn.state}</strong><small>{"State"}</small></div>
                    <div><strong>{props.snapshot.vpn.message}</strong><small>{"Message"}</small></div>
                </div>
            </div>
            <div class="tile events">
                <header><span>{"Recent Events"}</span><span class="muted">{"Warnings and tracker issues"}</span></header>
                <ul>
                    {for props.snapshot.recent_events.iter().map(|event| {
                        let badge = match event.kind {
                            EventKind::Info => "pill",
                            EventKind::Warning => "pill warn",
                            EventKind::Error => "pill error",
                        };
                        html! {
                            <li>
                                <span class={badge}>{format!("{:?}", event.kind)}</span>
                                <div>
                                    <strong>{event.label}</strong>
                                    <p class="muted">{event.detail}</p>
                                </div>
                            </li>
                        }
                    })}
                </ul>
            </div>
        </section>
    }
}

fn format_rate(value: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    if value as f64 >= GIB {
        format!("{:.1} GiB/s", value as f64 / GIB)
    } else if value as f64 >= MIB {
        format!("{:.1} MiB/s", value as f64 / MIB)
    } else if value as f64 >= KIB {
        format!("{:.1} KiB/s", value as f64 / KIB)
    } else {
        format!("{value} B/s")
    }
}

/// Demo snapshot used by the initial UI shell.
#[must_use]
pub fn demo_snapshot() -> DashboardSnapshot {
    DashboardSnapshot {
        download_bps: 142_000_000,
        upload_bps: 22_000_000,
        active: 12,
        paused: 4,
        completed: 187,
        disk_total_gb: 4200,
        disk_used_gb: 2830,
        paths: vec![
            PathUsage {
                label: "/data/media",
                used_gb: 1800,
                total_gb: 2600,
            },
            PathUsage {
                label: "/data/incomplete",
                used_gb: 120,
                total_gb: 400,
            },
            PathUsage {
                label: "/data/archive",
                used_gb: 910,
                total_gb: 1200,
            },
        ],
        recent_events: vec![
            DashboardEvent {
                label: "Tracker warn",
                detail: "udp://tracker.example: announce timeout; retrying in 5m",
                kind: EventKind::Warning,
            },
            DashboardEvent {
                label: "Filesystem move",
                detail: "Moved The.Expanse.S01E05 → /media/tv/The Expanse/Season 1",
                kind: EventKind::Info,
            },
            DashboardEvent {
                label: "VPN reconnection",
                detail: "Recovered tunnel after 12s; session resumed",
                kind: EventKind::Info,
            },
        ],
        tracker_health: TrackerHealth {
            ok: 24,
            warn: 3,
            error: 1,
        },
        queue: QueueStatus {
            active: 12,
            paused: 4,
            queued: 18,
            depth: 34,
        },
        vpn: VpnState {
            state: "connected",
            message: "Routing through wg0",
            last_change: "12s ago",
        },
    }
}
