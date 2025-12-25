use crate::app::Route;
use crate::components::detail::{DetailView, FileSelectionChange};
use crate::components::virtual_list::VirtualList;
use crate::core::breakpoints::Breakpoint;
use crate::core::logic::{
    ShortcutOutcome, format_rate, interpret_shortcut, plan_columns, select_all_or_clear,
    toggle_selection,
};
use crate::core::store::AppStore;
use crate::features::torrents::actions::TorrentAction;
use crate::features::torrents::state::{
    FsopsBadge, FsopsStatus, SelectionSet, TorrentProgressSlice, TorrentRow, TorrentRowBase,
    select_fsops_badge, select_is_selected, select_torrent_progress_slice, select_torrent_row_base,
};
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::{AddTorrentInput, ConfirmKind};
use crate::{Density, UiMode};
use std::rc::Rc;
use uuid::Uuid;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{DragEvent, File, HtmlElement, KeyboardEvent};
use yew::prelude::*;
use yew_router::prelude::{Link, use_navigator};
use yewdux::prelude::use_selector;

#[derive(Properties, PartialEq)]
pub(crate) struct TorrentProps {
    /// Current responsive breakpoint for layout decisions.
    pub breakpoint: Breakpoint,
    pub visible_ids: Vec<Uuid>,
    pub density: Density,
    pub mode: UiMode,
    pub on_density_change: Callback<Density>,
    pub on_bulk_action: Callback<(TorrentAction, Vec<Uuid>)>,
    pub on_action: Callback<(TorrentAction, Uuid)>,
    pub on_add: Callback<AddTorrentInput>,
    pub add_busy: bool,
    pub search: String,
    pub on_search: Callback<String>,
    pub selected_id: Option<Uuid>,
    pub selected_ids: SelectionSet,
    pub on_set_selected: Callback<SelectionSet>,
    /// Selected detail payload for the drawer.
    pub selected_detail: Option<crate::models::DetailData>,
    /// Request a detail refresh for a torrent id.
    pub on_select_detail: Callback<Uuid>,
    /// Request a file selection update for a torrent.
    pub on_update_selection: Callback<(Uuid, FileSelectionChange)>,
}

