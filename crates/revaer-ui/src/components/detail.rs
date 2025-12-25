use crate::Pane;
use crate::components::action_menu::{ActionMenuItem, render_action_menu};
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
    pub class: Classes,
}

#[function_component(DetailView)]
pub(crate) fn detail_view(props: &DetailProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
    let active = use_state(|| Pane::Overview);
    let connections_input = use_state(String::new);
    let connections_error = use_state(|| None as Option<String>);
    let queue_input = use_state(String::new);
    let queue_error = use_state(|| None as Option<String>);

    let Some(detail) = props.data.as_ref() else {
        return html! {
            <section class={classes!("detail-panel", "placeholder", props.class.clone())}>
                <h3>{t("detail.select_title")}</h3>
                <p class="muted">{t("detail.select_body")}</p>
            </section>
        };
    };

    let detail_key = props
        .data
        .as_ref()
        .map(|detail| (detail.summary.id, detail.settings.clone()));
    {
        let connections_input = connections_input.clone();
        let connections_error = connections_error.clone();
        let queue_input = queue_input.clone();
        let queue_error = queue_error.clone();
        use_effect_with_deps(
            move |detail_key| {
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
            },
            detail_key,
        );
    }

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
        <section class={classes!("detail-panel", props.class.clone())}>
            <header class="detail-header">
                <div>
                    <small class="muted">{t("detail.view_label")}</small>
                    <h3>{name.clone()}</h3>
                </div>
                <div class="pane-tabs mobile-only">
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
                            <button class={classes!("ghost", if active_state { "active" } else { "" })} onclick={onclick}>{label}</button>
                        }
                    })}
                </div>
            </header>

            <div class="detail-grid">
                <section class={pane_classes(Pane::Overview, *active)} data-pane="overview">
                    <header>
                        <h4>{t("detail.overview.title")}</h4>
                        <p class="muted">{t("detail.overview.body")}</p>
                    </header>
                    <div class="overview-actions">
                        <button class="ghost" onclick={pause}>{t("toolbar.pause")}</button>
                        <button class="ghost" onclick={resume}>{t("toolbar.resume")}</button>
                        {action_menu}
                    </div>
                    {last_error.as_ref().map(|message| html! {
                        <div class="detail-error">
                            <span class="pill error">{t("detail.overview.last_error")}</span>
                            <span>{message.clone()}</span>
                        </div>
                    }).unwrap_or_default()}
                    <div class="table-like">
                        <div class="table-row">
                            <div><strong>{t("detail.overview.status")}</strong></div>
                            <div class={classes!("pill", status_class)}>{status.clone()}</div>
                        </div>
                        <div class="table-row">
                            <div><strong>{t("detail.overview.progress")}</strong></div>
                            <div>{progress_label}</div>
                        </div>
                        <div class="table-row">
                            <div><strong>{t("detail.overview.rates")}</strong></div>
                            <div>{rates_label}</div>
                        </div>
                        <div class="table-row">
                            <div><strong>{t("detail.overview.ratio")}</strong></div>
                            <div>{ratio_label}</div>
                        </div>
                        <div class="table-row">
                            <div><strong>{t("detail.overview.category")}</strong></div>
                            <div>{category_label}</div>
                        </div>
                        <div class="table-row">
                            <div><strong>{t("detail.overview.tags")}</strong></div>
                            <div>{tags_label}</div>
                        </div>
                        <div class="table-row">
                            <div><strong>{t("detail.overview.trackers")}</strong></div>
                            <div>{trackers_label}</div>
                        </div>
                        <div class="table-row">
                            <div><strong>{t("detail.overview.save_path")}</strong></div>
                            <div>{save_path_label}</div>
                        </div>
                        <div class="table-row">
                            <div><strong>{t("detail.overview.updated")}</strong></div>
                            <div>{updated_label}</div>
                        </div>
                    </div>
                </section>

                <section class={pane_classes(Pane::Files, *active)} data-pane="files">
                    {files_tab}
                </section>

                <section class={pane_classes(Pane::Options, *active)} data-pane="options">
                    {options_tab}
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
        TorrentStateKind::Downloading | TorrentStateKind::Seeding => "ok",
        TorrentStateKind::Completed => "ok",
        TorrentStateKind::Failed => "error",
        TorrentStateKind::FetchingMetadata => "warn",
        _ => "muted",
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
    let t = |key: &str| bundle.text(key, "");
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
            <div class="file-tree">
                {for files.iter().map(|file| render_file_row(file, &on_update_selection, &bundle, detail_id))}
            </div>
        },
        _ => html! { <p class="muted">{empty_label}</p> },
    };

    html! {
        <>
            <header>
                <h4>{title}</h4>
                <p class="muted">{body}</p>
                <label class="switch">
                    <input
                        type="checkbox"
                        checked={skip_fluff}
                        aria-label={skip_fluff_label.clone()}
                        onchange={on_skip_fluff}
                    />
                    <span class="slider"></span>
                    <span class="switch-label">{skip_fluff_label}</span>
                </label>
            </header>
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
    let t = |key: &str| bundle.text(key, "");
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
        <div class="file-row">
            <div class="file-main">
                <span class="file-name">{file.path.clone()}</span>
                <div class="file-progress">
                    <span class="muted">{progress_label}</span>
                    <div class="bar" style={format!("width: {:.1}%", percent)}></div>
                </div>
            </div>
            <div class="file-actions">
                <label class="file-priority">
                    <span class="muted">{priority_label}</span>
                    <select value={priority_value(file.priority)} onchange={on_priority}>
                        <option value="skip">{t("detail.files.priority_skip")}</option>
                        <option value="low">{t("detail.files.priority_low")}</option>
                        <option value="normal">{t("detail.files.priority_normal")}</option>
                        <option value="high">{t("detail.files.priority_high")}</option>
                    </select>
                </label>
                <label class="switch">
                    <input
                        type="checkbox"
                        checked={file.selected}
                        aria-label={wanted_label}
                        onchange={on_toggle}
                    />
                    <span class="slider"></span>
                </label>
            </div>
        </div>
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
    let t = |key: &str| bundle.text(key, "");
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
                <header>
                    <h4>{title}</h4>
                    <p class="muted">{body}</p>
                </header>
                <p class="muted">{empty_label}</p>
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
            <header>
                <h4>{title}</h4>
                <p class="muted">{body}</p>
            </header>
            <div class="options-block">
                <h5>{editable_label}</h5>
                <div class="table-like">
                    <div class="table-row">
                        <div><strong>{connections_label}</strong></div>
                        <div class="option-control">
                            <input
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
                            <button class="ghost" onclick={on_connections_apply}>{apply_label.clone()}</button>
                            {if connections_error_msg.is_empty() {
                                html! {}
                            } else {
                                html! { <span class="pill warn">{connections_error_msg}</span> }
                            }}
                        </div>
                    </div>
                    <div class="table-row">
                        <div><strong>{pex_label}</strong></div>
                        <label class="switch">
                            <input type="checkbox" checked={pex_enabled} onchange={on_pex} />
                            <span class="slider"></span>
                        </label>
                    </div>
                    <div class="table-row">
                        <div><strong>{super_seeding_label}</strong></div>
                        <label class="switch">
                            <input type="checkbox" checked={super_seeding} onchange={on_super_seeding} />
                            <span class="slider"></span>
                        </label>
                    </div>
                    <div class="table-row">
                        <div><strong>{auto_managed_label}</strong></div>
                        <label class="switch">
                            <input type="checkbox" checked={auto_managed} onchange={on_auto_managed} />
                            <span class="slider"></span>
                        </label>
                    </div>
                    <div class="table-row">
                        <div><strong>{queue_position_label}</strong></div>
                        <div class="option-control">
                            <input
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
                            <button class="ghost" onclick={on_queue_apply} disabled={auto_managed}>{apply_label.clone()}</button>
                            {if queue_error_msg.is_empty() {
                                html! {}
                            } else {
                                html! { <span class="pill warn">{queue_error_msg}</span> }
                            }}
                        </div>
                    </div>
                </div>
            </div>
            <div class="options-block">
                <h5>{readonly_label}</h5>
                <div class="table-like">
                    <div class="table-row">
                        <div><strong>{category_label}</strong></div>
                        <div>{settings.category.clone().unwrap_or_else(|| unset_label.clone())}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{tags_label_title}</strong></div>
                        <div>{tags_label}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{trackers_label_title}</strong></div>
                        <div>{trackers_label}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{tracker_messages_label_title}</strong></div>
                        <div>{tracker_messages_label}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{download_dir_label_title}</strong></div>
                        <div>{download_dir_label}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{rate_limit_label_title}</strong></div>
                        <div>{rate_limit_label}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{storage_mode_label_title}</strong></div>
                        <div>{storage_mode_label}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{partfile_label_title}</strong></div>
                        <div>{partfile_label}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{sequential_label_title}</strong></div>
                        <div>{if settings.sequential { yes_label.clone() } else { no_label.clone() }}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{selection_label_title}</strong></div>
                        <div>{selection_label}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{seed_mode_label_title}</strong></div>
                        <div>{seed_mode_label}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{seed_ratio_label_title}</strong></div>
                        <div>{seed_ratio_label}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{seed_time_label_title}</strong></div>
                        <div>{seed_time_label}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{cleanup_label_title}</strong></div>
                        <div>{cleanup_label}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{private_label_title}</strong></div>
                        <div>{private_label}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{comment_label_title}</strong></div>
                        <div>{comment_label}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{source_label_title}</strong></div>
                        <div>{source_label}</div>
                    </div>
                    <div class="table-row">
                        <div><strong>{web_seeds_label_title}</strong></div>
                        <div>{web_seeds_label}</div>
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
