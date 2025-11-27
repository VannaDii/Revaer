use crate::breakpoints::Breakpoint;
use crate::components::detail::{DetailData, DetailView, demo_detail};
use crate::components::virtual_list::VirtualList;
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::TorrentSummary;
use crate::services::ApiClient;
use crate::state::{TorrentAction, TorrentRow};
use crate::{Density, UiMode};
use std::collections::BTreeSet;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{DragEvent, File, HtmlElement, KeyboardEvent};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct TorrentProps {
    /// Current responsive breakpoint for layout decisions.
    pub breakpoint: Breakpoint,
    /// API base URL for detail and add flows.
    pub base_url: String,
    /// Optional API key for authenticated calls.
    pub api_key: Option<String>,
    pub torrents: Vec<TorrentRow>,
    pub density: Density,
    pub mode: UiMode,
    pub on_density_change: Callback<Density>,
    pub on_bulk_action: Callback<(TorrentAction, Vec<String>)>,
    pub on_action: Callback<(TorrentAction, String)>,
    pub on_add: Callback<AddTorrentInput>,
    pub add_busy: bool,
    pub search: String,
    pub regex: bool,
    pub on_search: Callback<String>,
    pub on_toggle_regex: Callback<()>,
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
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
    let selected = use_state(|| demo_detail("1"));
    let selected_idx = use_state(|| 0usize);
    let selected_ids = use_state(BTreeSet::<String>::new);
    let action_banner = use_state(|| None as Option<String>);
    let confirm = use_state(|| None as Option<ConfirmKind>);
    let search_ref = use_node_ref();
    let is_mobile = props
        .breakpoint
        .max_width
        .is_some_and(|max| max < crate::breakpoints::MD.min_width);
    let density_class = match props.density {
        Density::Compact => "density-compact",
        Density::Normal => "density-normal",
        Density::Comfy => "density-comfy",
    };
    let row_height = if is_mobile {
        210
    } else {
        match props.density {
            Density::Compact => 120,
            Density::Normal => 148,
            Density::Comfy => 164,
        }
    };
    let mode_class = match props.mode {
        UiMode::Simple => "mode-simple",
        UiMode::Advanced => "mode-advanced",
    };
    let selected_id = props.torrents.get(*selected_idx).map(|row| row.id.clone());
    let selected_count = selected_ids.len();
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
        let base_url = props.base_url.clone();
        let api_key = props.api_key.clone();
        Callback::from(move |id: String| {
            if let Some(idx) = props.torrents.iter().position(|row| row.id == id) {
                selected_idx.set(idx);
            }
            let selected = selected.clone();
            let client = ApiClient::new(base_url.clone(), api_key.clone());
            yew::platform::spawn_local(async move {
                match client.fetch_torrent_detail(&id).await {
                    Ok(detail) => selected.set(Some(detail)),
                    Err(_) => selected.set(demo_detail(&id)),
                }
            });
        })
    };

    // Keyboard shortcuts: j/k navigation, space pause/resume, delete/shift+delete confirmations, p recheck, / focus search.
    {
        let torrents = props.torrents.clone();
        let selected_idx = selected_idx.clone();
        let on_select = on_select.clone();
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
                                    on_select.emit(row.id.clone());
                                }
                            }
                        }
                        "k" | "K" => {
                            event.prevent_default();
                            let next = selected_idx.saturating_sub(1);
                            if next != *selected_idx {
                                selected_idx.set(next);
                                if let Some(row) = torrents.get(next) {
                                    on_select.emit(row.id.clone());
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
                        "Escape" => {
                            if let Some(input) = search_ref.cast::<web_sys::HtmlInputElement>() {
                                input.set_value("");
                                let _ = input.blur();
                                props.on_search.emit(String::new());
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
                    <input
                        aria-label={t("torrents.search_label")}
                        placeholder={t("torrents.search_placeholder")}
                        ref={search_ref.clone()}
                        value={props.search.clone()}
                        oninput={{
                            let on_search = props.on_search.clone();
                            Callback::from(move |e: InputEvent| {
                                if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                    on_search.emit(input.value());
                                }
                            })
                        }}
                    />
                    <button
                        class={classes!("ghost", if props.regex { Some("active") } else { None })}
                        onclick={{
                            let cb = props.on_toggle_regex.clone();
                            Callback::from(move |_| cb.emit(()))
                        }}
                    >
                        {t("toolbar.regex")}
                    </button>
                </div>
                <div class="bulk-actions">
                    <button class="ghost" onclick={{
                        let selected_ids = selected_ids.clone();
                        let torrents = props.torrents.clone();
                        Callback::from(move |_| {
                            let mut next = selected_ids.clone();
                            if next.len() == torrents.len() {
                                next.clear();
                            } else {
                                next = torrents.iter().map(|t| t.id.clone()).collect();
                            }
                            selected_ids.set(next);
                        })
                    }}>
                        {t("torrents.select_all")}
                    </button>
                    <span class="muted">{format!("{} {}", selected_count, t("torrents.selected"))}</span>
                    <div class="bulk-buttons">
                        {for [
                            (TorrentAction::Pause, "toolbar.pause"),
                            (TorrentAction::Resume, "toolbar.resume"),
                            (TorrentAction::Recheck, "toolbar.recheck"),
                        ]
                        .iter()
                        .map(|(action, key)| {
                            let label = t(key);
                            let cb = {
                                let on_bulk = props.on_bulk_action.clone();
                                let ids = selected_ids.clone();
                                let action = action.clone();
                                Callback::from(move |_| {
                                    if !ids.is_empty() {
                                        on_bulk.emit((action.clone(), ids.iter().cloned().collect()));
                                    }
                                })
                            };
                            html! { <button class="ghost" onclick={cb}>{label}</button> }
                        })}
                        <button class="ghost danger" onclick={{
                            let on_bulk = props.on_bulk_action.clone();
                            let ids = selected_ids.clone();
                            Callback::from(move |_| {
                                if !ids.is_empty() {
                                    on_bulk.emit((TorrentAction::Delete { with_data: false }, ids.iter().cloned().collect()));
                                }
                            })
                        }}>{t("toolbar.delete")}</button>
                    </div>
                </div>
                <div class="actions">
                    <div class="segmented density">
                        {Density::all().iter().map(|option| {
                            let label = match option {
                                Density::Compact => t("density.compact"),
                                Density::Normal => t("density.normal"),
                                Density::Comfy => t("density.comfy"),
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
                    <button class="ghost" onclick={pause_selected}>{t("toolbar.pause")}</button>
                    <button class="ghost" onclick={resume_selected}>{t("toolbar.resume")}</button>
                    <button class="ghost danger" onclick={delete_selected}>{t("toolbar.delete")}</button>
                    <button class="solid">{t("toolbar.add")}</button>
                </div>
            </header>

            <AddTorrentPanel on_submit={props.on_add.clone()} pending={props.add_busy} />

            <VirtualList
                class={classes!("torrent-table", "virtualized")}
                len={props.torrents.len()}
                row_height={row_height}
                overscan={6}
                height={if is_mobile { Some("70vh".into()) } else { None }}
                render={{
                    let on_select = on_select.clone();
                    let torrents = props.torrents.clone();
                    let bundle = bundle.clone();
                    let selected_idx = *selected_idx;
                    let on_action = props.on_action.clone();
                    let is_mobile = is_mobile;
                    let selected_ids_handle = selected_ids.clone();
                    let toggle_select = Callback::from(move |id: String| {
                        let mut next = (*selected_ids_handle).clone();
                        if !next.remove(&id) {
                            next.insert(id);
                        }
                        selected_ids_handle.set(next);
                    });
                    Callback::from(move |idx: usize| {
                        if let Some(row) = torrents.get(idx) {
                            if is_mobile {
                                render_mobile_row(
                                    row,
                                    idx == selected_idx,
                                    selected_ids.contains(&row.id),
                                    on_select.clone(),
                                    toggle_select.clone(),
                                    on_action.clone(),
                                    bundle.clone(),
                                )
                            } else {
                                render_row(
                                    row,
                                    idx == selected_idx,
                                    selected_ids.contains(&row.id),
                                    on_select.clone(),
                                    toggle_select.clone(),
                                    on_action.clone(),
                                    bundle.clone(),
                                )
                            }
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
                    let bundle = bundle.clone();
                    Callback::from(move |kind: ConfirmKind| {
                        confirm.set(None);
                        if let Some(row) = torrents.get(selected_idx) {
                            let action = match kind {
                                ConfirmKind::Delete => TorrentAction::Delete { with_data: false },
                                ConfirmKind::DeleteData => TorrentAction::Delete { with_data: true },
                                ConfirmKind::Recheck => TorrentAction::Recheck,
                            };
                            on_action.emit((action.clone(), row.id.clone()));
                            action_banner
                                .set(Some(action_banner_message(&bundle, &action, &row.name)));
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
    checked: bool,
    on_select: Callback<String>,
    on_toggle: Callback<String>,
    on_action: Callback<(TorrentAction, String)>,
    bundle: TranslationBundle,
) -> Html {
    let t = |key: &str| bundle.text(key, "");
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
            <div class="row-checkbox">
                <input
                    type="checkbox"
                    aria-label={t("torrents.select_row")}
                    checked={checked}
                    onclick={{
                        let on_toggle = on_toggle.clone();
                        let id = row.id.clone();
                        Callback::from(move |_| on_toggle.emit(id.clone()))
                    }}
                />
            </div>
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
                        <span class="muted">{row.eta.clone().unwrap_or_else(|| t("torrents.eta_infinite"))}</span>
                    </div>
                </div>
            </div>
            <div class="row-secondary">
                <div class="stat">
                    <small>{t("torrents.down")}</small>
                    <strong>{format_rate(row.download_bps)}</strong>
                </div>
                <div class="stat">
                    <small>{t("torrents.up")}</small>
                    <strong>{format_rate(row.upload_bps)}</strong>
                </div>
                <div class="stat">
                    <small>{t("torrents.ratio")}</small>
                    <strong>{format!("{:.2}", row.ratio)}</strong>
                </div>
                <div class="stat">
                    <small>{t("torrents.size")}</small>
                    <strong>{row.size_label()}</strong>
                </div>
            </div>
            <div class="row-meta">
                <span class="muted">{row.path}</span>
                <div class="tags">
                    <span class="pill subtle">{row.category.clone()}</span>
                    {for row.tags.iter().map(|tag| html! { <span class="pill subtle">{tag.to_owned()}</span> })}
                </div>
            </div>
            <div class="row-actions">
                <button class="ghost" onclick={select.clone()}>{t("torrents.open_detail")}</button>
                <button class="ghost" onclick={pause}>{t("toolbar.pause")}</button>
                <button class="ghost" onclick={resume}>{t("toolbar.resume")}</button>
                <button class="ghost" onclick={recheck}>{t("toolbar.recheck")}</button>
                <button class="ghost danger" onclick={delete_data}>{t("toolbar.delete_data")}</button>
            </div>
        </article>
    }
}

fn render_mobile_row(
    row: &TorrentRow,
    selected: bool,
    checked: bool,
    on_select: Callback<String>,
    on_toggle: Callback<String>,
    on_action: Callback<(TorrentAction, String)>,
    bundle: TranslationBundle,
) -> Html {
    let t = |key: &str| bundle.text(key, "");
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
    let delete_data = {
        let on_action = on_action.clone();
        let id = row.id.clone();
        Callback::from(move |_| {
            on_action.emit((TorrentAction::Delete { with_data: true }, id.clone()))
        })
    };
    html! {
        <article class={classes!("torrent-row", "mobile", if selected { Some("selected") } else { None })} aria-selected={selected.to_string()}>
            <header class="title">
                <div class="row-checkbox">
                    <input
                        type="checkbox"
                        aria-label={t("torrents.select_row")}
                        checked={checked}
                        onclick={{
                            let on_toggle = on_toggle.clone();
                            let id = row.id.clone();
                            Callback::from(move |_| on_toggle.emit(id.clone()))
                        }}
                    />
                </div>
                <div>
                    <strong>{row.name.clone()}</strong>
                    <p class="muted ellipsis">{row.tracker.clone()}</p>
                </div>
                <span class={classes!("pill", status_class(row.status))}>{row.status.clone()}</span>
            </header>
            <div class="progress">
                <div class="bar" style={format!("width: {:.1}%", row.progress * 100.0)}></div>
                <div class="meta">
                    <span class="muted">{format!("{:.1}%", row.progress * 100.0)}</span>
                    <span class="muted">{row.eta.clone().unwrap_or_else(|| t("torrents.eta_infinite"))}</span>
                </div>
            </div>
            <div class="mobile-stats">
                <div><small>{t("torrents.down")}</small><strong>{format_rate(row.download_bps)}</strong></div>
                <div><small>{t("torrents.up")}</small><strong>{format_rate(row.upload_bps)}</strong></div>
                <div><small>{t("torrents.ratio")}</small><strong>{format!("{:.2}", row.ratio)}</strong></div>
                <div><small>{t("torrents.size")}</small><strong>{row.size_label()}</strong></div>
            </div>
            <div class="row-meta">
                <span class="muted ellipsis">{row.path.clone()}</span>
                <div class="tags">
                    <span class="pill subtle">{row.category.clone()}</span>
                    {for row.tags.iter().map(|tag| html! { <span class="pill subtle">{tag.to_owned()}</span> })}
                </div>
            </div>
            <div class="row-actions mobile-grid">
                <button class="ghost" onclick={select.clone()}>{t("torrents.open_detail")}</button>
                <button class="ghost" onclick={pause}>{t("toolbar.pause")}</button>
                <button class="ghost" onclick={resume}>{t("toolbar.resume")}</button>
                <button class="ghost danger" onclick={delete_data}>{t("toolbar.delete_data")}</button>
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

fn action_banner_message(bundle: &TranslationBundle, action: &TorrentAction, name: &str) -> String {
    match action {
        TorrentAction::Delete { with_data: true } => {
            format!("{} {name}", bundle.text("torrents.banner.removed_data", ""))
        }
        TorrentAction::Delete { with_data: false } => {
            format!("{} {name}", bundle.text("torrents.banner.removed", ""))
        }
        TorrentAction::Recheck => {
            format!("{} {name}", bundle.text("torrents.banner.recheck", ""))
        }
        TorrentAction::Pause => format!("{} {name}", bundle.text("torrents.banner.pause", "")),
        TorrentAction::Resume => format!("{} {name}", bundle.text("torrents.banner.resume", "")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_class_maps_states() {
        assert_eq!(status_class("downloading"), "ok");
        assert_eq!(status_class("paused"), "muted");
        assert_eq!(status_class("unknown"), "muted");
    }

    #[test]
    fn format_rate_scales_units() {
        assert_eq!(format_rate(512), "512 B/s");
        assert_eq!(format_rate(2048), "2.0 KiB/s");
        assert!(format_rate(5_242_880).contains("MiB"));
        assert!(format_rate(2_147_483_648).contains("GiB"));
    }

    #[test]
    fn action_banner_uses_locale_strings() {
        let bundle = TranslationBundle::new(DEFAULT_LOCALE);
        let msg = action_banner_message(&bundle, &TorrentAction::Pause, "alpha");
        assert!(msg.contains(&bundle.text("torrents.banner.pause", "")));
    }
}

#[derive(Properties, PartialEq)]
pub struct AddTorrentProps {
    pub on_submit: Callback<AddTorrentInput>,
    pub pending: bool,
}

#[function_component(AddTorrentPanel)]
fn add_torrent_panel(props: &AddTorrentProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
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
                error.set(Some(t("torrents.error.empty")));
                return;
            }
            if !has_file && !(is_magnet || is_url) {
                error.set(Some(t("torrents.error.invalid")));
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
                    error.set(Some(t("torrents.error.file_type")));
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
                aria-label={t("torrents.upload_label")}
                ondrop={on_drop}
                ondragover={on_drag_over}
                ondragleave={on_drag_leave}
            >
                <p><strong>{t("torrents.drop_help")}</strong></p>
                <p class="muted">{t("torrents.drop_sub")}</p>
                <div class="inputs">
                    <input aria-label={t("torrents.add_placeholder")} placeholder={t("torrents.add_placeholder")} value={(*input_value).clone()} oninput={on_input} />
                    <button class="solid" onclick={submit.clone()} disabled={props.pending}>
                        {if props.pending { t("torrents.adding") } else { t("toolbar.add") }}
                    </button>
                </div>
                {if let Some(err) = &*error {
                    html! { <p class="error-text">{err}</p> }
                } else if let Some(f) = &*file {
                    html! { <p class="muted">{format!("{} {}", t("torrents.ready_prefix"), f.name())}</p> }
                } else { html! {} }}
            </div>
            <div class="pre-flight">
                <div class="watch-folder">
                    <strong>{t("torrents.watch_folder")}</strong>
                    <p class="muted">{t("torrents.watch_folder_body")}</p>
                </div>
                <label>
                    <span>{t("torrents.category")}</span>
                    <input placeholder={t("torrents.category_placeholder")} value={(*category).clone()} oninput={{
                        let category = category.clone();
                        Callback::from(move |e: InputEvent| {
                            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                category.set(input.value());
                            }
                        })
                    }} />
                </label>
                <label>
                    <span>{t("torrents.tags")}</span>
                    <input placeholder={t("torrents.tags_placeholder")} value={(*tags).clone()} oninput={{
                        let tags = tags.clone();
                        Callback::from(move |e: InputEvent| {
                            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                tags.set(input.value());
                            }
                        })
                    }} />
                </label>
                <label>
                    <span>{t("torrents.save_path")}</span>
                    <input placeholder={t("torrents.save_path_placeholder")} value={(*save_path).clone()} oninput={{
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
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
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
            <button class="ghost" onclick={pause}>{t("toolbar.pause")}</button>
            <button class="ghost" onclick={resume}>{t("toolbar.resume")}</button>
            <button class="ghost danger" onclick={delete}>{t("toolbar.delete")}</button>
            <button class="solid">{t("torrents.more")}</button>
        </div>
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConfirmKind {
    Delete,
    DeleteData,
    Recheck,
}

#[derive(Properties, PartialEq)]
pub struct ConfirmProps {
    pub kind: Option<ConfirmKind>,
    pub on_close: Callback<()>,
    pub on_confirm: Callback<ConfirmKind>,
}

#[function_component(ConfirmDialog)]
fn confirm_dialog(props: &ConfirmProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
    let Some(kind) = &props.kind else {
        return html! {};
    };

    let (title, body, action) = match kind {
        ConfirmKind::Delete => (
            t("confirm.delete.title"),
            t("confirm.delete.body"),
            t("confirm.delete.cta"),
        ),
        ConfirmKind::DeleteData => (
            t("confirm.delete_data.title"),
            t("confirm.delete_data.body"),
            t("confirm.delete_data.cta"),
        ),
        ConfirmKind::Recheck => (
            t("confirm.recheck.title"),
            t("confirm.recheck.body"),
            t("confirm.recheck.cta"),
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
                    <button class="ghost" onclick={props.on_close.clone()}>{t("confirm.cancel")}</button>
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
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
    let Some(msg) = props.message.clone() else {
        return html! {};
    };
    html! {
        <div class="action-banner" role="status" aria-live="polite">
            <span class="pill subtle">{t("torrents.shortcut")}</span>
            <span>{msg}</span>
        </div>
    }
}

/// Demo torrent set referenced by the default view.
#[must_use]
pub fn demo_rows() -> Vec<TorrentRow> {
    const GIB: u64 = 1_073_741_824;
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
            size_bytes: 18 * GIB,
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
            size_bytes: 8 * GIB,
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
            size_bytes: 64 * GIB,
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
            size_bytes: 2 * GIB,
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
            size_bytes: 6 * GIB,
            upload_bps: 950_000,
            download_bps: 34_000_000,
        },
    ]
}

impl From<TorrentSummary> for TorrentRow {
    fn from(value: TorrentSummary) -> Self {
        crate::state::TorrentRow::from(value)
    }
}
