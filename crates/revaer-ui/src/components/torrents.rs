use crate::components::detail::{DetailData, DetailView, demo_detail};
use crate::components::virtual_list::VirtualList;
use crate::models::TorrentSummary;
use crate::{Density, UiMode};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{DragEvent, File, HtmlElement, KeyboardEvent};
use yew::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub struct TorrentRow {
    pub id: String,
    pub name: String,
    pub status: String,
    pub progress: f32,
    pub eta: Option<String>,
    pub ratio: f32,
    pub tags: Vec<String>,
    pub tracker: String,
    pub path: String,
    pub category: String,
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
    pub on_action: Callback<(TorrentAction, String)>,
    pub on_add: Callback<AddTorrentInput>,
    pub add_busy: bool,
}

#[derive(Clone, Debug)]
pub struct AddTorrentInput {
    pub value: Option<String>,
    pub file: Option<File>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub save_path: Option<String>,
}

impl PartialEq for AddTorrentInput {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
            && self.category == other.category
            && self.tags == other.tags
            && self.save_path == other.save_path
            && self.file.as_ref().map(|f| f.name()) == other.file.as_ref().map(|f| f.name())
    }
}

#[function_component(TorrentView)]
pub fn torrent_view(props: &TorrentProps) -> Html {
    let selected = use_state(|| demo_detail("1"));
    let selected_idx = use_state(|| 0usize);
    let action_banner = use_state(|| None as Option<String>);
    let confirm = use_state(|| None as Option<ConfirmKind>);
    let search_ref = use_node_ref();
    let density_class = match props.density {
        Density::Compact => "density-compact",
        Density::Normal => "density-normal",
        Density::Comfy => "density-comfy",
    };
    let row_height = match props.density {
        Density::Compact => 120,
        Density::Normal => 148,
        Density::Comfy => 164,
    };
    let mode_class = match props.mode {
        UiMode::Simple => "mode-simple",
        UiMode::Advanced => "mode-advanced",
    };
    let selected_id = props.torrents.get(*selected_idx).map(|row| row.id.clone());
    let pause_selected = {
        let on_action = props.on_action.clone();
        let selected_id = selected_id.clone();
        Callback::from(move |_| {
            if let Some(id) = &selected_id {
                on_action.emit((TorrentAction::Pause, id.clone()));
            }
        })
    };
    let resume_selected = {
        let on_action = props.on_action.clone();
        let selected_id = selected_id.clone();
        Callback::from(move |_| {
            if let Some(id) = &selected_id {
                on_action.emit((TorrentAction::Resume, id.clone()));
            }
        })
    };
    let delete_selected = {
        let on_action = props.on_action.clone();
        let selected_id = selected_id.clone();
        Callback::from(move |_| {
            if let Some(id) = &selected_id {
                on_action.emit((TorrentAction::Delete { with_data: false }, id.clone()));
            }
        })
    };

    let on_select = {
        let selected = selected.clone();
        let selected_idx = selected_idx.clone();
        Callback::from(move |id: String| {
            if let Some(idx) = props.torrents.iter().position(|row| row.id == id) {
                selected_idx.set(idx);
            }
            selected.set(demo_detail(&id));
        })
    };

    // Keyboard shortcuts: j/k navigation, space pause/resume, delete/shift+delete confirmations, p recheck, / focus search.
    {
        let torrents = props.torrents.clone();
        let selected_idx = selected_idx.clone();
        let selected = selected.clone();
        let search_ref = search_ref.clone();
        let action_banner = action_banner.clone();
        let confirm = confirm.clone();
        use_effect_with_deps(
            move |_| {
                let handler = Closure::<dyn FnMut(_)>::wrap(Box::new(move |event: KeyboardEvent| {
                    if let Some(target) = event.target()
                        && let Ok(element) = target.dyn_into::<HtmlElement>()
                        && matches!(element.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT")
                    {
                        return;
                    }

                    match event.key().as_str() {
                        "/" => {
                            event.prevent_default();
                            if let Some(input) = search_ref.cast::<web_sys::HtmlInputElement>() {
                                let _ = input.focus();
                            }
                        }
                        "j" | "J" => {
                            event.prevent_default();
                            let next = (*selected_idx + 1).min(torrents.len().saturating_sub(1));
                            if next != *selected_idx {
                                selected_idx.set(next);
                                if let Some(row) = torrents.get(next) {
                                    selected.set(demo_detail(row.id));
                                }
                            }
                        }
                        "k" | "K" => {
                            event.prevent_default();
                            let next = selected_idx.saturating_sub(1);
                            if next != *selected_idx {
                                selected_idx.set(next);
                                if let Some(row) = torrents.get(next) {
                                    selected.set(demo_detail(row.id));
                                }
                            }
                        }
                        " " => {
                            event.prevent_default();
                            if let Some(row) = torrents.get(*selected_idx) {
                                action_banner
                                    .set(Some(format!("Toggled pause/resume for {}", row.name)));
                            }
                        }
                        key if key == "Delete" && event.shift_key() => {
                            event.prevent_default();
                            confirm.set(Some(ConfirmKind::DeleteData));
                        }
                        "Delete" => {
                            event.prevent_default();
                            confirm.set(Some(ConfirmKind::Delete));
                        }
                        "p" | "P" => {
                            event.prevent_default();
                            confirm.set(Some(ConfirmKind::Recheck));
                        }
                        _ => {}
                    }
                })
                    as Box<dyn FnMut(_)>);

                let window = web_sys::window().expect("window");
                window
                    .add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref())
                    .expect("register keydown");

                move || {
                    let _ = web_sys::window()
                        .unwrap()
                        .remove_event_listener_with_callback(
                            "keydown",
                            handler.as_ref().unchecked_ref(),
                        );
                }
            },
            (),
        );
    }

    html! {
        <section class={classes!("torrents-view", density_class, mode_class)}>
            <header class="toolbar">
                <div class="search">
                    <input aria-label="Search torrents" placeholder="Search name, path, tracker" ref={search_ref.clone()} />
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
                    <button class="ghost" onclick={pause_selected}>{"Pause"}</button>
                    <button class="ghost" onclick={resume_selected}>{"Resume"}</button>
                    <button class="ghost danger" onclick={delete_selected}>{"Delete"}</button>
                    <button class="solid">{"Add"}</button>
                </div>
            </header>

            <AddTorrentPanel on_submit={props.on_add.clone()} pending={props.add_busy} />

            <VirtualList
                class={classes!("torrent-table", "virtualized")}
                len={props.torrents.len()}
                row_height={row_height}
                overscan={6}
                render={{
                    let on_select = on_select.clone();
                    let torrents = props.torrents.clone();
                    let selected_idx = *selected_idx;
                    let on_action = props.on_action.clone();
                    Callback::from(move |idx: usize| {
                        if let Some(row) = torrents.get(idx) {
                            render_row(row, idx == selected_idx, on_select.clone(), on_action.clone())
                        } else {
                            html! {}
                        }
                    })
                }}
            />

            <DetailView data={(*selected).clone()} />
            <MobileActionRow on_action={props.on_action.clone()} selected={props.torrents.get(*selected_idx).map(|t| t.id.clone())} />
            <ActionBanner message={(*action_banner).clone()} />
            <ConfirmDialog
                kind={(*confirm).clone()}
                on_close={{
                    let confirm = confirm.clone();
                    Callback::from(move |_| confirm.set(None))
                }}
                on_confirm={{
                    let confirm = confirm.clone();
                    let torrents = props.torrents.clone();
                    let selected_idx = *selected_idx;
                    let action_banner = action_banner.clone();
                    let on_action = props.on_action.clone();
                    Callback::from(move |kind: ConfirmKind| {
                        confirm.set(None);
                        if let Some(row) = torrents.get(selected_idx) {
                            let action = match kind {
                                ConfirmKind::Delete => TorrentAction::Delete { with_data: false },
                                ConfirmKind::DeleteData => TorrentAction::Delete { with_data: true },
                                ConfirmKind::Recheck => TorrentAction::Recheck,
                            };
                            on_action.emit((action.clone(), row.id.clone()));
                            let msg = match action {
                                TorrentAction::Delete { with_data: true } => format!("Removed torrent + data {}", row.name),
                                TorrentAction::Delete { with_data: false } => format!("Removed torrent {}", row.name),
                                TorrentAction::Recheck => format!("Rechecking {}", row.name),
                                TorrentAction::Pause => format!("Paused {}", row.name),
                                TorrentAction::Resume => format!("Resumed {}", row.name),
                            };
                            action_banner.set(Some(msg));
                        }
                    })
                }}
            />
        </section>
    }
}

