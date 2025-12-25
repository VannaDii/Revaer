use crate::Pane;
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::{DetailData, FileNode};
use yew::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileSelectionChange {
    pub path: String,
    pub wanted: bool,
}

#[derive(Properties, PartialEq)]
pub(crate) struct DetailProps {
    pub data: Option<DetailData>,
    pub on_toggle_file: Callback<FileSelectionChange>,
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
                        {for detail
                            .files
                            .iter()
                            .map(|node| render_file(node, 0, &wanted_label, &props.on_toggle_file))}
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

fn render_file(
    node: &FileNode,
    depth: usize,
    wanted_label: &str,
    on_toggle_file: &Callback<FileSelectionChange>,
) -> Html {
    let indent = depth * 12;
    let has_children = !node.children.is_empty();
    let on_toggle = {
        let on_toggle = on_toggle_file.clone();
        let path = node.name.clone();
        Callback::from(move |event: Event| {
            if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                on_toggle.emit(FileSelectionChange {
                    path: path.clone(),
                    wanted: input.checked(),
                });
            }
        })
    };
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
                    <input
                        type="checkbox"
                        checked={node.wanted}
                        aria-label={wanted_label.to_string()}
                        onchange={on_toggle}
                    />
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
                    {for node
                        .children
                        .iter()
                        .map(|child| render_file(child, depth + 1, wanted_label, on_toggle_file))}
                </div>
            </details>
        }
    } else {
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        TorrentDetail, TorrentFileView, TorrentProgressView, TorrentRatesView, TorrentStateKind,
        TorrentStateView, TorrentSummary,
    };
    use chrono::{DateTime, Utc};
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn detail_conversion_maps_sizes_and_events() {
        let now = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).expect("timestamp");
        let file: TorrentFileView = serde_json::from_value(json!({
            "index": 0,
            "path": "a.mkv",
            "size_bytes": 1_073_741_824,
            "bytes_completed": 536_870_912,
            "priority": "high",
            "selected": true
        }))
        .expect("file decode");
        let detail = TorrentDetail {
            summary: TorrentSummary {
                id: Uuid::nil(),
                name: Some("demo".into()),
                state: TorrentStateView {
                    kind: TorrentStateKind::Downloading,
                    failure_message: None,
                },
                progress: TorrentProgressView {
                    bytes_downloaded: 536_870_912,
                    bytes_total: 1_073_741_824,
                    percent_complete: 50.0,
                    eta_seconds: None,
                },
                rates: TorrentRatesView {
                    download_bps: 42,
                    upload_bps: 7,
                    ratio: 1.2,
                },
                library_path: None,
                download_dir: Some("/downloads".into()),
                sequential: false,
                tags: vec![],
                category: None,
                trackers: vec![],
                rate_limit: None,
                connections_limit: None,
                added_at: now,
                completed_at: None,
                last_updated: now,
            },
            settings: None,
            files: Some(vec![file]),
        };
        let mapped: DetailData = detail.into();
        assert_eq!(mapped.files.first().unwrap().completed_gb, 0.5);
        assert_eq!(mapped.files.first().unwrap().priority, "high");
        assert!(mapped.peers.is_empty());
        assert!(mapped.trackers.is_empty());
        assert!(mapped.events.is_empty());
        assert_eq!(mapped.metadata.piece_size_mb, 0);
        assert_eq!(mapped.metadata.hash, "-");
    }
}
