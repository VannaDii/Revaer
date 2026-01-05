//! Label policy views.
//!
//! # Design
//! - Keep API calls in the feature page controller.
//! - Drive rendering from the shared AppStore label caches.
//! - Use explicit form state to avoid implicit mutations.

use crate::app::api::ApiCtx;
use crate::components::atoms::EmptyState;
use crate::core::store::{AppStore, app_dispatch};
use crate::features::labels::actions::LabelAction;
use crate::features::labels::api::upsert_label;
use crate::features::labels::logic::policy_badges;
use crate::features::labels::state::{AutoManagedChoice, LabelFormState, LabelKind};
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::TorrentLabelEntry;
use yew::prelude::*;
use yewdux::prelude::use_selector;

#[derive(Properties, PartialEq)]
pub(crate) struct LabelsPageProps {
    pub kind: LabelKind,
}

#[function_component(LabelsPage)]
pub(crate) fn labels_page(props: &LabelsPageProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key);
    let api_ctx = use_context::<ApiCtx>();

    let kind = props.kind;
    let entries = use_selector(move |store: &AppStore| {
        let mut entries: Vec<TorrentLabelEntry> = match kind {
            LabelKind::Category => store.labels.categories.values().cloned().collect(),
            LabelKind::Tag => store.labels.tags.values().cloned().collect(),
        };
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        entries
    });

    let dispatch = app_dispatch();
    let selected = use_state(|| None as Option<String>);
    let form = use_state(LabelFormState::default);
    let error = use_state(|| None as Option<String>);
    let saving = use_state(|| false);
    let Some(api_ctx) = api_ctx else {
        return html! {
            <div class="card bg-base-100 border border-base-200 shadow">
                <div class="card-body">
                    <p class="text-sm text-error">{"Missing API context."}</p>
                </div>
            </div>
        };
    };

    let on_action = {
        let entries = entries.clone();
        let selected = selected.clone();
        let form = form.clone();
        let error = error.clone();
        Callback::from(move |action: LabelAction| match action {
            LabelAction::New => {
                selected.set(None);
                form.set(LabelFormState::default());
                error.set(None);
            }
            LabelAction::Select(name) => {
                if let Some(entry) = entries.iter().find(|entry| entry.name == name) {
                    selected.set(Some(entry.name.clone()));
                    form.set(LabelFormState::from_entry(entry));
                    error.set(None);
                }
            }
        })
    };

    let on_save = {
        let api_ctx = api_ctx.clone();
        let dispatch = dispatch.clone();
        let selected = selected.clone();
        let form = form.clone();
        let error = error.clone();
        let saving = saving.clone();
        Callback::from(move |_| {
            let name = form.name.trim().to_string();
            if name.is_empty() {
                error.set(Some("Name is required".to_string()));
                return;
            }
            let policy = match form.to_policy() {
                Ok(policy) => policy,
                Err(message) => {
                    error.set(Some(message));
                    return;
                }
            };
            error.set(None);
            saving.set(true);
            let client = api_ctx.client.clone();
            let dispatch = dispatch.clone();
            let selected = selected.clone();
            let form = form.clone();
            let error = error.clone();
            let saving = saving.clone();
            yew::platform::spawn_local(async move {
                match upsert_label(&client, kind, &name, &policy).await {
                    Ok(entry) => {
                        dispatch.reduce_mut(|store| match kind {
                            LabelKind::Category => {
                                store
                                    .labels
                                    .categories
                                    .insert(entry.name.clone(), entry.clone());
                            }
                            LabelKind::Tag => {
                                store.labels.tags.insert(entry.name.clone(), entry.clone());
                            }
                        });
                        selected.set(Some(entry.name.clone()));
                        form.set(LabelFormState::from_entry(&entry));
                        error.set(None);
                    }
                    Err(err) => {
                        error.set(Some(err.to_string()));
                    }
                }
                saving.set(false);
            });
        })
    };

    let title = match kind {
        LabelKind::Category => t("labels.categories_title"),
        LabelKind::Tag => t("labels.tags_title"),
    };
    let subtitle = match kind {
        LabelKind::Category => t("labels.categories_body"),
        LabelKind::Tag => t("labels.tags_body"),
    };

    let selected_label = selected
        .as_ref()
        .map(|name| format!("Editing: {name}"))
        .unwrap_or_else(|| t("labels.new_label"));

    let entries_empty = entries.is_empty();

    html! {
        <section class="space-y-6">
            <div class="card bg-base-100 shadow">
                <div class="card-body gap-6">
                    <div class="flex items-start justify-between gap-4">
                        <div class="space-y-1">
                            <p class="text-xs uppercase tracking-wide text-base-content/60">{title.clone()}</p>
                            <h3 class="text-lg font-semibold">{title}</h3>
                            <p class="text-sm text-base-content/60">{subtitle}</p>
                        </div>
                        <button
                            class="btn btn-ghost btn-sm"
                            type="button"
                            onclick={{
                                let on_action = on_action.clone();
                                Callback::from(move |_| on_action.emit(LabelAction::New))
                            }}
                        >
                            {t("labels.new")}
                        </button>
                    </div>
                    <div class="grid gap-6 lg:grid-cols-2">
                        <div class="space-y-3">
                            <div class="flex items-center justify-between">
                                <strong class="text-sm">{t("labels.list_title")}</strong>
                                <span class="badge badge-ghost badge-sm">{entries.len()}</span>
                            </div>
                            {if entries_empty {
                                html! {
                                    <EmptyState
                                        title={AttrValue::from(t("labels.empty"))}
                                        class="bg-base-200"
                                    />
                                }
                            } else {
                                html! {
                                    <ul class="menu bg-base-200 rounded-box p-1">
                                        {for entries.iter().map(|entry| {
                                            let name = entry.name.clone();
                                            let is_selected = selected.as_ref().is_some_and(|sel| sel == &entry.name);
                                            let badges = policy_badges(&entry.policy);
                                            let on_click = {
                                                let on_action = on_action.clone();
                                                let name = name.clone();
                                                Callback::from(move |_| on_action.emit(LabelAction::Select(name.clone())))
                                            };
                                            html! {
                                                <li>
                                                    <button
                                                        type="button"
                                                        class={classes!(
                                                            "flex",
                                                            "items-start",
                                                            "justify-between",
                                                            "gap-3",
                                                            is_selected.then_some("active")
                                                        )}
                                                        onclick={on_click}
                                                    >
                                                        <div class="min-w-0 text-left">
                                                            <strong class="truncate">{name}</strong>
                                                            <div class="mt-1 flex flex-wrap gap-1">
                                                                {for badges.into_iter().map(|badge| html! {
                                                                    <span class="badge badge-ghost badge-xs">{badge}</span>
                                                                })}
                                                            </div>
                                                        </div>
                                                        <span class="text-base-content/40">{"›"}</span>
                                                    </button>
                                                </li>
                                            }
                                        })}
                                    </ul>
                                }
                            }}
                        </div>
                        <div class="rounded-box border border-base-200 bg-base-200/40 p-4 space-y-4">
                            <div class="flex items-center justify-between">
                                <strong class="text-sm">{selected_label}</strong>
                                <span class="badge badge-ghost badge-sm">{kind.singular()}</span>
                            </div>
                            <div class="grid gap-3">
                                <label class="form-control gap-1">
                                    <span class="label-text text-xs">{t("labels.name")}</span>
                                    <input
                                        class="input input-bordered input-sm w-full"
                                        type="text"
                                        placeholder={t("labels.name_placeholder")}
                                        value={form.name.clone()}
                                        oninput={{
                                            let form = form.clone();
                                            Callback::from(move |event: InputEvent| {
                                                if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                    update_form_state(&form, |state| state.name = input.value());
                                                }
                                            })
                                        }}
                                    />
                                </label>
                                <label class="form-control gap-1">
                                    <span class="label-text text-xs">{t("labels.download_dir")}</span>
                                    <input
                                        class="input input-bordered input-sm w-full"
                                        type="text"
                                        placeholder={t("labels.download_dir_placeholder")}
                                        value={form.download_dir.clone()}
                                        oninput={{
                                            let form = form.clone();
                                            Callback::from(move |event: InputEvent| {
                                                if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                    update_form_state(&form, |state| state.download_dir = input.value());
                                                }
                                            })
                                        }}
                                    />
                                </label>
                            </div>
                            <details class="collapse collapse-arrow bg-base-200" open={false}>
                                <summary class="collapse-title text-sm font-medium">{t("labels.advanced")}</summary>
                                <div class="collapse-content space-y-4">
                                    <div class="grid gap-3">
                                        <label class="form-control gap-1">
                                            <span class="label-text text-xs">{t("labels.rate_limit_down")}</span>
                                            <input
                                                class="input input-bordered input-sm w-full"
                                                type="number"
                                                min="0"
                                                placeholder={t("labels.rate_limit_down_placeholder")}
                                                value={form.rate_limit_download.clone()}
                                                oninput={{
                                                    let form = form.clone();
                                                    Callback::from(move |event: InputEvent| {
                                                        if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                            update_form_state(&form, |state| state.rate_limit_download = input.value());
                                                        }
                                                    })
                                                }}
                                            />
                                        </label>
                                        <label class="form-control gap-1">
                                            <span class="label-text text-xs">{t("labels.rate_limit_up")}</span>
                                            <input
                                                class="input input-bordered input-sm w-full"
                                                type="number"
                                                min="0"
                                                placeholder={t("labels.rate_limit_up_placeholder")}
                                                value={form.rate_limit_upload.clone()}
                                                oninput={{
                                                    let form = form.clone();
                                                    Callback::from(move |event: InputEvent| {
                                                        if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                            update_form_state(&form, |state| state.rate_limit_upload = input.value());
                                                        }
                                                    })
                                                }}
                                            />
                                        </label>
                                        <label class="form-control gap-1">
                                            <span class="label-text text-xs">{t("labels.queue_position")}</span>
                                            <input
                                                class="input input-bordered input-sm w-full"
                                                type="number"
                                                min="0"
                                                placeholder={t("labels.queue_position_placeholder")}
                                                value={form.queue_position.clone()}
                                                oninput={{
                                                    let form = form.clone();
                                                    Callback::from(move |event: InputEvent| {
                                                        if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                            update_form_state(&form, |state| state.queue_position = input.value());
                                                        }
                                                    })
                                                }}
                                            />
                                        </label>
                                        <label class="form-control gap-1">
                                            <span class="label-text text-xs">{t("labels.auto_managed")}</span>
                                            <select
                                                class="select select-bordered select-sm w-full"
                                                value={form.auto_managed.as_value()}
                                                onchange={{
                                                    let form = form.clone();
                                                    Callback::from(move |event: Event| {
                                                        if let Some(select) = event.target_dyn_into::<web_sys::HtmlSelectElement>() {
                                                            let value = select.value();
                                                            let next = AutoManagedChoice::from_value(&value);
                                                            update_form_state(&form, |state| state.auto_managed = next);
                                                        }
                                                    })
                                                }}
                                            >
                                                <option value="default">{t("labels.auto_default")}</option>
                                                <option value="enabled">{t("labels.auto_enabled")}</option>
                                                <option value="disabled">{t("labels.auto_disabled")}</option>
                                            </select>
                                        </label>
                                        <label class="form-control gap-1">
                                            <span class="label-text text-xs">{t("labels.seed_ratio")}</span>
                                            <input
                                                class="input input-bordered input-sm w-full"
                                                type="number"
                                                min="0"
                                                step="0.01"
                                                placeholder={t("labels.seed_ratio_placeholder")}
                                                value={form.seed_ratio_limit.clone()}
                                                oninput={{
                                                    let form = form.clone();
                                                    Callback::from(move |event: InputEvent| {
                                                        if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                            update_form_state(&form, |state| state.seed_ratio_limit = input.value());
                                                        }
                                                    })
                                                }}
                                            />
                                        </label>
                                        <label class="form-control gap-1">
                                            <span class="label-text text-xs">{t("labels.seed_time")}</span>
                                            <input
                                                class="input input-bordered input-sm w-full"
                                                type="number"
                                                min="0"
                                                placeholder={t("labels.seed_time_placeholder")}
                                                value={form.seed_time_limit.clone()}
                                                oninput={{
                                                    let form = form.clone();
                                                    Callback::from(move |event: InputEvent| {
                                                        if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                            update_form_state(&form, |state| state.seed_time_limit = input.value());
                                                        }
                                                    })
                                                }}
                                            />
                                        </label>
                                    </div>
                                    <div class="space-y-2">
                                        <div class="flex items-start justify-between gap-2">
                                            <strong class="text-sm">{t("labels.cleanup_title")}</strong>
                                            <span class="text-xs text-base-content/60">{t("labels.cleanup_hint")}</span>
                                        </div>
                                        <div class="grid gap-3">
                                            <label class="form-control gap-1">
                                                <span class="label-text text-xs">{t("labels.cleanup_seed_ratio")}</span>
                                                <input
                                                    class="input input-bordered input-sm w-full"
                                                    type="number"
                                                    min="0"
                                                    step="0.01"
                                                    placeholder={t("labels.cleanup_seed_ratio_placeholder")}
                                                    value={form.cleanup_seed_ratio_limit.clone()}
                                                    oninput={{
                                                        let form = form.clone();
                                                        Callback::from(move |event: InputEvent| {
                                                            if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                                update_form_state(&form, |state| state.cleanup_seed_ratio_limit = input.value());
                                                            }
                                                        })
                                                    }}
                                                />
                                            </label>
                                            <label class="form-control gap-1">
                                                <span class="label-text text-xs">{t("labels.cleanup_seed_time")}</span>
                                                <input
                                                    class="input input-bordered input-sm w-full"
                                                    type="number"
                                                    min="0"
                                                    placeholder={t("labels.cleanup_seed_time_placeholder")}
                                                    value={form.cleanup_seed_time_limit.clone()}
                                                    oninput={{
                                                        let form = form.clone();
                                                        Callback::from(move |event: InputEvent| {
                                                            if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                                update_form_state(&form, |state| state.cleanup_seed_time_limit = input.value());
                                                            }
                                                        })
                                                    }}
                                                />
                                            </label>
                                            <label class="label cursor-pointer justify-between">
                                                <span class="label-text text-sm">{t("labels.cleanup_remove_data")}</span>
                                                <input
                                                    class="toggle toggle-sm"
                                                    type="checkbox"
                                                    checked={form.cleanup_remove_data}
                                                    onchange={{
                                                        let form = form.clone();
                                                        Callback::from(move |event: Event| {
                                                            if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                                let checked = input.checked();
                                                                update_form_state(&form, |state| state.cleanup_remove_data = checked);
                                                            }
                                                        })
                                                    }}
                                                />
                                            </label>
                                        </div>
                                    </div>
                                </div>
                            </details>
                            {if let Some(message) = error.as_ref() {
                                html! { <p class="text-sm text-error">{message.clone()}</p> }
                            } else { html! {} }}
                            <div class="flex justify-end">
                                <button class="btn btn-primary btn-sm" onclick={on_save} disabled={*saving}>
                                    {if *saving { t("labels.saving") } else { t("labels.save") }}
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </section>
    }
}

fn update_form_state(
    form: &UseStateHandle<LabelFormState>,
    update: impl FnOnce(&mut LabelFormState),
) {
    let mut next = (**form).clone();
    update(&mut next);
    form.set(next);
}
