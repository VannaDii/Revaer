use crate::app::Route;
use crate::components::action_menu::{ActionMenuItem, render_action_menu};
use crate::components::atoms::{BulkActionBar, SearchInput};
use crate::components::daisy::{Input, MultiSelect, Select};
use crate::components::detail::{DetailView, FileSelectionChange};
use crate::components::torrent_modals::{AddTorrentPanel, CopyKind, CreateTorrentPanel};
use crate::components::virtual_list::VirtualList;
use crate::core::breakpoints::Breakpoint;
use crate::core::logic::{
    ShortcutOutcome, format_rate, interpret_shortcut, parse_rate_input, parse_tags, plan_columns,
    select_all_or_clear, toggle_selection,
};
use crate::core::store::AppStore;
use crate::features::torrents::actions::TorrentAction;
use crate::features::torrents::state::{
    FsopsBadge, FsopsStatus, SelectionSet, TorrentProgressSlice, TorrentRow, TorrentRowBase,
    select_fsops_badge, select_is_selected, select_torrent_progress_slice, select_torrent_row_base,
};
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::{
    AddTorrentInput, ConfirmKind, TorrentAuthorRequest, TorrentAuthorResponse, TorrentDetail,
    TorrentOptionsRequest,
};
use crate::{Density, UiMode};
use std::rc::Rc;
use uuid::Uuid;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{HtmlElement, KeyboardEvent, MouseEvent};
use yew::prelude::*;
use yew_router::prelude::use_navigator;
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
    /// Latest create-torrent result (if any).
    pub create_result: Option<TorrentAuthorResponse>,
    /// Latest create-torrent error message.
    pub create_error: Option<String>,
    /// True when a create-torrent request is in flight.
    pub create_busy: bool,
    /// Create a new torrent file via authoring.
    pub on_create: Callback<TorrentAuthorRequest>,
    /// Reset create-torrent result/error state.
    pub on_reset_create: Callback<()>,
    /// Copy payload from the create-torrent result panel.
    pub on_copy_payload: Callback<(CopyKind, String)>,
    pub search: String,
    pub on_search: Callback<String>,
    /// Current state filter value.
    pub state_filter: String,
    /// Current tags filter selection.
    pub tags_filter: Vec<String>,
    /// Available tag options for the tags filter.
    pub tag_options: Vec<(AttrValue, AttrValue)>,
    /// Current tracker filter value.
    pub tracker_filter: String,
    /// Current extension filter value.
    pub extension_filter: String,
    /// Update the state filter.
    pub on_state_filter: Callback<String>,
    /// Update the tags filter.
    pub on_tags_filter: Callback<Vec<String>>,
    /// Update the tracker filter.
    pub on_tracker_filter: Callback<String>,
    /// Update the extension filter.
    pub on_extension_filter: Callback<String>,
    /// Whether another page is available.
    pub can_load_more: bool,
    /// Whether the list is loading (refresh or pagination).
    pub is_loading: bool,
    /// Request the next page of torrents.
    pub on_load_more: Callback<()>,
    pub selected_id: Option<Uuid>,
    pub selected_ids: SelectionSet,
    pub on_set_selected: Callback<SelectionSet>,
    /// Selected detail payload for the drawer.
    pub selected_detail: Option<TorrentDetail>,
    /// Request a detail refresh for a torrent id.
    pub on_select_detail: Callback<Uuid>,
    /// Request a file selection update for a torrent.
    pub on_update_selection: Callback<(Uuid, FileSelectionChange)>,
    /// Request a torrent options update for a torrent.
    pub on_update_options: Callback<(Uuid, TorrentOptionsRequest)>,
}

#[derive(Clone, PartialEq)]
struct ActionTarget {
    ids: Vec<Uuid>,
    label: String,
}

impl ActionTarget {
    fn single(id: Uuid, label: String) -> Self {
        Self {
            ids: vec![id],
            label,
        }
    }

    fn bulk(ids: Vec<Uuid>, label: String) -> Self {
        Self { ids, label }
    }
}

