//! Torrent detail drawer view.
//!
//! # Design
//! - Keep detail rendering stateless with respect to the parent; update requests flow via callbacks.
//! - Focus on layout + view composition; validation stays close to inputs.

use super::action_menu::{ActionMenuItem, render_action_menu};
use crate::Pane;
use crate::components::atoms::EmptyState;
use crate::core::logic::{format_bytes, format_rate};
use crate::features::torrents::actions::TorrentAction;
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::{FilePriority, TorrentDetail, TorrentOptionsRequest, TorrentStateKind};
use uuid::Uuid;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FileSelectionChange {
    Toggle {
        index: u32,
        path: String,
        selected: bool,
    },
    Priority {
        index: u32,
        priority: FilePriority,
    },
    SkipFluff {
        enabled: bool,
    },
}

#[derive(Properties, PartialEq)]
pub(crate) struct DetailProps {
    pub data: Option<TorrentDetail>,
    pub on_action: Callback<(TorrentAction, Uuid)>,
    pub on_prompt_rate: Callback<(Uuid, String)>,
    pub on_prompt_remove: Callback<(Uuid, String)>,
    pub on_update_selection: Callback<(Uuid, FileSelectionChange)>,
    pub on_update_options: Callback<(Uuid, TorrentOptionsRequest)>,
    #[prop_or_default]
    pub on_close: Callback<()>,
    #[prop_or_default]
    pub footer: Html,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(DetailView)]
