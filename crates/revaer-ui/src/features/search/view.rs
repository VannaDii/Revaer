//! Manual search page.
//!
//! # Design
//! - Submit parameterized search requests through the existing indexer search API.
//! - Keep transient UI state local to the feature instead of coupling it to global stores.
//! - Prefer explicit refresh and bulk-add actions over hidden background side effects.

use crate::app::api::ApiCtx;
use crate::components::atoms::EmptyState;
use crate::features::search::api::{
    add_selected_results, fetch_search_page, fetch_search_pages, submit_search,
};
use crate::features::search::logic::{format_size, result_meta, selection_key};
use crate::features::search::state::{SearchFormState, SearchRunState};
use crate::models::{SearchPageItemResponse, SearchRequestExplainabilityResponse};
use yew::platform::spawn_local;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct SearchPageProps {
    pub on_success_toast: Callback<String>,
    pub on_error_toast: Callback<String>,
}

#[function_component(SearchPage)]
pub(crate) fn search_page(props: &SearchPageProps) -> Html {
    let api = use_context::<ApiCtx>();
    let form = use_state(SearchFormState::with_defaults);
    let run = use_state(|| None::<SearchRunState>);
    let submit_busy = use_state(|| false);
    let refresh_busy = use_state(|| false);
    let add_busy = use_state(|| false);
    let error = use_state(|| None::<String>);
    let status = use_state(|| None::<String>);

    let on_query_text = input_callback(form.clone(), set_query_text);
    let on_query_type = select_callback(form.clone(), set_query_type);
    let on_torznab_mode = select_callback(form.clone(), set_torznab_mode);
    let on_media_domain = input_callback(form.clone(), set_media_domain);
    let on_page_size = input_callback(form.clone(), set_page_size);
    let on_season = input_callback(form.clone(), set_season_number);
    let on_episode = input_callback(form.clone(), set_episode_number);
    let on_identifier_types = input_callback(form.clone(), set_identifier_types);
    let on_identifier_values = input_callback(form.clone(), set_identifier_values);
    let on_torznab_cat_ids = input_callback(form.clone(), set_torznab_cat_ids);

    let on_submit = {
        let api = api.clone();
        let form = form.clone();
        let run = run.clone();
        let submit_busy = submit_busy.clone();
        let error = error.clone();
        let status = status.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                error.set(Some("Search API context is unavailable".to_string()));
                return;
            };
            submit_busy.set(true);
            error.set(None);
            status.set(None);
            let form_snapshot = (*form).clone();
            let run = run.clone();
            let submit_busy = submit_busy.clone();
            let error = error.clone();
            let status = status.clone();
            spawn_local(async move {
                let response = submit_search(&api.client, &form_snapshot).await;
                match response {
                    Ok(create) => {
                        let search_request_public_id = create.search_request_public_id;
                        let mut next_state = SearchRunState::new(create);
                        match fetch_search_pages(&api.client, search_request_public_id).await {
                            Ok(pages) => {
                                let first_page_number =
                                    pages.pages.first().map(|page| page.page_number);
                                next_state.pages = pages;
                                next_state.selected_page_number = first_page_number;
                                if let Some(page_number) = first_page_number {
                                    match fetch_search_page(
                                        &api.client,
                                        search_request_public_id,
                                        page_number,
                                    )
                                    .await
                                    {
                                        Ok(page) => {
                                            next_state.current_page = Some(page);
                                        }
                                        Err(fetch_error) => {
                                            error.set(Some(fetch_error));
                                        }
                                    }
                                }
                                status.set(Some(
                                    "Search request submitted. Refresh to pick up more sealed pages."
                                        .to_string(),
                                ));
                                run.set(Some(next_state));
                            }
                            Err(fetch_error) => {
                                error.set(Some(fetch_error));
                            }
                        }
                    }
                    Err(submit_error) => {
                        error.set(Some(submit_error));
                    }
                }
                submit_busy.set(false);
            });
        })
    };

    let on_refresh = {
        let api = api.clone();
        let run = run.clone();
        let refresh_busy = refresh_busy.clone();
        let error = error.clone();
        let status = status.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                error.set(Some("Search API context is unavailable".to_string()));
                return;
            };
            let Some(current_run) = (*run).clone() else {
                return;
            };
            refresh_busy.set(true);
            error.set(None);
            let run = run.clone();
            let refresh_busy = refresh_busy.clone();
            let error = error.clone();
            let status = status.clone();
            spawn_local(async move {
                match fetch_search_pages(&api.client, current_run.search_request_public_id).await {
                    Ok(pages) => {
                        let page_number = current_run
                            .selected_page_number
                            .or_else(|| pages.pages.first().map(|page| page.page_number));
                        let mut next_state = current_run.clone();
                        next_state.pages = pages;
                        next_state.selected_page_number = page_number;
                        if let Some(selected_page_number) = page_number {
                            match fetch_search_page(
                                &api.client,
                                current_run.search_request_public_id,
                                selected_page_number,
                            )
                            .await
                            {
                                Ok(page) => {
                                    next_state.current_page = Some(page);
                                }
                                Err(fetch_error) => {
                                    error.set(Some(fetch_error));
                                }
                            }
                        }
                        status.set(Some("Search results refreshed.".to_string()));
                        run.set(Some(next_state));
                    }
                    Err(fetch_error) => {
                        error.set(Some(fetch_error));
                    }
                }
                refresh_busy.set(false);
            });
        })
    };

    let on_page_select = {
        let api = api.clone();
        let run = run.clone();
        let refresh_busy = refresh_busy.clone();
        let error = error.clone();
        Callback::from(move |page_number: i32| {
            let Some(api) = api.clone() else {
                error.set(Some("Search API context is unavailable".to_string()));
                return;
            };
            let Some(current_run) = (*run).clone() else {
                return;
            };
            refresh_busy.set(true);
            error.set(None);
            let run = run.clone();
            let refresh_busy = refresh_busy.clone();
            let error = error.clone();
            spawn_local(async move {
                match fetch_search_page(
                    &api.client,
                    current_run.search_request_public_id,
                    page_number,
                )
                .await
                {
                    Ok(page) => {
                        let mut next_state = current_run;
                        next_state.current_page = Some(page);
                        next_state.selected_page_number = Some(page_number);
                        next_state.selected_result_keys.clear();
                        run.set(Some(next_state));
                    }
                    Err(fetch_error) => error.set(Some(fetch_error)),
                }
                refresh_busy.set(false);
            });
        })
    };

    let on_toggle_result = {
        let run = run.clone();
        Callback::from(move |key: String| {
            let Some(mut current_run) = (*run).clone() else {
                return;
            };
            if !current_run.selected_result_keys.insert(key.clone()) {
                let removed = current_run.selected_result_keys.remove(&key);
                debug_assert!(removed);
            }
            run.set(Some(current_run));
        })
    };

    let on_toggle_all = {
        let run = run.clone();
        Callback::from(move |_event: Event| {
            let Some(mut current_run) = (*run).clone() else {
                return;
            };
            let Some(current_page) = current_run.current_page.as_ref() else {
                return;
            };
            let visible_keys = current_page
                .items
                .iter()
                .map(selection_key)
                .collect::<Vec<String>>();
            let all_selected = visible_keys
                .iter()
                .all(|key| current_run.selected_result_keys.contains(key));
            if all_selected {
                for key in visible_keys {
                    let removed = current_run.selected_result_keys.remove(&key);
                    debug_assert!(removed);
                }
            } else {
                current_run.selected_result_keys.extend(visible_keys);
            }
            run.set(Some(current_run));
        })
    };

    let on_add_selected = {
        let api = api.clone();
        let run = run.clone();
        let add_busy = add_busy.clone();
        let error = error.clone();
        let status = status.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                error.set(Some("Search API context is unavailable".to_string()));
                return;
            };
            let Some(current_run) = (*run).clone() else {
                return;
            };
            let Some(current_page) = current_run.current_page.clone() else {
                return;
            };
            let selected_items = current_page
                .items
                .into_iter()
                .filter(|item| {
                    current_run
                        .selected_result_keys
                        .contains(&selection_key(item))
                })
                .collect::<Vec<SearchPageItemResponse>>();
            add_busy.set(true);
            let run = run.clone();
            let add_busy = add_busy.clone();
            let error = error.clone();
            let status = status.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            spawn_local(async move {
                match add_selected_results(&api.client, &selected_items).await {
                    Ok(count) => {
                        let mut next_run = current_run;
                        next_run.selected_result_keys.clear();
                        run.set(Some(next_run));
                        let message = format!("Added {count} result(s) to the download client.");
                        status.set(Some(message.clone()));
                        on_success_toast.emit(message);
                    }
                    Err(add_error) => {
                        error.set(Some(add_error.clone()));
                        on_error_toast.emit(add_error);
                    }
                }
                add_busy.set(false);
            });
        })
    };

    let query_type = form.query_type.clone();
    let torznab_mode = form.torznab_mode.clone();
    let can_submit = !*submit_busy && !query_type.trim().is_empty();
    let current_run = (*run).clone();
    let current_page = current_run
        .as_ref()
        .and_then(|state| state.current_page.as_ref());
    let selected_count = current_run
        .as_ref()
        .map(|state| state.selected_result_keys.len())
        .unwrap_or_default();
    let all_selected = current_run
        .as_ref()
        .and_then(|state| {
            state.current_page.as_ref().map(|page| {
                !page.items.is_empty()
                    && page
                        .items
                        .iter()
                        .all(|item| state.selected_result_keys.contains(&selection_key(item)))
            })
        })
        .unwrap_or(false);

    let page_list = render_page_list(current_run.as_ref(), on_page_select, *refresh_busy);
    let results = render_results(ResultsSectionProps {
        current_page,
        run: current_run.as_ref(),
        on_toggle_result,
        on_toggle_all,
        all_selected,
        add_busy: *add_busy,
        selected_count,
        on_add_selected,
    });

    html! {
        <section class="space-y-6">
            <div class="card border border-base-200 bg-base-100 shadow-sm">
                <div class="card-body gap-5">
                    <div class="flex flex-wrap items-start justify-between gap-3">
                        <div class="space-y-1">
                            <h2 class="text-xl font-semibold">{"Manual Search"}</h2>
                            <p class="text-sm text-base-content/60">
                                {"Run an ERD-backed search request, filter by Torznab categories, and push selected results into the active download client."}
                            </p>
                        </div>
                        <div class="flex gap-2">
                            <button class="btn btn-outline btn-sm" disabled={current_run.is_none() || *refresh_busy} onclick={on_refresh}>
                                {if *refresh_busy { "Refreshing..." } else { "Refresh pages" }}
                            </button>
                            <button class="btn btn-primary btn-sm" disabled={!can_submit} onclick={on_submit}>
                                {if *submit_busy { "Submitting..." } else { "Search" }}
                            </button>
                        </div>
                    </div>
                    <div class="grid gap-4 xl:grid-cols-4 md:grid-cols-2">
                        {render_text_field("Query text", &form.query_text, on_query_text, "Movie title, release name, or free text")}
                        {render_select_field(
                            "Query type",
                            &query_type,
                            on_query_type,
                            &[("free_text", "Free text"), ("imdb", "IMDb"), ("tmdb", "TMDb"), ("tvdb", "TVDb"), ("season_episode", "Season/Episode")],
                        )}
                        {render_select_field(
                            "Torznab mode",
                            &torznab_mode,
                            on_torznab_mode,
                            &[("", "Any"), ("generic", "Generic"), ("movie", "Movie"), ("tv", "TV")],
                        )}
                        {render_text_field("Media domain", &form.requested_media_domain_key, on_media_domain, "movies, tv, audiobooks")}
                        {render_text_field("Page size", &form.page_size, on_page_size, "Default 50")}
                        {render_text_field("Season", &form.season_number, on_season, "Optional for TV")}
                        {render_text_field("Episode", &form.episode_number, on_episode, "Optional for TV")}
                        {render_text_field("Torznab categories", &form.torznab_cat_ids, on_torznab_cat_ids, "Comma-separated category IDs")}
                        {render_text_field("Identifier types", &form.identifier_types, on_identifier_types, "Comma-separated: imdb, tmdb, tvdb")}
                        {render_text_field("Identifier values", &form.identifier_values, on_identifier_values, "Comma-separated values matching the types")}
                    </div>
                    {if let Some(message) = (*status).clone() {
                        html! { <div class="alert alert-info"><span>{message}</span></div> }
                    } else {
                        html! {}
                    }}
                    {if let Some(message) = (*error).clone() {
                        html! { <div class="alert alert-error"><span>{message}</span></div> }
                    } else {
                        html! {}
                    }}
                </div>
            </div>
            <div class="grid gap-6 xl:grid-cols-[20rem_minmax(0,1fr)]">
                <div class="space-y-4">
                    {render_run_summary(current_run.as_ref())}
                    {page_list}
                    {render_explainability(current_run.as_ref().map(|state| &state.pages.explainability))}
                </div>
                <div class="space-y-4">
                    {results}
                </div>
            </div>
        </section>
    }
}

