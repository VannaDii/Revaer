use crate::app::api::ApiCtx;
use crate::app::sse::{SseHandle, connect_sse};
use crate::components::auth::AuthPrompt;
use crate::components::dashboard::DashboardPanel;
use crate::components::detail::FileSelectionChange;
use crate::components::setup::SetupPrompt;
use crate::components::shell::AppShell;
use crate::components::status::SseOverlay;
use crate::components::toast::ToastHost;
use crate::components::torrents::{TorrentView, demo_rows};
use crate::core::auth::{AuthMode, AuthState};
use crate::core::breakpoints::Breakpoint;
use crate::core::events::UiEventEnvelope;
use crate::core::logic::{
    SseView, build_sse_query, build_torrent_filter_query, parse_tags, parse_torrent_filter_query,
};
use crate::core::store::{
    AppModeState, AppStore, HealthMetricsSnapshot, HealthSnapshot, SseApplyOutcome, SystemRates,
    TorrentHealthSnapshot, apply_sse_envelope, select_sse_status, select_system_rates,
};
use crate::core::theme::ThemeMode;
use crate::core::ui::{Density, UiMode};
use crate::features::health::view::HealthPage;
use crate::features::labels::state::LabelKind;
use crate::features::labels::view::LabelsPage;
use crate::features::settings::view::SettingsPage;
use crate::features::torrents::actions::{TorrentAction, success_message};
use crate::features::torrents::state::{
    ProgressPatch, SelectionSet, TorrentRow, TorrentsPaging, TorrentsQueryModel, append_rows,
    apply_progress_patch, remove_row, select_selected_detail, select_visible_ids,
    select_visible_rows, set_rows, set_selected, set_selected_id, update_detail_file_selection,
    upsert_detail,
};
use crate::i18n::{DEFAULT_LOCALE, LocaleCode, TranslationBundle};
use crate::models::{
    AddTorrentInput, NavLabels, SseState, Toast, ToastKind, TorrentSelectionRequest, demo_detail,
    demo_snapshot,
};
use crate::services::sse::SseDecodeError;
use gloo::events::EventListener;
use gloo::storage::{LocalStorage, Storage};
use gloo::utils::window;
use gloo_timers::callback::{Interval, Timeout};
use gloo_timers::future::TimeoutFuture;
use preferences::{
    DENSITY_KEY, LOCALE_KEY, MODE_KEY, THEME_KEY, allow_anonymous, api_base_url, load_auth_mode,
    load_auth_state, load_bypass_local, load_density, load_locale, load_mode, load_theme,
    persist_auth_state, persist_bypass_local,
};
pub(crate) use routes::Route;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use uuid::Uuid;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::{Dispatch, use_selector};

pub(crate) mod api;
mod preferences;
mod routes;
mod sse;