pub(crate) fn detail_view(props: &DetailProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key);
    let active = use_state(|| Pane::Overview);
    let connections_input = use_state(String::new);
    let connections_error = use_state(|| None as Option<String>);
    let queue_input = use_state(String::new);
    let queue_error = use_state(|| None as Option<String>);

    let detail_key = props
        .data
        .as_ref()
        .map(|detail| (detail.summary.id, detail.settings.clone()));
    {
        let connections_input = connections_input.clone();
        let connections_error = connections_error.clone();
        let queue_input = queue_input.clone();
        let queue_error = queue_error.clone();
        use_effect_with(detail_key, move |detail_key| {
            if let Some((_, settings)) = detail_key.as_ref() {
                let connections_value = settings
                    .as_ref()
                    .and_then(|settings| settings.connections_limit)
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                let queue_value = settings
                    .as_ref()
                    .and_then(|settings| settings.queue_position)
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                connections_input.set(connections_value);
                queue_input.set(queue_value);
            } else {
                connections_input.set(String::new());
                queue_input.set(String::new());
            }
            connections_error.set(None);
            queue_error.set(None);
            || ()
        });
    }

    let Some(detail) = props.data.as_ref() else {
        return html! {};
    };

    let name = detail
        .summary
        .name
        .clone()
        .unwrap_or_else(|| t("detail.unnamed"));
    let detail_id = detail.summary.id;
    let status = state_label(&detail.summary.state.kind).to_string();
    let status_class = status_class(&detail.summary.state.kind);
    let last_error = detail.summary.state.failure_message.clone();
    let progress_percent = detail.summary.progress.percent_complete;
    let progress_label = format!(
        "{:.1}% ({}/{})",
        progress_percent,
        format_bytes(detail.summary.progress.bytes_downloaded),
        format_bytes(detail.summary.progress.bytes_total)
    );
    let rates_label = format!(
        "{} / {}",
        format_rate(detail.summary.rates.download_bps),
        format_rate(detail.summary.rates.upload_bps)
    );
    let ratio_label = format!("{:.2}", detail.summary.rates.ratio);
    let tags_label = if detail.summary.tags.is_empty() {
        t("detail.value_none")
    } else {
        detail.summary.tags.join(", ")
    };
    let trackers_label = if detail.summary.trackers.is_empty() {
        t("detail.value_none")
    } else {
        detail.summary.trackers.join(", ")
    };
    let category_label = detail
        .summary
        .category
        .clone()
        .unwrap_or_else(|| t("detail.value_unset"));
    let save_path_label = detail
        .summary
        .download_dir
        .clone()
        .or(detail.summary.library_path.clone())
        .unwrap_or_else(|| t("detail.value_unset"));
    let updated_label = detail
        .summary
        .last_updated
        .format("%Y-%m-%d %H:%M UTC")
        .to_string();

    let pause = {
        let on_action = props.on_action.clone();
        Callback::from(move |_| on_action.emit((TorrentAction::Pause, detail_id)))
    };
    let resume = {
        let on_action = props.on_action.clone();
        Callback::from(move |_| on_action.emit((TorrentAction::Resume, detail_id)))
    };
    let reannounce = {
        let on_action = props.on_action.clone();
        Callback::from(move |_| on_action.emit((TorrentAction::Reannounce, detail_id)))
    };
    let recheck = {
        let on_action = props.on_action.clone();
        Callback::from(move |_| on_action.emit((TorrentAction::Recheck, detail_id)))
    };
    let sequential_on = {
        let on_action = props.on_action.clone();
        Callback::from(move |_| {
            on_action.emit((TorrentAction::Sequential { enable: true }, detail_id))
        })
    };
    let sequential_off = {
        let on_action = props.on_action.clone();
        Callback::from(move |_| {
            on_action.emit((TorrentAction::Sequential { enable: false }, detail_id))
        })
    };
    let prompt_rate = {
        let on_prompt_rate = props.on_prompt_rate.clone();
        let label = name.clone();
        Callback::from(move |_| on_prompt_rate.emit((detail_id, label.clone())))
    };
    let prompt_remove = {
        let on_prompt_remove = props.on_prompt_remove.clone();
        let label = name.clone();
        Callback::from(move |_| on_prompt_remove.emit((detail_id, label.clone())))
    };
    let on_close = {
        let on_close = props.on_close.clone();
        Callback::from(move |_| on_close.emit(()))
    };
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

    let files_tab = render_files_tab(detail, props.on_update_selection.clone(), bundle.clone());
    let options_tab = render_options_tab(
        detail,
        props.on_update_options.clone(),
        connections_input.clone(),
        connections_error.clone(),
        queue_input.clone(),
        queue_error.clone(),
        bundle.clone(),
    );

    html! {
        <section
            class={classes!(
                "card",
                "bg-base-100",
                "border",
                "border-base-200",
                "shadow",
                "relative",
                props.class.clone()
            )}>
            <div class="card-body gap-4">
                <div class="flex items-start justify-between gap-3">
                    <div class="space-y-1">
                        <p class="text-xs text-base-content/60">{t("detail.view_label")}</p>
                        <h3 class="text-base font-semibold break-words">{name.clone()}</h3>
                    </div>
                    <div class="flex items-start gap-2">
                        <div role="tablist" class="tabs tabs-boxed tabs-sm md:hidden">
                            {for [Pane::Overview, Pane::Files, Pane::Options].iter().map(|pane| {
                                let label = match pane {
                                    Pane::Overview => t("detail.tab.overview"),
                                    Pane::Files => t("detail.tab.files"),
                                    Pane::Options => t("detail.tab.options"),
                                };
                                let active_state = *active == *pane;
                                let onclick = {
                                    let active = active.clone();
                                    let pane = *pane;
                                    Callback::from(move |_| active.set(pane))
                                };
                                html! {
                                    <button
                                        role="tab"
                                        class={classes!("tab", if active_state { "tab-active" } else { "" })}
                                        onclick={onclick}>
                                        {label}
                                    </button>
                                }
                            })}
                        </div>
                        <button
                            class="btn btn-ghost btn-xs btn-circle"
                            aria-label={t("confirm.cancel")}
                            onclick={on_close}>
                            <span class="iconify lucide--x size-4"></span>
                        </button>
                    </div>
                </div>

                <div class="grid gap-4 lg:grid-cols-2">
                    <section class={pane_classes(Pane::Overview, *active)} data-pane="overview">
                        <div class="space-y-1">
                            <h4 class="text-sm font-semibold">{t("detail.overview.title")}</h4>
                            <p class="text-sm text-base-content/60">{t("detail.overview.body")}</p>
                        </div>
                        <div class="flex flex-wrap gap-2">
                            <button class="btn btn-sm btn-ghost" onclick={pause}>{t("toolbar.pause")}</button>
                            <button class="btn btn-sm btn-ghost" onclick={resume}>{t("toolbar.resume")}</button>
                            {action_menu}
                        </div>
                        {last_error.as_ref().map(|message| html! {
                            <div class="alert alert-error text-sm">
                                <span class="iconify lucide--alert-triangle size-4"></span>
                                <div class="flex flex-wrap items-center gap-2">
                                    <span class="badge badge-sm badge-error badge-soft">
                                        {t("detail.overview.last_error")}
                                    </span>
                                    <span>{message.clone()}</span>
                                </div>
                            </div>
                        }).unwrap_or_default()}
                        <div class="overflow-x-auto">
                            <table class="table table-sm bg-base-200">
                                <tbody>
                                    <tr class="row-hover">
                                        <th class="text-xs font-medium text-base-content/70">
                                            {t("detail.overview.status")}
                                        </th>
                                        <td>
                                            <span class={classes!("badge", "badge-sm", "badge-soft", status_class)}>
                                                {status.clone()}
                                            </span>
                                        </td>
                                    </tr>
                                    <tr class="row-hover">
                                        <th class="text-xs font-medium text-base-content/70">
                                            {t("detail.overview.progress")}
                                        </th>
                                        <td class="text-sm">{progress_label}</td>
                                    </tr>
                                    <tr class="row-hover">
                                        <th class="text-xs font-medium text-base-content/70">
                                            {t("detail.overview.rates")}
                                        </th>
                                        <td class="text-sm">{rates_label}</td>
                                    </tr>
                                    <tr class="row-hover">
                                        <th class="text-xs font-medium text-base-content/70">
                                            {t("detail.overview.ratio")}
                                        </th>
                                        <td class="text-sm">{ratio_label}</td>
                                    </tr>
                                    <tr class="row-hover">
                                        <th class="text-xs font-medium text-base-content/70">
                                            {t("detail.overview.category")}
                                        </th>
                                        <td class="text-sm">{category_label}</td>
                                    </tr>
                                    <tr class="row-hover">
                                        <th class="text-xs font-medium text-base-content/70">
                                            {t("detail.overview.tags")}
                                        </th>
                                        <td class="text-sm">{tags_label}</td>
                                    </tr>
                                    <tr class="row-hover">
                                        <th class="text-xs font-medium text-base-content/70">
                                            {t("detail.overview.trackers")}
                                        </th>
                                        <td class="text-sm">{trackers_label}</td>
                                    </tr>
                                    <tr class="row-hover">
                                        <th class="text-xs font-medium text-base-content/70">
                                            {t("detail.overview.save_path")}
                                        </th>
                                        <td class="text-sm">{save_path_label}</td>
                                    </tr>
                                    <tr class="row-hover">
                                        <th class="text-xs font-medium text-base-content/70">
                                            {t("detail.overview.updated")}
                                        </th>
                                        <td class="text-sm">{updated_label}</td>
                                    </tr>
                                </tbody>
                            </table>
                        </div>
                    </section>

                    <section class={pane_classes(Pane::Files, *active)} data-pane="files">
                        {files_tab}
                    </section>

                    <section class={pane_classes(Pane::Options, *active)} data-pane="options">
                        {options_tab}
                    </section>
                </div>
                {props.footer.clone()}
            </div>
        </section>
    }
}

