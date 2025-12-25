//! Torrent add/create modal panels.
//!
//! # Design
//! - Keep side effects out of these components; they emit typed requests via callbacks.
//! - Local form state stays inside the modal panels to avoid polluting shared store slices.
//! - Validation errors are surfaced inline with clear, localized copy.

use crate::app::Route;
use crate::core::logic::{AddInputError, build_add_payload, format_bytes};
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::{AddTorrentInput, TorrentAuthorRequest, TorrentAuthorResponse};
use wasm_bindgen::JsCast;
use web_sys::{DragEvent, Event, File, HtmlInputElement};
use yew::prelude::*;
use yew_router::prelude::Link;

/// Copy actions supported by the create-torrent result panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CopyKind {
    /// Copy the magnet URI.
    Magnet,
    /// Copy the base64-encoded metainfo.
    Metainfo,
}

#[derive(Properties, PartialEq)]
pub(crate) struct AddTorrentProps {
    pub on_submit: Callback<AddTorrentInput>,
    pub pending: bool,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(AddTorrentPanel)]
pub(crate) fn add_torrent_panel(props: &AddTorrentProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
    let bundle_for_submit = bundle.clone();
    let bundle_for_drop = bundle.clone();
    let input_value = use_state(String::new);
    let category = use_state(String::new);
    let tags = use_state(String::new);
    let save_path = use_state(String::new);
    let download_limit = use_state(String::new);
    let upload_limit = use_state(String::new);
    let file = use_state(|| None as Option<File>);
    let error = use_state(|| None as Option<String>);
    let drag_over = use_state(|| false);
    let file_input = use_node_ref();

    let submit = {
        let input_value = input_value.clone();
        let category = category.clone();
        let tags = tags.clone();
        let save_path = save_path.clone();
        let download_limit = download_limit.clone();
        let upload_limit = upload_limit.clone();
        let file = file.clone();
        let error = error.clone();
        let on_submit = props.on_submit.clone();
        let bundle = bundle_for_submit.clone();
        Callback::from(move |_| {
            let value = input_value.trim().to_string();
            let has_file = (*file).is_some();
            let payload = match build_add_payload(
                &value,
                category.as_str(),
                tags.as_str(),
                save_path.as_str(),
                download_limit.as_str(),
                upload_limit.as_str(),
                has_file,
            ) {
                Ok(payload) => {
                    error.set(None);
                    payload
                }
                Err(AddInputError::Empty) => {
                    error.set(Some(bundle.text("torrents.error.empty", "")));
                    return;
                }
                Err(AddInputError::Invalid) => {
                    error.set(Some(bundle.text("torrents.error.invalid", "")));
                    return;
                }
                Err(AddInputError::RateInvalid) => {
                    error.set(Some(bundle.text("torrents.rate_invalid", "")));
                    return;
                }
            };
            on_submit.emit(AddTorrentInput {
                value: payload.value,
                file: (*file).clone(),
                category: payload.category,
                tags: payload.tags,
                save_path: payload.save_path,
                max_download_bps: payload.max_download_bps,
                max_upload_bps: payload.max_upload_bps,
            });
            input_value.set(String::new());
            category.set(String::new());
            tags.set(String::new());
            save_path.set(String::new());
            download_limit.set(String::new());
            upload_limit.set(String::new());
            file.set(None);
        })
    };

    let on_input = {
        let input_value = input_value.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
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
                let Some(file) = files.get(0) else {
                    return;
                };
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

    let on_pick_file = {
        let file_input = file_input.clone();
        Callback::from(move |_| {
            if let Some(input) = file_input.cast::<HtmlInputElement>() {
                input.click();
            }
        })
    };

    let on_file_change = {
        let file_state = file.clone();
        let input_value = input_value.clone();
        let error = error.clone();
        let bundle = bundle.clone();
        Callback::from(move |event: Event| {
            let Some(input) = event
                .target()
                .and_then(|node| node.dyn_into::<HtmlInputElement>().ok())
            else {
                return;
            };
            let Some(files) = input.files() else {
                return;
            };
            if files.length() == 0 {
                return;
            }
            let Some(file) = files.get(0) else {
                return;
            };
            let name = file.name();
            if !name.ends_with(".torrent") {
                error.set(Some(bundle.text("torrents.error.file_type", "")));
            } else {
                error.set(None);
                file_state.set(Some(file));
                input_value.set(name);
            }
        })
    };

    html! {
        <div class={classes!("add-panel", props.class.clone())}>
            <input
                ref={file_input}
                class="file-input-hidden"
                type="file"
                accept=".torrent"
                onchange={on_file_change}
            />
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
                    <input
                        aria-label={t("torrents.add_placeholder")}
                        placeholder={t("torrents.add_placeholder")}
                        value={(*input_value).clone()}
                        oninput={on_input}
                    />
                    <button class="ghost" type="button" onclick={on_pick_file}>
                        {t("torrents.browse_file")}
                    </button>
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
                    <input
                        placeholder={t("torrents.category_placeholder")}
                        value={(*category).clone()}
                        oninput={{
                            let category = category.clone();
                            Callback::from(move |e: InputEvent| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    category.set(input.value());
                                }
                            })
                        }}
                    />
                </label>
                <label>
                    <span>{t("torrents.tags")}</span>
                    <input
                        placeholder={t("torrents.tags_placeholder")}
                        value={(*tags).clone()}
                        oninput={{
                            let tags = tags.clone();
                            Callback::from(move |e: InputEvent| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    tags.set(input.value());
                                }
                            })
                        }}
                    />
                </label>
                <label>
                    <span>{t("torrents.save_path")}</span>
                    <input
                        placeholder={t("torrents.save_path_placeholder")}
                        value={(*save_path).clone()}
                        oninput={{
                            let save_path = save_path.clone();
                            Callback::from(move |e: InputEvent| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    save_path.set(input.value());
                                }
                            })
                        }}
                    />
                </label>
                <label>
                    <span>{t("torrents.rate_download")}</span>
                    <input
                        placeholder={t("torrents.rate_placeholder")}
                        value={(*download_limit).clone()}
                        oninput={{
                            let download_limit = download_limit.clone();
                            Callback::from(move |e: InputEvent| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    download_limit.set(input.value());
                                }
                            })
                        }}
                    />
                </label>
                <label>
                    <span>{t("torrents.rate_upload")}</span>
                    <input
                        placeholder={t("torrents.rate_placeholder")}
                        value={(*upload_limit).clone()}
                        oninput={{
                            let upload_limit = upload_limit.clone();
                            Callback::from(move |e: InputEvent| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    upload_limit.set(input.value());
                                }
                            })
                        }}
                    />
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

