use crate::app::Route;
use crate::components::action_menu::{ActionMenuItem, render_action_menu};
use crate::components::atoms::{BulkActionBar, EmptyState, SearchInput};
use crate::components::daisy::{DaisySize, Drawer, Input, Loading, Modal, MultiSelect, Select};
use crate::components::detail::{DetailView, FileSelectionChange};
use crate::components::torrent_modals::{AddTorrentPanel, CopyKind, CreateTorrentPanel};
use crate::core::logic::{
    ShortcutOutcome, format_rate, interpret_shortcut, parse_rate_input, parse_tags,
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
use gloo::console;
use gloo::events::EventListener;
use gloo::utils::window;
use uuid::Uuid;
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, KeyboardEvent, MouseEvent};
use yew::prelude::*;
use yew_router::prelude::{Link, use_navigator};
use yewdux::prelude::{Dispatch, use_selector};

#[derive(Properties, PartialEq)]
pub(crate) struct TorrentProps {
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
    /// Optional class hook for the torrent view container.
    #[prop_or_default]
    pub class: Classes,
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
    let t = move |key: &str| bundle_for_t.text(key);
    let selected_idx = use_state(|| 0usize);
    let action_banner = use_state(|| None as Option<String>);
    let confirm = use_state(|| None as Option<ConfirmKind>);
    let remove_target = use_state(|| None as Option<ActionTarget>);
    let rate_target = use_state(|| None as Option<ActionTarget>);
    let show_add_modal = use_state(|| false);
    let show_create_modal = use_state(|| false);
    let sort_state = use_state(|| None as Option<SortState>);
    let sorted_ids = use_state(|| props.visible_ids.clone());
    let search_ref = use_node_ref();
    let navigator = use_navigator();
    let dispatch = Dispatch::<AppStore>::new();
    let density_class = match props.density {
        Density::Compact => "density-compact",
        Density::Normal => "density-normal",
        Density::Comfy => "density-comfy",
    };
    let mode_class = match props.mode {
        UiMode::Simple => "mode-simple",
        UiMode::Advanced => "mode-advanced",
    };
    {
        let sorted_ids = sorted_ids.clone();
        let dispatch = dispatch.clone();
        use_effect_with_deps(
            move |(ids, sort)| {
                let store = dispatch.get();
                let next = sort_ids(ids, &store, *sort);
                sorted_ids.set(next);
                || ()
            },
            (props.visible_ids.clone(), (*sort_state).clone()),
        );
    }