fn pane_classes(pane: Pane, active: Pane) -> Classes {
    classes!(
        "rounded-box",
        "border",
        "border-base-200",
        "bg-base-200/40",
        "p-4",
        "space-y-3",
        if pane == active { "block" } else { "hidden" },
        "lg:block"
    )
}

fn state_label(state: &TorrentStateKind) -> &'static str {
    match state {
        TorrentStateKind::Queued => "queued",
        TorrentStateKind::FetchingMetadata => "fetching_metadata",
        TorrentStateKind::Downloading => "downloading",
        TorrentStateKind::Seeding => "seeding",
        TorrentStateKind::Completed => "completed",
        TorrentStateKind::Failed => "failed",
        TorrentStateKind::Stopped => "stopped",
    }
}

fn status_class(state: &TorrentStateKind) -> &'static str {
    match state {
        TorrentStateKind::Downloading | TorrentStateKind::Seeding | TorrentStateKind::Completed => {
            "badge-success"
        }
        TorrentStateKind::Failed => "badge-error",
        TorrentStateKind::FetchingMetadata => "badge-warning",
        _ => "badge-ghost",
    }
}

fn priority_value(priority: FilePriority) -> &'static str {
    match priority {
        FilePriority::Skip => "skip",
        FilePriority::Low => "low",
        FilePriority::Normal => "normal",
        FilePriority::High => "high",
    }
}

