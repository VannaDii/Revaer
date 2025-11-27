use crate::Pane;
use yew::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub struct FileNode {
    pub name: &'static str,
    pub size_gb: f32,
    pub completed_gb: f32,
    pub priority: &'static str,
    pub wanted: bool,
    pub children: Vec<FileNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PeerRow {
    pub ip: &'static str,
    pub client: &'static str,
    pub flags: &'static str,
    pub country: &'static str,
    pub download_bps: u64,
    pub upload_bps: u64,
    pub progress: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrackerRow {
    pub url: &'static str,
    pub status: &'static str,
    pub next_announce: &'static str,
    pub last_error: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventRow {
    pub timestamp: &'static str,
    pub level: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Metadata {
    pub hash: &'static str,
    pub magnet: &'static str,
    pub size_gb: f32,
    pub piece_count: u32,
    pub piece_size_mb: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DetailData {
    pub name: &'static str,
    pub files: Vec<FileNode>,
    pub peers: Vec<PeerRow>,
    pub trackers: Vec<TrackerRow>,
    pub events: Vec<EventRow>,
    pub metadata: Metadata,
}

#[derive(Properties, PartialEq)]
pub struct DetailProps {
    pub data: Option<DetailData>,
}

#[function_component(DetailView)]
pub fn detail_view(props: &DetailProps) -> Html {
    let active = use_state(|| Pane::Files);
    let Some(detail) = props.data.clone() else {
        return html! {
            <section class="detail-panel placeholder">
                <h3>{"Select a torrent"}</h3>
                <p class="muted">{"Open a torrent to view files, peers, trackers, and metadata."}</p>
            </section>
        };
    };

    html! {
        <section class="detail-panel">
            <header class="detail-header">
                <div>
                    <small class="muted">{"Detail view"}</small>
                    <h3>{detail.name}</h3>
                </div>
                <div class="pane-tabs mobile-only">
                    {for [Pane::Files, Pane::Peers, Pane::Trackers, Pane::Log, Pane::Info].iter().map(|pane| {
                        let label = pane_label(*pane);
                        let active_state = *active == *pane;
                        let onclick = {
                            let active = active.clone();
                            let pane = *pane;
                            Callback::from(move |_| active.set(pane))
                        };
                        html! {
                            <button class={classes!("ghost", if active_state { "active" } else { "" })} onclick={onclick}>{label}</button>
                        }
                    })}
                </div>
            </header>

            <div class="detail-grid">
                <section class={pane_classes(Pane::Files, *active)} data-pane="files">
                    <header>
                        <h4>{"Files"}</h4>
                        <p class="muted">{"Per-file priority + wanted/unwanted toggles. Accordion on mobile."}</p>
                    </header>
                    <div class="file-tree">
                        {for detail.files.iter().map(|node| render_file(node, 0))}
                    </div>
                </section>

                <section class={pane_classes(Pane::Peers, *active)} data-pane="peers">
                    <header>
                        <h4>{"Peers"}</h4>
                        <p class="muted">{"Sortable by speed/progress; shows flags + country where available."}</p>
                    </header>
                    <div class="table-like">
                        {for detail.peers.iter().map(|peer| html! {
                            <div class="table-row">
                                <div>
                                    <strong>{peer.ip}</strong>
                                    <span class="muted">{peer.client}</span>
                                </div>
                                <div class="pill subtle">{peer.flags}</div>
                                <div class="pill subtle">{peer.country}</div>
                                <div class="stat"><small>{"Down"}</small><strong>{format_rate(peer.download_bps)}</strong></div>
                                <div class="stat"><small>{"Up"}</small><strong>{format_rate(peer.upload_bps)}</strong></div>
                                <div class="stat"><small>{"Prog"}</small><strong>{format!("{:.0}%", peer.progress * 100.0)}</strong></div>
                            </div>
                        })}
                    </div>
                </section>

                <section class={pane_classes(Pane::Trackers, *active)} data-pane="trackers">
                    <header>
                        <h4>{"Trackers"}</h4>
                        <p class="muted">{"Status, next announce, and errors with warning badges."}</p>
                    </header>
                    <div class="table-like">
                        {for detail.trackers.iter().map(|tracker| html! {
                            <div class="table-row">
                                <div class="tracker-url">
                                    <strong>{tracker.url}</strong>
                                    {if let Some(err) = tracker.last_error {
                                        html! { <span class="pill warn">{err}</span> }
                                    } else {
                                        html! {}
                                    }}
                                </div>
                                <div class="pill subtle">{tracker.status}</div>
                                <span class="muted">{tracker.next_announce}</span>
                            </div>
                        })}
                    </div>
                </section>

                <section class={pane_classes(Pane::Log, *active)} data-pane="log">
                    <header>
                        <h4>{"Event Log"}</h4>
                        <p class="muted">{"Warnings, tracker issues, state transitions."}</p>
                    </header>
                    <ul class="event-log">
                        {for detail.events.iter().map(|entry| html! {
                            <li>
                                <span class="muted">{entry.timestamp}</span>
                                <span class={classes!("pill", log_level(&entry.level))}>{entry.level}</span>
                                <span>{entry.message}</span>
                            </li>
                        })}
                    </ul>
                </section>

                <section class={pane_classes(Pane::Info, *active)} data-pane="info">
                    <header>
                        <h4>{"Metadata"}</h4>
                        <p class="muted">{"Hash, magnet, size, piece count, piece size."}</p>
                    </header>
                    <dl class="metadata">
                        <div><dt>{"Hash"}</dt><dd>{detail.metadata.hash}</dd></div>
                        <div><dt>{"Magnet"}</dt><dd class="truncate">{detail.metadata.magnet}</dd></div>
                        <div><dt>{"Size"}</dt><dd>{format!("{:.2} GB", detail.metadata.size_gb)}</dd></div>
                        <div><dt>{"Pieces"}</dt><dd>{detail.metadata.piece_count}</dd></div>
                        <div><dt>{"Piece size"}</dt><dd>{format!("{} MB", detail.metadata.piece_size_mb)}</dd></div>
                    </dl>
                </section>
            </div>
        </section>
    }
}

fn pane_classes(pane: Pane, active: Pane) -> Classes {
    classes!(
        "detail-pane",
        if pane == active { Some("active") } else { None }
    )
}

fn pane_label(pane: Pane) -> &'static str {
    match pane {
        Pane::Files => "Files",
        Pane::Peers => "Peers",
        Pane::Trackers => "Trackers",
        Pane::Log => "Log",
        Pane::Info => "Info",
    }
}

fn log_level(level: &str) -> &'static str {
    match level {
        "warn" => "warn",
        "error" => "error",
        _ => "pill",
    }
}

fn render_file(node: &FileNode, depth: usize) -> Html {
    let indent = depth * 12;
    let has_children = !node.children.is_empty();
    let summary = html! {
        <div class="file-row">
            <div class="file-main">
                <span class="file-name" style={format!("padding-inline-start: {}px", indent)}>
                    {node.name}
                </span>
                <div class="file-progress">
                    <span class="muted">{format!("{:.2} / {:.2} GB", node.completed_gb, node.size_gb)}</span>
                    <div class="bar" style={format!("width: {:.1}%", (node.completed_gb / node.size_gb) * 100.0)}></div>
                </div>
            </div>
            <div class="file-actions">
                <span class="pill subtle">{node.priority}</span>
                <label class="switch">
                    <input type="checkbox" checked={node.wanted} aria-label="Wanted" />
                    <span class="slider"></span>
                </label>
            </div>
        </div>
    };

    if has_children {
        html! {
            <details open={depth == 0}>
                <summary>{summary}</summary>
                <div class="file-children">
                    {for node.children.iter().map(|child| render_file(child, depth + 1))}
                </div>
            </details>
        }
    } else {
        summary
    }
}

fn format_rate(value: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    if value as f64 >= MIB {
        format!("{:.1} MiB/s", value as f64 / MIB)
    } else if value as f64 >= KIB {
        format!("{:.1} KiB/s", value as f64 / KIB)
    } else {
        format!("{value} B/s")
    }
}

/// Demo detail record used by the torrent view.
#[must_use]
pub fn demo_detail(id: &str) -> Option<DetailData> {
    let name = match id {
        "2" => "The.Expanse.S01E05.1080p.BluRay.DTS.x264",
        "3" => "Dune.Part.One.2021.2160p.REMUX.DV.DTS-HD.MA.7.1",
        "4" => "Ubuntu-24.04.1-live-server-amd64.iso",
        "5" => "Arcane.S02E02.1080p.NF.WEB-DL.DDP5.1.Atmos.x264",
        _ => "Foundation.S02E08.2160p.WEB-DL.DDP5.1.Atmos.HDR10",
    };

    Some(DetailData {
        name,
        files: vec![
            FileNode {
                name: "Foundation.S02E08.mkv",
                size_gb: 14.2,
                completed_gb: 6.1,
                priority: "high",
                wanted: true,
                children: vec![],
            },
            FileNode {
                name: "Extras",
                size_gb: 3.2,
                completed_gb: 1.4,
                priority: "normal",
                wanted: true,
                children: vec![
                    FileNode {
                        name: "Featurette-01.mkv",
                        size_gb: 1.1,
                        completed_gb: 1.1,
                        priority: "normal",
                        wanted: true,
                        children: vec![],
                    },
                    FileNode {
                        name: "Interview-01.mkv",
                        size_gb: 0.9,
                        completed_gb: 0.2,
                        priority: "low",
                        wanted: false,
                        children: vec![],
                    },
                ],
            },
        ],
        peers: vec![
            PeerRow {
                ip: "203.0.113.24",
                client: "qBittorrent 4.6",
                flags: "DIXE",
                country: "CA",
                download_bps: 8_400_000,
                upload_bps: 650_000,
                progress: 0.54,
            },
            PeerRow {
                ip: "198.51.100.18",
                client: "Transmission 4.0",
                flags: "UXE",
                country: "US",
                download_bps: 2_200_000,
                upload_bps: 120_000,
                progress: 0.12,
            },
            PeerRow {
                ip: "203.0.113.88",
                client: "libtorrent/2.0",
                flags: "HSD",
                country: "DE",
                download_bps: 0,
                upload_bps: 320_000,
                progress: 1.0,
            },
        ],
        trackers: vec![
            TrackerRow {
                url: "udp://tracker.hypothetical.org",
                status: "working",
                next_announce: "3m",
                last_error: None,
            },
            TrackerRow {
                url: "https://movies.example.net/announce",
                status: "warning",
                next_announce: "5m",
                last_error: Some("timeout"),
            },
        ],
        events: vec![
            EventRow {
                timestamp: "08:01:12",
                level: "info",
                message: "Added via magnet",
            },
            EventRow {
                timestamp: "08:03:44",
                level: "warn",
                message: "Tracker timeout, retrying in 5m",
            },
            EventRow {
                timestamp: "08:04:22",
                level: "info",
                message: "Peers discovered (23)",
            },
            EventRow {
                timestamp: "08:12:40",
                level: "info",
                message: "Reannounce triggered by user",
            },
        ],
        metadata: Metadata {
            hash: "0123456789ABCDEF0123456789ABCDEF01234567",
            magnet: "magnet:?xt=urn:btih:0123456789ABCDEF0123456789ABCDEF01234567",
            size_gb: 17.4,
            piece_count: 6840,
            piece_size_mb: 4,
        },
    })
}