    let display_ids = (*sorted_ids).clone();
    let selected_id = display_ids.get(*selected_idx).copied();
    let on_sort = on_sort_callback(sort_state.clone());
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
            AttrValue::from(bundle.text("torrents.state_all")),
        ),
        (
            AttrValue::from("queued"),
            AttrValue::from(bundle.text("torrents.state_queued")),
        ),
        (
            AttrValue::from("fetching_metadata"),
            AttrValue::from(bundle.text("torrents.state_fetching")),
        ),
        (
            AttrValue::from("downloading"),
            AttrValue::from(bundle.text("torrents.state_downloading")),
        ),
        (
            AttrValue::from("seeding"),
            AttrValue::from(bundle.text("torrents.state_seeding")),
        ),
        (
            AttrValue::from("completed"),
            AttrValue::from(bundle.text("torrents.state_completed")),
        ),
        (
            AttrValue::from("stopped"),
            AttrValue::from(bundle.text("torrents.state_stopped")),
        ),
        (
            AttrValue::from("failed"),
            AttrValue::from(bundle.text("torrents.state_failed")),
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
    let on_select = {
        let selected_idx = selected_idx.clone();
        let visible_ids = display_ids.clone();
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
        Callback::from(move |_| {
            show_add_modal.set(true);
        })
    };
    let open_create_modal = {
        let show_create_modal = show_create_modal.clone();
        let on_reset_create = props.on_reset_create.clone();
        Callback::from(move |_| {
            on_reset_create.emit(());
            show_create_modal.set(true);
        })
    };
    let submit_add = {
        let on_add = props.on_add.clone();
        let show_add_modal = show_add_modal.clone();
        Callback::from(move |input: AddTorrentInput| {
            on_add.emit(input);
            show_add_modal.set(false);
        })
    };
    let has_filters = !props.search.is_empty()
        || !props.state_filter.is_empty()
        || !props.tags_filter.is_empty()
        || !props.tracker_filter.is_empty()
        || !props.extension_filter.is_empty();
    let clear_filters = {
        let on_search = props.on_search.clone();
        let on_state = props.on_state_filter.clone();
        let on_tags = props.on_tags_filter.clone();
        let on_tracker = props.on_tracker_filter.clone();
        let on_extension = props.on_extension_filter.clone();
        Callback::from(move |_| {
            on_search.emit(String::new());
            on_state.emit(String::new());
            on_tags.emit(Vec::new());
            on_tracker.emit(String::new());
            on_extension.emit(String::new());
        })
    };
    let toggle_select = {
        let selected_ids = selected_ids.clone();
        let on_set_selected = props.on_set_selected.clone();
        Callback::from(move |id: Uuid| {
            on_set_selected.emit(toggle_selection(&selected_ids, &id));
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
            (props.selected_id.clone(), display_ids.clone()),
        );
    }

    // Keyboard shortcuts: j/k navigation, space pause/resume, delete/shift+delete confirmations, p recheck, / focus search.
    {
        let visible_ids = display_ids.clone();
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
                let listener = EventListener::new(&window(), "keydown", move |event| {
                    let Some(event) = event.dyn_ref::<KeyboardEvent>() else {
                        return;
                    };
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
                                    if let Err(err) = input.focus() {
                                        console::error!("input focus failed", err);
                                    }
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
                                    action_banner.set(Some(bundle.text("toast.pause")));
                                    on_action.emit((TorrentAction::Pause, *id));
                                }
                            }
                            ShortcutOutcome::ClearSearch => {
                                if let Some(input) = search_ref.cast::<web_sys::HtmlInputElement>()
                                {
                                    input.set_value("");
                                    if let Err(err) = input.blur() {
                                        console::error!("input blur failed", err);
                                    }
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
                });

                move || drop(listener)
            },
            (),
        );
    }

    let drawer_open = props.selected_id.is_some();
    let close_drawer = {
        let navigator = navigator.clone();
        Callback::from(move |_| {
            if let Some(navigator) = navigator.clone() {
                navigator.push(&Route::Torrents);
            }
        })
    };

    html! {
        <section class={classes!(density_class, mode_class, props.class.clone())}>
            <Drawer
                open={drawer_open}
                on_close={close_drawer.clone()}
                class="drawer-end"
                content={html! {
                    <div class="space-y-4">
                        <div class="flex items-center justify-between">
                            <p class="text-lg font-medium">{bundle.text("torrents.title")}</p>
                            <div class="breadcrumbs hidden p-0 text-sm sm:inline">
                                <ul>
                                    <li>
                                        <Link<Route> to={Route::Dashboard}>
                                            {bundle.text("nav.dashboard")}
                                        </Link<Route>>
                                    </li>
                                    <li class="opacity-80">{bundle.text("torrents.title")}</li>
                                </ul>
                            </div>
                        </div>

                        <BulkActionBar
                            class={classes!("sticky", "top-2", "z-10")}
                            select_label={AttrValue::from(t("torrents.select_all"))}
                            selected_label={AttrValue::from(t("torrents.selected"))}
                            selected_count={selected_count}
                            on_toggle_all={{
                                let selected_ids = selected_ids.clone();
                                let on_set_selected = props.on_set_selected.clone();
                                let visible_ids = display_ids.clone();
                                Callback::from(move |_| {
                                    on_set_selected.emit(select_all_or_clear(&selected_ids, &visible_ids));
                                })
                            }}
                        >
                            <button
                                class="btn btn-ghost btn-sm"
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
                                class="btn btn-ghost btn-sm"
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
                                class="btn btn-ghost btn-sm"
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
                                {bundle.text("toolbar.reannounce")}
                            </button>
                            <button
                                class="btn btn-ghost btn-sm"
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
                                class="btn btn-ghost btn-sm"
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
                                {bundle.text("toolbar.sequential_on")}
                            </button>
                            <button
                                class="btn btn-ghost btn-sm"
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
                                {bundle.text("toolbar.sequential_off")}
                            </button>
                            <button
                                class="btn btn-ghost btn-sm"
                                disabled={selected_count == 0}
                                onclick={{
                                    let rate_target = rate_target.clone();
                                    let ids = selected_ids.clone();
                                    let label = format!(
                                        "{} {}",
                                        ids.len(),
                                        bundle.text("torrents.selected")
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
                                {bundle.text("toolbar.rate")}
                            </button>
                            <button
                                class="btn btn-ghost btn-sm text-error"
                                disabled={selected_count == 0}
                                onclick={{
                                    let remove_target = remove_target.clone();
                                    let ids = selected_ids.clone();
                                    let label = format!(
                                        "{} {}",
                                        ids.len(),
                                        bundle.text("torrents.selected")
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

                        <div class="card bg-base-100 mt-5 shadow">
                            <div class="card-body p-0">
                                <div class="flex items-center justify-between px-5 pt-5">
                                    <div class="inline-flex flex-wrap items-center gap-3">
                                        <SearchInput
                                            aria_label={Some(AttrValue::from(t("torrents.search_label")))}
                                            placeholder={Some(AttrValue::from(t("torrents.search_placeholder")))}
                                            input_ref={search_ref.clone()}
                                            value={AttrValue::from(props.search.clone())}
                                            debounce_ms={250}
                                            size={DaisySize::Sm}
                                            input_class="w-24 sm:w-36"
                                            on_search={props.on_search.clone()}
                                        />
                                        <Select
                                            aria_label={Some(AttrValue::from(bundle.text("torrents.state_label")))}
                                            value={Some(AttrValue::from(props.state_filter.clone()))}
                                            options={state_options.clone()}
                                            size={DaisySize::Sm}
                                            class="w-36"
                                            onchange={{
                                                let on_state = props.on_state_filter.clone();
                                                Callback::from(move |value: AttrValue| on_state.emit(value.to_string()))
                                            }}
                                        />
                                    </div>
                                    <div class="inline-flex items-center gap-2">
                                        <div class="join hidden md:flex">
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
                                                    <button
                                                        class={classes!(
                                                            "btn",
                                                            "btn-sm",
                                                            "join-item",
                                                            if active { "btn-primary" } else { "btn-ghost" }
                                                        )}
                                                        onclick={callback}>
                                                        {label}
                                                    </button>
                                                }
                                            }).collect::<Html>()}
                                        </div>
                                        <button class="btn btn-outline btn-sm" onclick={open_create_modal.clone()}>
                                            <span class="iconify lucide--plus size-4"></span>
                                            <span class="hidden sm:inline">
                                                {bundle.text("torrents.create_title")}
                                            </span>
                                        </button>
                                        <button class="btn btn-primary btn-sm" onclick={open_add_modal.clone()}>
                                            <span class="iconify lucide--upload size-4"></span>
                                            <span class="hidden sm:inline">{t("toolbar.add")}</span>
                                        </button>
                                    </div>
                                </div>
                                <div class="mt-3 flex flex-wrap items-center gap-2 px-5 pb-4">
                                    {if props.tag_options.is_empty() {
                                        html! {
                                            <Input
                                                aria_label={Some(AttrValue::from(bundle.text("torrents.tags_label")))}
                                                placeholder={Some(AttrValue::from(bundle.text("torrents.tags_placeholder")))}
                                                value={AttrValue::from(tag_input_value.clone())}
                                                size={DaisySize::Sm}
                                                class="w-40"
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
                                                aria_label={Some(AttrValue::from(bundle.text("torrents.tags_label")))}
                                                options={props.tag_options.clone()}
                                                values={tag_values.clone()}
                                                size={DaisySize::Sm}
                                                class="w-40"
                                                onchange={{
                                                    let on_tags = props.on_tags_filter.clone();
                                                    Callback::from(move |values: Vec<AttrValue>| {
                                                        on_tags.emit(values.into_iter().map(|value| value.to_string()).collect());
                                                    })
                                                }}
                                            />
                                        }
                                    }}
                                    <Input
                                        aria_label={Some(AttrValue::from(bundle.text("torrents.tracker_label")))}
                                        placeholder={Some(AttrValue::from(bundle.text("torrents.tracker_placeholder")))}
                                        value={AttrValue::from(props.tracker_filter.clone())}
                                        size={DaisySize::Sm}
                                        class="w-40"
                                        oninput={{
                                            let on_tracker = props.on_tracker_filter.clone();
                                            Callback::from(move |value: String| on_tracker.emit(value))
                                        }}
                                    />
                                    <Input
                                        aria_label={Some(AttrValue::from(bundle.text("torrents.extension_label")))}
                                        placeholder={Some(AttrValue::from(bundle.text("torrents.extension_placeholder")))}
                                        value={AttrValue::from(props.extension_filter.clone())}
                                        size={DaisySize::Sm}
                                        class="w-32"
                                        oninput={{
                                            let on_extension = props.on_extension_filter.clone();
                                            Callback::from(move |value: String| on_extension.emit(value))
                                        }}
                                    />
                                    <button
                                        class="btn btn-ghost btn-sm"
                                        disabled={!has_filters}
                                        onclick={clear_filters.clone()}>
                                        {bundle.text("torrents.clear_filters")}
                                    </button>
                                    <span class="text-xs text-base-content/60">
                                        {format!("{} {}", display_ids.len(), bundle.text("torrents.results"))}
                                    </span>
                                </div>
                                <div class="mt-4 overflow-auto">
                                    <table class="table bg-base-200">
                                        <thead>
                                            <tr>
                                                <th class="px-6">
                                                    <input
                                                        aria-label={t("torrents.select_all")}
                                                        class="checkbox checkbox-sm"
                                                        type="checkbox"
                                                        checked={selected_count > 0 && selected_count == display_ids.len()}
                                                        onclick={{
                                                            let selected_ids = selected_ids.clone();
                                                            let on_set_selected = props.on_set_selected.clone();
                                                            let visible_ids = display_ids.clone();
                                                            Callback::from(move |_| {
                                                                on_set_selected.emit(select_all_or_clear(&selected_ids, &visible_ids));
                                                            })
                                                        }}
                                                    />
                                                </th>
                                                {sortable_header(
                                                    AttrValue::from(bundle.text("torrents.name")),
                                                    SortKey::Name,
                                                    *sort_state,
                                                    on_sort.clone(),
                                                )}
                                                {sortable_header(
                                                    AttrValue::from(bundle.text("torrents.state")),
                                                    SortKey::State,
                                                    *sort_state,
                                                    on_sort.clone(),
                                                )}
                                                {sortable_header(
                                                    AttrValue::from(bundle.text("torrents.progress")),
                                                    SortKey::Progress,
                                                    *sort_state,
                                                    on_sort.clone(),
                                                )}
                                                {sortable_header(
                                                    AttrValue::from(bundle.text("torrents.down")),
                                                    SortKey::Down,
                                                    *sort_state,
                                                    on_sort.clone(),
                                                )}
                                                {sortable_header(
                                                    AttrValue::from(bundle.text("torrents.up")),
                                                    SortKey::Up,
                                                    *sort_state,
                                                    on_sort.clone(),
                                                )}
                                                {sortable_header(
                                                    AttrValue::from(bundle.text("torrents.ratio")),
                                                    SortKey::Ratio,
                                                    *sort_state,
                                                    on_sort.clone(),
                                                )}
                                                {sortable_header(
                                                    AttrValue::from(bundle.text("torrents.size")),
                                                    SortKey::Size,
                                                    *sort_state,
                                                    on_sort.clone(),
                                                )}
                                                {sortable_header(
                                                    AttrValue::from(bundle.text("torrents.eta")),
                                                    SortKey::Eta,
                                                    *sort_state,
                                                    on_sort.clone(),
                                                )}
                                                {sortable_header(
                                                    AttrValue::from(bundle.text("torrents.tags")),
                                                    SortKey::Tags,
                                                    *sort_state,
                                                    on_sort.clone(),
                                                )}
                                                {sortable_header(
                                                    AttrValue::from(bundle.text("torrents.trackers")),
                                                    SortKey::Trackers,
                                                    *sort_state,
                                                    on_sort.clone(),
                                                )}
                                                {sortable_header(
                                                    AttrValue::from(bundle.text("torrents.updated")),
                                                    SortKey::Updated,
                                                    *sort_state,
                                                    on_sort.clone(),
                                                )}
                                                <th>{bundle.text("torrents.actions")}</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {if display_ids.is_empty() {
                                                html! {
                                                    <tr>
                                                        <td colspan="13" class="px-6 py-8">
                                                            <EmptyState title={AttrValue::from(bundle.text("torrents.empty"))} />
                                                        </td>
                                                    </tr>
                                                }
                                            } else {
                                                html! {for display_ids.iter().enumerate().map(|(idx, id)| {
                                                    let active = Some(*id) == selected_id;
                                                    html! {
                                                        <TorrentTableRow
                                                            id={*id}
                                                            active={active || idx == *selected_idx}
                                                            on_select={on_select.clone()}
                                                            on_toggle={toggle_select.clone()}
                                                            on_action={props.on_action.clone()}
                                                            on_prompt_remove={on_prompt_remove.clone()}
                                                            on_prompt_rate={on_prompt_rate.clone()}
                                                            bundle={bundle.clone()}
                                                        />
                                                    }
                                                })}
                                            }}
                                        </tbody>
                                    </table>
                                </div>
                                {if props.can_load_more {
                                    let label = if props.is_loading {
                                        bundle.text("torrents.loading_more")
                                    } else {
                                        bundle.text("torrents.load_more")
                                    };
                                    let on_load_more = props.on_load_more.clone();
                                    html! {
                                        <div class="flex justify-center px-5 py-4">
                                            <button
                                                class="btn btn-outline btn-sm"
                                                onclick={Callback::from(move |_| on_load_more.emit(()))}
                                                disabled={props.is_loading}>
                                                {label}
                                            </button>
                                        </div>
                                    }
                                } else { html! {} }}
                            </div>
                        </div>
                    </div>
                }}
                side={{
                    if drawer_open {
                        if let Some(detail) = props.selected_detail.clone() {
                            html! {
                                <div class="min-h-full w-96 max-w-full bg-base-200 p-4">
                                    <DetailView
                                        data={Some(detail)}
                                        on_action={props.on_action.clone()}
                                        on_prompt_rate={on_prompt_rate_detail.clone()}
                                        on_prompt_remove={on_prompt_remove_detail.clone()}
                                        on_update_selection={props.on_update_selection.clone()}
                                        on_update_options={props.on_update_options.clone()}
                                        on_close={close_drawer.clone()}
                                        footer={html! {}}
                                    />
                                </div>
                            }
                        } else {
                            html! {
                                <div class="min-h-full w-96 max-w-full bg-base-200 p-4">
                                    <div class="card bg-base-100 border border-base-200 shadow">
                                        <div class="card-body items-center justify-center">
                                            <Loading size={DaisySize::Sm} />
                                        </div>
                                    </div>
                                </div>
                            }
                        }
                    } else {
                        html! {}
                    }
                }}
            />
            {if *show_add_modal {
                html! {
                    <Modal open={true} on_close={close_add_modal.clone()}>
                        <div class="flex items-start justify-between gap-3">
                            <div>
                                <h3 class="text-lg font-semibold">{t("torrents.add_title")}</h3>
                                <p class="text-sm text-base-content/60">{t("torrents.add_subtitle")}</p>
                            </div>
                            <button
                                class="btn btn-circle btn-ghost btn-sm"
                                onclick={{
                                    let close_add_modal = close_add_modal.clone();
                                    Callback::from(move |_| close_add_modal.emit(()))
                                }}
                                aria-label={t("torrents.close_modal")}>
                                <span class="iconify lucide--x size-4"></span>
                            </button>
                        </div>
                        <div class="mt-4">
                            <AddTorrentPanel on_submit={submit_add.clone()} pending={props.add_busy} />
                        </div>
                    </Modal>
                }
            } else { html! {} }}
            {if *show_create_modal {
                html! {
                    <Modal
                        open={true}
                        on_close={close_create_modal.clone()}
                        class={classes!("modal-bottom", "sm:modal-middle")}
                    >
                        <div class="flex items-start justify-between gap-3">
                            <div>
                                <h3 class="text-lg font-semibold">{t("torrents.create_title")}</h3>
                                <p class="text-sm text-base-content/60">{t("torrents.create_subtitle")}</p>
                            </div>
                            <button
                                class="btn btn-circle btn-ghost btn-sm"
                                onclick={{
                                    let close_create_modal = close_create_modal.clone();
                                    Callback::from(move |_| close_create_modal.emit(()))
                                }}
                                aria-label={t("torrents.close_modal")}>
                                <span class="iconify lucide--x size-4"></span>
                            </button>
                        </div>
                        <div class="mt-4">
                            <CreateTorrentPanel
                                on_submit={props.on_create.clone()}
                                on_copy={props.on_copy_payload.clone()}
                                pending={props.create_busy}
                                result={props.create_result.clone()}
                                error={props.create_error.clone()}
                            />
                        </div>
                    </Modal>
                }
            } else { html! {} }}
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SortDirection {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SortKey {
    Name,
    State,
    Progress,
    Down,
    Up,
    Ratio,
    Size,
    Eta,
    Tags,
    Trackers,
    Updated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SortState {
    key: SortKey,
    direction: SortDirection,
}

fn on_sort_callback(state: UseStateHandle<Option<SortState>>) -> Callback<SortKey> {
    Callback::from(move |key| {
        let next = next_sort_state(*state, key);
        state.set(next);
    })
}

fn next_sort_state(current: Option<SortState>, key: SortKey) -> Option<SortState> {
    match current {
        None => Some(SortState {
            key,
            direction: SortDirection::Asc,
        }),
        Some(state) if state.key != key => Some(SortState {
            key,
            direction: SortDirection::Asc,
        }),
        Some(state) => match state.direction {
            SortDirection::Asc => Some(SortState {
                key,
                direction: SortDirection::Desc,
            }),
            SortDirection::Desc => None,
        },
    }
}

fn sortable_header(
    label: AttrValue,
    key: SortKey,
    sort_state: Option<SortState>,
    on_sort: Callback<SortKey>,
) -> Html {
    let sorting = match sort_state {
        Some(state) if state.key == key => match state.direction {
            SortDirection::Asc => "asc",
            SortDirection::Desc => "desc",
        },
        _ => "none",
    };
    let onclick = Callback::from(move |_| on_sort.emit(key));
    html! {
        <th>
            <div
                class="group flex cursor-pointer items-center justify-between"
                data-sorting={sorting}
                onclick={onclick}>
                <span>{label}</span>
                <div class="flex flex-col items-center justify-center -space-y-1.5">
                    <span class="iconify lucide--chevron-up text-base-content size-3.5 opacity-40 group-data-[sorting=asc]:opacity-100"></span>
                    <span class="iconify lucide--chevron-down text-base-content size-3.5 opacity-40 group-data-[sorting=desc]:opacity-100"></span>
                </div>
            </div>
        </th>
    }
}

fn sort_ids(ids: &[Uuid], store: &AppStore, sort_state: Option<SortState>) -> Vec<Uuid> {
    let mut next: Vec<Uuid> = ids.to_vec();
    let Some(sort_state) = sort_state else {
        return next;
    };
    next.sort_by(|a, b| compare_row(store, a, b, sort_state));
    next
}

fn compare_row(
    store: &AppStore,
    left: &Uuid,
    right: &Uuid,
    sort_state: SortState,
) -> std::cmp::Ordering {
    let Some(left_row) = store.torrents.by_id.get(left) else {
        return std::cmp::Ordering::Equal;
    };
    let Some(right_row) = store.torrents.by_id.get(right) else {
        return std::cmp::Ordering::Equal;
    };
    let order = match sort_state.key {
        SortKey::Name => left_row.name.cmp(&right_row.name),
        SortKey::State => left_row.status.cmp(&right_row.status),
        SortKey::Progress => cmp_f64(left_row.progress, right_row.progress),
        SortKey::Down => left_row.download_bps.cmp(&right_row.download_bps),
        SortKey::Up => left_row.upload_bps.cmp(&right_row.upload_bps),
        SortKey::Ratio => cmp_f64(left_row.ratio, right_row.ratio),
        SortKey::Size => left_row.size_bytes.cmp(&right_row.size_bytes),
        SortKey::Eta => eta_value(&left_row.eta).cmp(&eta_value(&right_row.eta)),
        SortKey::Tags => first_tag(left_row).cmp(first_tag(right_row)),
        SortKey::Trackers => left_row.tracker.cmp(&right_row.tracker),
        SortKey::Updated => left_row.updated.cmp(&right_row.updated),
    };
    match sort_state.direction {
        SortDirection::Asc => order,
        SortDirection::Desc => order.reverse(),
    }
}

fn cmp_f64(left: f64, right: f64) -> std::cmp::Ordering {
    left.partial_cmp(&right)
        .unwrap_or(std::cmp::Ordering::Equal)
}

fn eta_value(eta: &Option<String>) -> u64 {
    eta.as_ref()
        .and_then(|value| value.trim_end_matches('s').parse::<u64>().ok())
        .unwrap_or(u64::MAX)
}

fn first_tag(row: &TorrentRow) -> &str {
    row.tags.first().map(String::as_str).unwrap_or("")
}

#[derive(Properties, PartialEq)]
struct TorrentTableRowProps {
    id: Uuid,
    active: bool,
    on_select: Callback<Uuid>,
    on_toggle: Callback<Uuid>,
    on_action: Callback<(TorrentAction, Uuid)>,
    on_prompt_remove: Callback<ActionTarget>,
    on_prompt_rate: Callback<ActionTarget>,
    bundle: TranslationBundle,
}

#[function_component(TorrentTableRow)]
fn torrent_table_row(props: &TorrentTableRowProps) -> Html {
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
    )
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
) -> Html {
    let t = |key: &str| bundle.text(key);
    let progress_percent = (progress.progress * 100.0).clamp(0.0, 100.0);
    let eta_label = progress
        .eta
        .clone()
        .unwrap_or_else(|| t("torrents.eta_infinite"));
    let extra_tags = base.tags.len().saturating_sub(2);
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
    let can_pause = !matches!(
        progress.status.as_str(),
        "paused" | "stopped" | "error" | "failed"
    );
    let can_resume = matches!(
        progress.status.as_str(),
        "paused" | "stopped" | "error" | "failed"
    );
    let action_menu = render_action_menu(
        &bundle,
        vec![
            ActionMenuItem::new(bundle.text("toolbar.reannounce"), reannounce),
            ActionMenuItem::new(bundle.text("toolbar.recheck"), recheck),
            ActionMenuItem::new(bundle.text("toolbar.sequential_on"), sequential_on),
            ActionMenuItem::new(bundle.text("toolbar.sequential_off"), sequential_off),
            ActionMenuItem::new(bundle.text("toolbar.rate"), prompt_rate),
            ActionMenuItem::danger(bundle.text("toolbar.delete"), prompt_remove),
        ],
    );
    let fsops_label = fsops_label(&bundle, fsops);
    let row_class = classes!(
        "row-hover",
        "cursor-pointer",
        "*:text-nowrap",
        selected.then_some("bg-base-100")
    );
    html! {
        <tr class={row_class} aria-selected={selected.to_string()} onclick={row_click}>
            <th class="px-6">
                <input
                    type="checkbox"
                    class="checkbox checkbox-sm"
                    aria-label={t("torrents.select_row")}
                    checked={checked}
                    onclick={{
                        let on_toggle = on_toggle.clone();
                        let id = base.id;
                        Callback::from(move |_| on_toggle.emit(id))
                    }}
                />
            </th>
            <td class="min-w-64">
                <div class="min-w-0">
                    <p class="font-medium truncate">{base.name.clone()}</p>
                    <p class="text-base-content/60 text-xs truncate">
                        {if base.path.is_empty() {
                            t("torrents.path_unknown")
                        } else {
                            base.path.clone()
                        }}
                    </p>
                </div>
            </td>
            <td>
                <div class="flex flex-wrap items-center gap-2">
                    <span class={classes!("badge", "badge-sm", "badge-soft", status_badge_class(&progress.status))}>
                        {progress.status.clone()}
                    </span>
                    {if let Some(label) = fsops_label {
                        html! {
                            <span
                                class={classes!("badge", "badge-sm", "badge-outline", fsops_badge_class(fsops))}
                                title={fsops.and_then(|badge| badge.detail.clone()).unwrap_or_default()}>
                                {label}
                            </span>
                        }
                    } else { html!{} }}
                </div>
            </td>
            <td class="min-w-40">
                <progress
                    class="progress progress-primary h-2"
                    max="100"
                    value={format!("{progress_percent:.1}")}></progress>
                <div class="mt-1 flex items-center justify-between text-xs text-base-content/60">
                    <span>{format!("{progress_percent:.1}%")}</span>
                </div>
            </td>
            <td class="text-sm">{format_rate(progress.download_bps)}</td>
            <td class="text-sm">{format_rate(progress.upload_bps)}</td>
            <td class="text-sm">{format!("{:.2}", base.ratio)}</td>
            <td class="text-sm">{base.size_label()}</td>
            <td class="text-sm">{eta_label}</td>
            <td>
                <div class="flex flex-wrap items-center gap-1.5 text-xs">
                    <span class="badge badge-ghost badge-sm">{base.category.clone()}</span>
                    {for base.tags.iter().take(2).map(|tag| html! {
                        <span class="badge badge-ghost badge-sm">{tag.to_string()}</span>
                    })}
                    {if extra_tags > 0 {
                        html! { <span class="badge badge-ghost badge-sm">{format!("+{extra_tags}")}</span> }
                    } else { html!{} }}
                </div>
            </td>
            <td class="text-sm">
                {if base.tracker.is_empty() {
                    t("torrents.tracker_none")
                } else {
                    base.tracker.clone()
                }}
            </td>
            <td class="text-xs">{base.updated.clone()}</td>
            <td>
                <div class="flex items-center gap-2">
                    <button
                        class={classes!(
                            "btn",
                            "btn-xs",
                            if can_pause { "btn-primary" } else { "btn-ghost" }
                        )}
                        disabled={!can_pause}
                        onclick={pause}>
                        {t("toolbar.pause")}
                    </button>
                    <button
                        class={classes!(
                            "btn",
                            "btn-xs",
                            if can_resume { "btn-primary" } else { "btn-ghost" }
                        )}
                        disabled={!can_resume}
                        onclick={resume}>
                        {t("toolbar.resume")}
                    </button>
                    {action_menu}
                </div>
            </td>
        </tr>
    }
}

fn status_badge_class(status: &str) -> &'static str {
    match status {
        "downloading" | "seeding" | "completed" => "badge-success",
        "fetching_metadata" | "queued" => "badge-info",
        "checking" => "badge-warning",
        "paused" | "stopped" => "badge-neutral",
        "error" | "failed" => "badge-error",
        _ => "badge-neutral",
    }
}

fn fsops_badge_class(fsops: Option<&FsopsBadge>) -> &'static str {
    match fsops.map(|badge| &badge.status) {
        Some(FsopsStatus::InProgress) => "badge-warning",
        Some(FsopsStatus::Completed) => "badge-success",
        Some(FsopsStatus::Failed) => "badge-error",
        None => "badge-neutral",
    }
}

fn fsops_label(bundle: &TranslationBundle, fsops: Option<&FsopsBadge>) -> Option<String> {
    let label = match fsops.map(|badge| &badge.status) {
        Some(FsopsStatus::InProgress) => bundle.text("torrents.fsops_in_progress"),
        Some(FsopsStatus::Completed) => bundle.text("torrents.fsops_done"),
        Some(FsopsStatus::Failed) => bundle.text("torrents.fsops_failed"),
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
            format!("{} {name}", bundle.text("torrents.banner.removed_data"))
        }
        TorrentAction::Delete { with_data: false } => {
            format!("{} {name}", bundle.text("torrents.banner.removed"))
        }
        TorrentAction::Reannounce => {
            format!("{} {name}", bundle.text("torrents.banner.reannounce"))
        }
        TorrentAction::Recheck => {
            format!("{} {name}", bundle.text("torrents.banner.recheck"))
        }
        TorrentAction::Pause => format!("{} {name}", bundle.text("torrents.banner.pause")),
        TorrentAction::Resume => format!("{} {name}", bundle.text("torrents.banner.resume")),
        TorrentAction::Sequential { enable } => {
            if *enable {
                format!("{} {name}", bundle.text("torrents.banner.sequential_on"))
            } else {
                format!("{} {name}", bundle.text("torrents.banner.sequential_off"))
            }
        }
        TorrentAction::Rate { .. } => {
            format!("{} {name}", bundle.text("torrents.banner.rate"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_class_maps_states() {
        assert_eq!(status_badge_class("downloading"), "badge-success");
        assert_eq!(status_badge_class("paused"), "badge-neutral");
        assert_eq!(status_badge_class("unknown"), "badge-neutral");
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
        assert!(msg.contains(&bundle.text("torrents.banner.pause")));
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
    let t = |key: &str| bundle.text(key);
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
        <Modal open={true} on_close={props.on_close.clone()}>
            <div class="space-y-4">
                <div>
                    <h3 class="text-lg font-semibold">{title}</h3>
                    <p class="text-sm text-base-content/60">{body}</p>
                </div>
                <div class="flex justify-end gap-2">
                    <button
                        class="btn btn-ghost btn-sm"
                        onclick={{
                            let cb = props.on_close.clone();
                            Callback::from(move |_| cb.emit(()))
                        }}>
                        {t("confirm.cancel")}
                    </button>
                    <button
                        class={classes!(
                            "btn",
                            "btn-sm",
                            if matches!(kind, ConfirmKind::Recheck) { "btn-primary" } else { "btn-error" }
                        )}
                        onclick={confirm}>
                        {action}
                    </button>
                </div>
            </div>
        </Modal>
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
    let target = props.target.clone();
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
    let Some(target) = target else {
        return html! {};
    };
    let title = format!("{} {}", bundle.text("confirm.remove.title"), target.label);
    let body = bundle.text("confirm.remove.body");
    let toggle_label = bundle.text("confirm.remove_toggle");
    let confirm_label = bundle.text("confirm.remove.cta");
    let confirm = {
        let delete_data = delete_data.clone();
        let cb = props.on_confirm.clone();
        Callback::from(move |_| cb.emit(*delete_data))
    };
    html! {
        <Modal open={true} on_close={props.on_close.clone()}>
            <div class="space-y-4">
                <div>
                    <h3 class="text-lg font-semibold">{title}</h3>
                    <p class="text-sm text-base-content/60">{body}</p>
                </div>
                <label class="label cursor-pointer justify-start gap-3">
                    <input
                        type="checkbox"
                        class="checkbox checkbox-sm"
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
                    <span class="label-text">{toggle_label}</span>
                </label>
                <div class="flex justify-end gap-2">
                    <button
                        class="btn btn-ghost btn-sm"
                        onclick={{
                            let cb = props.on_close.clone();
                            Callback::from(move |_| cb.emit(()))
                        }}>
                        {bundle.text("confirm.cancel")}
                    </button>
                    <button class="btn btn-error btn-sm" onclick={confirm}>
                        {confirm_label}
                    </button>
                </div>
            </div>
        </Modal>
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
    let target = props.target.clone();
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
    let Some(target) = target else {
        return html! {};
    };
    let title = format!("{} {}", bundle.text("torrents.rate_title"), target.label);
    let body = bundle.text("torrents.rate_body");
    let confirm_label = bundle.text("torrents.rate_apply");
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
                    error.set(Some(bundle.text("torrents.rate_invalid")));
                    return;
                }
            };
            let upload = match parse_rate_input(&upload_value) {
                Ok(parsed) => parsed,
                Err(_) => {
                    error.set(Some(bundle.text("torrents.rate_invalid")));
                    return;
                }
            };
            if download.is_none() && upload.is_none() {
                error.set(Some(bundle.text("torrents.rate_empty")));
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
        <Modal open={true} on_close={props.on_close.clone()}>
            <div class="space-y-4">
                <div>
                    <h3 class="text-lg font-semibold">{title}</h3>
                    <p class="text-sm text-base-content/60">{body}</p>
                </div>
                <div class="grid gap-3 sm:grid-cols-2">
                    <div class="form-control w-full">
                        <label class="label pb-1">
                            <span class="label-text text-xs">
                                {bundle.text("torrents.rate_download")}
                            </span>
                        </label>
                        <Input
                            value={AttrValue::from((*download_input).clone())}
                            placeholder={Some(AttrValue::from(bundle.text("torrents.rate_placeholder")))}
                            class="w-full"
                            oninput={{
                                let download_input = download_input.clone();
                                Callback::from(move |value: String| download_input.set(value))
                            }}
                        />
                    </div>
                    <div class="form-control w-full">
                        <label class="label pb-1">
                            <span class="label-text text-xs">
                                {bundle.text("torrents.rate_upload")}
                            </span>
                        </label>
                        <Input
                            value={AttrValue::from((*upload_input).clone())}
                            placeholder={Some(AttrValue::from(bundle.text("torrents.rate_placeholder")))}
                            class="w-full"
                            oninput={{
                                let upload_input = upload_input.clone();
                                Callback::from(move |value: String| upload_input.set(value))
                            }}
                        />
                    </div>
                </div>
                {if let Some(msg) = &*error {
                    html! {
                        <div role="alert" class="alert alert-error">
                            <span>{msg}</span>
                        </div>
                    }
                } else { html! {} }}
                <div class="flex justify-end gap-2">
                    <button
                        class="btn btn-ghost btn-sm"
                        onclick={{
                            let cb = props.on_close.clone();
                            Callback::from(move |_| cb.emit(()))
                        }}>
                        {bundle.text("confirm.cancel")}
                    </button>
                    <button class="btn btn-primary btn-sm" onclick={confirm}>
                        {confirm_label}
                    </button>
                </div>
            </div>
        </Modal>
    }
}

#[derive(Properties, PartialEq)]
pub(crate) struct BannerProps {
    pub message: Option<String>,
}

#[function_component(ActionBanner)]
fn action_banner(props: &BannerProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key);
    let Some(msg) = props.message.clone() else {
        return html! {};
    };
    html! {
        <div class="toast toast-end toast-bottom" role="status" aria-live="polite">
            <div class="alert alert-info shadow">
                <span class="badge badge-ghost badge-sm">{t("torrents.shortcut")}</span>
                <span class="text-sm">{msg}</span>
            </div>
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
