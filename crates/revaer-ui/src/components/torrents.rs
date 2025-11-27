use crate::components::detail::{DetailData, DetailView, demo_detail};
use crate::{Density, UiMode};
use yew::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub struct TorrentRow {
    pub id: &'static str,
    pub name: &'static str,
    pub status: &'static str,
    pub progress: f32,
    pub eta: Option<&'static str>,
    pub ratio: f32,
    pub tags: Vec<&'static str>,
    pub tracker: &'static str,
    pub path: &'static str,
    pub category: &'static str,
    pub size_gb: f32,
    pub upload_bps: u64,
    pub download_bps: u64,
}

#[derive(Properties, PartialEq)]
pub struct TorrentProps {
    pub torrents: Vec<TorrentRow>,
    pub density: Density,
    pub mode: UiMode,
    pub on_density_change: Callback<Density>,
}

#[function_component(TorrentView)]
pub fn torrent_view(props: &TorrentProps) -> Html {
    let selected = use_state(|| demo_detail("1"));
    let density_class = match props.density {
        Density::Compact => "density-compact",
        Density::Normal => "density-normal",
        Density::Comfy => "density-comfy",
    };
    let mode_class = match props.mode {
        UiMode::Simple => "mode-simple",
        UiMode::Advanced => "mode-advanced",
    };

    let on_select = {
        let selected = selected.clone();
        Callback::from(move |id: String| {
            selected.set(demo_detail(&id));
        })
    };

    html! {
        <section class={classes!("torrents-view", density_class, mode_class)}>
            <header class="toolbar">
                <div class="search">
                    <input aria-label="Search torrents" placeholder="Search name, path, tracker" />
                    <button class="ghost">{"Regex"}</button>
                </div>
                <div class="actions">
                    <div class="segmented density">
                        {Density::all().iter().map(|option| {
                            let label = match option {
                                Density::Compact => "Compact",
                                Density::Normal => "Normal",
                                Density::Comfy => "Comfy",
                            };
                            let active = props.density == *option;
                            let callback = {
                                let on_change = props.on_density_change.clone();
                                let option = *option;
                                Callback::from(move |_| on_change.emit(option))
                            };
                            html! {
                                <button class={classes!(if active { "active" } else { "" })} onclick={callback}>{label}</button>
                            }
                        }).collect::<Html>()}
                    </div>
                    <button class="ghost">{"Pause"}</button>
                    <button class="ghost">{"Resume"}</button>
                    <button class="ghost danger">{"Delete"}</button>
                    <button class="solid">{"Add"}</button>
                </div>
            </header>

            <AddTorrentPanel />

            <div class="torrent-table virtualized" role="grid" aria-label="Torrents">
                {for props.torrents.iter().map(|row| render_row(row, on_select.clone()))}
            </div>

            <DetailView data={(*selected).clone()} />
            <MobileActionRow />
        </section>
    }
}

fn render_row(row: &TorrentRow, on_select: Callback<String>) -> Html {
    let select = {
        let on_select = on_select.clone();
        let id = row.id.to_string();
        Callback::from(move |_| on_select.emit(id.clone()))
    };
    html! {
        <article class="torrent-row">
            <div class="row-primary">
                <div class="title">
                    <strong>{row.name}</strong>
                    <span class="muted">{row.tracker}</span>
                </div>
                <div class="status">
                    <span class={classes!("pill", status_class(row.status))}>{row.status}</span>
                    <div class="progress">
                        <div class="bar" style={format!("width: {:.1}%", row.progress * 100.0)}></div>
                        <span class="muted">{format!("{:.1}%", row.progress * 100.0)}</span>
                        <span class="muted">{row.eta.unwrap_or("∞")}</span>
                    </div>
                </div>
            </div>
            <div class="row-secondary">
                <div class="stat">
                    <small>{"Down"}</small>
                    <strong>{format_rate(row.download_bps)}</strong>
                </div>
                <div class="stat">
                    <small>{"Up"}</small>
                    <strong>{format_rate(row.upload_bps)}</strong>
                </div>
                <div class="stat">
                    <small>{"Ratio"}</small>
                    <strong>{format!("{:.2}", row.ratio)}</strong>
                </div>
                <div class="stat">
                    <small>{"Size"}</small>
                    <strong>{format!("{:.2} GB", row.size_gb)}</strong>
                </div>
            </div>
            <div class="row-meta">
                <span class="muted">{row.path}</span>
                <div class="tags">
                    <span class="pill subtle">{row.category}</span>
                    {for row.tags.iter().map(|tag| html! { <span class="pill subtle">{tag.to_owned()}</span> })}
                </div>
            </div>
            <div class="row-actions">
                <button class="ghost" onclick={select.clone()}>{"Open detail"}</button>
                <button class="ghost">{"Pause"}</button>
                <button class="ghost">{"Recheck"}</button>
                <button class="ghost danger">{"Delete + data"}</button>
            </div>
        </article>
    }
}