#[function_component(RevaerApp)]
pub fn revaer_app() -> Html {
    let mode = use_state(load_mode);
    let density = use_state(load_density);
    let locale = use_state(load_locale);
    let breakpoint = use_state(current_breakpoint);
    let allow_anon = allow_anonymous();
    let dispatch = Dispatch::<AppStore>::new();
    let api_ctx = use_memo(|_| ApiCtx::new(api_base_url()), ());
    let dashboard = use_state(demo_snapshot);
    let toast_id = use_state(|| 0u64);
    let sse_handle = use_mut_ref(|| None as Option<SseHandle>);
    let sse_reset = use_state(|| 0u32);
    let refresh_timer = use_mut_ref(|| None as Option<Timeout>);
    let progress_buffer = use_mut_ref(|| HashMap::<Uuid, ProgressPatch>::new());
    let progress_flush = use_mut_ref(|| None as Option<Interval>);
    let bundle = {
        let locale = *locale;
        use_memo(move |_| TranslationBundle::new(locale), locale)
    };

    let nav_labels = {
        let bundle = (*bundle).clone();
        NavLabels {
            torrents: bundle.text("nav.torrents", "Torrents"),
            categories: bundle.text("nav.categories", "Categories"),
            tags: bundle.text("nav.tags", "Tags"),
            settings: bundle.text("nav.settings", "Settings"),
            health: bundle.text("nav.health", "Health"),
        }
    };

    let auth_mode = use_selector(|store: &AppStore| store.auth.mode);
    let auth_state = use_selector(|store: &AppStore| store.auth.state.clone());
    let app_mode = use_selector(|store: &AppStore| store.auth.app_mode);
    let bypass_local = use_selector(|store: &AppStore| store.auth.bypass_local);
    let setup_token = use_selector(|store: &AppStore| store.auth.setup_token.clone());
    let setup_expires = use_selector(|store: &AppStore| store.auth.setup_expires_at.clone());
    let setup_error = use_selector(|store: &AppStore| store.auth.setup_error.clone());
    let setup_busy = use_selector(|store: &AppStore| store.auth.setup_busy);
    let theme = use_selector(|store: &AppStore| store.ui.theme);
    let toasts = use_selector(|store: &AppStore| store.ui.toasts.clone());
    let add_busy = use_selector(|store: &AppStore| store.ui.busy.add_torrent);
    let torrents_rows = use_selector(|store: &AppStore| select_visible_rows(&store.torrents));
    let visible_ids = use_selector(|store: &AppStore| select_visible_ids(&store.torrents));
    let selected_id = use_selector(|store: &AppStore| store.torrents.selected_id);
    let selected_ids = use_selector(|store: &AppStore| store.torrents.selected.clone());
    let selected_detail = use_selector(|store: &AppStore| select_selected_detail(&store.torrents));
    let filters = use_selector(|store: &AppStore| store.torrents.filters.clone());
    let paging_state = use_selector(|store: &AppStore| store.torrents.paging.clone());
    let paging_limit = use_selector(|store: &AppStore| store.torrents.paging.limit);
    let system_rates = use_selector(select_system_rates);
    let sse_state = use_selector(select_sse_status);

    let auth_mode = *auth_mode;
    let auth_state_value = (*auth_state).clone();
    let app_mode_value = *app_mode;
    let bypass_local_value = *bypass_local;
    let setup_token_value = (*setup_token).clone();
    let setup_expires_value = (*setup_expires).clone();
    let setup_error_value = (*setup_error).clone();
    let setup_busy_value = *setup_busy;
    let theme_value = *theme;
    let toasts_value = (*toasts).clone();
    let add_busy_value = *add_busy;
    let torrents_rows = (*torrents_rows).clone();
    let visible_ids = (*visible_ids).clone();
    let selected_id_value = *selected_id;
    let selected_ids_value = (*selected_ids).clone();
    let selected_detail_value = (*selected_detail).clone();
    let filters_value = (*filters).clone();
    let paging_state_value = (*paging_state).clone();
    let search = filters_value.name.clone();
    let state_filter_value = filters_value.state.clone().unwrap_or_default();
    let tags_filter_value = if filters_value.tags.is_empty() {
        String::new()
    } else {
        filters_value.tags.join(", ")
    };
    let tracker_filter_value = filters_value.tracker.clone().unwrap_or_default();
    let extension_filter_value = filters_value.extension.clone().unwrap_or_default();
    let can_load_more = paging_state_value.next_cursor.is_some();
    let paging_is_loading = paging_state_value.is_loading;
    let system_rates_value = *system_rates;
    let sse_state_value = (*sse_state).clone();

    let location = use_location();
    let current_route = use_route::<Route>().unwrap_or(Route::Torrents);
    let selected_route_id = match current_route.clone() {
        Route::TorrentDetail { id } => Uuid::parse_str(&id).ok(),
        _ => None,
    };

    {
        let dispatch = dispatch.clone();
        let location = location.clone();
        use_effect_with_deps(
            move |(location, route)| {
                let Some(location) = location.as_ref() else {
                    return;
                };
                if !matches!(route, Route::Torrents | Route::TorrentDetail { .. }) {
                    return;
                }
                let parsed = parse_torrent_filter_query(location.query_str());
                if parsed != dispatch.get().torrents.filters {
                    dispatch.reduce_mut(|store| {
                        store.torrents.filters = parsed;
                        store.torrents.paging.cursor = None;
                        store.torrents.paging.next_cursor = None;
                    });
                }
            },
            (location.clone(), current_route.clone()),
        );
    }
    {
        let location = location.clone();
        let filters = filters.clone();
        use_effect_with_deps(
            move |(filters, location, route)| {
                let Some(location) = location.as_ref() else {
                    return;
                };
                if !matches!(route, Route::Torrents | Route::TorrentDetail { .. }) {
                    return;
                }
                let desired = build_torrent_filter_query(&**filters);
                let desired_query = if desired.is_empty() {
                    String::new()
                } else {
                    format!("?{desired}")
                };
                if desired_query == location.query_str() {
                    return;
                }
                replace_url_query(location.path(), location.hash(), &desired);
            },
            (filters.clone(), location.clone(), current_route.clone()),
        );
    }

    {
        let dispatch = dispatch.clone();
        use_effect_with_deps(
            move |_| {
                let theme = load_theme();
                dispatch.reduce_mut(|store| {
                    store.ui.theme = theme;
                });
                || ()
            },
            (),
        );
    }
    {
        let theme = *theme;
        use_effect_with_deps(
            move |_| {
                apply_theme(theme);
                LocalStorage::set(THEME_KEY, theme.as_str()).ok();
                || ()
            },
            theme,
        );
    }
    {
        let dispatch = dispatch.clone();
        use_effect_with_deps(
            move |_| {
                let mode = load_auth_mode();
                let state = load_auth_state(mode, allow_anon);
                let bypass_local = load_bypass_local();
                dispatch.reduce_mut(|store| {
                    store.auth.mode = mode;
                    store.auth.state = state;
                    store.auth.bypass_local = bypass_local;
                });
                || ()
            },
            (),
        );
    }
    {
        let api_ctx = (*api_ctx).clone();
        let auth_state = auth_state.clone();
        use_effect_with_deps(
            move |auth_state| {
                api_ctx.client.set_auth((**auth_state).clone());
                || ()
            },
            auth_state,
        );
    }
    {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        use_effect_with_deps(
            move |_| {
                let client = api_ctx.client.clone();
                let dispatch = dispatch.clone();
                yew::platform::spawn_local(async move {
                    match client.fetch_health().await {
                        Ok(health) => {
                            dispatch.reduce_mut(|store| {
                                store.auth.setup_error = None;
                                store.health.basic = Some(HealthSnapshot {
                                    status: health.status.clone(),
                                    mode: health.mode.clone(),
                                    database_status: Some(health.database.status),
                                    database_revision: health.database.revision,
                                });
                                store.auth.app_mode = if health.mode == "setup" {
                                    AppModeState::Setup
                                } else {
                                    AppModeState::Active
                                };
                            });
                        }
                        Err(err) => {
                            dispatch.reduce_mut(|store| {
                                store.auth.setup_error = Some(format!("{err}"));
                                store.auth.app_mode = AppModeState::Active;
                                store.health.basic = None;
                            });
                        }
                    }
                });
                || ()
            },
            (),
        );
    }
    {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        use_effect_with_deps(
            move |route| {
                if matches!(route, Route::Health) {
                    let dispatch = dispatch.clone();
                    let client = api_ctx.client.clone();
                    yew::platform::spawn_local(async move {
                        let full = client.fetch_health_full().await;
                        let metrics = client.fetch_metrics().await;
                        dispatch.reduce_mut(|store| {
                            store.health.metrics_text = metrics.ok();
                            match full {
                                Ok(full) => {
                                    store.health.full =
                                        Some(crate::core::store::FullHealthSnapshot {
                                            status: full.status,
                                            mode: full.mode,
                                            revision: full.revision,
                                            build: full.build,
                                            degraded: full.degraded,
                                            metrics: HealthMetricsSnapshot {
                                                config_watch_latency_ms: full
                                                    .metrics
                                                    .config_watch_latency_ms,
                                                config_apply_latency_ms: full
                                                    .metrics
                                                    .config_apply_latency_ms,
                                                config_update_failures_total: full
                                                    .metrics
                                                    .config_update_failures_total,
                                                config_watch_slow_total: full
                                                    .metrics
                                                    .config_watch_slow_total,
                                                guardrail_violations_total: full
                                                    .metrics
                                                    .guardrail_violations_total,
                                                rate_limit_throttled_total: full
                                                    .metrics
                                                    .rate_limit_throttled_total,
                                            },
                                            torrent: TorrentHealthSnapshot {
                                                active: full.torrent.active,
                                                queue_depth: full.torrent.queue_depth,
                                            },
                                        });
                                }
                                Err(_) => {
                                    store.health.full = None;
                                }
                            }
                        });
                    });
                }
                || ()
            },
            current_route.clone(),
        );
    }

    let request_setup_token = {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        Callback::from(move |_| {
            dispatch.reduce_mut(|store| {
                store.auth.setup_busy = true;
            });
            let dispatch = dispatch.clone();
            let client = api_ctx.client.clone();
            yew::platform::spawn_local(async move {
                match client.setup_start().await {
                    Ok(response) => {
                        dispatch.reduce_mut(|store| {
                            store.auth.setup_token = Some(response.token);
                            store.auth.setup_expires_at = Some(response.expires_at);
                            store.auth.setup_error = None;
                        });
                    }
                    Err(err) => {
                        if err.status == 409 {
                            dispatch.reduce_mut(|store| {
                                store.auth.app_mode = AppModeState::Active;
                                store.auth.setup_error = None;
                            });
                        } else {
                            dispatch.reduce_mut(|store| {
                                store.auth.setup_error = Some(format!("{err}"));
                            });
                        }
                    }
                }
                dispatch.reduce_mut(|store| {
                    store.auth.setup_busy = false;
                });
            });
        })
    };

    let complete_setup = {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        Callback::from(move |token: String| {
            dispatch.reduce_mut(|store| {
                store.auth.setup_busy = true;
            });
            let dispatch = dispatch.clone();
            let client = api_ctx.client.clone();
            yew::platform::spawn_local(async move {
                match client.setup_complete(&token).await {
                    Ok(_) => {
                        dispatch.reduce_mut(|store| {
                            store.auth.setup_error = None;
                            store.auth.app_mode = AppModeState::Active;
                        });
                    }
                    Err(err) => {
                        dispatch.reduce_mut(|store| {
                            store.auth.setup_error = Some(format!("{err}"));
                        });
                    }
                }
                dispatch.reduce_mut(|store| {
                    store.auth.setup_busy = false;
                });
            });
        })
    };

    {
        let app_mode = app_mode.clone();
        let request_setup_token = request_setup_token.clone();
        let setup_token = setup_token.clone();
        use_effect_with_deps(
            move |(mode, token)| {
                if *mode == AppModeState::Setup && token.is_none() {
                    request_setup_token.emit(());
                }
                || ()
            },
            ((*app_mode).clone(), (*setup_token).clone()),
        );
    }
    {
        let dashboard = dashboard.clone();
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        use_effect_with_deps(
            move |auth_state| {
                if auth_state.as_ref().is_some() {
                    let dashboard_client = api_ctx.client.clone();
                    let dispatch = dispatch.clone();
                    yew::platform::spawn_local(async move {
                        if let Ok(snapshot) = dashboard_client.fetch_dashboard().await {
                            let rates = SystemRates {
                                download_bps: snapshot.download_bps,
                                upload_bps: snapshot.upload_bps,
                            };
                            dispatch.reduce_mut(|store| {
                                store.system.rates = rates;
                            });
                            dashboard.set(snapshot);
                        }
                    });
                }
                || ()
            },
            auth_state.clone(),
        );
    }
    {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        use_effect_with_deps(
            move |auth_state| {
                if auth_state.as_ref().is_some() {
                    let dispatch = dispatch.clone();
                    let client = api_ctx.client.clone();
                    yew::platform::spawn_local(async move {
                        let categories = client.fetch_categories().await;
                        let tags = client.fetch_tags().await;
                        dispatch.reduce_mut(|store| {
                            if let Ok(entries) = categories {
                                store.labels.categories = entries
                                    .into_iter()
                                    .map(|entry| (entry.name.clone(), entry))
                                    .collect();
                            }
                            if let Ok(entries) = tags {
                                store.labels.tags = entries
                                    .into_iter()
                                    .map(|entry| (entry.name.clone(), entry))
                                    .collect();
                            }
                        });
                    });
                } else {
                    dispatch.reduce_mut(|store| {
                        store.labels.categories.clear();
                        store.labels.tags.clear();
                    });
                }
                || ()
            },
            auth_state.clone(),
        );
    }
    {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        let filters = filters.clone();
        let paging_limit = paging_limit.clone();
        let auth_state = auth_state.clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        use_effect_with_deps(
            move |(filters, paging_limit, auth_state)| {
                let auth_state = (**auth_state).clone();
                let filters = (**filters).clone();
                let paging = TorrentsPaging {
                    cursor: None,
                    next_cursor: None,
                    limit: **paging_limit,
                    is_loading: false,
                };
                let dispatch = dispatch.clone();
                let client = api_ctx.client.clone();
                let toast_id = toast_id.clone();
                let bundle = bundle.clone();
                dispatch.reduce_mut(|store| {
                    store.torrents.paging.is_loading = true;
                });
                yew::platform::spawn_local(async move {
                    if auth_state.is_some() {
                        fetch_torrent_list_with_retry(
                            client,
                            dispatch.clone(),
                            toast_id,
                            bundle,
                            filters,
                            paging,
                        )
                        .await;
                    } else {
                        dispatch.reduce_mut(|store| {
                            set_rows(&mut store.torrents, demo_rows());
                            store.torrents.paging.next_cursor = None;
                        });
                    }
                    dispatch.reduce_mut(|store| {
                        store.torrents.paging.is_loading = false;
                    });
                });
                || ()
            },
            (filters.clone(), paging_limit.clone(), auth_state.clone()),
        );
    }

    let schedule_refresh = {
        let refresh_timer = refresh_timer.clone();
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |_| {
            if refresh_timer.borrow().is_some() {
                return;
            }
            let refresh_timer_handle = refresh_timer.clone();
            let dispatch = dispatch.clone();
            let client = api_ctx.client.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            let handle = Timeout::new(1200, move || {
                refresh_timer_handle.borrow_mut().take();
                let state = dispatch.get();
                let auth_state = state.auth.state.clone();
                let filters = state.torrents.filters.clone();
                let paging = refresh_paging(&state.torrents.paging);
                if auth_state.is_none() {
                    dispatch.reduce_mut(|store| {
                        set_rows(&mut store.torrents, demo_rows());
                        store.torrents.paging.next_cursor = None;
                    });
                    return;
                }
                yew::platform::spawn_local(async move {
                    fetch_torrent_list_with_retry(
                        client,
                        dispatch.clone(),
                        toast_id,
                        bundle,
                        filters,
                        paging,
                    )
                    .await;
                });
            });
            *refresh_timer.borrow_mut() = Some(handle);
        })
    };

    {
        let dispatch = dispatch.clone();
        use_effect_with_deps(
            move |selected_id| {
                dispatch.reduce_mut(|store| {
                    set_selected_id(&mut store.torrents, *selected_id);
                });
                || ()
            },
            selected_route_id,
        );
    }
    {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        let selected_id = selected_id.clone();
        let auth_state = auth_state.clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        use_effect_with_deps(
            move |(selected_id, auth_state)| {
                let cleanup = || ();
                let auth_state = (**auth_state).clone();
                if let Some(id) = **selected_id {
                    if !dispatch.get().torrents.details_by_id.contains_key(&id) {
                        let dispatch = dispatch.clone();
                        let client = api_ctx.client.clone();
                        let toast_id = toast_id.clone();
                        let bundle = bundle.clone();
                        yew::platform::spawn_local(async move {
                            if auth_state.is_some() {
                                if let Some(detail) = fetch_torrent_detail_with_retry(
                                    client,
                                    dispatch.clone(),
                                    toast_id,
                                    bundle,
                                    id,
                                )
                                .await
                                {
                                    dispatch.reduce_mut(|store| {
                                        upsert_detail(&mut store.torrents, id, detail);
                                    });
                                }
                            } else if let Some(detail) = demo_detail(&id.to_string()) {
                                dispatch.reduce_mut(|store| {
                                    upsert_detail(&mut store.torrents, id, detail);
                                });
                            }
                        });
                    }
                }
                cleanup
            },
            (selected_id.clone(), auth_state.clone()),
        );
    }
    {
        let dispatch = dispatch.clone();
        let progress_buffer = progress_buffer.clone();
        let progress_flush = progress_flush.clone();
        use_effect_with_deps(
            move |_| {
                let handle = Interval::new(80, move || {
                    let patches = {
                        let mut buffer = progress_buffer.borrow_mut();
                        if buffer.is_empty() {
                            return;
                        }
                        buffer.drain().map(|(_, patch)| patch).collect::<Vec<_>>()
                    };
                    dispatch.reduce_mut(|store| {
                        for patch in patches {
                            apply_progress_patch(&mut store.torrents, patch);
                        }
                    });
                });
                *progress_flush.borrow_mut() = Some(handle);
                move || {
                    progress_flush.borrow_mut().take();
                }
            },
            (),
        );
    }

    let sse_query = {
        let view = if matches!(current_route, Route::TorrentDetail { .. }) {
            SseView::Detail
        } else {
            SseView::List
        };
        build_sse_query(
            &visible_ids,
            selected_route_id,
            filters_value.state.clone(),
            view,
        )
    };
    {
        let sse_handle = sse_handle.clone();
        let dispatch = dispatch.clone();
        let auth_state = auth_state.clone();
        let progress_buffer = progress_buffer.clone();
        let schedule_refresh = schedule_refresh.clone();
        let sse_query = sse_query.clone();
        let sse_reset = *sse_reset;
        use_effect_with_deps(
            move |(auth_state_value, _reset, query)| {
                if let Some(handle) = sse_handle.borrow_mut().take() {
                    handle.close();
                }
                let auth_state_value = (**auth_state_value).clone();
                if let Some(auth_state_value) = auth_state_value {
                    let on_state = {
                        let dispatch = dispatch.clone();
                        Callback::from(move |state: SseState| {
                            dispatch.reduce_mut(|store| {
                                store.system.sse_state = state;
                            });
                        })
                    };
                    let on_event = {
                        let dispatch = dispatch.clone();
                        let progress_buffer = progress_buffer.clone();
                        let schedule_refresh = schedule_refresh.clone();
                        Callback::from(move |envelope: UiEventEnvelope| {
                            handle_sse_envelope(
                                envelope,
                                &dispatch,
                                &progress_buffer,
                                &schedule_refresh,
                            );
                        })
                    };
                    let on_error = {
                        let schedule_refresh = schedule_refresh.clone();
                        Callback::from(move |_err: SseDecodeError| schedule_refresh.emit(()))
                    };
                    if let Some(handle) = connect_sse(
                        api_base_url(),
                        Some(auth_state_value),
                        query.clone(),
                        on_event,
                        on_error,
                        on_state,
                    ) {
                        *sse_handle.borrow_mut() = Some(handle);
                    } else {
                        dispatch.reduce_mut(|store| {
                            store.system.sse_state = SseState::Reconnecting {
                                retry_in_secs: 5,
                                last_event: "connect".to_string(),
                                reason: "SSE unavailable".to_string(),
                            };
                        });
                    }
                } else {
                    dispatch.reduce_mut(|store| {
                        store.system.sse_state = SseState::Reconnecting {
                            retry_in_secs: 5,
                            last_event: "auth".to_string(),
                            reason: "awaiting authentication".to_string(),
                        };
                    });
                }
                move || {
                    if let Some(handle) = sse_handle.borrow_mut().take() {
                        handle.close();
                    }
                }
            },
            (auth_state.clone(), sse_reset, sse_query),
        );
    }
    {
        let breakpoint = breakpoint.clone();
        use_effect(move || {
            apply_breakpoint(*breakpoint);
            let handler = EventListener::new(&gloo::utils::window(), "resize", {
                let breakpoint = breakpoint.clone();
                move |_event| {
                    let bp = current_breakpoint();
                    if bp != *breakpoint {
                        breakpoint.set(bp);
                    }
                }
            });
            move || drop(handler)
        });
    }
    {
        let mode = mode.clone();
        use_effect_with_deps(
            move |mode| {
                LocalStorage::set(
                    MODE_KEY,
                    match **mode {
                        UiMode::Simple => "simple",
                        UiMode::Advanced => "advanced",
                    },
                )
                .ok();
                || ()
            },
            mode.clone(),
        );
    }
    {
        let density = density.clone();
        use_effect_with_deps(
            move |density| {
                LocalStorage::set(
                    DENSITY_KEY,
                    match **density {
                        Density::Compact => "compact",
                        Density::Normal => "normal",
                        Density::Comfy => "comfy",
                    },
                )
                .ok();
                || ()
            },
            density.clone(),
        );
    }
    {
        let locale = locale.clone();
        use_effect_with_deps(
            move |locale| {
                LocalStorage::set(LOCALE_KEY, locale.code()).ok();
                apply_direction(TranslationBundle::new(**locale).rtl());
                || ()
            },
            locale.clone(),
        );
    }

    let toggle_theme = {
        let dispatch = dispatch.clone();
        Callback::from(move |_| {
            dispatch.reduce_mut(|store| {
                store.ui.theme = if store.ui.theme == ThemeMode::Light {
                    ThemeMode::Dark
                } else {
                    ThemeMode::Light
                };
            });
        })
    };

    let set_mode = {
        let mode = mode.clone();
        Callback::from(move |next: UiMode| mode.set(next))
    };
    let set_density = {
        let density = density.clone();
        Callback::from(move |next: Density| density.set(next))
    };
    let set_search = {
        let dispatch = dispatch.clone();
        Callback::from(move |value: String| {
            dispatch.reduce_mut(|store| {
                store.torrents.filters.name = value;
                store.torrents.paging.cursor = None;
                store.torrents.paging.next_cursor = None;
            });
        })
    };
    let set_state_filter = {
        let dispatch = dispatch.clone();
        Callback::from(move |value: String| {
            dispatch.reduce_mut(|store| {
                let trimmed = value.trim();
                store.torrents.filters.state = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
                store.torrents.paging.cursor = None;
                store.torrents.paging.next_cursor = None;
            });
        })
    };
    let set_tags_filter = {
        let dispatch = dispatch.clone();
        Callback::from(move |value: String| {
            let parsed = parse_tags(&value).unwrap_or_default();
            dispatch.reduce_mut(|store| {
                store.torrents.filters.tags = parsed;
                store.torrents.paging.cursor = None;
                store.torrents.paging.next_cursor = None;
            });
        })
    };
    let set_tracker_filter = {
        let dispatch = dispatch.clone();
        Callback::from(move |value: String| {
            dispatch.reduce_mut(|store| {
                let trimmed = value.trim();
                store.torrents.filters.tracker = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
                store.torrents.paging.cursor = None;
                store.torrents.paging.next_cursor = None;
            });
        })
    };
    let set_extension_filter = {
        let dispatch = dispatch.clone();
        Callback::from(move |value: String| {
            dispatch.reduce_mut(|store| {
                let normalized = value.trim().trim_start_matches('.');
                store.torrents.filters.extension = if normalized.is_empty() {
                    None
                } else {
                    Some(normalized.to_string())
                };
                store.torrents.paging.cursor = None;
                store.torrents.paging.next_cursor = None;
            });
        })
    };
    let on_set_selected = {
        let dispatch = dispatch.clone();
        Callback::from(move |next: SelectionSet| {
            dispatch.reduce_mut(|store| {
                set_selected(&mut store.torrents, next);
            });
        })
    };
    let on_load_more = {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |_| {
            let state = dispatch.get();
            if state.torrents.paging.is_loading || state.auth.state.is_none() {
                return;
            }
            let Some(cursor) = state.torrents.paging.next_cursor.clone() else {
                return;
            };
            let filters = state.torrents.filters.clone();
            let paging = TorrentsPaging {
                cursor: Some(cursor),
                next_cursor: None,
                limit: state.torrents.paging.limit,
                is_loading: false,
            };
            dispatch.reduce_mut(|store| {
                store.torrents.paging.is_loading = true;
            });
            let dispatch = dispatch.clone();
            let client = api_ctx.client.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            yew::platform::spawn_local(async move {
                fetch_torrent_list_with_retry(
                    client,
                    dispatch.clone(),
                    toast_id,
                    bundle,
                    filters,
                    paging,
                )
                .await;
                dispatch.reduce_mut(|store| {
                    store.torrents.paging.is_loading = false;
                });
            });
        })
    };
    let trigger_sse_reconnect = {
        let sse_reset = sse_reset.clone();
        let dispatch = dispatch.clone();
        Callback::from(move |_| {
            dispatch.reduce_mut(|store| {
                store.system.sse_state = SseState::Reconnecting {
                    retry_in_secs: 3,
                    last_event: "manual".to_string(),
                    reason: "manual reconnect".to_string(),
                };
            });
            sse_reset.set(*sse_reset + 1);
        })
    };
    let dismiss_toast = {
        let dispatch = dispatch.clone();
        Callback::from(move |id: u64| {
            dispatch.reduce_mut(|store| {
                store.ui.toasts.retain(|toast| toast.id != id);
            });
        })
    };
    let on_toggle_bypass_local = {
        let dispatch = dispatch.clone();
        Callback::from(move |value: bool| {
            persist_bypass_local(value);
            dispatch.reduce_mut(|store| {
                store.auth.bypass_local = value;
            });
        })
    };
    let on_copy_metrics = {
        let dispatch = dispatch.clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |text: String| {
            let dispatch = dispatch.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            yew::platform::spawn_local(async move {
                match copy_text_to_clipboard(text).await {
                    Ok(()) => push_toast(
                        &dispatch,
                        &toast_id,
                        ToastKind::Success,
                        bundle.text("toast.metrics_copied", "Metrics copied"),
                    ),
                    Err(err) => push_toast(
                        &dispatch,
                        &toast_id,
                        ToastKind::Error,
                        format!(
                            "{} {err}",
                            bundle.text("toast.metrics_copy_failed", "Failed to copy metrics.")
                        ),
                    ),
                }
            });
        })
    };
    let on_add_torrent = {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |input: AddTorrentInput| {
            let client = api_ctx.client.clone();
            let dispatch = dispatch.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            dispatch.reduce_mut(|store| {
                store.ui.busy.add_torrent = true;
            });
            yew::platform::spawn_local(async move {
                match client.add_torrent(input).await {
                    Ok(_id) => {
                        push_toast(
                            &dispatch,
                            &toast_id,
                            ToastKind::Success,
                            bundle.text("toast.add_success", "Torrent added"),
                        );
                        let (filters, paging) = {
                            let state = dispatch.get();
                            (
                                state.torrents.filters.clone(),
                                refresh_paging(&state.torrents.paging),
                            )
                        };
                        fetch_torrent_list_with_retry(
                            client,
                            dispatch.clone(),
                            toast_id.clone(),
                            bundle.clone(),
                            filters,
                            paging,
                        )
                        .await;
                    }
                    Err(err) => push_toast(
                        &dispatch,
                        &toast_id,
                        ToastKind::Error,
                        format!("{} {err}", bundle.text("toast.add_failed", "")),
                    ),
                }
                dispatch.reduce_mut(|store| {
                    store.ui.busy.add_torrent = false;
                });
            });
        })
    };
    let on_action = {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |(action, id): (TorrentAction, Uuid)| {
            let client = api_ctx.client.clone();
            let dispatch = dispatch.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            yew::platform::spawn_local(async move {
                let id_str = id.to_string();
                let display_name = dispatch
                    .get()
                    .torrents
                    .by_id
                    .get(&id)
                    .map(|row| row.name.clone())
                    .unwrap_or_else(|| {
                        format!("{} {id}", bundle.text("toast.torrent_placeholder", ""))
                    });
                match client.perform_action(&id_str, action.clone()).await {
                    Ok(_) => {
                        if matches!(action, TorrentAction::Delete { .. }) {
                            dispatch.reduce_mut(|store| {
                                remove_row(&mut store.torrents, id);
                            });
                        }
                        push_toast(
                            &dispatch,
                            &toast_id,
                            ToastKind::Success,
                            success_message(&bundle, &action, &display_name),
                        );
                    }
                    Err(err) => push_toast(
                        &dispatch,
                        &toast_id,
                        ToastKind::Error,
                        format!(
                            "{} {display_name}: {err}",
                            bundle.text("toast.action_failed", "")
                        ),
                    ),
                }
            });
        })
    };
    let on_bulk_action = {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |(action, ids): (TorrentAction, Vec<Uuid>)| {
            let client = api_ctx.client.clone();
            let dispatch = dispatch.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            yew::platform::spawn_local(async move {
                for id in ids.clone() {
                    let id_str = id.to_string();
                    let display_name = dispatch
                        .get()
                        .torrents
                        .by_id
                        .get(&id)
                        .map(|row| row.name.clone())
                        .unwrap_or_else(|| {
                            format!("{} {id}", bundle.text("toast.torrent_placeholder", ""))
                        });
                    if let Err(err) = client.perform_action(&id_str, action.clone()).await {
                        push_toast(
                            &dispatch,
                            &toast_id,
                            ToastKind::Error,
                            format!(
                                "{} {display_name}: {err}",
                                bundle.text("toast.action_failed", "")
                            ),
                        );
                    }
                }
                if matches!(action, TorrentAction::Delete { .. }) {
                    dispatch.reduce_mut(|store| {
                        for id in &ids {
                            remove_row(&mut store.torrents, *id);
                        }
                    });
                }
                push_toast(
                    &dispatch,
                    &toast_id,
                    ToastKind::Success,
                    format!("{} {}", bundle.text("toast.bulk_done", ""), ids.len()),
                );
            });
        })
    };
    let on_select_detail = {
        let dispatch = dispatch.clone();
        Callback::from(move |id: Uuid| {
            dispatch.reduce_mut(|store| {
                set_selected_id(&mut store.torrents, Some(id));
            });
        })
    };
    let on_update_selection = {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |(id, change): (Uuid, FileSelectionChange)| {
            let client = api_ctx.client.clone();
            let dispatch = dispatch.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            dispatch.reduce_mut(|store| {
                update_detail_file_selection(&mut store.torrents, id, &change.path, change.wanted);
            });
            let request = TorrentSelectionRequest {
                include: if change.wanted {
                    vec![change.path.clone()]
                } else {
                    Vec::new()
                },
                exclude: if change.wanted {
                    Vec::new()
                } else {
                    vec![change.path.clone()]
                },
                skip_fluff: None,
                priorities: Vec::new(),
            };
            yew::platform::spawn_local(async move {
                if let Err(err) = client
                    .update_torrent_selection(&id.to_string(), &request)
                    .await
                {
                    push_toast(
                        &dispatch,
                        &toast_id,
                        ToastKind::Error,
                        format!(
                            "{} {err}",
                            bundle.text(
                                "toast.file_selection_failed",
                                "Failed to update file selection."
                            )
                        ),
                    );
                    if let Some(detail) = fetch_torrent_detail_with_retry(
                        client,
                        dispatch.clone(),
                        toast_id,
                        bundle,
                        id,
                    )
                    .await
                    {
                        dispatch.reduce_mut(|store| {
                            upsert_detail(&mut store.torrents, id, detail);
                        });
                    }
                }
            });
        })
    };

    let locale_selector = {
        let locale = locale.clone();
        html! {
            <select value={locale.code().to_string()} onchange={{
                let locale = locale.clone();
                Callback::from(move |e: Event| {
                    let target: web_sys::HtmlSelectElement = e.target().unwrap().dyn_into().unwrap();
                    let code = target.value();
                    if let Some(next) = LocaleCode::from_lang_tag(&code) {
                        locale.set(next);
                    }
                })
            }}>
                {for LocaleCode::all().iter().map(|lc| html! {
                    <option value={lc.code()} selected={*lc == *locale}>{lc.label()}</option>
                })}
            </select>
        }
    };

    let bundle_ctx = bundle.clone();
    let bundle_routes = bundle.clone();
    let bundle_sse = bundle.clone();

    html! {
        <ContextProvider<ApiCtx> context={(*api_ctx).clone()}>
            <ContextProvider<TranslationBundle> context={(*bundle_ctx).clone()}>
                <BrowserRouter>
                    <AppShell
                        theme={theme_value}
                        on_toggle_theme={toggle_theme}
                        mode={*mode}
                        on_mode_change={set_mode}
                        active={current_route}
                        locale_selector={locale_selector}
                        nav={nav_labels}
                        breakpoint={*breakpoint}
                        sse_state={sse_state_value.clone()}
                        on_sse_retry={trigger_sse_reconnect.clone()}
                        network_mode={bundle_ctx.text("shell.network_connected", "")}
                    >
                        <Switch<Route> render={move |route| {
                            let bundle = (*bundle_routes).clone();
                            match route {
                                Route::Home => html! { <Redirect<Route> to={Route::Torrents} /> },
                                Route::Torrents => html! {
                                    <div class="torrents-stack">
                                        <DashboardPanel snapshot={(*dashboard).clone()} system_rates={system_rates_value} mode={*mode} density={*density} torrents={torrents_rows.clone()} />
                                        <TorrentView
                                            breakpoint={*breakpoint}
                                            visible_ids={visible_ids.clone()}
                                            density={*density}
                                            mode={*mode}
                                            on_density_change={set_density.clone()}
                                            on_bulk_action={on_bulk_action.clone()}
                                            on_action={on_action.clone()}
                                            on_add={on_add_torrent.clone()}
                                            add_busy={add_busy_value}
                                            search={search.clone()}
                                            on_search={set_search.clone()}
                                            state_filter={state_filter_value.clone()}
                                            tags_filter={tags_filter_value.clone()}
                                            tracker_filter={tracker_filter_value.clone()}
                                            extension_filter={extension_filter_value.clone()}
                                            on_state_filter={set_state_filter.clone()}
                                            on_tags_filter={set_tags_filter.clone()}
                                            on_tracker_filter={set_tracker_filter.clone()}
                                            on_extension_filter={set_extension_filter.clone()}
                                            can_load_more={can_load_more}
                                            is_loading={paging_is_loading}
                                            on_load_more={on_load_more.clone()}
                                            selected_id={selected_id_value}
                                            selected_ids={selected_ids_value.clone()}
                                            on_set_selected={on_set_selected.clone()}
                                            selected_detail={selected_detail_value.clone()}
                                            on_select_detail={on_select_detail.clone()}
                                            on_update_selection={on_update_selection.clone()}
                                        />
                                    </div>
                                },
                                Route::TorrentDetail { id } => html! {
                                    <div class="torrents-stack">
                                        <DashboardPanel snapshot={(*dashboard).clone()} system_rates={system_rates_value} mode={*mode} density={*density} torrents={torrents_rows.clone()} />
                                        <TorrentView
                                            breakpoint={*breakpoint}
                                            visible_ids={visible_ids.clone()}
                                            density={*density}
                                            mode={*mode}
                                            on_density_change={set_density.clone()}
                                            on_bulk_action={on_bulk_action.clone()}
                                            on_action={on_action.clone()}
                                            on_add={on_add_torrent.clone()}
                                            add_busy={add_busy_value}
                                            search={search.clone()}
                                            on_search={set_search.clone()}
                                            state_filter={state_filter_value.clone()}
                                            tags_filter={tags_filter_value.clone()}
                                            tracker_filter={tracker_filter_value.clone()}
                                            extension_filter={extension_filter_value.clone()}
                                            on_state_filter={set_state_filter.clone()}
                                            on_tags_filter={set_tags_filter.clone()}
                                            on_tracker_filter={set_tracker_filter.clone()}
                                            on_extension_filter={set_extension_filter.clone()}
                                            can_load_more={can_load_more}
                                            is_loading={paging_is_loading}
                                            on_load_more={on_load_more.clone()}
                                            selected_id={Uuid::parse_str(&id).ok()}
                                            selected_ids={selected_ids_value.clone()}
                                            on_set_selected={on_set_selected.clone()}
                                            selected_detail={selected_detail_value.clone()}
                                            on_select_detail={on_select_detail.clone()}
                                            on_update_selection={on_update_selection.clone()}
                                        />
                                    </div>
                                },
                                Route::Categories => html! { <LabelsPage kind={LabelKind::Category} /> },
                                Route::Tags => html! { <LabelsPage kind={LabelKind::Tag} /> },
                                Route::Settings => html! { <SettingsPage bypass_local={bypass_local_value} on_toggle_bypass_local={on_toggle_bypass_local.clone()} /> },
                                Route::Health => html! { <HealthPage on_copy_metrics={on_copy_metrics.clone()} /> },
                                Route::NotFound => html! { <Placeholder title={bundle.text("placeholder.not_found_title", "Not found")} body={bundle.text("placeholder.not_found_body", "Use navigation to return to a supported view.")} /> },
                            }
                        }} />
                    </AppShell>
                    <ToastHost toasts={toasts_value.clone()} on_dismiss={dismiss_toast.clone()} />
                    <SseOverlay state={sse_state_value.clone()} on_retry={trigger_sse_reconnect} network_mode={bundle_sse.text("shell.network_remote", "")} />
                    {if app_mode_value == AppModeState::Setup {
                        html! {
                            <SetupPrompt
                                token={setup_token_value.clone()}
                                expires_at={setup_expires_value.clone()}
                                busy={setup_busy_value}
                                error={setup_error_value.clone()}
                                on_request_token={request_setup_token.clone()}
                                on_complete={complete_setup.clone()}
                            />
                        }
                    } else if auth_state_value.is_none() {
                        html! {
                            <AuthPrompt
                                allow_anonymous={allow_anon}
                                default_mode={if bypass_local_value { AuthMode::ApiKey } else { auth_mode }}
                                on_submit={{
                                    let dispatch = dispatch.clone();
                                    Callback::from(move |value: AuthState| {
                                        let next_mode = match value {
                                            AuthState::Local(_) => AuthMode::Local,
                                            _ => AuthMode::ApiKey,
                                        };
                                        persist_auth_state(&value);
                                        dispatch.reduce_mut(|store| {
                                            store.auth.mode = next_mode;
                                            store.auth.state = Some(value);
                                        });
                                    })
                                }}
                            />
                        }
                    } else { html!{} }}
                </BrowserRouter>
            </ContextProvider<TranslationBundle>>
        </ContextProvider<ApiCtx>>
    }
}