fn parse_priority(value: &str) -> Option<FilePriority> {
    match value {
        "skip" => Some(FilePriority::Skip),
        "low" => Some(FilePriority::Low),
        "normal" => Some(FilePriority::Normal),
        "high" => Some(FilePriority::High),
        _ => None,
    }
}

fn render_files_tab(
    detail: &TorrentDetail,
    on_update_selection: Callback<(Uuid, FileSelectionChange)>,
    bundle: TranslationBundle,
) -> Html {
    let t = |key: &str| bundle.text(key);
    let title = t("detail.files.title");
    let body = t("detail.files.body");
    let empty_label = t("detail.files.empty");
    let skip_fluff_label = t("detail.files.skip_fluff");
    let detail_id = detail.summary.id;
    let skip_fluff = detail
        .settings
        .as_ref()
        .and_then(|settings| settings.selection.as_ref())
        .map(|selection| selection.skip_fluff)
        .unwrap_or(false);
    let on_skip_fluff = {
        let on_update_selection = on_update_selection.clone();
        Callback::from(move |event: Event| {
            if let Some(input) = event.target_dyn_into::<HtmlInputElement>() {
                on_update_selection.emit((
                    detail_id,
                    FileSelectionChange::SkipFluff {
                        enabled: input.checked(),
                    },
                ));
            }
        })
    };

    let rows = match detail.files.as_ref() {
        Some(files) if !files.is_empty() => html! {
            <div class="overflow-x-auto">
                <table class="table table-sm bg-base-200">
                    <thead>
                        <tr>
                            <th>{t("torrents.name")}</th>
                            <th>{t("detail.overview.progress")}</th>
                            <th>{t("detail.files.priority")}</th>
                            <th>{t("detail.files.wanted")}</th>
                        </tr>
                    </thead>
                    <tbody>
                        {for files.iter().map(|file| render_file_row(file, &on_update_selection, &bundle, detail_id))}
                    </tbody>
                </table>
            </div>
        },
        _ => html! { <EmptyState title={AttrValue::from(empty_label)} /> },
    };

    html! {
        <>
            <div class="space-y-1">
                <h4 class="text-sm font-semibold">{title}</h4>
                <p class="text-sm text-base-content/60">{body}</p>
            </div>
            <label class="label cursor-pointer justify-start gap-2">
                <input
                    type="checkbox"
                    class="toggle toggle-sm"
                    checked={skip_fluff}
                    aria-label={skip_fluff_label.clone()}
                    onchange={on_skip_fluff}
                />
                <span class="label-text text-sm">{skip_fluff_label}</span>
            </label>
            {rows}
        </>
    }
}