fn render_run_summary(run: Option<&SearchRunState>) -> Html {
    let Some(run) = run else {
        return html! {
            <EmptyState
                title={"No active search"}
                body={Some(AttrValue::from("Submit a request to see sealed pages and result explainability."))}
            />
        };
    };
    html! {
        <div class="card border border-base-200 bg-base-100 shadow-sm">
            <div class="card-body gap-2">
                <h3 class="text-sm font-semibold uppercase tracking-wide text-base-content/70">{"Request"}</h3>
                <div class="text-sm">
                    <span class="text-base-content/60">{"Search request ID"}</span>
                    <p class="font-mono text-xs break-all">{run.search_request_public_id.to_string()}</p>
                </div>
                <div class="text-sm">
                    <span class="text-base-content/60">{"Policy snapshot"}</span>
                    <p class="font-mono text-xs break-all">{run.request_policy_set_public_id.to_string()}</p>
                </div>
                <div class="stats stats-vertical border border-base-200 bg-base-200/30">
                    <div class="stat py-3">
                        <div class="stat-title">{"Pages"}</div>
                        <div class="stat-value text-2xl">{run.pages.pages.len()}</div>
                    </div>
                    <div class="stat py-3">
                        <div class="stat-title">{"Current page items"}</div>
                        <div class="stat-value text-2xl">{run.current_page.as_ref().map_or(0, |page| page.item_count)}</div>
                    </div>
                </div>
            </div>
        </div>
    }
}