#[derive(Clone, PartialEq)]
struct RateValues {
    download_bps: Option<u64>,
    upload_bps: Option<u64>,
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
    let remove_target = use_state(|| None as Option<ActionTarget>);
    let rate_target = use_state(|| None as Option<ActionTarget>);
    let show_add_modal = use_state(|| false);
    let show_create_modal = use_state(|| false);
    let fab_open = use_state(|| false);
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
    let tag_values: Vec<AttrValue> = props
        .tags_filter
        .iter()
        .cloned()
        .map(AttrValue::from)
        .collect();
    let tag_input_value = if props.tags_filter.is_empty() {
        String::new()
    } else {
        props.tags_filter.join(", ")
    };
    let state_options = vec![
        (
            AttrValue::from(""),
            AttrValue::from(bundle.text("torrents.state_all", "All states")),
        ),
        (
            AttrValue::from("queued"),
            AttrValue::from(bundle.text("torrents.state_queued", "Queued")),
        ),
        (
            AttrValue::from("fetching_metadata"),
            AttrValue::from(bundle.text("torrents.state_fetching", "Metadata")),
        ),
        (
            AttrValue::from("downloading"),
            AttrValue::from(bundle.text("torrents.state_downloading", "Downloading")),
        ),
        (
            AttrValue::from("seeding"),
            AttrValue::from(bundle.text("torrents.state_seeding", "Seeding")),
        ),
        (
            AttrValue::from("completed"),
            AttrValue::from(bundle.text("torrents.state_completed", "Completed")),
        ),
        (
            AttrValue::from("stopped"),
            AttrValue::from(bundle.text("torrents.state_stopped", "Stopped")),
        ),
        (
            AttrValue::from("failed"),
            AttrValue::from(bundle.text("torrents.state_failed", "Failed")),
        ),
    ];
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
        let remove_target = remove_target.clone();
        let selected_name = selected_name.clone();
        Callback::from(move |_| {
            if let Some(id) = selected_id {
                remove_target.set(Some(ActionTarget::single(id, selected_name.clone())));
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

    let on_prompt_remove = {
        let remove_target = remove_target.clone();
        Callback::from(move |target: ActionTarget| {
            remove_target.set(Some(target));
        })
    };

    let on_prompt_rate = {
        let rate_target = rate_target.clone();
        Callback::from(move |target: ActionTarget| {
            rate_target.set(Some(target));
        })
    };
    let on_prompt_rate_detail = {
        let on_prompt_rate = on_prompt_rate.clone();
        Callback::from(move |(id, label): (Uuid, String)| {
            on_prompt_rate.emit(ActionTarget::single(id, label));
        })
    };
    let on_prompt_remove_detail = {
        let on_prompt_remove = on_prompt_remove.clone();
        Callback::from(move |(id, label): (Uuid, String)| {
            on_prompt_remove.emit(ActionTarget::single(id, label));
        })
    };
    let close_add_modal = {
        let show_add_modal = show_add_modal.clone();
        Callback::from(move |_| show_add_modal.set(false))
    };
    let close_create_modal = {
        let show_create_modal = show_create_modal.clone();
        let on_reset_create = props.on_reset_create.clone();
        Callback::from(move |_| {
            show_create_modal.set(false);
            on_reset_create.emit(());
        })
    };
    let open_add_modal = {
        let show_add_modal = show_add_modal.clone();
        let fab_open = fab_open.clone();
        Callback::from(move |_| {
            show_add_modal.set(true);
            fab_open.set(false);
        })
    };
    let open_create_modal = {
        let show_create_modal = show_create_modal.clone();
        let fab_open = fab_open.clone();
        let on_reset_create = props.on_reset_create.clone();
        Callback::from(move |_| {
            on_reset_create.emit(());
            show_create_modal.set(true);
            fab_open.set(false);
        })
    };
    let toggle_fab = {
        let fab_open = fab_open.clone();
        Callback::from(move |_| fab_open.set(!*fab_open))
    };
    let stop_propagation = Callback::from(|event: MouseEvent| event.stop_propagation());
    let submit_add = {
        let on_add = props.on_add.clone();
        let show_add_modal = show_add_modal.clone();
        Callback::from(move |input: AddTorrentInput| {
            on_add.emit(input);
            show_add_modal.set(false);
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

                let window = web_sys::window();
                let attached = if let Some(window_ref) = window.as_ref() {
                    window_ref
                        .add_event_listener_with_callback(
                            "keydown",
                            handler.as_ref().unchecked_ref(),
                        )
                        .is_ok()
                } else {
                    false
                };

                move || {
                    if attached {
                        if let Some(window_ref) = window {
                            let _ = window_ref.remove_event_listener_with_callback(
                                "keydown",
                                handler.as_ref().unchecked_ref(),
                            );
                        }
                    }
                }
            },
            (),
        );
    }

    html! {
        <section class={classes!("torrents-view", density_class, mode_class)}>
            <header class="toolbar">
                <div class="search">
                    <SearchInput
                        aria_label={Some(AttrValue::from(t("torrents.search_label")))}
                        placeholder={Some(AttrValue::from(t("torrents.search_placeholder")))}
                        input_ref={search_ref.clone()}
                        value={AttrValue::from(props.search.clone())}
                        debounce_ms={250}
                        on_search={props.on_search.clone()}
                    />
                </div>
                <div class="filters">
                    <label class="filter">
                        <span>{bundle.text("torrents.filter_state", "State")}</span>
                        <Select
                            aria_label={Some(AttrValue::from(bundle.text("torrents.state_label", "Torrent state")))}
                            value={Some(AttrValue::from(props.state_filter.clone()))}
                            options={state_options.clone()}
                            onchange={{
                                let on_state = props.on_state_filter.clone();
                                Callback::from(move |value: AttrValue| on_state.emit(value.to_string()))
                            }}
                        />
                    </label>
                    <label class="filter">
                        <span>{bundle.text("torrents.filter_tags", "Tags")}</span>
                        {if props.tag_options.is_empty() {
                            html! {
                                <Input
                                    aria_label={Some(AttrValue::from(bundle.text("torrents.tags_label", "Tags")))}
                                    placeholder={Some(AttrValue::from(bundle.text("torrents.tags_placeholder", "tag1, tag2")))}
                                    value={AttrValue::from(tag_input_value.clone())}
                                    oninput={{
                                        let on_tags = props.on_tags_filter.clone();
                                        Callback::from(move |value: String| {
                                            on_tags.emit(parse_tags(&value).unwrap_or_default());
                                        })
                                    }}
                                />
                            }
                        } else {
                            html! {
                                <MultiSelect
                                    aria_label={Some(AttrValue::from(bundle.text("torrents.tags_label", "Tags")))}
                                    options={props.tag_options.clone()}
                                    values={tag_values.clone()}
                                    onchange={{
                                        let on_tags = props.on_tags_filter.clone();
                                        Callback::from(move |values: Vec<AttrValue>| {
                                            on_tags.emit(values.into_iter().map(|value| value.to_string()).collect());
                                        })
                                    }}
                                />
                            }
                        }}
                    </label>
                    <label class="filter">
                        <span>{bundle.text("torrents.filter_tracker", "Tracker")}</span>
                        <Input
                            aria_label={Some(AttrValue::from(bundle.text("torrents.tracker_label", "Tracker")))}
                            placeholder={Some(AttrValue::from(bundle.text("torrents.tracker_placeholder", "tracker url")))}
                            value={AttrValue::from(props.tracker_filter.clone())}
                            oninput={{
                                let on_tracker = props.on_tracker_filter.clone();
                                Callback::from(move |value: String| on_tracker.emit(value))
                            }}
                        />
                    </label>
                    <label class="filter">
                        <span>{bundle.text("torrents.filter_extension", "Extension")}</span>
                        <Input
                            aria_label={Some(AttrValue::from(bundle.text("torrents.extension_label", "Extension")))}
                            placeholder={Some(AttrValue::from(bundle.text("torrents.extension_placeholder", ".mkv")))}
                            value={AttrValue::from(props.extension_filter.clone())}
                            oninput={{
                                let on_extension = props.on_extension_filter.clone();
                                Callback::from(move |value: String| on_extension.emit(value))
                            }}
                        />
                    </label>
                </div>
                <BulkActionBar
                    select_label={AttrValue::from(t("torrents.select_all"))}
                    selected_label={AttrValue::from(t("torrents.selected"))}
                    selected_count={selected_count}
                    on_toggle_all={{
                        let selected_ids = selected_ids.clone();
                        let on_set_selected = props.on_set_selected.clone();
                        let visible_ids = props.visible_ids.clone();
                        Callback::from(move |_| {
                            on_set_selected.emit(select_all_or_clear(&selected_ids, &visible_ids));
                        })
                    }}
                >
                        <button
                            class="ghost"
                            disabled={selected_count == 0}
                            onclick={{
                                let on_bulk = props.on_bulk_action.clone();
                                let ids = selected_ids.clone();
                                Callback::from(move |_| {
                                    if !ids.is_empty() {
                                        on_bulk.emit((TorrentAction::Pause, ids.iter().cloned().collect()));
                                    }
                                })
                            }}
                        >
                            {t("toolbar.pause")}
                        </button>
                        <button
                            class="ghost"
                            disabled={selected_count == 0}
                            onclick={{
                                let on_bulk = props.on_bulk_action.clone();
                                let ids = selected_ids.clone();
                                Callback::from(move |_| {
                                    if !ids.is_empty() {
                                        on_bulk.emit((TorrentAction::Resume, ids.iter().cloned().collect()));
                                    }
                                })
                            }}
                        >
                            {t("toolbar.resume")}
                        </button>
                        <button
                            class="ghost"
                            disabled={selected_count == 0}
                            onclick={{
                                let on_bulk = props.on_bulk_action.clone();
                                let ids = selected_ids.clone();
                                Callback::from(move |_| {
                                    if !ids.is_empty() {
                                        on_bulk.emit((TorrentAction::Reannounce, ids.iter().cloned().collect()));
                                    }
                                })
                            }}
                        >
                            {bundle.text("toolbar.reannounce", "Reannounce")}
                        </button>
                        <button
                            class="ghost"
                            disabled={selected_count == 0}
                            onclick={{
                                let on_bulk = props.on_bulk_action.clone();
                                let ids = selected_ids.clone();
                                Callback::from(move |_| {
                                    if !ids.is_empty() {
                                        on_bulk.emit((TorrentAction::Recheck, ids.iter().cloned().collect()));
                                    }
                                })
                            }}
                        >
                            {t("toolbar.recheck")}
                        </button>
                        <button
                            class="ghost"
                            disabled={selected_count == 0}
                            onclick={{
                                let on_bulk = props.on_bulk_action.clone();
                                let ids = selected_ids.clone();
                                Callback::from(move |_| {
                                    if !ids.is_empty() {
                                        on_bulk.emit((
                                            TorrentAction::Sequential { enable: true },
                                            ids.iter().cloned().collect(),
                                        ));
                                    }
                                })
                            }}
                        >
                            {bundle.text("toolbar.sequential_on", "Sequential on")}
                        </button>
                        <button
                            class="ghost"
                            disabled={selected_count == 0}
                            onclick={{
                                let on_bulk = props.on_bulk_action.clone();
                                let ids = selected_ids.clone();
                                Callback::from(move |_| {
                                    if !ids.is_empty() {
                                        on_bulk.emit((
                                            TorrentAction::Sequential { enable: false },
                                            ids.iter().cloned().collect(),
                                        ));
                                    }
                                })
                            }}
                        >
                            {bundle.text("toolbar.sequential_off", "Sequential off")}
                        </button>
                        <button
                            class="ghost"
                            disabled={selected_count == 0}
                            onclick={{
                                let rate_target = rate_target.clone();
                                let ids = selected_ids.clone();
                                let label = format!(
                                    "{} {}",
                                    ids.len(),
                                    bundle.text("torrents.selected", "selected")
                                );
                                Callback::from(move |_| {
                                    if !ids.is_empty() {
                                        rate_target.set(Some(ActionTarget::bulk(
                                            ids.iter().cloned().collect(),
                                            label.clone(),
                                        )));
                                    }
                                })
                            }}
                        >
                            {bundle.text("toolbar.rate", "Set rate")}
                        </button>
                        <button
                            class="ghost danger"
                            disabled={selected_count == 0}
                            onclick={{
                                let remove_target = remove_target.clone();
                                let ids = selected_ids.clone();
                                let label = format!(
                                    "{} {}",
                                    ids.len(),
                                    bundle.text("torrents.selected", "selected")
                                );
                                Callback::from(move |_| {
                                    if !ids.is_empty() {
                                        remove_target.set(Some(ActionTarget::bulk(
                                            ids.iter().cloned().collect(),
                                            label.clone(),
                                        )));
                                    }
                                })
                            }}
                        >
                            {t("toolbar.delete")}
                        </button>
                </BulkActionBar>
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
                    <button class="solid" onclick={open_add_modal.clone()}>{t("toolbar.add")}</button>
                </div>
            </header>

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
                    let on_prompt_remove = on_prompt_remove.clone();
                    let on_prompt_rate = on_prompt_rate.clone();
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
                                    on_prompt_remove={on_prompt_remove.clone()}
                                    on_prompt_rate={on_prompt_rate.clone()}
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
            {if props.can_load_more {
                let label = if props.is_loading {
                    bundle.text("torrents.loading_more", "Loading...")
                } else {
                    bundle.text("torrents.load_more", "Load more")
                };
                let on_load_more = props.on_load_more.clone();
                html! {
                    <div class="load-more">
                        <button class="ghost" onclick={Callback::from(move |_| on_load_more.emit(()))} disabled={props.is_loading}>
                            {label}
                        </button>
                    </div>
                }
            } else {
                html! {}
            }}

            <DetailView
                data={props.selected_detail.clone()}
                on_action={props.on_action.clone()}
                on_prompt_rate={on_prompt_rate_detail}
                on_prompt_remove={on_prompt_remove_detail}
                on_update_selection={props.on_update_selection.clone()}
                on_update_options={props.on_update_options.clone()}
            />
            <MobileActionRow
                on_action={props.on_action.clone()}
                on_prompt_remove={on_prompt_remove.clone()}
                on_prompt_rate={on_prompt_rate.clone()}
                selected={props.visible_ids.get(*selected_idx).copied()}
                selected_label={selected_name.clone()}
            />
            {if *show_add_modal {
                html! {
                    <div class="modal-overlay" onclick={close_add_modal.clone()}>
                        <div class="card modal-card" onclick={stop_propagation.clone()}>
                            <div class="modal-header">
                                <div>
                                    <h3>{t("torrents.add_title")}</h3>
                                    <p class="muted">{t("torrents.add_subtitle")}</p>
                                </div>
                                <button class="ghost" onclick={close_add_modal.clone()} aria-label={t("torrents.close_modal")}>{"✕"}</button>
                            </div>
                            <AddTorrentPanel on_submit={submit_add.clone()} pending={props.add_busy} />
                        </div>
                    </div>
                }
            } else { html! {} }}
            {if *show_create_modal {
                html! {
                    <div class="modal-overlay" onclick={close_create_modal.clone()}>
                        <div class={classes!("card", "modal-card", "modal-card-wide")} onclick={stop_propagation.clone()}>
                            <div class="modal-header">
                                <div>
                                    <h3>{t("torrents.create_title")}</h3>
                                    <p class="muted">{t("torrents.create_subtitle")}</p>
                                </div>
                                <button class="ghost" onclick={close_create_modal.clone()} aria-label={t("torrents.close_modal")}>{"✕"}</button>
                            </div>
                            <CreateTorrentPanel
                                on_submit={props.on_create.clone()}
                                on_copy={props.on_copy_payload.clone()}
                                pending={props.create_busy}
                                result={props.create_result.clone()}
                                error={props.create_error.clone()}
                            />
                        </div>
                    </div>
                }
            } else { html! {} }}
            <div class={classes!("fab-dial", if *fab_open { "open" } else { "" })}>
                <div class="fab-actions">
                    <button class="fab-action" onclick={open_add_modal.clone()}>
                        {t("torrents.fab_add")}
                    </button>
                    <button class="fab-action" onclick={open_create_modal.clone()}>
                        {t("torrents.fab_create")}
                    </button>
                </div>
                <button class="fab-button" onclick={toggle_fab} aria-label={t("torrents.fab_toggle")}>
                    <span class="fab-icon">{if *fab_open { "✕" } else { "+" }}</span>
                </button>
            </div>
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
            <RemoveDialog
                target={(*remove_target).clone()}
                on_close={{
                    let remove_target = remove_target.clone();
                    Callback::from(move |_| remove_target.set(None))
                }}
                on_confirm={{
                    let remove_target = remove_target.clone();
                    let on_action = props.on_action.clone();
                    let on_bulk_action = props.on_bulk_action.clone();
                    Callback::from(move |delete_data: bool| {
                        let target = (*remove_target).clone();
                        remove_target.set(None);
                        let Some(target) = target else {
                            return;
                        };
                        let action = TorrentAction::Delete {
                            with_data: delete_data,
                        };
                        if target.ids.len() == 1 {
                            if let Some(id) = target.ids.first().copied() {
                                on_action.emit((action, id));
                            }
                        } else {
                            on_bulk_action.emit((action, target.ids));
                        }
                    })
                }}
            />
            <RateDialog
                target={(*rate_target).clone()}
                on_close={{
                    let rate_target = rate_target.clone();
                    Callback::from(move |_| rate_target.set(None))
                }}
                on_confirm={{
                    let rate_target = rate_target.clone();
                    let on_action = props.on_action.clone();
                    let on_bulk_action = props.on_bulk_action.clone();
                    Callback::from(move |values: RateValues| {
                        let target = (*rate_target).clone();
                        rate_target.set(None);
                        let Some(target) = target else {
                            return;
                        };
                        let action = TorrentAction::Rate {
                            download_bps: values.download_bps,
                            upload_bps: values.upload_bps,
                        };
                        if target.ids.len() == 1 {
                            if let Some(id) = target.ids.first().copied() {
                                on_action.emit((action, id));
                            }
                        } else {
                            on_bulk_action.emit((action, target.ids));
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
    on_prompt_remove: Callback<ActionTarget>,
    on_prompt_rate: Callback<ActionTarget>,
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
            props.on_prompt_remove.clone(),
            props.on_prompt_rate.clone(),
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
            props.on_prompt_remove.clone(),
            props.on_prompt_rate.clone(),
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
    on_prompt_remove: Callback<ActionTarget>,
    on_prompt_rate: Callback<ActionTarget>,
    bundle: TranslationBundle,
    visible_cols: &[&str],
    overflow_cols: &[&str],
) -> Html {
    let t = |key: &str| bundle.text(key, "");
    let show_eta = visible_cols.contains(&"eta");
    let show_ratio = visible_cols.contains(&"ratio");
    let show_size = visible_cols.contains(&"size");
    let show_updated = visible_cols.contains(&"updated");
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
    if overflow_cols.contains(&"updated") {
        overflow.push((
            bundle.text("torrents.updated", "Updated"),
            base.updated.clone(),
        ));
    }
    if overflow_cols.contains(&"tags") && !base.tags.is_empty() {
        overflow.push((t("torrents.tags"), base.tags.join(", ")));
    }
    if overflow_cols.contains(&"path") && !base.path.is_empty() {
        overflow.push((t("torrents.save_path"), base.path.clone()));
    }
    let row_click = {
        let on_select = on_select.clone();
        let id = base.id;
        Callback::from(move |event: MouseEvent| {
            if is_interactive_target(&event) {
                return;
            }
            on_select.emit(id);
        })
    };
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
    let reannounce = {
        let on_action = on_action.clone();
        let id = base.id;
        Callback::from(move |_| on_action.emit((TorrentAction::Reannounce, id)))
    };
    let recheck = {
        let on_action = on_action.clone();
        let id = base.id;
        Callback::from(move |_| on_action.emit((TorrentAction::Recheck, id)))
    };
    let sequential_on = {
        let on_action = on_action.clone();
        let id = base.id;
        Callback::from(move |_| on_action.emit((TorrentAction::Sequential { enable: true }, id)))
    };
    let sequential_off = {
        let on_action = on_action.clone();
        let id = base.id;
        Callback::from(move |_| on_action.emit((TorrentAction::Sequential { enable: false }, id)))
    };
    let prompt_rate = {
        let on_prompt_rate = on_prompt_rate.clone();
        let id = base.id;
        let label = base.name.clone();
        Callback::from(move |_| on_prompt_rate.emit(ActionTarget::single(id, label.clone())))
    };
    let prompt_remove = {
        let on_prompt_remove = on_prompt_remove.clone();
        let id = base.id;
        let label = base.name.clone();
        Callback::from(move |_| on_prompt_remove.emit(ActionTarget::single(id, label.clone())))
    };
    let action_menu = render_action_menu(
        &bundle,
        vec![
            ActionMenuItem::new(bundle.text("toolbar.reannounce", "Reannounce"), reannounce),
            ActionMenuItem::new(bundle.text("toolbar.recheck", "Recheck"), recheck),
            ActionMenuItem::new(
                bundle.text("toolbar.sequential_on", "Sequential on"),
                sequential_on,
            ),
            ActionMenuItem::new(
                bundle.text("toolbar.sequential_off", "Sequential off"),
                sequential_off,
            ),
            ActionMenuItem::new(bundle.text("toolbar.rate", "Set rate"), prompt_rate),
            ActionMenuItem::danger(bundle.text("toolbar.delete", "Remove"), prompt_remove),
        ],
    );
    let fsops_label = fsops_label(&bundle, fsops);
    html! {
        <article
            class={classes!("torrent-row", if selected { Some("selected") } else { None })}
            aria-selected={selected.to_string()}
            onclick={row_click}
        >
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
                {if show_updated { html! {
                    <div class="stat">
                        <small>{bundle.text("torrents.updated", "Updated")}</small>
                        <strong>{base.updated.clone()}</strong>
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
                {action_menu}
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
    on_prompt_remove: Callback<ActionTarget>,
    on_prompt_rate: Callback<ActionTarget>,
    bundle: TranslationBundle,
    visible_cols: &[&str],
    overflow_cols: &[&str],
) -> Html {
    let t = |key: &str| bundle.text(key, "");
    let show_eta = visible_cols.contains(&"eta");
    let show_ratio = visible_cols.contains(&"ratio");
    let show_size = visible_cols.contains(&"size");
    let show_updated = visible_cols.contains(&"updated");
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
    if overflow_cols.contains(&"updated") {
        overflow.push((
            bundle.text("torrents.updated", "Updated"),
            base.updated.clone(),
        ));
    }
    if overflow_cols.contains(&"tags") && !base.tags.is_empty() {
        overflow.push((t("torrents.tags"), base.tags.join(", ")));
    }
    if overflow_cols.contains(&"path") && !base.path.is_empty() {
        overflow.push((t("torrents.save_path"), base.path.clone()));
    }
    let row_click = {
        let on_select = on_select.clone();
        let id = base.id;
        Callback::from(move |event: MouseEvent| {
            if is_interactive_target(&event) {
                return;
            }
            on_select.emit(id);
        })
    };
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
    let reannounce = {
        let on_action = on_action.clone();
        let id = base.id;
        Callback::from(move |_| on_action.emit((TorrentAction::Reannounce, id)))
    };
    let recheck = {
        let on_action = on_action.clone();
        let id = base.id;
        Callback::from(move |_| on_action.emit((TorrentAction::Recheck, id)))
    };
    let sequential_on = {
        let on_action = on_action.clone();
        let id = base.id;
        Callback::from(move |_| on_action.emit((TorrentAction::Sequential { enable: true }, id)))
    };
    let sequential_off = {
        let on_action = on_action.clone();
        let id = base.id;
        Callback::from(move |_| on_action.emit((TorrentAction::Sequential { enable: false }, id)))
    };
    let prompt_rate = {
        let on_prompt_rate = on_prompt_rate.clone();
        let id = base.id;
        let label = base.name.clone();
        Callback::from(move |_| on_prompt_rate.emit(ActionTarget::single(id, label.clone())))
    };
    let prompt_remove = {
        let on_prompt_remove = on_prompt_remove.clone();
        let id = base.id;
        let label = base.name.clone();
        Callback::from(move |_| on_prompt_remove.emit(ActionTarget::single(id, label.clone())))
    };
    let action_menu = render_action_menu(
        &bundle,
        vec![
            ActionMenuItem::new(bundle.text("toolbar.reannounce", "Reannounce"), reannounce),
            ActionMenuItem::new(bundle.text("toolbar.recheck", "Recheck"), recheck),
            ActionMenuItem::new(
                bundle.text("toolbar.sequential_on", "Sequential on"),
                sequential_on,
            ),
            ActionMenuItem::new(
                bundle.text("toolbar.sequential_off", "Sequential off"),
                sequential_off,
            ),
            ActionMenuItem::new(bundle.text("toolbar.rate", "Set rate"), prompt_rate),
            ActionMenuItem::danger(bundle.text("toolbar.delete", "Remove"), prompt_remove),
        ],
    );
    let fsops_label = fsops_label(&bundle, fsops);
    html! {
        <article
            class={classes!("torrent-row", "mobile", if selected { Some("selected") } else { None })}
            aria-selected={selected.to_string()}
            onclick={row_click}
        >
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
                {if show_updated { html! { <div><small>{bundle.text("torrents.updated", "Updated")}</small><strong>{base.updated.clone()}</strong></div> }} else { html!{} }}
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
                {action_menu}
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

fn is_interactive_target(event: &MouseEvent) -> bool {
    let Some(target) = event.target() else {
        return false;
    };
    let Ok(element) = target.dyn_into::<web_sys::Element>() else {
        return false;
    };
    element
        .closest("button, a, input, select, textarea, label, summary, [role=\"button\"]")
        .ok()
        .flatten()
        .is_some()
}

fn action_banner_message(bundle: &TranslationBundle, action: &TorrentAction, name: &str) -> String {
    match action {
        TorrentAction::Delete { with_data: true } => {
            format!("{} {name}", bundle.text("torrents.banner.removed_data", ""))
        }
        TorrentAction::Delete { with_data: false } => {
            format!("{} {name}", bundle.text("torrents.banner.removed", ""))
        }
        TorrentAction::Reannounce => {
            format!("{} {name}", bundle.text("torrents.banner.reannounce", ""))
        }
        TorrentAction::Recheck => {
            format!("{} {name}", bundle.text("torrents.banner.recheck", ""))
        }
        TorrentAction::Pause => format!("{} {name}", bundle.text("torrents.banner.pause", "")),
        TorrentAction::Resume => format!("{} {name}", bundle.text("torrents.banner.resume", "")),
        TorrentAction::Sequential { enable } => {
            if *enable {
                format!(
                    "{} {name}",
                    bundle.text("torrents.banner.sequential_on", "")
                )
            } else {
                format!(
                    "{} {name}",
                    bundle.text("torrents.banner.sequential_off", "")
                )
            }
        }
        TorrentAction::Rate { .. } => {
            format!("{} {name}", bundle.text("torrents.banner.rate", ""))
        }
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

#[function_component(MobileActionRow)]
fn mobile_action_row(props: &MobileActionProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
    let label = if props.selected_label.trim().is_empty() {
        bundle.text("toast.torrent_placeholder", "Torrent")
    } else {
        props.selected_label.clone()
    };
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
    let reannounce = {
        let on_action = props.on_action.clone();
        let id = props.selected;
        Callback::from(move |_| {
            if let Some(id) = id {
                on_action.emit((TorrentAction::Reannounce, id));
            }
        })
    };
    let recheck = {
        let on_action = props.on_action.clone();
        let id = props.selected;
        Callback::from(move |_| {
            if let Some(id) = id {
                on_action.emit((TorrentAction::Recheck, id));
            }
        })
    };
    let sequential_on = {
        let on_action = props.on_action.clone();
        let id = props.selected;
        Callback::from(move |_| {
            if let Some(id) = id {
                on_action.emit((TorrentAction::Sequential { enable: true }, id));
            }
        })
    };
    let sequential_off = {
        let on_action = props.on_action.clone();
        let id = props.selected;
        Callback::from(move |_| {
            if let Some(id) = id {
                on_action.emit((TorrentAction::Sequential { enable: false }, id));
            }
        })
    };
    let prompt_rate = {
        let on_prompt_rate = props.on_prompt_rate.clone();
        let id = props.selected;
        let label = label.clone();
        Callback::from(move |_| {
            if let Some(id) = id {
                on_prompt_rate.emit(ActionTarget::single(id, label.clone()));
            }
        })
    };
    let prompt_remove = {
        let on_prompt_remove = props.on_prompt_remove.clone();
        let id = props.selected;
        let label = label.clone();
        Callback::from(move |_| {
            if let Some(id) = id {
                on_prompt_remove.emit(ActionTarget::single(id, label.clone()));
            }
        })
    };
    let action_menu = render_action_menu(
        &bundle,
        vec![
            ActionMenuItem::new(bundle.text("toolbar.reannounce", "Reannounce"), reannounce),
            ActionMenuItem::new(bundle.text("toolbar.recheck", "Recheck"), recheck),
            ActionMenuItem::new(
                bundle.text("toolbar.sequential_on", "Sequential on"),
                sequential_on,
            ),
            ActionMenuItem::new(
                bundle.text("toolbar.sequential_off", "Sequential off"),
                sequential_off,
            ),
            ActionMenuItem::new(bundle.text("toolbar.rate", "Set rate"), prompt_rate),
            ActionMenuItem::danger(bundle.text("toolbar.delete", "Remove"), prompt_remove),
        ],
    );
    html! {
        <div class="mobile-action-row">
            <button class="ghost" onclick={pause}>{t("toolbar.pause")}</button>
            <button class="ghost" onclick={resume}>{t("toolbar.resume")}</button>
            {action_menu}
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
struct RemoveDialogProps {
    pub target: Option<ActionTarget>,
    pub on_close: Callback<()>,
    pub on_confirm: Callback<bool>,
}

#[function_component(RemoveDialog)]
fn remove_dialog(props: &RemoveDialogProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let Some(target) = props.target.clone() else {
        return html! {};
    };
    let delete_data = use_state(|| false);
    {
        let delete_data = delete_data.clone();
        let target = target.clone();
        use_effect_with_deps(
            move |_| {
                delete_data.set(false);
                || ()
            },
            target,
        );
    }
    let title = format!(
        "{} {}",
        bundle.text("confirm.remove.title", "Remove"),
        target.label
    );
    let body = bundle.text(
        "confirm.remove.body",
        "Files remain on disk unless delete data is enabled.",
    );
    let toggle_label = bundle.text("confirm.remove_toggle", "Delete data");
    let confirm_label = bundle.text("confirm.remove.cta", "Remove");
    let confirm = {
        let delete_data = delete_data.clone();
        let cb = props.on_confirm.clone();
        Callback::from(move |_| cb.emit(*delete_data))
    };
    html! {
        <div class="confirm-overlay" role="dialog" aria-modal="true">
            <div class="card">
                <header>
                    <h4>{title}</h4>
                </header>
                <p class="muted">{body}</p>
                <label class="toggle-row">
                    <input
                        type="checkbox"
                        checked={*delete_data}
                        onchange={{
                            let delete_data = delete_data.clone();
                            Callback::from(move |event: web_sys::Event| {
                                if let Some(input) =
                                    event.target_dyn_into::<web_sys::HtmlInputElement>()
                                {
                                    delete_data.set(input.checked());
                                }
                            })
                        }}
                    />
                    <span>{toggle_label}</span>
                </label>
                <div class="actions">
                    <button class="ghost" onclick={{
                        let cb = props.on_close.clone();
                        Callback::from(move |_| cb.emit(()))
                    }}>{bundle.text("confirm.cancel", "Cancel")}</button>
                    <button class="solid danger" onclick={confirm}>{confirm_label}</button>
                </div>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct RateDialogProps {
    pub target: Option<ActionTarget>,
    pub on_close: Callback<()>,
    pub on_confirm: Callback<RateValues>,
}

#[function_component(RateDialog)]
fn rate_dialog(props: &RateDialogProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let Some(target) = props.target.clone() else {
        return html! {};
    };
    let download_input = use_state(String::new);
    let upload_input = use_state(String::new);
    let error = use_state(|| None as Option<String>);
    {
        let download_input = download_input.clone();
        let upload_input = upload_input.clone();
        let error = error.clone();
        let target = target.clone();
        use_effect_with_deps(
            move |_| {
                download_input.set(String::new());
                upload_input.set(String::new());
                error.set(None);
                || ()
            },
            target,
        );
    }
    let title = format!(
        "{} {}",
        bundle.text("torrents.rate_title", "Set rate limits for"),
        target.label
    );
    let body = bundle.text(
        "torrents.rate_body",
        "Leave a field blank to keep it unchanged.",
    );
    let confirm_label = bundle.text("torrents.rate_apply", "Apply");
    let confirm = {
        let download_input = download_input.clone();
        let upload_input = upload_input.clone();
        let error = error.clone();
        let bundle = bundle.clone();
        let cb = props.on_confirm.clone();
        Callback::from(move |_| {
            let download_value = (*download_input).clone();
            let upload_value = (*upload_input).clone();
            let download = match parse_rate_input(&download_value) {
                Ok(parsed) => parsed,
                Err(_) => {
                    error.set(Some(
                        bundle.text("torrents.rate_invalid", "Enter whole numbers for rates."),
                    ));
                    return;
                }
            };
            let upload = match parse_rate_input(&upload_value) {
                Ok(parsed) => parsed,
                Err(_) => {
                    error.set(Some(
                        bundle.text("torrents.rate_invalid", "Enter whole numbers for rates."),
                    ));
                    return;
                }
            };
            if download.is_none() && upload.is_none() {
                error.set(Some(
                    bundle.text("torrents.rate_empty", "Provide at least one rate limit."),
                ));
                return;
            }
            error.set(None);
            cb.emit(RateValues {
                download_bps: download,
                upload_bps: upload,
            });
        })
    };
    html! {
        <div class="confirm-overlay" role="dialog" aria-modal="true">
            <div class="card">
                <header>
                    <h4>{title}</h4>
                </header>
                <p class="muted">{body}</p>
                <div class="form-grid">
                    <label>
                        <span>{bundle.text("torrents.rate_download", "Download cap (B/s)")}</span>
                        <input
                            value={(*download_input).clone()}
                            oninput={{
                                let download_input = download_input.clone();
                                Callback::from(move |event: InputEvent| {
                                    if let Some(input) =
                                        event.target_dyn_into::<web_sys::HtmlInputElement>()
                                    {
                                        download_input.set(input.value());
                                    }
                                })
                            }}
                        />
                    </label>
                    <label>
                        <span>{bundle.text("torrents.rate_upload", "Upload cap (B/s)")}</span>
                        <input
                            value={(*upload_input).clone()}
                            oninput={{
                                let upload_input = upload_input.clone();
                                Callback::from(move |event: InputEvent| {
                                    if let Some(input) =
                                        event.target_dyn_into::<web_sys::HtmlInputElement>()
                                    {
                                        upload_input.set(input.value());
                                    }
                                })
                            }}
                        />
                    </label>
                </div>
                {if let Some(msg) = &*error {
                    html! { <p class="error-text">{msg}</p> }
                } else { html! {} }}
                <div class="actions">
                    <button class="ghost" onclick={{
                        let cb = props.on_close.clone();
                        Callback::from(move |_| cb.emit(()))
                    }}>{bundle.text("confirm.cancel", "Cancel")}</button>
                    <button class="solid" onclick={confirm}>{confirm_label}</button>
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
struct MobileActionProps {
    pub on_action: Callback<(TorrentAction, Uuid)>,
    pub on_prompt_remove: Callback<ActionTarget>,
    pub on_prompt_rate: Callback<ActionTarget>,
    pub selected: Option<Uuid>,
    pub selected_label: String,
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
            updated: "2024-12-12 09:14 UTC".into(),
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
            updated: "2024-12-12 08:02 UTC".into(),
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
            updated: "2024-12-12 07:44 UTC".into(),
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
            updated: "2024-12-12 06:55 UTC".into(),
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
            updated: "2024-12-12 09:01 UTC".into(),
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