#[function_component(Placeholder)]
fn placeholder(props: &PlaceholderProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    html! {
        <div class="placeholder">
            <h2>{&props.title}</h2>
            <p class="muted">{&props.body}</p>
            <div class="pill subtle">{bundle.text("placeholder.badge", "")}</div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct PlaceholderProps {
    pub title: String,
    pub body: String,
}

async fn fetch_torrent_list_with_retry(
    client: std::rc::Rc<crate::services::api::ApiClient>,
    dispatch: Dispatch<AppStore>,
    toast_id: UseStateHandle<u64>,
    bundle: TranslationBundle,
    filters: TorrentsQueryModel,
    paging: TorrentsPaging,
) {
    let append = paging.cursor.is_some();
    match client.fetch_torrents(&filters, &paging).await {
        Ok(list) => apply_torrent_list(&dispatch, list, append),
        Err(err) if err.is_rate_limited() => {
            if let Some(delay) = err.retry_after_secs {
                push_toast(
                    &dispatch,
                    &toast_id,
                    ToastKind::Info,
                    format!(
                        "{} {}s",
                        bundle.text("toast.rate_limited", "Rate limited, retrying in"),
                        delay
                    ),
                );
                TimeoutFuture::new(retry_delay_ms(delay)).await;
                match client.fetch_torrents(&filters, &paging).await {
                    Ok(list) => apply_torrent_list(&dispatch, list, append),
                    Err(err) => push_toast(
                        &dispatch,
                        &toast_id,
                        ToastKind::Error,
                        format!(
                            "{} {err}",
                            bundle.text("toast.list_failed", "Failed to load torrents.")
                        ),
                    ),
                }
            } else {
                push_toast(
                    &dispatch,
                    &toast_id,
                    ToastKind::Error,
                    format!(
                        "{} {err}",
                        bundle.text("toast.list_failed", "Failed to load torrents.")
                    ),
                );
            }
        }
        Err(err) => push_toast(
            &dispatch,
            &toast_id,
            ToastKind::Error,
            format!(
                "{} {err}",
                bundle.text("toast.list_failed", "Failed to load torrents.")
            ),
        ),
    }
}

async fn fetch_torrent_detail_with_retry(
    client: std::rc::Rc<crate::services::api::ApiClient>,
    dispatch: Dispatch<AppStore>,
    toast_id: UseStateHandle<u64>,
    bundle: TranslationBundle,
    id: Uuid,
) -> Option<crate::models::DetailData> {
    let id_str = id.to_string();
    match client.fetch_torrent_detail(&id_str).await {
        Ok(detail) => Some(detail),
        Err(err) if err.is_rate_limited() => {
            if let Some(delay) = err.retry_after_secs {
                push_toast(
                    &dispatch,
                    &toast_id,
                    ToastKind::Info,
                    format!(
                        "{} {}s",
                        bundle.text("toast.rate_limited", "Rate limited, retrying in"),
                        delay
                    ),
                );
                TimeoutFuture::new(retry_delay_ms(delay)).await;
                match client.fetch_torrent_detail(&id_str).await {
                    Ok(detail) => Some(detail),
                    Err(err) => {
                        push_toast(
                            &dispatch,
                            &toast_id,
                            ToastKind::Error,
                            format!(
                                "{} {err}",
                                bundle
                                    .text("toast.detail_failed", "Failed to load torrent details.")
                            ),
                        );
                        None
                    }
                }
            } else {
                push_toast(
                    &dispatch,
                    &toast_id,
                    ToastKind::Error,
                    format!(
                        "{} {err}",
                        bundle.text("toast.detail_failed", "Failed to load torrent details.")
                    ),
                );
                None
            }
        }
        Err(err) => {
            push_toast(
                &dispatch,
                &toast_id,
                ToastKind::Error,
                format!(
                    "{} {err}",
                    bundle.text("toast.detail_failed", "Failed to load torrent details.")
                ),
            );
            None
        }
    }
}

fn apply_torrent_list(
    dispatch: &Dispatch<AppStore>,
    list: crate::models::TorrentListResponse,
    append: bool,
) {
    let rows = list.torrents.into_iter().map(TorrentRow::from).collect();
    dispatch.reduce_mut(|store| {
        if append {
            append_rows(&mut store.torrents, rows);
        } else {
            set_rows(&mut store.torrents, rows);
        }
        store.torrents.paging.next_cursor = list.next;
    });
}

fn refresh_paging(paging: &TorrentsPaging) -> TorrentsPaging {
    TorrentsPaging {
        cursor: None,
        next_cursor: None,
        limit: paging.limit,
        is_loading: false,
    }
}

fn replace_url_query(path: &str, hash: &str, query: &str) {
    let mut url = path.to_string();
    if !query.is_empty() {
        url.push('?');
        url.push_str(query);
    }
    if !hash.is_empty() {
        url.push_str(hash);
    }
    if let Ok(history) = window().history() {
        let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&url));
    }
}