fn render_file_row(
    file: &crate::models::TorrentFileView,
    on_update_selection: &Callback<(Uuid, FileSelectionChange)>,
    bundle: &TranslationBundle,
    detail_id: Uuid,
) -> Html {
    let t = |key: &str| bundle.text(key);
    let wanted_label = t("detail.files.wanted");
    let priority_label = t("detail.files.priority");
    let percent = if file.size_bytes == 0 {
        0.0
    } else {
        (file.bytes_completed as f64 / file.size_bytes as f64) * 100.0
    };
    let progress_label = format!(
        "{} / {}",
        format_bytes(file.bytes_completed),
        format_bytes(file.size_bytes)
    );
    let on_toggle = {
        let on_update_selection = on_update_selection.clone();
        let path = file.path.clone();
        let index = file.index;
        Callback::from(move |event: Event| {
            if let Some(input) = event.target_dyn_into::<HtmlInputElement>() {
                on_update_selection.emit((
                    detail_id,
                    FileSelectionChange::Toggle {
                        index,
                        path: path.clone(),
                        selected: input.checked(),
                    },
                ));
            }
        })
    };
    let on_priority = {
        let on_update_selection = on_update_selection.clone();
        let index = file.index;
        Callback::from(move |event: Event| {
            if let Some(select) = event.target_dyn_into::<HtmlSelectElement>() {
                if let Some(priority) = parse_priority(&select.value()) {
                    on_update_selection
                        .emit((detail_id, FileSelectionChange::Priority { index, priority }));
                }
            }
        })
    };

    html! {
        <tr class="row-hover">
            <td class="max-w-[16rem]">
                <span class="block truncate text-sm font-medium">{file.path.clone()}</span>
            </td>
            <td>
                <div class="space-y-1">
                    <progress
                        class="progress progress-primary h-2 w-32"
                        max="100"
                        value={format!("{percent:.1}")}></progress>
                    <span class="text-xs text-base-content/60">{progress_label}</span>
                </div>
            </td>
            <td>
                <label class="form-control gap-1">
                    <span class="label-text text-xs">{priority_label}</span>
                    <select
                        class="select select-bordered select-xs"
                        value={priority_value(file.priority)}
                        onchange={on_priority}>
                        <option value="skip">{t("detail.files.priority_skip")}</option>
                        <option value="low">{t("detail.files.priority_low")}</option>
                        <option value="normal">{t("detail.files.priority_normal")}</option>
                        <option value="high">{t("detail.files.priority_high")}</option>
                    </select>
                </label>
            </td>
            <td class="text-center">
                <input
                    type="checkbox"
                    class="checkbox checkbox-sm"
                    checked={file.selected}
                    aria-label={wanted_label}
                    onchange={on_toggle}
                />
            </td>
        </tr>
    }
}