#[derive(Properties, PartialEq)]
pub(crate) struct CreateTorrentProps {
    pub on_submit: Callback<TorrentAuthorRequest>,
    pub on_copy: Callback<(CopyKind, String)>,
    pub pending: bool,
    pub result: Option<TorrentAuthorResponse>,
    pub error: Option<String>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(CreateTorrentPanel)]
pub(crate) fn create_torrent_panel(props: &CreateTorrentProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
    let root_path = use_state(String::new);
    let trackers = use_state(String::new);
    let web_seeds = use_state(String::new);
    let include = use_state(String::new);
    let exclude = use_state(String::new);
    let piece_length = use_state(String::new);
    let private = use_state(|| false);
    let skip_fluff = use_state(|| false);
    let comment = use_state(String::new);
    let source = use_state(String::new);
    let local_error = use_state(|| None as Option<String>);

    let submit = {
        let root_path = root_path.clone();
        let trackers = trackers.clone();
        let web_seeds = web_seeds.clone();
        let include = include.clone();
        let exclude = exclude.clone();
        let piece_length = piece_length.clone();
        let private = private.clone();
        let skip_fluff = skip_fluff.clone();
        let comment = comment.clone();
        let source = source.clone();
        let on_submit = props.on_submit.clone();
        let local_error = local_error.clone();
        let bundle = bundle.clone();
        Callback::from(move |_| {
            let root = root_path.trim().to_string();
            if root.is_empty() {
                local_error.set(Some(bundle.text("torrents.create_error_root", "")));
                return;
            }
            let piece_length_value = if piece_length.trim().is_empty() {
                None
            } else if let Ok(value) = piece_length.trim().parse::<u32>() {
                if value == 0 {
                    local_error.set(Some(bundle.text("torrents.create_error_piece", "")));
                    return;
                }
                Some(value)
            } else {
                local_error.set(Some(bundle.text("torrents.create_error_piece", "")));
                return;
            };
            let request = TorrentAuthorRequest {
                root_path: root,
                trackers: parse_list(&trackers),
                web_seeds: parse_list(&web_seeds),
                include: parse_list(&include),
                exclude: parse_list(&exclude),
                skip_fluff: *skip_fluff,
                piece_length: piece_length_value,
                private: *private,
                comment: optional_string(&comment),
                source: optional_string(&source),
            };
            local_error.set(None);
            on_submit.emit(request);
        })
    };

    let error_message = local_error
        .as_ref()
        .cloned()
        .or_else(|| props.error.clone());

    html! {
        <div class={classes!("create-panel", props.class.clone())}>
            <div class="create-form">
                <label>
                    <span>{t("torrents.create_root_label")}</span>
                    <input
                        placeholder={t("torrents.create_root_placeholder")}
                        value={(*root_path).clone()}
                        oninput={{
                            let root_path = root_path.clone();
                            Callback::from(move |e: InputEvent| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    root_path.set(input.value());
                                }
                            })
                        }}
                    />
                </label>
                <label>
                    <span>{t("torrents.create_trackers")}</span>
                    <textarea
                        rows="2"
                        placeholder={t("torrents.create_trackers_placeholder")}
                        value={(*trackers).clone()}
                        oninput={{
                            let trackers = trackers.clone();
                            Callback::from(move |e: InputEvent| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    trackers.set(input.value());
                                }
                            })
                        }}
                    />
                </label>
                <label>
                    <span>{t("torrents.create_web_seeds")}</span>
                    <textarea
                        rows="2"
                        placeholder={t("torrents.create_web_seeds_placeholder")}
                        value={(*web_seeds).clone()}
                        oninput={{
                            let web_seeds = web_seeds.clone();
                            Callback::from(move |e: InputEvent| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    web_seeds.set(input.value());
                                }
                            })
                        }}
                    />
                </label>
                <label>
                    <span>{t("torrents.create_include")}</span>
                    <textarea
                        rows="2"
                        placeholder={t("torrents.create_include_placeholder")}
                        value={(*include).clone()}
                        oninput={{
                            let include = include.clone();
                            Callback::from(move |e: InputEvent| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    include.set(input.value());
                                }
                            })
                        }}
                    />
                </label>
                <label>
                    <span>{t("torrents.create_exclude")}</span>
                    <textarea
                        rows="2"
                        placeholder={t("torrents.create_exclude_placeholder")}
                        value={(*exclude).clone()}
                        oninput={{
                            let exclude = exclude.clone();
                            Callback::from(move |e: InputEvent| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    exclude.set(input.value());
                                }
                            })
                        }}
                    />
                </label>
                <label class="inline-toggle">
                    <input
                        type="checkbox"
                        checked={*skip_fluff}
                        onchange={{
                            let skip_fluff = skip_fluff.clone();
                            Callback::from(move |e: Event| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    skip_fluff.set(input.checked());
                                }
                            })
                        }}
                    />
                    <span>{t("torrents.create_skip_fluff")}</span>
                </label>
                <label>
                    <span>{t("torrents.create_piece_length")}</span>
                    <input
                        placeholder={t("torrents.create_piece_placeholder")}
                        value={(*piece_length).clone()}
                        oninput={{
                            let piece_length = piece_length.clone();
                            Callback::from(move |e: InputEvent| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    piece_length.set(input.value());
                                }
                            })
                        }}
                    />
                </label>
                <label class="inline-toggle">
                    <input
                        type="checkbox"
                        checked={*private}
                        onchange={{
                            let private = private.clone();
                            Callback::from(move |e: Event| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    private.set(input.checked());
                                }
                            })
                        }}
                    />
                    <span>{t("torrents.create_private")}</span>
                </label>
                <label>
                    <span>{t("torrents.create_comment")}</span>
                    <input
                        placeholder={t("torrents.create_comment_placeholder")}
                        value={(*comment).clone()}
                        oninput={{
                            let comment = comment.clone();
                            Callback::from(move |e: InputEvent| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    comment.set(input.value());
                                }
                            })
                        }}
                    />
                </label>
                <label>
                    <span>{t("torrents.create_source")}</span>
                    <input
                        placeholder={t("torrents.create_source_placeholder")}
                        value={(*source).clone()}
                        oninput={{
                            let source = source.clone();
                            Callback::from(move |e: InputEvent| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    source.set(input.value());
                                }
                            })
                        }}
                    />
                </label>
                {if let Some(message) = error_message {
                    html! { <p class="error-text">{message}</p> }
                } else { html! {} }}
                <div class="create-actions">
                    <button class="solid" onclick={submit} disabled={props.pending}>
                        {if props.pending { t("torrents.create_pending") } else { t("torrents.create_submit") }}
                    </button>
                </div>
            </div>
            {if let Some(result) = props.result.as_ref() {
                render_create_result(&bundle, result, &props.on_copy)
            } else {
                html! {}
            }}
        </div>
    }
}