fn retry_delay_ms(delay_secs: u64) -> u32 {
    let millis = delay_secs.saturating_mul(1_000);
    match u32::try_from(millis) {
        Ok(value) => value,
        Err(_) => u32::MAX,
    }
}

fn push_toast(
    dispatch: &Dispatch<AppStore>,
    next_id: &UseStateHandle<u64>,
    kind: ToastKind,
    message: String,
) {
    let id = **next_id + 1;
    next_id.set(id);
    dispatch.reduce_mut(|store| {
        store.ui.toasts.push(Toast { id, message, kind });
        if store.ui.toasts.len() > 4 {
            let drain = store.ui.toasts.len() - 4;
            store.ui.toasts.drain(0..drain);
        }
    });
}

async fn copy_text_to_clipboard(text: String) -> Result<(), String> {
    let clipboard = window().navigator().clipboard();
    let promise = clipboard.write_text(&text);
    JsFuture::from(promise)
        .await
        .map_err(|_| "Clipboard write failed".to_string())?;
    Ok(())
}

fn apply_breakpoint(bp: Breakpoint) {
    if let Some(document) = window().document() {
        if let Some(body) = document.body() {
            let _ = body.set_attribute("data-bp", bp.name);
        }
    }
}

fn apply_theme(theme: ThemeMode) {
    if let Some(document) = window().document() {
        if let Some(body) = document.body() {
            let _ = body.set_attribute("data-theme", theme.as_str());
        }
    }
}