fn render_row(
    row: &TorrentRow,
    selected: bool,
    on_select: Callback<String>,
    on_action: Callback<(TorrentAction, String)>,
) -> Html {
    let select = {
        let on_select = on_select.clone();
        let id = row.id.to_string();
        Callback::from(move |_| on_select.emit(id.clone()))
    };
    let pause = {
        let on_action = on_action.clone();
        let id = row.id.clone();
        Callback::from(move |_| on_action.emit((TorrentAction::Pause, id.clone())))
    };
    let resume = {
        let on_action = on_action.clone();
        let id = row.id.clone();
        Callback::from(move |_| on_action.emit((TorrentAction::Resume, id.clone())))
    };
    let recheck = {
        let on_action = on_action.clone();
        let id = row.id.clone();
        Callback::from(move |_| on_action.emit((TorrentAction::Recheck, id.clone())))
    };
    let delete_data = {
        let on_action = on_action.clone();
        let id = row.id.clone();
        Callback::from(move |_| {
            on_action.emit((TorrentAction::Delete { with_data: true }, id.clone()))
        })
    };
    html! {
        <article class={classes!("torrent-row", if selected { Some("selected") } else { None })} aria-selected={selected.to_string()}>
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
                <button class="ghost" onclick={pause}>{"Pause"}</button>
                <button class="ghost" onclick={resume}>{"Resume"}</button>
                <button class="ghost" onclick={recheck}>{"Recheck"}</button>
                <button class="ghost danger" onclick={delete_data}>{"Delete + data"}</button>
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

#[derive(Properties, PartialEq)]
pub struct AddTorrentProps {
    pub on_submit: Callback<AddTorrentInput>,
    pub pending: bool,
}

#[function_component(AddTorrentPanel)]
fn add_torrent_panel(props: &AddTorrentProps) -> Html {
    let input_value = use_state(String::new);
    let category = use_state(String::new);
    let tags = use_state(String::new);
    let save_path = use_state(String::new);
    let file = use_state(|| None as Option<File>);
    let error = use_state(|| None as Option<String>);
    let drag_over = use_state(|| false);

    let submit = {
        let input_value = input_value.clone();
        let category = category.clone();
        let tags = tags.clone();
        let save_path = save_path.clone();
        let file = file.clone();
        let error = error.clone();
        let on_submit = props.on_submit.clone();
        Callback::from(move |_| {
            let value = input_value.trim().to_string();
            let has_file = (*file).is_some();
            let is_magnet = value.starts_with("magnet:?xt=urn:btih:");
            let is_url = value.starts_with("http://") || value.starts_with("https://");
            if value.is_empty() && !has_file {
                error.set(Some(
                    "Enter a magnet link, URL, or drop a .torrent".to_string(),
                ));
                return;
            }
            if !has_file && !(is_magnet || is_url) {
                error.set(Some("Invalid magnet or URL".to_string()));
                return;
            }
            error.set(None);
            let tags_value = (*tags).clone();
            let tags_vec = if tags_value.is_empty() {
                None
            } else {
                let parsed: Vec<String> = tags_value
                    .split(',')
                    .map(|t| t.trim())
                    .filter(|t| !t.is_empty())
                    .map(str::to_string)
                    .collect();
                if parsed.is_empty() {
                    None
                } else {
                    Some(parsed)
                }
            };
            on_submit.emit(AddTorrentInput {
                value: if has_file { None } else { Some(value) },
                file: (*file).clone(),
                category: if category.is_empty() {
                    None
                } else {
                    Some((*category).clone())
                },
                tags: tags_vec,
                save_path: if save_path.is_empty() {
                    None
                } else {
                    Some((*save_path).clone())
                },
            });
        })
    };

    let on_input = {
        let input_value = input_value.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                input_value.set(input.value());
            }
        })
    };

    let on_drop = {
        let drag_over = drag_over.clone();
        let error = error.clone();
        let input_value = input_value.clone();
        let file_state = file.clone();
        Callback::from(move |event: DragEvent| {
            event.prevent_default();
            drag_over.set(false);
            if let Some(files) = event.data_transfer().and_then(|dt| dt.files()) {
                if files.length() == 0 {
                    return;
                }
                let file: File = files.get(0).unwrap();
                let name = file.name();
                if !name.ends_with(".torrent") {
                    error.set(Some("Unsupported file type".to_string()));
                } else {
                    error.set(None);
                    file_state.set(Some(file));
                    input_value.set(name);
                }
            }
        })
    };

    let on_drag_over = {
        let drag_over = drag_over.clone();
        Callback::from(move |event: DragEvent| {
            event.prevent_default();
            drag_over.set(true);
        })
    };

    let on_drag_leave = {
        let drag_over = drag_over.clone();
        Callback::from(move |_event: DragEvent| {
            drag_over.set(false);
        })
    };

    html! {
        <div class="add-panel">
            <div
                class={classes!("drop-zone", if *drag_over { "drag-over" } else { "" })}
                role="button"
                aria-label="Upload torrent"
                ondrop={on_drop}
                ondragover={on_drag_over}
                ondragleave={on_drag_leave}
            >
                <p><strong>{"Drop .torrent or paste magnet"}</strong></p>
                <p class="muted">{"Validates magnet/URL, supports drag/drop and file selection."}</p>
                <div class="inputs">
                    <input aria-label="Magnet or URL" placeholder="Magnet or URL" value={(*input_value).clone()} oninput={on_input} />
                    <button class="solid" onclick={submit.clone()} disabled={props.pending}>
                        {if props.pending { "Adding…" } else { "Add" }}
                    </button>
                </div>
                {if let Some(err) = &*error {
                    html! { <p class="error-text">{err}</p> }
                } else if let Some(f) = &*file {
                    html! { <p class="muted">{format!("Ready to upload {}", f.name())}</p> }
                } else { html! {} }}
            </div>
            <div class="pre-flight">
                <label>
                    <span>{"Category"}</span>
                    <input placeholder="tv, movies, music" value={(*category).clone()} oninput={{
                        let category = category.clone();
                        Callback::from(move |e: InputEvent| {
                            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                category.set(input.value());
                            }
                        })
                    }} />
                </label>
                <label>
                    <span>{"Tags"}</span>
                    <input placeholder="4K, hevc, scene" value={(*tags).clone()} oninput={{
                        let tags = tags.clone();
                        Callback::from(move |e: InputEvent| {
                            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                tags.set(input.value());
                            }
                        })
                    }} />
                </label>
                <label>
                    <span>{"Save path"}</span>
                    <input placeholder="/data/incomplete" value={(*save_path).clone()} oninput={{
                        let save_path = save_path.clone();
                        Callback::from(move |e: InputEvent| {
                            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                save_path.set(input.value());
                            }
                        })
                    }} />
                </label>
            </div>
        </div>
    }
}