fn parse_list(value: &str) -> Vec<String> {
    value
        .split([',', '\n'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn render_create_result(
    bundle: &TranslationBundle,
    result: &TorrentAuthorResponse,
    on_copy: &Callback<(CopyKind, String)>,
) -> Html {
    let copy_magnet = {
        let on_copy = on_copy.clone();
        let magnet = result.magnet_uri.clone();
        Callback::from(move |_| on_copy.emit((CopyKind::Magnet, magnet.clone())))
    };
    let copy_metainfo = {
        let on_copy = on_copy.clone();
        let metainfo = result.metainfo.clone();
        Callback::from(move |_| on_copy.emit((CopyKind::Metainfo, metainfo.clone())))
    };
    let warnings = if result.warnings.is_empty() {
        None
    } else {
        Some(result.warnings.clone())
    };
    html! {
        <div class="create-result">
            <h4>{bundle.text("torrents.create_result_title", "Created torrent")}</h4>
            <div class="result-row">
                <div class="result-label">{bundle.text("torrents.create_result_magnet", "Magnet URI")}</div>
                <div class="result-actions">
                    <button class="ghost" onclick={copy_magnet}>{bundle.text("torrents.copy_magnet", "Copy magnet")}</button>
                </div>
                <textarea readonly={true} rows="2" value={result.magnet_uri.clone()} />
            </div>
            <div class="result-row">
                <div class="result-label">{bundle.text("torrents.create_result_metainfo", "Metainfo (b64)")}</div>
                <div class="result-actions">
                    <button class="ghost" onclick={copy_metainfo}>{bundle.text("torrents.copy_metainfo", "Copy metainfo")}</button>
                </div>
                <textarea readonly={true} rows="3" value={result.metainfo.clone()} />
            </div>
            <div class="result-grid">
                <div>
                    <span class="muted">{bundle.text("torrents.create_result_hash", "Info hash")}</span>
                    <p class="mono">{result.info_hash.clone()}</p>
                </div>
                <div>
                    <span class="muted">{bundle.text("torrents.create_result_piece", "Piece length")}</span>
                    <p>{format_bytes(u64::from(result.piece_length))}</p>
                </div>
                <div>
                    <span class="muted">{bundle.text("torrents.create_result_size", "Total size")}</span>
                    <p>{format_bytes(result.total_size)}</p>
                </div>
                <div>
                    <span class="muted">{bundle.text("torrents.create_result_files", "Files")}</span>
                    <p>{result.files.len()}</p>
                </div>
            </div>
            {if let Some(warnings) = warnings {
                html! {
                    <div class="result-warnings">
                        <span class="muted">{bundle.text("torrents.create_result_warnings", "Warnings")}</span>
                        <ul>
                            {for warnings.into_iter().map(|warn| html! { <li>{warn}</li> })}
                        </ul>
                    </div>
                }
            } else { html! {} }}
        </div>
    }
}
