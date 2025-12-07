use crate::Pane;
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::TorrentDetail;
use yew::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FileNode {
    pub name: String,
    pub size_gb: f32,
    pub completed_gb: f32,
    pub priority: String,
    pub wanted: bool,
    pub children: Vec<FileNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PeerRow {
    pub ip: String,
    pub client: String,
    pub flags: String,
    pub country: String,
    pub download_bps: u64,
    pub upload_bps: u64,
    pub progress: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TrackerRow {
    pub url: String,
    pub status: String,
    pub next_announce: String,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EventRow {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Metadata {
    pub hash: String,
    pub magnet: String,
    pub size_gb: f32,
    pub piece_count: u32,
    pub piece_size_mb: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DetailData {
    pub name: String,
    pub files: Vec<FileNode>,
    pub peers: Vec<PeerRow>,
    pub trackers: Vec<TrackerRow>,
    pub events: Vec<EventRow>,
    pub metadata: Metadata,
}

#[derive(Properties, PartialEq)]
pub(crate) struct DetailProps {
    pub data: Option<DetailData>,
}

#[function_component(DetailView)]
pub(crate) fn detail_view(props: &DetailProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
    let wanted_label = t("detail.files.wanted");
    let active = use_state(|| Pane::Files);
    let Some(detail) = props.data.clone() else {
        return html! {
            <section class="detail-panel placeholder">
                <h3>{t("detail.select_title")}</h3>
                <p class="muted">{t("detail.select_body")}</p>
            </section>
        };
    };

    html! {
        <section class="detail-panel">
            <header class="detail-header">
                <div>
                    <small class="muted">{t("detail.view_label")}</small>
                    <h3>{detail.name}</h3>
                </div>
                <div class="pane-tabs mobile-only">
                    {for [Pane::Files, Pane::Peers, Pane::Trackers, Pane::Log, Pane::Info].iter().map(|pane| {
                        let label = match pane {
                            Pane::Files => t("detail.tab.files"),
                            Pane::Peers => t("detail.tab.peers"),
                            Pane::Trackers => t("detail.tab.trackers"),
                            Pane::Log => t("detail.tab.log"),
                            Pane::Info => t("detail.tab.info"),
                        };
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
                        <h4>{t("detail.files.title")}</h4>
                        <p class="muted">{t("detail.files.body")}</p>
                    </header>
                    <div class="file-tree">
                        {for detail.files.iter().map(|node| render_file(node, 0, &wanted_label))}
                    </div>
                </section>

                <section class={pane_classes(Pane::Peers, *active)} data-pane="peers">
                    <header>
                        <h4>{t("detail.peers.title")}</h4>
                        <p class="muted">{t("detail.peers.body")}</p>
                    </header>
                    <div class="table-like">
                        {for detail.peers.iter().map(|peer| html! {
                            <div class="table-row">
                                <div>
                                    <strong>{peer.ip.clone()}</strong>
                                    <span class="muted">{peer.client.clone()}</span>
                                </div>
                                <div class="pill subtle">{peer.flags.clone()}</div>
                                <div class="pill subtle">{peer.country.clone()}</div>
                                <div class="stat"><small>{t("detail.peers.down")}</small><strong>{crate::core::logic::format_rate(peer.download_bps)}</strong></div>
                                <div class="stat"><small>{t("detail.peers.up")}</small><strong>{crate::core::logic::format_rate(peer.upload_bps)}</strong></div>
                                <div class="stat"><small>{t("detail.peers.progress")}</small><strong>{format!("{:.0}%", peer.progress * 100.0)}</strong></div>
                            </div>
                        })}
                    </div>
                </section>

                <section class={pane_classes(Pane::Trackers, *active)} data-pane="trackers">
                    <header>
                        <h4>{t("detail.trackers.title")}</h4>
                        <p class="muted">{t("detail.trackers.body")}</p>
                    </header>
                    <div class="table-like">
                        {for detail.trackers.iter().map(|tracker| html! {
                            <div class="table-row">
                                <div class="tracker-url">
                                    <strong>{tracker.url.clone()}</strong>
                                    {if let Some(err) = tracker.last_error.as_ref() {
                                        html! { <span class="pill warn">{err.clone()}</span> }
                                    } else {
                                        html! {}
                                    }}
                                </div>
                                <div class="pill subtle">{tracker.status.clone()}</div>
                                <span class="muted">{tracker.next_announce.clone()}</span>
                            </div>
                        })}
                    </div>
                </section>

                <section class={pane_classes(Pane::Log, *active)} data-pane="log">
                    <header>
                        <h4>{t("detail.log.title")}</h4>
                        <p class="muted">{t("detail.log.body")}</p>
                    </header>
                    <ul class="event-log">
                        {for detail.events.iter().map(|entry| html! {
                            <li>
                                <span class="muted">{entry.timestamp.clone()}</span>
                                <span class={classes!("pill", log_level(&entry.level))}>{entry.level.clone()}</span>
                                <span>{entry.message.clone()}</span>
                            </li>
                        })}
                    </ul>
                </section>

                <section class={pane_classes(Pane::Info, *active)} data-pane="info">
                    <header>
                        <h4>{t("detail.info.title")}</h4>
                        <p class="muted">{t("detail.info.body")}</p>
                    </header>
                    <dl class="metadata">
                        <div><dt>{t("detail.info.hash")}</dt><dd>{detail.metadata.hash}</dd></div>
                        <div><dt>{t("detail.info.magnet")}</dt><dd class="truncate">{detail.metadata.magnet}</dd></div>
                        <div><dt>{t("detail.info.size")}</dt><dd>{format!("{:.2} GB", detail.metadata.size_gb)}</dd></div>
                        <div><dt>{t("detail.info.pieces")}</dt><dd>{detail.metadata.piece_count}</dd></div>
                        <div><dt>{t("detail.info.piece_size")}</dt><dd>{format!("{} MB", detail.metadata.piece_size_mb)}</dd></div>
                    </dl>
                </section>
            </div>
        </section>
    }
}

impl From<TorrentDetail> for DetailData {
    fn from(detail: TorrentDetail) -> Self {
        let files = detail
            .files
            .into_iter()
            .map(|file| FileNode {
                name: file.path,
                size_gb: file.size_bytes as f32 / (1024.0 * 1024.0 * 1024.0),
                completed_gb: file.completed_bytes as f32 / (1024.0 * 1024.0 * 1024.0),
                priority: file.priority,
                wanted: file.wanted,
                children: vec![],
            })
            .collect();
        let peers = detail
            .peers
            .into_iter()
            .map(|peer| PeerRow {
                ip: peer.ip,
                client: peer.client,
                flags: peer.flags,
                country: peer.country.unwrap_or_default(),
                download_bps: peer.download_bps,
                upload_bps: peer.upload_bps,
                progress: peer.progress,
            })
            .collect();
        let trackers = detail
            .trackers
            .into_iter()
            .map(|tracker| TrackerRow {
                url: tracker.url,
                status: tracker.status,
                next_announce: tracker.next_announce_at.unwrap_or_else(|| "-".to_string()),
                last_error: tracker.last_error,
            })
            .collect();
        let events = detail
            .events
            .into_iter()
            .map(|event| EventRow {
                timestamp: event.timestamp,
                level: event.level,
                message: event.message,
            })
            .collect();
        Self {
            name: detail.name,
            files,
            peers,
            trackers,
            events,
            metadata: Metadata {
                hash: detail.hash,
                magnet: detail.magnet,
                size_gb: detail.size_bytes as f32 / (1024.0 * 1024.0 * 1024.0),
                piece_count: detail.piece_count,
                piece_size_mb: detail.piece_size_bytes / 1024 / 1024,
            },
        }
    }
}

fn pane_classes(pane: Pane, active: Pane) -> Classes {
    classes!(
        "detail-pane",
        if pane == active { Some("active") } else { None }
    )
}

fn log_level(level: &str) -> &'static str {
    match level {
        "warn" => "warn",
        "error" => "error",
        _ => "pill",
    }
}

fn render_file(node: &FileNode, depth: usize, wanted_label: &str) -> Html {
    let indent = depth * 12;
    let has_children = !node.children.is_empty();
    let summary = html! {
        <div class="file-row">
            <div class="file-main">
                <span class="file-name" style={format!("padding-inline-start: {}px", indent)}>
                    {node.name.clone()}
                </span>
                <div class="file-progress">
                    <span class="muted">{format!("{:.2} / {:.2} GB", node.completed_gb, node.size_gb)}</span>
                    <div class="bar" style={format!("width: {:.1}%", (node.completed_gb / node.size_gb) * 100.0)}></div>
                </div>
            </div>
            <div class="file-actions">
                <span class="pill subtle">{node.priority.clone()}</span>
                <label class="switch">
                    <input type="checkbox" checked={node.wanted} aria-label={wanted_label.to_string()} />
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
                    {for node.children.iter().map(|child| render_file(child, depth + 1, wanted_label))}
                </div>
            </details>
        }
    } else {
        summary
    }
}

/// Demo detail record used by the torrent view.
#[must_use]
pub(crate) fn demo_detail(id: &str) -> Option<DetailData> {
    let name = match id {
        "2" => "The.Expanse.S01E05.1080p.BluRay.DTS.x264",
        "3" => "Dune.Part.One.2021.2160p.REMUX.DV.DTS-HD.MA.7.1",
        "4" => "Ubuntu-24.04.1-live-server-amd64.iso",
        "5" => "Arcane.S02E02.1080p.NF.WEB-DL.DDP5.1.Atmos.x264",
        _ => "Foundation.S02E08.2160p.WEB-DL.DDP5.1.Atmos.HDR10",
    };

    Some(DetailData {
        name: name.to_string(),
        files: vec![
            FileNode {
                name: "Foundation.S02E08.mkv".to_string(),
                size_gb: 14.2,
                completed_gb: 6.1,
                priority: "high".to_string(),
                wanted: true,
                children: vec![],
            },
            FileNode {
                name: "Extras".to_string(),
                size_gb: 3.2,
                completed_gb: 1.4,
                priority: "normal".to_string(),
                wanted: true,
                children: vec![
                    FileNode {
                        name: "Featurette-01.mkv".to_string(),
                        size_gb: 1.1,
                        completed_gb: 1.1,
                        priority: "normal".to_string(),
                        wanted: true,
                        children: vec![],
                    },
                    FileNode {
                        name: "Interview-01.mkv".to_string(),
                        size_gb: 0.9,
                        completed_gb: 0.2,
                        priority: "low".to_string(),
                        wanted: false,
                        children: vec![],
                    },
                ],
            },
        ],
        peers: vec![
            PeerRow {
                ip: "203.0.113.24".to_string(),
                client: "qBittorrent 4.6".to_string(),
                flags: "DIXE".to_string(),
                country: "CA".to_string(),
                download_bps: 8_400_000,
                upload_bps: 650_000,
                progress: 0.54,
            },
            PeerRow {
                ip: "198.51.100.18".to_string(),
                client: "Transmission 4.0".to_string(),
                flags: "UXE".to_string(),
                country: "US".to_string(),
                download_bps: 2_200_000,
                upload_bps: 120_000,
                progress: 0.12,
            },
            PeerRow {
                ip: "203.0.113.88".to_string(),
                client: "libtorrent/2.0".to_string(),
                flags: "HSD".to_string(),
                country: "DE".to_string(),
                download_bps: 0,
                upload_bps: 320_000,
                progress: 1.0,
            },
        ],
        trackers: vec![
            TrackerRow {
                url: "udp://tracker.hypothetical.org".to_string(),
                status: "working".to_string(),
                next_announce: "3m".to_string(),
                last_error: None,
            },
            TrackerRow {
                url: "https://movies.example.net/announce".to_string(),
                status: "warning".to_string(),
                next_announce: "5m".to_string(),
                last_error: Some("timeout".to_string()),
            },
        ],
        events: vec![
            EventRow {
                timestamp: "08:01:12".to_string(),
                level: "info".to_string(),
                message: "Added via magnet".to_string(),
            },
            EventRow {
                timestamp: "08:03:44".to_string(),
                level: "warn".to_string(),
                message: "Tracker timeout, retrying in 5m".to_string(),
            },
            EventRow {
                timestamp: "08:04:22".to_string(),
                level: "info".to_string(),
                message: "Peers discovered (23)".to_string(),
            },
            EventRow {
                timestamp: "08:12:40".to_string(),
                level: "info".to_string(),
                message: "Reannounce triggered by user".to_string(),
            },
        ],
        metadata: Metadata {
            hash: "0123456789ABCDEF0123456789ABCDEF01234567".to_string(),
            magnet: "magnet:?xt=urn:btih:0123456789ABCDEF0123456789ABCDEF01234567".to_string(),
            size_gb: 17.4,
            piece_count: 6840,
            piece_size_mb: 4,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Peer, TorrentDetail, TorrentFile, Tracker};
    use uuid::Uuid;

    #[test]
    fn detail_conversion_maps_sizes_and_events() {
        let detail = TorrentDetail {
            id: Uuid::nil(),
            name: "demo".into(),
            files: vec![TorrentFile {
                path: "a.mkv".into(),
                size_bytes: 1_073_741_824,
                completed_bytes: 536_870_912,
                priority: "high".into(),
                wanted: true,
            }],
            peers: vec![Peer {
                ip: "1.1.1.1".into(),
                client: "qB".into(),
                flags: "D".into(),
                country: Some("US".into()),
                download_bps: 42,
                upload_bps: 7,
                progress: 0.5,
            }],
            trackers: vec![Tracker {
                url: "udp://tracker".into(),
                status: "ok".into(),
                next_announce_at: Some("soon".into()),
                last_error: None,
                last_error_at: None,
            }],
            events: vec![DetailEvent {
                timestamp: "now".into(),
                level: "info".into(),
                message: "started".into(),
            }],
            hash: "h".into(),
            magnet: "m".into(),
            size_bytes: 1_073_741_824,
            piece_count: 2,
            piece_size_bytes: 512 * 1024,
        };
        let mapped: DetailData = detail.into();
        assert_eq!(mapped.files.first().unwrap().completed_gb, 0.5);
        assert_eq!(mapped.peers.first().unwrap().country, "US");
        assert_eq!(mapped.trackers.first().unwrap().next_announce, "soon");
        assert_eq!(mapped.events.len(), 1);
        assert_eq!(mapped.metadata.piece_size_mb, 0);
    }
}