fn apply_direction(is_rtl: bool) {
    if let Some(document) = window().document() {
        if let Some(body) = document.body() {
            let _ = body.set_attribute("dir", if is_rtl { "rtl" } else { "ltr" });
        }
    }
}

fn current_breakpoint() -> Breakpoint {
    let width = window()
        .inner_width()
        .ok()
        .and_then(|w| w.as_f64())
        .unwrap_or(1280.0) as u16;
    crate::breakpoints::for_width(width)
}

fn handle_sse_envelope(
    envelope: UiEventEnvelope,
    dispatch: &Dispatch<AppStore>,
    progress_buffer: &Rc<RefCell<HashMap<Uuid, ProgressPatch>>>,
    schedule_refresh: &Callback<()>,
) {
    let mut outcome = None;
    let mut envelope = Some(envelope);
    dispatch.reduce_mut(|store| {
        if let Some(envelope) = envelope.take() {
            outcome = Some(apply_sse_envelope(store, envelope));
        }
    });
    match outcome.unwrap_or(SseApplyOutcome::Applied) {
        SseApplyOutcome::Applied => {}
        SseApplyOutcome::Progress(patch) => {
            progress_buffer.borrow_mut().insert(patch.id, patch);
        }
        SseApplyOutcome::Refresh => schedule_refresh.emit(()),
        SseApplyOutcome::SystemRates {
            download_bps,
            upload_bps,
        } => {
            let rates = SystemRates {
                download_bps,
                upload_bps,
            };
            dispatch.reduce_mut(|store| {
                store.system.rates = rates;
            });
        }
    }
}

/// Entrypoint invoked by Trunk for wasm32 builds.
pub fn run_app() {
    console_error_panic_hook::set_once();
    if let Some(root) = gloo::utils::document().get_element_by_id("root") {
        yew::Renderer::<RevaerApp>::with_root(root).render();
    } else {
        yew::Renderer::<RevaerApp>::new().render();
    }
}