fn render_page_list(
    run: Option<&SearchRunState>,
    on_page_select: Callback<i32>,
    refresh_busy: bool,
) -> Html {
    let Some(run) = run else {
        return html! {};
    };
    if run.pages.pages.is_empty() {
        return html! {
            <EmptyState
                title={"No sealed pages yet"}
                body={Some(AttrValue::from("Refresh after the search runner emits its first page."))}
            />
        };
    }
    html! {
        <div class="card border border-base-200 bg-base-100 shadow-sm">
            <div class="card-body gap-3">
                <div class="flex items-center justify-between gap-2">
                    <h3 class="text-sm font-semibold uppercase tracking-wide text-base-content/70">{"Pages"}</h3>
                    {if refresh_busy {
                        html! { <span class="loading loading-spinner loading-sm"></span> }
                    } else {
                        html! {}
                    }}
                </div>
                <div class="flex flex-wrap gap-2">
                    {for run.pages.pages.iter().map(|page| {
                        let page_number = page.page_number;
                        let selected = run.selected_page_number == Some(page_number);
                        let class = if selected {
                            "btn btn-primary btn-sm"
                        } else if page.sealed_at.is_some() {
                            "btn btn-outline btn-sm"
                        } else {
                            "btn btn-ghost btn-sm"
                        };
                        let on_click = {
                            let on_page_select = on_page_select.clone();
                            Callback::from(move |_| on_page_select.emit(page_number))
                        };
                        html! {
                            <button class={class} onclick={on_click}>
                                {format!("Page {} ({})", page_number, page.item_count)}
                            </button>
                        }
                    })}
                </div>
            </div>
        </div>
    }
}