fn render_options_tab(
    detail: &TorrentDetail,
    on_update_options: Callback<(Uuid, TorrentOptionsRequest)>,
    connections_input: UseStateHandle<String>,
    connections_error: UseStateHandle<Option<String>>,
    queue_input: UseStateHandle<String>,
    queue_error: UseStateHandle<Option<String>>,
    bundle: TranslationBundle,
) -> Html {
    let t = |key: &str| bundle.text(key);
    let title = t("detail.options.title");
    let body = t("detail.options.body");
    let empty_label = t("detail.options.empty");
    let editable_label = t("detail.options.editable");
    let readonly_label = t("detail.options.readonly");
    let apply_label = t("detail.options.apply");
    let connections_label = t("detail.options.connections_limit");
    let pex_label = t("detail.options.pex_enabled");
    let super_seeding_label = t("detail.options.super_seeding");
    let auto_managed_label = t("detail.options.auto_managed");
    let queue_position_label = t("detail.options.queue_position");
    let unset_label = t("detail.value_unset");
    let none_label = t("detail.value_none");
    let yes_label = t("detail.value_yes");
    let no_label = t("detail.value_no");
    let invalid_label = t("detail.value_invalid");
    let invalid_label_for_connections = invalid_label.clone();
    let invalid_label_for_queue = invalid_label.clone();
    let category_label = t("detail.options.category");
    let tags_label_title = t("detail.options.tags");
    let trackers_label_title = t("detail.options.trackers");
    let tracker_messages_label_title = t("detail.options.tracker_messages");
    let download_dir_label_title = t("detail.options.download_dir");
    let rate_limit_label_title = t("detail.options.rate_limit");
    let storage_mode_label_title = t("detail.options.storage_mode");
    let partfile_label_title = t("detail.options.use_partfile");
    let sequential_label_title = t("detail.options.sequential");
    let selection_label_title = t("detail.options.selection");
    let seed_mode_label_title = t("detail.options.seed_mode");
    let seed_ratio_label_title = t("detail.options.seed_ratio");
    let seed_time_label_title = t("detail.options.seed_time");
    let cleanup_label_title = t("detail.options.cleanup");
    let private_label_title = t("detail.options.private");
    let comment_label_title = t("detail.options.comment");
    let source_label_title = t("detail.options.source");
    let web_seeds_label_title = t("detail.options.web_seeds");
    let detail_id = detail.summary.id;
    let Some(settings) = detail.settings.as_ref() else {
        return html! {
            <>
                <div class="space-y-1">
                    <h4 class="text-sm font-semibold">{title}</h4>
                    <p class="text-sm text-base-content/60">{body}</p>
                </div>
                <EmptyState title={AttrValue::from(empty_label)} />
            </>
        };
    };

    let pex_enabled = settings.pex_enabled.unwrap_or(false);
    let super_seeding = settings.super_seeding.unwrap_or(false);
    let auto_managed = settings.auto_managed.unwrap_or(true);

    let on_toggle = |current: bool, field: fn(&mut TorrentOptionsRequest, bool)| {
        let on_update_options = on_update_options.clone();
        Callback::from(move |event: Event| {
            if let Some(input) = event.target_dyn_into::<HtmlInputElement>() {
                let value = input.checked();
                if value == current {
                    return;
                }
                let mut request = TorrentOptionsRequest::default();
                field(&mut request, value);
                on_update_options.emit((detail_id, request));
            }
        })
    };

    let on_pex = on_toggle(pex_enabled, |request, value| {
        request.pex_enabled = Some(value);
    });
    let on_super_seeding = on_toggle(super_seeding, |request, value| {
        request.super_seeding = Some(value);
    });
    let on_auto_managed = on_toggle(auto_managed, |request, value| {
        request.auto_managed = Some(value);
    });

    let on_connections_apply = {
        let on_update_options = on_update_options.clone();
        let connections_input = connections_input.clone();
        let connections_error = connections_error.clone();
        Callback::from(move |_| match parse_optional_i32(&connections_input) {
            Ok(value) => {
                let mut request = TorrentOptionsRequest::default();
                request.connections_limit = value;
                connections_error.set(None);
                on_update_options.emit((detail_id, request));
            }
            Err(()) => connections_error.set(Some(invalid_label_for_connections.clone())),
        })
    };

    let on_queue_apply = {
        let on_update_options = on_update_options.clone();
        let queue_input = queue_input.clone();
        let queue_error = queue_error.clone();
        Callback::from(move |_| match parse_optional_i32(&queue_input) {
            Ok(value) => {
                let mut request = TorrentOptionsRequest::default();
                request.queue_position = value;
                queue_error.set(None);
                on_update_options.emit((detail_id, request));
            }
            Err(()) => queue_error.set(Some(invalid_label_for_queue.clone())),
        })
    };

    let connections_error_msg = (*connections_error).clone().unwrap_or_default();
    let queue_error_msg = (*queue_error).clone().unwrap_or_default();
    let connections_value = (*connections_input).clone();
    let queue_value = (*queue_input).clone();

    let rate_limit_label = settings.rate_limit.as_ref().map_or_else(
        || none_label.clone(),
        |limits| {
            let download = limits
                .download_bps
                .map(format_rate)
                .unwrap_or_else(|| none_label.clone());
            let upload = limits
                .upload_bps
                .map(format_rate)
                .unwrap_or_else(|| none_label.clone());
            format!("{} / {}", download, upload)
        },
    );
    let tags_label = if settings.tags.is_empty() {
        none_label.clone()
    } else {
        settings.tags.join(", ")
    };
    let trackers_label = if settings.trackers.is_empty() {
        none_label.clone()
    } else {
        settings.trackers.join(", ")
    };
    let tracker_messages_label = if settings.tracker_messages.is_empty() {
        none_label.clone()
    } else {
        format!("{}", settings.tracker_messages.len())
    };
    let storage_mode_label = settings
        .storage_mode
        .as_ref()
        .map(|mode| format!("{mode:?}"))
        .unwrap_or_else(|| unset_label.clone());
    let partfile_label = settings
        .use_partfile
        .map(|value| {
            if value {
                yes_label.clone()
            } else {
                no_label.clone()
            }
        })
        .unwrap_or_else(|| unset_label.clone());
    let selection_label = settings.selection.as_ref().map_or_else(
        || unset_label.clone(),
        |selection| {
            if selection.include.is_empty() && selection.exclude.is_empty() {
                none_label.clone()
            } else {
                format!(
                    "include: {}, exclude: {}",
                    selection.include.len(),
                    selection.exclude.len()
                )
            }
        },
    );
    let seed_mode_label = settings
        .seed_mode
        .map(|value| {
            if value {
                yes_label.clone()
            } else {
                no_label.clone()
            }
        })
        .unwrap_or_else(|| unset_label.clone());
    let seed_ratio_label = settings
        .seed_ratio_limit
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| unset_label.clone());
    let seed_time_label = settings
        .seed_time_limit
        .map(|value| format!("{value}s"))
        .unwrap_or_else(|| unset_label.clone());
    let cleanup_label = settings.cleanup.as_ref().map_or_else(
        || unset_label.clone(),
        |cleanup| {
            let ratio = cleanup
                .seed_ratio_limit
                .map(|value| format!("{value:.2}"))
                .unwrap_or_else(|| unset_label.clone());
            let time = cleanup
                .seed_time_limit
                .map(|value| format!("{value}s"))
                .unwrap_or_else(|| unset_label.clone());
            let remove_data = if cleanup.remove_data {
                yes_label.clone()
            } else {
                no_label.clone()
            };
            format!("ratio: {ratio}, time: {time}, remove data: {remove_data}")
        },
    );
    let private_label = settings
        .private
        .map(|value| {
            if value {
                yes_label.clone()
            } else {
                no_label.clone()
            }
        })
        .unwrap_or_else(|| unset_label.clone());
    let comment_label = settings
        .comment
        .clone()
        .unwrap_or_else(|| unset_label.clone());
    let source_label = settings
        .source
        .clone()
        .unwrap_or_else(|| unset_label.clone());
    let download_dir_label = settings
        .download_dir
        .clone()
        .unwrap_or_else(|| unset_label.clone());
    let web_seeds_label = if settings.web_seeds.is_empty() {
        none_label.clone()
    } else {
        settings.web_seeds.join(", ")
    };

    html! {
        <>
            <div class="space-y-1">
                <h4 class="text-sm font-semibold">{title}</h4>
                <p class="text-sm text-base-content/60">{body}</p>
            </div>
            <div class="space-y-3">
                <div class="rounded-box border border-base-200 bg-base-200/40 p-3 space-y-2">
                    <h5 class="text-xs font-semibold uppercase tracking-wide text-base-content/60">
                        {editable_label}
                    </h5>
                    <div class="overflow-x-auto">
                        <table class="table table-sm bg-base-200">
                            <tbody>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{connections_label}</th>
                                    <td>
                                        <div class="flex flex-wrap items-center gap-2">
                                            <input
                                                class="input input-bordered input-xs w-24"
                                                type="number"
                                                value={connections_value}
                                                placeholder={unset_label.clone()}
                                                onchange={{
                                                    let connections_input = connections_input.clone();
                                                    Callback::from(move |event: Event| {
                                                        if let Some(input) = event.target_dyn_into::<HtmlInputElement>() {
                                                            connections_input.set(input.value());
                                                        }
                                                    })
                                                }}
                                            />
                                            <button class="btn btn-xs btn-ghost" onclick={on_connections_apply}>
                                                {apply_label.clone()}
                                            </button>
                                            {if connections_error_msg.is_empty() {
                                                html! {}
                                            } else {
                                                html! { <span class="badge badge-warning badge-sm">{connections_error_msg}</span> }
                                            }}
                                        </div>
                                    </td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{pex_label}</th>
                                    <td>
                                        <input type="checkbox" class="toggle toggle-sm" checked={pex_enabled} onchange={on_pex} />
                                    </td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{super_seeding_label}</th>
                                    <td>
                                        <input type="checkbox" class="toggle toggle-sm" checked={super_seeding} onchange={on_super_seeding} />
                                    </td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{auto_managed_label}</th>
                                    <td>
                                        <input type="checkbox" class="toggle toggle-sm" checked={auto_managed} onchange={on_auto_managed} />
                                    </td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{queue_position_label}</th>
                                    <td>
                                        <div class="flex flex-wrap items-center gap-2">
                                            <input
                                                class="input input-bordered input-xs w-24"
                                                type="number"
                                                value={queue_value}
                                                placeholder={unset_label.clone()}
                                                disabled={auto_managed}
                                                onchange={{
                                                    let queue_input = queue_input.clone();
                                                    Callback::from(move |event: Event| {
                                                        if let Some(input) = event.target_dyn_into::<HtmlInputElement>() {
                                                            queue_input.set(input.value());
                                                        }
                                                    })
                                                }}
                                            />
                                            <button
                                                class="btn btn-xs btn-ghost"
                                                onclick={on_queue_apply}
                                                disabled={auto_managed}>
                                                {apply_label.clone()}
                                            </button>
                                            {if queue_error_msg.is_empty() {
                                                html! {}
                                            } else {
                                                html! { <span class="badge badge-warning badge-sm">{queue_error_msg}</span> }
                                            }}
                                        </div>
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
                <div class="rounded-box border border-base-200 bg-base-200/20 p-3 space-y-2">
                    <h5 class="text-xs font-semibold uppercase tracking-wide text-base-content/60">
                        {readonly_label}
                    </h5>
                    <div class="overflow-x-auto">
                        <table class="table table-sm bg-base-200">
                            <tbody>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{category_label}</th>
                                    <td class="text-sm break-words">
                                        {settings.category.clone().unwrap_or_else(|| unset_label.clone())}
                                    </td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{tags_label_title}</th>
                                    <td class="text-sm break-words">{tags_label}</td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{trackers_label_title}</th>
                                    <td class="text-sm break-words">{trackers_label}</td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{tracker_messages_label_title}</th>
                                    <td class="text-sm">{tracker_messages_label}</td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{download_dir_label_title}</th>
                                    <td class="text-sm break-words">{download_dir_label}</td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{rate_limit_label_title}</th>
                                    <td class="text-sm">{rate_limit_label}</td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{storage_mode_label_title}</th>
                                    <td class="text-sm">{storage_mode_label}</td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{partfile_label_title}</th>
                                    <td class="text-sm">{partfile_label}</td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{sequential_label_title}</th>
                                    <td class="text-sm">{if settings.sequential { yes_label.clone() } else { no_label.clone() }}</td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{selection_label_title}</th>
                                    <td class="text-sm break-words">{selection_label}</td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{seed_mode_label_title}</th>
                                    <td class="text-sm">{seed_mode_label}</td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{seed_ratio_label_title}</th>
                                    <td class="text-sm">{seed_ratio_label}</td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{seed_time_label_title}</th>
                                    <td class="text-sm">{seed_time_label}</td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{cleanup_label_title}</th>
                                    <td class="text-sm break-words">{cleanup_label}</td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{private_label_title}</th>
                                    <td class="text-sm">{private_label}</td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{comment_label_title}</th>
                                    <td class="text-sm break-words">{comment_label}</td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{source_label_title}</th>
                                    <td class="text-sm break-words">{source_label}</td>
                                </tr>
                                <tr class="row-hover">
                                    <th class="text-xs font-medium text-base-content/70">{web_seeds_label_title}</th>
                                    <td class="text-sm break-words">{web_seeds_label}</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>
        </>
    }
}

fn parse_optional_i32(input: &UseStateHandle<String>) -> Result<Option<i32>, ()> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed.parse::<i32>().map(Some).map_err(|_| ())
}