#[function_component(MobileActionRow)]
fn mobile_action_row(props: &MobileActionProps) -> Html {
    let pause = {
        let on_action = props.on_action.clone();
        let id = props.selected.clone();
        Callback::from(move |_| {
            if let Some(id) = &id {
                on_action.emit((TorrentAction::Pause, id.clone()));
            }
        })
    };
    let resume = {
        let on_action = props.on_action.clone();
        let id = props.selected.clone();
        Callback::from(move |_| {
            if let Some(id) = &id {
                on_action.emit((TorrentAction::Resume, id.clone()));
            }
        })
    };
    let delete = {
        let on_action = props.on_action.clone();
        let id = props.selected.clone();
        Callback::from(move |_| {
            if let Some(id) = &id {
                on_action.emit((TorrentAction::Delete { with_data: false }, id.clone()));
            }
        })
    };
    html! {
        <div class="mobile-action-row">
            <button class="ghost" onclick={pause}>{"Pause"}</button>
            <button class="ghost" onclick={resume}>{"Resume"}</button>
            <button class="ghost danger" onclick={delete}>{"Delete"}</button>
            <button class="solid">{"More…"}</button>
        </div>
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConfirmKind {
    Delete,
    DeleteData,
    Recheck,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TorrentAction {
    Pause,
    Resume,
    Recheck,
    Delete { with_data: bool },
}

#[derive(Properties, PartialEq)]
pub struct ConfirmProps {
    pub kind: Option<ConfirmKind>,
    pub on_close: Callback<()>,
    pub on_confirm: Callback<ConfirmKind>,
}

#[function_component(ConfirmDialog)]
fn confirm_dialog(props: &ConfirmProps) -> Html {
    let Some(kind) = &props.kind else {
        return html! {};
    };

    let (title, body, action) = match kind {
        ConfirmKind::Delete => (
            "Remove torrent?",
            "Files remain on disk. This cannot be undone without data deletion.",
            "Remove",
        ),
        ConfirmKind::DeleteData => (
            "Remove torrent and delete data?",
            "This permanently deletes files from disk.",
            "Delete data",
        ),
        ConfirmKind::Recheck => (
            "Recheck data?",
            "Hash check will verify existing data.",
            "Recheck",
        ),
    };

    let confirm = {
        let kind = kind.clone();
        let cb = props.on_confirm.clone();
        Callback::from(move |_| cb.emit(kind.clone()))
    };

    html! {
        <div class="confirm-overlay" role="dialog" aria-modal="true">
            <div class="card">
                <header>
                    <h4>{title}</h4>
                </header>
                <p class="muted">{body}</p>
                <div class="actions">
                    <button class="ghost" onclick={props.on_close.clone()}>{"Cancel"}</button>
                    <button class="solid danger" onclick={confirm}>{action}</button>
                </div>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct BannerProps {
    pub message: Option<String>,
}

#[derive(Properties, PartialEq)]
pub struct MobileActionProps {
    pub on_action: Callback<(TorrentAction, String)>,
    pub selected: Option<String>,
}

#[function_component(ActionBanner)]
fn action_banner(props: &BannerProps) -> Html {
    let Some(msg) = props.message.clone() else {
        return html! {};
    };
    html! {
        <div class="action-banner" role="status" aria-live="polite">
            <span class="pill subtle">{"Shortcut"}</span>
            <span>{msg}</span>
        </div>
    }
}

/// Demo torrent set referenced by the default view.
#[must_use]
pub fn demo_rows() -> Vec<TorrentRow> {
    vec![
        TorrentRow {
            id: "1".into(),
            name: "Foundation.S02E08.2160p.WEB-DL.DDP5.1.Atmos.HDR10".into(),
            status: "downloading".into(),
            progress: 0.41,
            eta: Some("12m".into()),
            ratio: 0.12,
            tags: vec!["4K", "HDR10", "hevc"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            tracker: "tracker.hypothetical.org".into(),
            path: "/data/incomplete/foundation-s02e08".into(),
            category: "tv".into(),
            size_gb: 18.4,
            upload_bps: 1_200_000,
            download_bps: 82_000_000,
        },
        TorrentRow {
            id: "2".into(),
            name: "The.Expanse.S01E05.1080p.BluRay.DTS.x264".into(),
            status: "seeding".into(),
            progress: 1.0,
            eta: None,
            ratio: 3.82,
            tags: vec!["blu-ray", "lossless"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            tracker: "tracker.space.example".into(),
            path: "/data/media/TV/The Expanse/Season 1".into(),
            category: "tv".into(),
            size_gb: 7.8,
            upload_bps: 5_400_000,
            download_bps: 0,
        },
        TorrentRow {
            id: "3".into(),
            name: "Dune.Part.One.2021.2160p.REMUX.DV.DTS-HD.MA.7.1".into(),
            status: "paused".into(),
            progress: 0.77,
            eta: Some("–".into()),
            ratio: 0.44,
            tags: vec!["remux", "dolby vision"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            tracker: "movies.example.net".into(),
            path: "/data/incomplete/dune-part-one".into(),
            category: "movies".into(),
            size_gb: 64.3,
            upload_bps: 0,
            download_bps: 0,
        },
        TorrentRow {
            id: "4".into(),
            name: "Ubuntu-24.04.1-live-server-amd64.iso".into(),
            status: "checking".into(),
            progress: 0.13,
            eta: Some("3m".into()),
            ratio: 0.02,
            tags: vec!["iso"].into_iter().map(str::to_string).collect(),
            tracker: "releases.ubuntu.com".into(),
            path: "/data/incomplete/ubuntu".into(),
            category: "os".into(),
            size_gb: 1.2,
            upload_bps: 240_000,
            download_bps: 12_000_000,
        },
        TorrentRow {
            id: "5".into(),
            name: "Arcane.S02E02.1080p.NF.WEB-DL.DDP5.1.Atmos.x264".into(),
            status: "downloading".into(),
            progress: 0.63,
            eta: Some("8m".into()),
            ratio: 0.56,
            tags: vec!["nf", "dolby atmos"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            tracker: "tracker.hypothetical.org".into(),
            path: "/data/incomplete/arcane-s02e02".into(),
            category: "tv".into(),
            size_gb: 5.4,
            upload_bps: 950_000,
            download_bps: 34_000_000,
        },
    ]
}

impl From<TorrentSummary> for TorrentRow {
    fn from(value: TorrentSummary) -> Self {
        Self {
            id: value.id.to_string(),
            name: value.name,
            status: value.status,
            progress: value.progress,
            eta: value.eta_seconds.map(|eta| {
                if eta == 0 {
                    "–".to_string()
                } else {
                    format!("{eta}s")
                }
            }),
            ratio: value.ratio,
            tags: value.tags,
            tracker: value.tracker.unwrap_or_default(),
            path: value.save_path.unwrap_or_default(),
            category: value
                .category
                .unwrap_or_else(|| "uncategorized".to_string()),
            size_gb: value.size_bytes as f32 / (1024.0 * 1024.0 * 1024.0),
            upload_bps: value.upload_bps,
            download_bps: value.download_bps,
        }
    }
}