struct ResultsSectionProps<'a> {
    current_page: Option<&'a crate::models::SearchPageResponse>,
    run: Option<&'a SearchRunState>,
    on_toggle_result: Callback<String>,
    on_toggle_all: Callback<Event>,
    all_selected: bool,
    add_busy: bool,
    selected_count: usize,
    on_add_selected: Callback<MouseEvent>,
}

fn render_results(props: ResultsSectionProps<'_>) -> Html {
    let Some(page) = props.current_page else {
        return html! {
            <EmptyState
                title={"No page selected"}
                body={Some(AttrValue::from("Select a sealed page to inspect results and add them to the download client."))}
            />
        };
    };
    let selected_keys = props
        .run
        .map(|value| value.selected_result_keys.clone())
        .unwrap_or_default();
    html! {
        <div class="card border border-base-200 bg-base-100 shadow-sm">
            <div class="card-body gap-4">
                <div class="flex flex-wrap items-center justify-between gap-3">
                    <div>
                        <h3 class="text-sm font-semibold uppercase tracking-wide text-base-content/70">{"Results"}</h3>
                        <p class="text-sm text-base-content/60">{format!("Page {} with {} result(s)", page.page_number, page.item_count)}</p>
                    </div>
                    <div class="flex gap-2">
                        <label class="label cursor-pointer gap-2">
                            <span class="label-text text-sm">{"Select all"}</span>
                            <input class="checkbox checkbox-sm" type="checkbox" checked={props.all_selected} onchange={props.on_toggle_all} />
                        </label>
                        <button class="btn btn-primary btn-sm" disabled={props.selected_count == 0 || props.add_busy} onclick={props.on_add_selected}>
                            {if props.add_busy {
                                "Adding...".to_string()
                            } else {
                                format!("Add selected ({})", props.selected_count)
                            }}
                        </button>
                    </div>
                </div>
                <div class="space-y-3">
                    {for page.items.iter().map(|item| {
                        let key = selection_key(item);
                        let checked = selected_keys.contains(&key);
                        render_result_row(item, checked, props.on_toggle_result.clone())
                    })}
                </div>
            </div>
        </div>
    }
}