#[function_component(TorrentView)]
pub(crate) fn torrent_view(props: &TorrentProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let bundle_for_t = bundle.clone();
    let bundle = bundle.clone();
    let t = move |key: &str| bundle_for_t.text(key, "");
    let selected_idx = use_state(|| 0usize);
    let action_banner = use_state(|| None as Option<String>);
    let confirm = use_state(|| None as Option<ConfirmKind>);
    let search_ref = use_node_ref();
    let navigator = use_navigator();
    let is_mobile = props
        .breakpoint
        .max_width
        .is_some_and(|max| max < crate::breakpoints::MD.min_width);
    let density_class = match props.density {
        Density::Compact => "density-compact",
        Density::Normal => "density-normal",
        Density::Comfy => "density-comfy",
    };
    let row_height = crate::core::logic::row_height(
        props.density,
        if is_mobile {
            crate::core::logic::LayoutMode::Card
        } else {
            crate::core::logic::LayoutMode::Table
        },
    );
    let mode_class = match props.mode {
        UiMode::Simple => "mode-simple",
        UiMode::Advanced => "mode-advanced",
    };
    let is_mobile = crate::core::logic::layout_for_breakpoint(props.breakpoint)
        == crate::core::logic::LayoutMode::Card;
    let selected_id = props.visible_ids.get(*selected_idx).copied();
    let selected_ids = props.selected_ids.clone();
    let selected_count = selected_ids.len();
    let selected_base = {
        let selected_id = selected_id;
        use_selector(move |store: &AppStore| {
            selected_id.and_then(|id| select_torrent_row_base(&store.torrents, &id))
        })
    };
    let selected_name = (*selected_base)
        .as_ref()
        .map(|base| base.name.clone())
        .unwrap_or_default();
    let pause_selected = {
        let on_action = props.on_action.clone();
        Callback::from(move |_| {
            if let Some(id) = selected_id {
                on_action.emit((TorrentAction::Pause, id));
            }
        })
    };
    let resume_selected = {
        let on_action = props.on_action.clone();
        Callback::from(move |_| {
            if let Some(id) = selected_id {
                on_action.emit((TorrentAction::Resume, id));
            }
        })
    };
    let delete_selected = {
        let on_action = props.on_action.clone();
        Callback::from(move |_| {
            if let Some(id) = selected_id {
                on_action.emit((TorrentAction::Delete { with_data: false }, id));
            }
        })
    };
    let on_toggle_file = {
        let selected_id = props.selected_id;
        let on_update_selection = props.on_update_selection.clone();
        Callback::from(move |change: FileSelectionChange| {
            if let Some(id) = selected_id {
                on_update_selection.emit((id, change));
            }
        })
    };

    let on_select = {
        let selected_idx = selected_idx.clone();
        let visible_ids = props.visible_ids.clone();
        let navigator = navigator.clone();
        let on_select_detail = props.on_select_detail.clone();
        Callback::from(move |id: Uuid| {
            if let Some(idx) = visible_ids.iter().position(|row_id| *row_id == id) {
                selected_idx.set(idx);
            }
            if let Some(navigator) = navigator.clone() {
                navigator.push(&Route::TorrentDetail { id: id.to_string() });
            }
            on_select_detail.emit(id);
        })
    };

    {
        let selected_idx = selected_idx.clone();
        use_effect_with_deps(
            move |(selected_id, torrents)| {
                if let Some(id) = selected_id.clone() {
                    if let Some(idx) = torrents.iter().position(|row_id| *row_id == id) {
                        selected_idx.set(idx);
                    }
                }
                || ()
            },
            (props.selected_id.clone(), props.visible_ids.clone()),
        );
    }

    // Keyboard shortcuts: j/k navigation, space pause/resume, delete/shift+delete confirmations, p recheck, / focus search.
    {
        let visible_ids = props.visible_ids.clone();
        let selected_idx = selected_idx.clone();
        let on_select = on_select.clone();
        let search_ref = search_ref.clone();
        let action_banner = action_banner.clone();
        let confirm = confirm.clone();
        let on_action = props.on_action.clone();
        let on_search = props.on_search.clone();
        let bundle = bundle.clone();
        use_effect_with_deps(
            move |_| {
                let handler = Closure::<dyn FnMut(_)>::wrap(Box::new(move |event: KeyboardEvent| {
                    if let Some(target) = event.target()
                        && let Ok(element) = target.dyn_into::<HtmlElement>()
                        && matches!(element.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT")
                    {
                        return;
                    }

                    if let Some(action) = interpret_shortcut(&event.key(), event.shift_key()) {
                        event.prevent_default();
                        match action {
                            ShortcutOutcome::FocusSearch => {
                                if let Some(input) = search_ref.cast::<web_sys::HtmlInputElement>()
                                {
                                    let _ = input.focus();
                                }
                            }
                            ShortcutOutcome::SelectNext => {
                                if let Some(next) = crate::core::logic::advance_selection(
                                    ShortcutOutcome::SelectNext,
                                    *selected_idx,
                                    visible_ids.len(),
                                ) {
                                    selected_idx.set(next);
                                    if let Some(id) = visible_ids.get(next) {
                                        on_select.emit(*id);
                                    }
                                }
                            }
                            ShortcutOutcome::SelectPrev => {
                                if let Some(next) = crate::core::logic::advance_selection(
                                    ShortcutOutcome::SelectPrev,
                                    *selected_idx,
                                    visible_ids.len(),
                                ) {
                                    selected_idx.set(next);
                                    if let Some(id) = visible_ids.get(next) {
                                        on_select.emit(*id);
                                    }
                                }
                            }
                            ShortcutOutcome::TogglePauseResume => {
                                if let Some(id) = visible_ids.get(*selected_idx) {
                                    action_banner.set(Some(bundle.text("toast.pause", "")));
                                    on_action.emit((TorrentAction::Pause, *id));
                                }
                            }
                            ShortcutOutcome::ClearSearch => {
                                if let Some(input) = search_ref.cast::<web_sys::HtmlInputElement>()
                                {
                                    input.set_value("");
                                    let _ = input.blur();
                                    on_search.emit(String::new());
                                }
                            }
                            ShortcutOutcome::ConfirmDelete => {
                                confirm.set(Some(ConfirmKind::Delete))
                            }
                            ShortcutOutcome::ConfirmDeleteData => {
                                confirm.set(Some(ConfirmKind::DeleteData))
                            }
                            ShortcutOutcome::ConfirmRecheck => {
                                confirm.set(Some(ConfirmKind::Recheck))
                            }
                        }
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
                </div>
                <div class="bulk-actions">
                    <button class="ghost" onclick={{
                        let selected_ids = selected_ids.clone();
                        let on_set_selected = props.on_set_selected.clone();
                        let visible_ids = props.visible_ids.clone();
                        Callback::from(move |_| {
                            on_set_selected.emit(select_all_or_clear(&selected_ids, &visible_ids));
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
                len={props.visible_ids.len()}
                row_height={row_height}
                overscan={6}
                height={if is_mobile { Some(String::from("70vh")) } else { Option::<String>::None }}
                render={{
                    let on_select = on_select.clone();
                    let visible_ids = props.visible_ids.clone();
                    let bundle = bundle.clone();
                    let selected_idx = *selected_idx;
                    let on_action = props.on_action.clone();
                    let is_mobile = is_mobile;
                    let width_hint =
                        props.breakpoint.max_width.unwrap_or(props.breakpoint.min_width);
                    let (visible_cols, overflow_cols) = plan_columns(width_hint);
                    let visible_cols = std::rc::Rc::new(visible_cols);
                    let overflow_cols = std::rc::Rc::new(overflow_cols);
                    let selected_ids_for_toggle = selected_ids.clone();
                    let on_set_selected = props.on_set_selected.clone();
                    let toggle_select = Callback::from(move |id: Uuid| {
                        on_set_selected.emit(toggle_selection(&selected_ids_for_toggle, &id));
                    });
                    Callback::from(move |idx: usize| {
                        if let Some(id) = visible_ids.get(idx) {
                            html! {
                                <TorrentRowItem
                                    id={*id}
                                    active={idx == selected_idx}
                                    is_mobile={is_mobile}
                                    on_select={on_select.clone()}
                                    on_toggle={toggle_select.clone()}
                                    on_action={on_action.clone()}
                                    bundle={bundle.clone()}
                                    visible_cols={visible_cols.clone()}
                                    overflow_cols={overflow_cols.clone()}
                                />
                            }
                        } else {
                            html! {}
                        }
                    })
                }}
            />

            <DetailView data={props.selected_detail.clone()} on_toggle_file={on_toggle_file} />
            <MobileActionRow
                on_action={props.on_action.clone()}
                selected={props.visible_ids.get(*selected_idx).copied()}
            />
            <ActionBanner message={(*action_banner).clone()} />
            <ConfirmDialog
                kind={(*confirm).clone()}
                on_close={{
                    let confirm = confirm.clone();
                    Callback::from(move |_| confirm.set(None))
                }}
                on_confirm={{
                    let confirm = confirm.clone();
                    let selected_id = selected_id;
                    let selected_name = selected_name.clone();
                    let action_banner = action_banner.clone();
                    let on_action = props.on_action.clone();
                    let bundle = bundle.clone();
                    Callback::from(move |kind: ConfirmKind| {
                        confirm.set(None);
                        if let Some(id) = selected_id {
                            let action = match kind {
                                ConfirmKind::Delete => TorrentAction::Delete { with_data: false },
                                ConfirmKind::DeleteData => TorrentAction::Delete { with_data: true },
                                ConfirmKind::Recheck => TorrentAction::Recheck,
                            };
                            on_action.emit((action.clone(), id));
                            action_banner
                                .set(Some(action_banner_message(&bundle, &action, &selected_name)));
                        }
                    })
                }}
            />
        </section>
    }
}

#[derive(Properties, PartialEq)]
struct TorrentRowItemProps {
    id: Uuid,
    active: bool,
    is_mobile: bool,
    on_select: Callback<Uuid>,
    on_toggle: Callback<Uuid>,
    on_action: Callback<(TorrentAction, Uuid)>,
    bundle: TranslationBundle,
    visible_cols: Rc<Vec<&'static str>>,
    overflow_cols: Rc<Vec<&'static str>>,
}

#[function_component(TorrentRowItem)]
fn torrent_row_item(props: &TorrentRowItemProps) -> Html {
    let id = props.id;
    let base = use_selector(move |store: &AppStore| select_torrent_row_base(&store.torrents, &id));
    let progress =
        use_selector(move |store: &AppStore| select_torrent_progress_slice(&store.torrents, &id));
    let fsops = use_selector(move |store: &AppStore| select_fsops_badge(&store.torrents, &id));
    let checked = use_selector(move |store: &AppStore| select_is_selected(&store.torrents, &id));

    let base = (*base).clone();
    let progress = (*progress).clone();
    let fsops = (*fsops).clone();
    let checked = *checked;

    let Some(base) = base else {
        return html! {};
    };
    let Some(progress) = progress else {
        return html! {};
    };

    if props.is_mobile {
        render_mobile_row(
            &base,
            &progress,
            fsops.as_ref(),
            props.active,
            checked,
            props.on_select.clone(),
            props.on_toggle.clone(),
            props.on_action.clone(),
            props.bundle.clone(),
            props.visible_cols.as_slice(),
            props.overflow_cols.as_slice(),
        )
    } else {
        render_row(
            &base,
            &progress,
            fsops.as_ref(),
            props.active,
            checked,
            props.on_select.clone(),
            props.on_toggle.clone(),
            props.on_action.clone(),
            props.bundle.clone(),
            props.visible_cols.as_slice(),
            props.overflow_cols.as_slice(),
        )
    }
}

fn render_row(
    base: &TorrentRowBase,
    progress: &TorrentProgressSlice,
    fsops: Option<&FsopsBadge>,
    selected: bool,
    checked: bool,
    on_select: Callback<Uuid>,
    on_toggle: Callback<Uuid>,
    on_action: Callback<(TorrentAction, Uuid)>,
    bundle: TranslationBundle,
    visible_cols: &[&str],
    overflow_cols: &[&str],
) -> Html {
    let t = |key: &str| bundle.text(key, "");
    let show_eta = visible_cols.contains(&"eta");
    let show_ratio = visible_cols.contains(&"ratio");
    let show_size = visible_cols.contains(&"size");
    let show_tags = visible_cols.contains(&"tags");
    let show_path = visible_cols.contains(&"path");
    let mut overflow = Vec::new();
    if overflow_cols.contains(&"eta") {
        overflow.push((
            t("torrents.eta"),
            progress
                .eta
                .clone()
                .unwrap_or_else(|| t("torrents.eta_infinite")),
        ));
    }
    if overflow_cols.contains(&"ratio") {
        overflow.push((t("torrents.ratio"), format!("{:.2}", base.ratio)));
    }
    if overflow_cols.contains(&"size") {
        overflow.push((t("torrents.size"), base.size_label()));
    }
    if overflow_cols.contains(&"tags") && !base.tags.is_empty() {
        overflow.push((t("torrents.tags"), base.tags.join(", ")));
    }
    if overflow_cols.contains(&"path") && !base.path.is_empty() {
        overflow.push((t("torrents.save_path"), base.path.clone()));
    }
    let select = {
        let on_select = on_select.clone();
        let id = base.id;
        Callback::from(move |_| on_select.emit(id))
    };
    let pause = {
        let on_action = on_action.clone();
        let id = base.id;
        Callback::from(move |_| on_action.emit((TorrentAction::Pause, id)))
    };
    let resume = {
        let on_action = on_action.clone();
        let id = base.id;
        Callback::from(move |_| on_action.emit((TorrentAction::Resume, id)))
    };
    let recheck = {
        let on_action = on_action.clone();
        let id = base.id;
        Callback::from(move |_| on_action.emit((TorrentAction::Recheck, id)))
    };
    let delete_data = {
        let on_action = on_action.clone();
        let id = base.id;
        Callback::from(move |_| on_action.emit((TorrentAction::Delete { with_data: true }, id)))
    };
    let fsops_label = fsops_label(&bundle, fsops);
    html! {
        <article class={classes!("torrent-row", if selected { Some("selected") } else { None })} aria-selected={selected.to_string()}>
            <div class="row-checkbox">
                <input
                    type="checkbox"
                    aria-label={t("torrents.select_row")}
                    checked={checked}
                    onclick={{
                        let on_toggle = on_toggle.clone();
                        let id = base.id;
                        Callback::from(move |_| on_toggle.emit(id))
                    }}
                />
            </div>
            <div class="row-primary">
                <div class="title">
                    <strong>{base.name.clone()}</strong>
                    <span class="muted">{base.tracker.clone()}</span>
                </div>
                <div class="status">
                    <span class={classes!("pill", status_class(&progress.status))}>{progress.status.clone()}</span>
                    {if let Some(label) = fsops_label {
                        html! {
                            <span class={classes!("pill", fsops_class(fsops), "subtle")} title={fsops.and_then(|badge| badge.detail.clone()).unwrap_or_default()}>
                                {label}
                            </span>
                        }
                    } else { html!{} }}
                    <div class="progress">
                        <div class="bar" style={format!("width: {:.1}%", progress.progress * 100.0)}></div>
                        <span class="muted">{format!("{:.1}%", progress.progress * 100.0)}</span>
                        {if show_eta {
                            html! { <span class="muted">{progress.eta.clone().unwrap_or_else(|| t("torrents.eta_infinite"))}</span> }
                        } else { html!{} }}
                    </div>
                </div>
            </div>
            <div class="row-secondary">
                <div class="stat">
                    <small>{t("torrents.down")}</small>
                    <strong>{format_rate(progress.download_bps)}</strong>
                </div>
                <div class="stat">
                    <small>{t("torrents.up")}</small>
                    <strong>{format_rate(progress.upload_bps)}</strong>
                </div>
                {if show_ratio { html! {
                    <div class="stat">
                        <small>{t("torrents.ratio")}</small>
                        <strong>{format!("{:.2}", base.ratio)}</strong>
                    </div>
                }} else { html!{} }}
                {if show_size { html! {
                    <div class="stat">
                        <small>{t("torrents.size")}</small>
                        <strong>{base.size_label()}</strong>
                    </div>
                }} else { html!{} }}
            </div>
            <div class="row-meta">
                {if show_path { html! { <span class="muted">{base.path.clone()}</span> }} else { html!{} }}
                <div class="tags">
                    <span class="pill subtle">{base.category.clone()}</span>
                    {if show_tags {
                        html! {for base.tags.iter().map(|tag| html! { <span class="pill subtle">{tag.to_owned()}</span> }) }
                    } else { html!{} }}
                </div>
            </div>
            {if overflow.is_empty() { html!{} } else {
                html! {
                    <div class="row-overflow">
                        {for overflow.iter().map(|(label, value)| html!{
                            <span class="pill subtle">{format!("{label}: {value}")}</span>
                        })}
                    </div>
                }
            }}
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
    base: &TorrentRowBase,
    progress: &TorrentProgressSlice,
    fsops: Option<&FsopsBadge>,
    selected: bool,
    checked: bool,
    on_select: Callback<Uuid>,
    on_toggle: Callback<Uuid>,
    on_action: Callback<(TorrentAction, Uuid)>,
    bundle: TranslationBundle,
    visible_cols: &[&str],
    overflow_cols: &[&str],
) -> Html {
    let t = |key: &str| bundle.text(key, "");
    let show_eta = visible_cols.contains(&"eta");
    let show_ratio = visible_cols.contains(&"ratio");
    let show_size = visible_cols.contains(&"size");
    let show_tags = visible_cols.contains(&"tags");
    let show_path = visible_cols.contains(&"path");
    let mut overflow = Vec::new();
    if overflow_cols.contains(&"eta") {
        overflow.push((
            t("torrents.eta"),
            progress
                .eta
                .clone()
                .unwrap_or_else(|| t("torrents.eta_infinite")),
        ));
    }
    if overflow_cols.contains(&"ratio") {
        overflow.push((t("torrents.ratio"), format!("{:.2}", base.ratio)));
    }
    if overflow_cols.contains(&"size") {
        overflow.push((t("torrents.size"), base.size_label()));
    }
    if overflow_cols.contains(&"tags") && !base.tags.is_empty() {
        overflow.push((t("torrents.tags"), base.tags.join(", ")));
    }
    if overflow_cols.contains(&"path") && !base.path.is_empty() {
        overflow.push((t("torrents.save_path"), base.path.clone()));
    }
    let select = {
        let on_select = on_select.clone();
        let id = base.id;
        Callback::from(move |_| on_select.emit(id))
    };
    let pause = {
        let on_action = on_action.clone();
        let id = base.id;
        Callback::from(move |_| on_action.emit((TorrentAction::Pause, id)))
    };
    let resume = {
        let on_action = on_action.clone();
        let id = base.id;
        Callback::from(move |_| on_action.emit((TorrentAction::Resume, id)))
    };
    let delete_data = {
        let on_action = on_action.clone();
        let id = base.id;
        Callback::from(move |_| on_action.emit((TorrentAction::Delete { with_data: true }, id)))
    };
    let fsops_label = fsops_label(&bundle, fsops);
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
                            let id = base.id;
                            Callback::from(move |_| on_toggle.emit(id))
                        }}
                    />
                </div>
                <div>
                    <strong>{base.name.clone()}</strong>
                    <p class="muted ellipsis">{base.tracker.clone()}</p>
                </div>
                <div class="status-stack">
                    <span class={classes!("pill", status_class(&progress.status))}>{progress.status.clone()}</span>
                    {if let Some(label) = fsops_label {
                        html! {
                            <span class={classes!("pill", fsops_class(fsops), "subtle")} title={fsops.and_then(|badge| badge.detail.clone()).unwrap_or_default()}>
                                {label}
                            </span>
                        }
                    } else { html!{} }}
                </div>
            </header>
            <div class="progress">
                <div class="bar" style={format!("width: {:.1}%", progress.progress * 100.0)}></div>
                <div class="meta">
                    <span class="muted">{format!("{:.1}%", progress.progress * 100.0)}</span>
                    {if show_eta {
                        html! { <span class="muted">{progress.eta.clone().unwrap_or_else(|| t("torrents.eta_infinite"))}</span> }
                    } else { html!{} }}
                </div>
            </div>
            <div class="mobile-stats">
                <div><small>{t("torrents.down")}</small><strong>{format_rate(progress.download_bps)}</strong></div>
                <div><small>{t("torrents.up")}</small><strong>{format_rate(progress.upload_bps)}</strong></div>
                {if show_ratio { html! { <div><small>{t("torrents.ratio")}</small><strong>{format!("{:.2}", base.ratio)}</strong></div> }} else { html!{} }}
                {if show_size { html! { <div><small>{t("torrents.size")}</small><strong>{base.size_label()}</strong></div> }} else { html!{} }}
            </div>
            <div class="row-meta">
                {if show_path { html! { <span class="muted ellipsis">{base.path.clone()}</span> }} else { html!{} }}
                <div class="tags">
                    <span class="pill subtle">{base.category.clone()}</span>
                    {if show_tags { html! {for base.tags.iter().map(|tag| html! { <span class="pill subtle">{tag.to_owned()}</span> }) }} else { html!{} }}
                </div>
            </div>
            {if overflow.is_empty() { html!{} } else {
                html! {
                    <div class="row-overflow">
                        {for overflow.iter().map(|(label, value)| html!{
                            <span class="pill subtle">{format!("{label}: {value}")}</span>
                        })}
                    </div>
                }
            }}
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

fn fsops_class(fsops: Option<&FsopsBadge>) -> &'static str {
    match fsops.map(|badge| &badge.status) {
        Some(FsopsStatus::InProgress) => "warn",
        Some(FsopsStatus::Completed) => "ok",
        Some(FsopsStatus::Failed) => "error",
        None => "muted",
    }
}

fn fsops_label(bundle: &TranslationBundle, fsops: Option<&FsopsBadge>) -> Option<String> {
    let label = match fsops.map(|badge| &badge.status) {
        Some(FsopsStatus::InProgress) => bundle.text("torrents.fsops_in_progress", "FSOps"),
        Some(FsopsStatus::Completed) => bundle.text("torrents.fsops_done", "FSOps done"),
        Some(FsopsStatus::Failed) => bundle.text("torrents.fsops_failed", "FSOps failed"),
        None => return None,
    };
    Some(label)
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
        assert_eq!(crate::core::logic::format_rate(512), "512 B/s");
        assert_eq!(crate::core::logic::format_rate(2048), "2.0 KiB/s");
        assert!(crate::core::logic::format_rate(5_242_880).contains("MiB"));
        assert!(crate::core::logic::format_rate(2_147_483_648).contains("GiB"));
    }

    #[test]
    fn action_banner_uses_locale_strings() {
        let bundle = TranslationBundle::new(DEFAULT_LOCALE);
        let msg = action_banner_message(&bundle, &TorrentAction::Pause, "alpha");
        assert!(msg.contains(&bundle.text("torrents.banner.pause", "")));
    }
}

#[derive(Properties, PartialEq)]
pub(crate) struct AddTorrentProps {
    pub on_submit: Callback<AddTorrentInput>,
    pub pending: bool,
}

#[function_component(AddTorrentPanel)]
fn add_torrent_panel(props: &AddTorrentProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
    let bundle_for_submit = bundle.clone();
    let bundle_for_drop = bundle.clone();
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
        let bundle = bundle_for_submit.clone();
        Callback::from(move |_| {
            let value = input_value.trim().to_string();
            let has_file = (*file).is_some();
            let payload = match crate::core::logic::build_add_payload(
                &value, &category, &tags, &save_path, has_file,
            ) {
                Ok(payload) => {
                    error.set(None);
                    payload
                }
                Err(crate::core::logic::AddInputError::Empty) => {
                    error.set(Some(bundle.text("torrents.error.empty", "")));
                    return;
                }
                Err(crate::core::logic::AddInputError::Invalid) => {
                    error.set(Some(bundle.text("torrents.error.invalid", "")));
                    return;
                }
            };
            on_submit.emit(AddTorrentInput {
                value: payload.value,
                file: (*file).clone(),
                category: payload.category,
                tags: payload.tags,
                save_path: payload.save_path,
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
        let bundle = bundle_for_drop.clone();
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
                    error.set(Some(bundle.text("torrents.error.file_type", "")));
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
                <div class="label-shortcuts">
                    <span class="muted">{bundle.text("torrents.manage_labels", "Manage labels")}</span>
                    <div class="chip-group">
                        <Link<Route> to={Route::Categories} classes="chip ghost">
                            {bundle.text("torrents.manage_categories", "Categories")}
                        </Link<Route>>
                        <Link<Route> to={Route::Tags} classes="chip ghost">
                            {bundle.text("torrents.manage_tags", "Tags")}
                        </Link<Route>>
                    </div>
                </div>
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
        let id = props.selected;
        Callback::from(move |_| {
            if let Some(id) = id {
                on_action.emit((TorrentAction::Pause, id));
            }
        })
    };
    let resume = {
        let on_action = props.on_action.clone();
        let id = props.selected;
        Callback::from(move |_| {
            if let Some(id) = id {
                on_action.emit((TorrentAction::Resume, id));
            }
        })
    };
    let delete = {
        let on_action = props.on_action.clone();
        let id = props.selected;
        Callback::from(move |_| {
            if let Some(id) = id {
                on_action.emit((TorrentAction::Delete { with_data: false }, id));
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

#[derive(Properties, PartialEq)]
pub(crate) struct ConfirmProps {
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
                    <button class="ghost" onclick={{
                        let cb = props.on_close.clone();
                        Callback::from(move |_| cb.emit(()))
                    }}>{t("confirm.cancel")}</button>
                    <button class="solid danger" onclick={confirm}>{action}</button>
                </div>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub(crate) struct BannerProps {
    pub message: Option<String>,
}

#[derive(Properties, PartialEq)]
pub(crate) struct MobileActionProps {
    pub on_action: Callback<(TorrentAction, Uuid)>,
    pub selected: Option<Uuid>,
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
pub(crate) fn demo_rows() -> Vec<TorrentRow> {
    const GIB: u64 = 1_073_741_824;
    vec![
        TorrentRow {
            id: Uuid::from_u128(1),
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
            id: Uuid::from_u128(2),
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
            id: Uuid::from_u128(3),
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
            id: Uuid::from_u128(4),
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
            id: Uuid::from_u128(5),
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

// Intentionally rely on `crate::features::torrents::state::TorrentRow` conversion to avoid duplication.