fn status_class(status: &str) -> &'static str {
    match status {
        "downloading" => "ok",
        "seeding" => "ok",
        "checking" => "warn",
        "paused" => "muted",
        "error" => "error",
        _ => "muted",
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

#[function_component(AddTorrentPanel)]
fn add_torrent_panel() -> Html {
    html! {
        <div class="add-panel">
            <div class="drop-zone" role="button" aria-label="Upload torrent">
                <p><strong>{"Drop .torrent or paste magnet"}</strong></p>
                <p class="muted">{"Validates magnet/URL, supports drag/drop and file selection."}</p>
                <div class="inputs">
                    <input placeholder="Magnet or URL" />
                    <button class="solid">{"Add"}</button>
                </div>
            </div>
            <div class="pre-flight">
                <label>
                    <span>{"Category"}</span>
                    <input placeholder="tv, movies, music" />
                </label>
                <label>
                    <span>{"Tags"}</span>
                    <input placeholder="4K, hevc, scene" />
                </label>
                <label>
                    <span>{"Save path"}</span>
                    <input placeholder="/data/incomplete" />
                </label>
            </div>
        </div>
    }
}

#[function_component(MobileActionRow)]
fn mobile_action_row() -> Html {
    html! {
        <div class="mobile-action-row">
            <button class="ghost">{"Pause"}</button>
            <button class="ghost">{"Resume"}</button>
            <button class="ghost danger">{"Delete"}</button>
            <button class="solid">{"More…"}</button>
        </div>
    }
}

/// Demo torrent set referenced by the default view.
#[must_use]
pub fn demo_rows() -> Vec<TorrentRow> {
    vec![
        TorrentRow {
            id: "1",
            name: "Foundation.S02E08.2160p.WEB-DL.DDP5.1.Atmos.HDR10",
            status: "downloading",
            progress: 0.41,
            eta: Some("12m"),
            ratio: 0.12,
            tags: vec!["4K", "HDR10", "hevc"],
            tracker: "tracker.hypothetical.org",
            path: "/data/incomplete/foundation-s02e08",
            category: "tv",
            size_gb: 18.4,
            upload_bps: 1_200_000,
            download_bps: 82_000_000,
        },
        TorrentRow {
            id: "2",
            name: "The.Expanse.S01E05.1080p.BluRay.DTS.x264",
            status: "seeding",
            progress: 1.0,
            eta: None,
            ratio: 3.82,
            tags: vec!["blu-ray", "lossless"],
            tracker: "tracker.space.example",
            path: "/data/media/TV/The Expanse/Season 1",
            category: "tv",
            size_gb: 7.8,
            upload_bps: 5_400_000,
            download_bps: 0,
        },
        TorrentRow {
            id: "3",
            name: "Dune.Part.One.2021.2160p.REMUX.DV.DTS-HD.MA.7.1",
            status: "paused",
            progress: 0.77,
            eta: Some("–"),
            ratio: 0.44,
            tags: vec!["remux", "dolby vision"],
            tracker: "movies.example.net",
            path: "/data/incomplete/dune-part-one",
            category: "movies",
            size_gb: 64.3,
            upload_bps: 0,
            download_bps: 0,
        },
        TorrentRow {
            id: "4",
            name: "Ubuntu-24.04.1-live-server-amd64.iso",
            status: "checking",
            progress: 0.13,
            eta: Some("3m"),
            ratio: 0.02,
            tags: vec!["iso"],
            tracker: "releases.ubuntu.com",
            path: "/data/incomplete/ubuntu",
            category: "os",
            size_gb: 1.2,
            upload_bps: 240_000,
            download_bps: 12_000_000,
        },
        TorrentRow {
            id: "5",
            name: "Arcane.S02E02.1080p.NF.WEB-DL.DDP5.1.Atmos.x264",
            status: "downloading",
            progress: 0.63,
            eta: Some("8m"),
            ratio: 0.56,
            tags: vec!["nf", "dolby atmos"],
            tracker: "tracker.hypothetical.org",
            path: "/data/incomplete/arcane-s02e02",
            category: "tv",
            size_gb: 5.4,
            upload_bps: 950_000,
            download_bps: 34_000_000,
        },
    ]
}