fn render_result_row(
    item: &SearchPageItemResponse,
    checked: bool,
    on_toggle_result: Callback<String>,
) -> Html {
    let key = selection_key(item);
    let on_change = Callback::from(move |_| on_toggle_result.emit(key.clone()));
    html! {
        <article class="rounded-box border border-base-200 bg-base-200/20 p-4">
            <div class="flex flex-wrap items-start gap-3">
                <input class="checkbox checkbox-sm mt-1" type="checkbox" checked={checked} onchange={on_change} />
                <div class="min-w-0 flex-1 space-y-2">
                    <div class="flex flex-wrap items-start justify-between gap-3">
                        <div class="space-y-1">
                            <h4 class="font-medium leading-tight break-words">{item.title_display.clone()}</h4>
                            <p class="text-sm text-base-content/60">{result_meta(item)}</p>
                        </div>
                        <div class="flex flex-wrap gap-2">
                            <span class="badge badge-outline">{format_size(item.size_bytes)}</span>
                            {if item.magnet_uri.is_some() {
                                html! { <span class="badge badge-success badge-soft">{"Magnet"}</span> }
                            } else {
                                html! {}
                            }}
                            {if item.download_url.is_some() {
                                html! { <span class="badge badge-info badge-soft">{"Torrent URL"}</span> }
                            } else {
                                html! {}
                            }}
                        </div>
                    </div>
                    <div class="grid gap-2 text-sm text-base-content/70 sm:grid-cols-2">
                        <div class="rounded-box bg-base-100/70 px-3 py-2">
                            <span class="text-base-content/50">{"Canonical torrent"}</span>
                            <p class="font-mono text-xs break-all">{item.canonical_torrent_public_id.to_string()}</p>
                        </div>
                        <div class="rounded-box bg-base-100/70 px-3 py-2">
                            <span class="text-base-content/50">{"Source"}</span>
                            <p class="break-all">{item.details_url.clone().or_else(|| item.download_url.clone()).or_else(|| item.magnet_uri.clone()).unwrap_or_else(|| "-".to_string())}</p>
                        </div>
                    </div>
                </div>
            </div>
        </article>
    }
}

fn render_explainability(explainability: Option<&SearchRequestExplainabilityResponse>) -> Html {
    let Some(explainability) = explainability else {
        return html! {};
    };
    html! {
        <div class="card border border-base-200 bg-base-100 shadow-sm">
            <div class="card-body gap-3">
                <h3 class="text-sm font-semibold uppercase tracking-wide text-base-content/70">{"Explainability"}</h3>
                <div class="grid gap-2 sm:grid-cols-2">
                    {render_metric_card("Zero runnable", explainability.zero_runnable_indexers.to_string())}
                    {render_metric_card("Blocked results", explainability.blocked_results.to_string())}
                    {render_metric_card("Rate-limited indexers", explainability.rate_limited_indexers.to_string())}
                    {render_metric_card("Retrying indexers", explainability.retrying_indexers.to_string())}
                    {render_metric_card("Skipped (failed)", explainability.skipped_failed_indexers.to_string())}
                    {render_metric_card("Skipped (canceled)", explainability.skipped_canceled_indexers.to_string())}
                </div>
            </div>
        </div>
    }
}

fn render_metric_card(label: &str, value: String) -> Html {
    html! {
        <div class="rounded-box border border-base-200 bg-base-200/30 px-3 py-2">
            <p class="text-xs uppercase tracking-wide text-base-content/50">{label}</p>
            <p class="text-lg font-semibold">{value}</p>
        </div>
    }
}

fn render_text_field(
    label: &'static str,
    value: &str,
    oninput: Callback<InputEvent>,
    placeholder: &'static str,
) -> Html {
    html! {
        <label class="form-control gap-2">
            <span class="label-text text-sm font-medium">{label}</span>
            <input class="input input-bordered w-full" type="text" value={value.to_string()} {oninput} placeholder={placeholder} />
        </label>
    }
}

fn render_select_field(
    label: &'static str,
    value: &str,
    onchange: Callback<Event>,
    options: &[(&'static str, &'static str)],
) -> Html {
    html! {
        <label class="form-control gap-2">
            <span class="label-text text-sm font-medium">{label}</span>
            <select class="select select-bordered w-full" {onchange}>
                {for options.iter().map(|(option_value, option_label)| html! {
                    <option value={option_value.to_string()} selected={*option_value == value}>{option_label.to_string()}</option>
                })}
            </select>
        </label>
    }
}

fn input_callback(
    form: UseStateHandle<SearchFormState>,
    update: fn(&mut SearchFormState, String),
) -> Callback<InputEvent> {
    Callback::from(move |event: InputEvent| {
        let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() else {
            return;
        };
        let mut next = (*form).clone();
        update(&mut next, input.value());
        form.set(next);
    })
}

fn select_callback(
    form: UseStateHandle<SearchFormState>,
    update: fn(&mut SearchFormState, String),
) -> Callback<Event> {
    Callback::from(move |event: Event| {
        let Some(select) = event.target_dyn_into::<web_sys::HtmlSelectElement>() else {
            return;
        };
        let mut next = (*form).clone();
        update(&mut next, select.value());
        form.set(next);
    })
}

fn set_query_text(form: &mut SearchFormState, value: String) {
    form.query_text = value;
}

fn set_query_type(form: &mut SearchFormState, value: String) {
    form.query_type = value;
}

fn set_torznab_mode(form: &mut SearchFormState, value: String) {
    form.torznab_mode = value;
}

fn set_media_domain(form: &mut SearchFormState, value: String) {
    form.requested_media_domain_key = value;
}

fn set_page_size(form: &mut SearchFormState, value: String) {
    form.page_size = value;
}

fn set_season_number(form: &mut SearchFormState, value: String) {
    form.season_number = value;
}

fn set_episode_number(form: &mut SearchFormState, value: String) {
    form.episode_number = value;
}

fn set_identifier_types(form: &mut SearchFormState, value: String) {
    form.identifier_types = value;
}

fn set_identifier_values(form: &mut SearchFormState, value: String) {
    form.identifier_values = value;
}

fn set_torznab_cat_ids(form: &mut SearchFormState, value: String) {
    form.torznab_cat_ids = value;
}
