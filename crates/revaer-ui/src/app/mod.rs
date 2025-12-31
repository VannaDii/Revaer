use crate::app::api::ApiCtx;
use crate::app::sse::{SseHandle, connect_sse};
use crate::components::auth::AuthPrompt;
use crate::components::detail::FileSelectionChange;
use crate::components::factory_reset::FactoryResetModal;
use crate::components::setup::{SetupAuthMode, SetupCompleteInput, SetupPrompt};
use crate::components::shell::AppShell;
use crate::components::toast::ToastHost;
use crate::components::torrent_modals::CopyKind;
use crate::components::torrents::{TorrentView, demo_rows};
use crate::core::auth::{AuthMode, AuthState};
use crate::core::breakpoints::Breakpoint;
use crate::core::events::UiEventEnvelope;
use crate::core::logic::{
    SseView, build_sse_query, build_torrent_filter_query, parse_torrent_filter_query,
};
use crate::core::store::{
    AppModeState, AppStore, HealthSnapshot, SseApplyOutcome, SseConnectionState, SseError,
    SseStatus, SystemRates, apply_sse_envelope, select_system_rates,
};
use crate::core::theme::ThemeMode;
use crate::core::ui::{Density, UiMode};
use crate::features::dashboard::DashboardPage;
use crate::features::logs::view::LogsPage;
use crate::features::settings::view::SettingsPage;
use crate::features::torrents::actions::{TorrentAction, success_message};
use crate::features::torrents::state::{
    ProgressPatch, SelectionSet, TorrentRow, TorrentsPaging, TorrentsQueryModel, append_rows,
    apply_progress_patch, remove_row, select_selected_detail, select_visible_ids, set_rows,
    set_selected, set_selected_id, update_detail_file_priority, update_detail_file_selection,
    update_detail_options, update_detail_skip_fluff, upsert_detail,
};
use crate::i18n::{DEFAULT_LOCALE, LocaleCode, TranslationBundle};
use crate::models::{
    AddTorrentInput, FilePriorityOverride, NavLabels, Toast, ToastKind, TorrentAuthorRequest,
    TorrentOptionsRequest, TorrentSelectionRequest, demo_detail, demo_snapshot,
};
use crate::services::sse::SseDecodeError;
use gloo::console;
use gloo::events::EventListener;
use gloo::storage::{LocalStorage, Storage};
use gloo::utils::window;
use gloo_timers::callback::{Interval, Timeout};
use gloo_timers::future::TimeoutFuture;
use js_sys::Date;
use preferences::{
    DENSITY_KEY, LOCALE_KEY, MODE_KEY, THEME_KEY, allow_anonymous, api_base_url,
    clear_auth_storage, load_api_key_expires_at_ms, load_auth_mode, load_auth_state,
    load_bypass_local, load_density, load_locale, load_mode, load_theme,
    persist_api_key_with_expiry, persist_auth_state, persist_bypass_local,
};
pub(crate) use routes::Route;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use uuid::Uuid;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::{Dispatch, use_selector};

pub(crate) mod api;
pub(crate) mod logs_sse;
mod preferences;
mod routes;
mod sse;

const TOKEN_REFRESH_SKEW_MS: i64 = 86_400_000;

#[function_component(RevaerApp)]
pub fn revaer_app() -> Html {
    let breakpoint = use_state(current_breakpoint);
    let allow_anon = allow_anonymous();
    let dispatch = Dispatch::<AppStore>::new();
    let api_ctx = use_memo(|_| ApiCtx::new(api_base_url()), ());
    let dashboard = use_state(demo_snapshot);
    let config_snapshot = use_state(|| None::<Value>);
    let config_error = use_state(|| None::<String>);
    let config_busy = use_state(|| false);
    let config_save_busy = use_state(|| false);
    let test_busy = use_state(|| false);
    let factory_reset_open = use_state(|| false);
    let factory_reset_busy = use_state(|| false);
    let toast_id = use_state(|| 0u64);
    let sse_handle = use_mut_ref(|| None as Option<SseHandle>);
    let sse_reset = use_state(|| 0u32);
    let refresh_timer = use_mut_ref(|| None as Option<Timeout>);
    let token_refresh_timer = use_mut_ref(|| None as Option<Timeout>);
    let token_refresh_tick = use_state(|| 0u32);
    let progress_buffer = use_mut_ref(|| HashMap::<Uuid, ProgressPatch>::new());
    let progress_flush = use_mut_ref(|| None as Option<Interval>);
    let mode = use_selector(|store: &AppStore| store.ui.mode);
    let density = use_selector(|store: &AppStore| store.ui.density);
    let locale = use_selector(|store: &AppStore| store.ui.locale);
    let bundle = {
        let locale = *locale;
        use_memo(move |_| TranslationBundle::new(locale), locale)
    };

    let nav_labels = {
        let bundle = (*bundle).clone();
        NavLabels {
            dashboard: bundle.text("nav.dashboard"),
            torrents: bundle.text("nav.torrents"),
            logs: bundle.text("nav.logs"),
            categories: bundle.text("nav.categories"),
            tags: bundle.text("nav.tags"),
            settings: bundle.text("nav.settings"),
            health: bundle.text("nav.health"),
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
    let setup_auth_mode = use_state(|| SetupAuthMode::ApiKey);
    let theme = use_selector(|store: &AppStore| store.ui.theme);
    let toasts = use_selector(|store: &AppStore| store.ui.toasts.clone());
    let add_busy = use_selector(|store: &AppStore| store.ui.busy.add_torrent);
    let create_busy = use_selector(|store: &AppStore| store.ui.busy.create_torrent);
    let visible_ids = use_selector(|store: &AppStore| select_visible_ids(&store.torrents));
    let selected_id = use_selector(|store: &AppStore| store.torrents.selected_id);
    let selected_ids = use_selector(|store: &AppStore| store.torrents.selected.clone());
    let selected_detail = use_selector(|store: &AppStore| select_selected_detail(&store.torrents));
    let create_result = use_selector(|store: &AppStore| store.torrents.create_result.clone());
    let create_error = use_selector(|store: &AppStore| store.torrents.create_error.clone());
    let filters = use_selector(|store: &AppStore| store.torrents.filters.clone());
    let tag_options = use_selector(|store: &AppStore| {
        let mut tags: Vec<String> = store.labels.tags.keys().cloned().collect();
        tags.sort();
        tags.into_iter()
            .map(|tag| {
                let value = AttrValue::from(tag);
                (value.clone(), value)
            })
            .collect::<Vec<(AttrValue, AttrValue)>>()
    });
    let paging_state = use_selector(|store: &AppStore| store.torrents.paging.clone());
    let paging_limit = use_selector(|store: &AppStore| store.torrents.paging.limit);
    let system_rates = use_selector(select_system_rates);
    let auth_prompt_dismissed = use_state(|| false);

    let auth_mode = *auth_mode;
    let auth_state_value = (*auth_state).clone();
    let app_mode_value = *app_mode;
    let bypass_local_value = *bypass_local;
    let setup_token_value = (*setup_token).clone();
    let setup_expires_value = (*setup_expires).clone();
    let setup_error_value = (*setup_error).clone();
    let setup_busy_value = *setup_busy;
    let setup_auth_mode_value = *setup_auth_mode;
    let theme_value = *theme;
    let mode_value = *mode;
    let density_value = *density;
    let toasts_value = (*toasts).clone();
    let add_busy_value = *add_busy;
    let create_busy_value = *create_busy;
    let visible_ids = (*visible_ids).clone();
    let selected_id_value = *selected_id;
    let selected_ids_value = (*selected_ids).clone();
    let selected_detail_value = (*selected_detail).clone();
    let create_result_value = (*create_result).clone();
    let create_error_value = (*create_error).clone();
    let filters_value = (*filters).clone();
    let tag_options_value = (*tag_options).clone();
    let paging_state_value = (*paging_state).clone();
    let search = filters_value.name.clone();
    let state_filter_value = filters_value.state.clone().unwrap_or_default();
    let tags_filter_value = filters_value.tags.clone();
    let tracker_filter_value = filters_value.tracker.clone().unwrap_or_default();
    let extension_filter_value = filters_value.extension.clone().unwrap_or_default();
    let can_load_more = paging_state_value.next_cursor.is_some();
    let paging_is_loading = paging_state_value.is_loading;
    let system_rates_value = *system_rates;
    let config_snapshot_value = (*config_snapshot).clone();
    let config_error_value = (*config_error).clone();
    let config_busy_value = *config_busy;
    let config_save_busy_value = *config_save_busy;
    let test_busy_value = *test_busy;
    let settings_base_url = api_base_url();
    let dismiss_auth_prompt = {
        let auth_prompt_dismissed = auth_prompt_dismissed.clone();
        Callback::from(move |_| auth_prompt_dismissed.set(true))
    };

    let location = use_location();
    let navigator = use_navigator();
    let current_route = use_route::<Route>().unwrap_or_else(|| {
        let Some(location) = location.as_ref() else {
            return Route::NotFound;
        };
        let path = location.path();
        match path {
            "/" => Route::Dashboard,
            "/torrents" => Route::Torrents,
            "/settings" => Route::Settings,
            "/logs" => Route::Logs,
            _ => path
                .strip_prefix("/torrents/")
                .map(|id| Route::TorrentDetail { id: id.to_string() })
                .unwrap_or(Route::NotFound),
        }
    });
    let selected_route_id = match current_route.clone() {
        Route::TorrentDetail { id } => Uuid::parse_str(&id).ok(),
        _ => None,
    };
    {
        let auth_prompt_dismissed = auth_prompt_dismissed.clone();
        let auth_state_value = auth_state_value.clone();
        use_effect_with_deps(
            move |auth_state| {
                if auth_state.is_some() {
                    auth_prompt_dismissed.set(false);
                }
                || ()
            },
            auth_state_value,
        );
    }

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
                let mode = load_mode();
                let density = load_density();
                let locale = load_locale();
                dispatch.reduce_mut(|store| {
                    store.ui.theme = theme;
                    store.ui.mode = mode;
                    store.ui.density = density;
                    store.ui.locale = locale;
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
        let token_refresh_timer = token_refresh_timer.clone();
        let auth_state = auth_state.clone();
        let token_refresh_tick = token_refresh_tick.clone();
        let token_refresh_tick_value = *token_refresh_tick;
        let api_ctx = (*api_ctx).clone();
        let dispatch = dispatch.clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        use_effect_with_deps(
            move |(auth_state, _tick)| {
                let cleanup = || ();
                token_refresh_timer.borrow_mut().take();
                let Some(AuthState::ApiKey(api_key)) = auth_state.as_ref().clone() else {
                    return cleanup;
                };
                let now_ms = Date::now() as i64;
                let delay_ms = if let Some(expires_at_ms) = load_api_key_expires_at_ms() {
                    if expires_at_ms <= now_ms {
                        0
                    } else {
                        let refresh_at_ms = expires_at_ms.saturating_sub(TOKEN_REFRESH_SKEW_MS);
                        refresh_at_ms.saturating_sub(now_ms).max(0)
                    }
                } else {
                    0
                };
                let delay_ms_u32 = u32::try_from(delay_ms).unwrap_or(u32::MAX);
                let token_refresh_timer_handle = token_refresh_timer.clone();
                let dispatch = dispatch.clone();
                let client = api_ctx.client.clone();
                let toast_id = toast_id.clone();
                let token_refresh_tick = token_refresh_tick.clone();
                let handle = Timeout::new(delay_ms_u32, move || {
                    token_refresh_timer_handle.borrow_mut().take();
                    let dispatch = dispatch.clone();
                    let toast_id = toast_id.clone();
                    let client = client.clone();
                    let bundle = bundle.clone();
                    let token_refresh_tick = token_refresh_tick.clone();
                    let api_key = api_key.clone();
                    yew::platform::spawn_local(async move {
                        let state = dispatch.get();
                        if !matches!(state.auth.state, Some(AuthState::ApiKey(_))) {
                            return;
                        }
                        match client.refresh_api_key().await {
                            Ok(response) => {
                                persist_api_key_with_expiry(&api_key, &response.api_key_expires_at);
                                token_refresh_tick.set(*token_refresh_tick + 1);
                            }
                            Err(err) => {
                                let detail = detail_or_fallback(
                                    err.detail.clone(),
                                    bundle.text("toast.api_key_refresh_failed"),
                                );
                                push_toast(&dispatch, &toast_id, ToastKind::Error, detail);
                            }
                        }
                    });
                });
                *token_refresh_timer.borrow_mut() = Some(handle);
                cleanup
            },
            (auth_state.clone(), token_refresh_tick_value),
        );
    }
    {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        let toast_id = toast_id.clone();
        use_effect_with_deps(
            move |_| {
                let client = api_ctx.client.clone();
                let dispatch = dispatch.clone();
                let toast_id = toast_id.clone();
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
                            let message = detail_or_fallback(
                                err.detail.clone(),
                                "Health check failed.".to_string(),
                            );
                            dispatch.reduce_mut(|store| {
                                store.auth.setup_error = Some(message.clone());
                                store.auth.app_mode = AppModeState::Active;
                                store.health.basic = None;
                            });
                            push_toast(&dispatch, &toast_id, ToastKind::Error, message);
                        }
                    }
                });
                || ()
            },
            (),
        );
    }
    let request_setup_token = {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        let toast_id = toast_id.clone();
        Callback::from(move |_| {
            dispatch.reduce_mut(|store| {
                store.auth.setup_busy = true;
            });
            let dispatch = dispatch.clone();
            let client = api_ctx.client.clone();
            let toast_id = toast_id.clone();
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
                            let message = detail_or_fallback(
                                err.detail.clone(),
                                "Setup token request failed.".to_string(),
                            );
                            dispatch.reduce_mut(|store| {
                                store.auth.setup_error = Some(message.clone());
                            });
                            push_toast(&dispatch, &toast_id, ToastKind::Error, message);
                        }
                    }
                }
                dispatch.reduce_mut(|store| {
                    store.auth.setup_busy = false;
                });
            });
        })
    };
    let on_setup_auth_mode_change = {
        let setup_auth_mode = setup_auth_mode.clone();
        Callback::from(move |mode: SetupAuthMode| {
            setup_auth_mode.set(mode);
        })
    };

    let complete_setup = {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        let toast_id = toast_id.clone();
        Callback::from(move |input: SetupCompleteInput| {
            dispatch.reduce_mut(|store| {
                store.auth.setup_busy = true;
            });
            let dispatch = dispatch.clone();
            let client = api_ctx.client.clone();
            let toast_id = toast_id.clone();
            yew::platform::spawn_local(async move {
                let auth_mode = input.auth_mode;
                let mut changeset = serde_json::Value::Object(serde_json::Map::new());
                if auth_mode == SetupAuthMode::NoAuth {
                    let snapshot = match client.fetch_well_known_snapshot().await {
                        Ok(value) => value,
                        Err(err) => {
                            let message = detail_or_fallback(
                                err.detail.clone(),
                                "Setup snapshot request failed.".to_string(),
                            );
                            dispatch.reduce_mut(|store| {
                                store.auth.setup_error = Some(message.clone());
                                store.auth.setup_busy = false;
                            });
                            push_toast(&dispatch, &toast_id, ToastKind::Error, message);
                            return;
                        }
                    };
                    let mut app_profile = match snapshot.get("app_profile") {
                        Some(value) => value.clone(),
                        None => {
                            let message = "Setup snapshot missing app profile.".to_string();
                            dispatch.reduce_mut(|store| {
                                store.auth.setup_error = Some(message.clone());
                                store.auth.setup_busy = false;
                            });
                            push_toast(&dispatch, &toast_id, ToastKind::Error, message);
                            return;
                        }
                    };
                    let Some(map) = app_profile.as_object_mut() else {
                        let message = "Setup snapshot app profile is invalid.".to_string();
                        dispatch.reduce_mut(|store| {
                            store.auth.setup_error = Some(message.clone());
                            store.auth.setup_busy = false;
                        });
                        push_toast(&dispatch, &toast_id, ToastKind::Error, message);
                        return;
                    };
                    map.insert(
                        "auth_mode".to_string(),
                        serde_json::Value::String("none".into()),
                    );
                    changeset = serde_json::json!({ "app_profile": app_profile });
                }
                match client.setup_complete(&input.token, changeset).await {
                    Ok(response) => {
                        let snapshot_auth_mode = response
                            .snapshot
                            .get("app_profile")
                            .and_then(|profile| profile.get("auth_mode"))
                            .and_then(|value| value.as_str());
                        let use_anonymous = snapshot_auth_mode == Some("none")
                            || auth_mode == SetupAuthMode::NoAuth;
                        if use_anonymous {
                            let state = AuthState::Anonymous;
                            persist_auth_state(&state);
                            dispatch.reduce_mut(|store| {
                                store.auth.mode = AuthMode::ApiKey;
                                store.auth.state = Some(state);
                                store.auth.setup_error = None;
                                store.auth.setup_token = None;
                                store.auth.setup_expires_at = None;
                                store.auth.app_mode = AppModeState::Active;
                            });
                        } else {
                            let api_key = response.api_key.clone();
                            let expires_at = response.api_key_expires_at.clone();
                            persist_api_key_with_expiry(&api_key, &expires_at);
                            dispatch.reduce_mut(|store| {
                                store.auth.mode = AuthMode::ApiKey;
                                store.auth.state = Some(AuthState::ApiKey(api_key));
                                store.auth.setup_error = None;
                                store.auth.setup_token = None;
                                store.auth.setup_expires_at = None;
                                store.auth.app_mode = AppModeState::Active;
                            });
                        }
                    }
                    Err(err) => {
                        let message = detail_or_fallback(
                            err.detail.clone(),
                            "Setup completion failed.".to_string(),
                        );
                        dispatch.reduce_mut(|store| {
                            store.auth.setup_error = Some(message.clone());
                        });
                        push_toast(&dispatch, &toast_id, ToastKind::Error, message);
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
        let app_mode = app_mode.clone();
        let progress_buffer = progress_buffer.clone();
        let schedule_refresh = schedule_refresh.clone();
        let sse_query = sse_query.clone();
        let sse_reset = *sse_reset;
        let app_mode_value = *app_mode;
        use_effect_with_deps(
            move |(auth_state_value, app_mode_value, _reset, query)| {
                let cleanup_handle = sse_handle.clone();
                let cleanup = move || {
                    if let Some(handle) = cleanup_handle.borrow_mut().take() {
                        handle.close();
                    }
                };
                if let Some(handle) = sse_handle.borrow_mut().take() {
                    handle.close();
                }
                if *app_mode_value == AppModeState::Setup {
                    dispatch.reduce_mut(|store| {
                        store.system.sse_status = SseStatus {
                            state: SseConnectionState::Disconnected,
                            backoff_ms: None,
                            next_retry_at_ms: None,
                            last_event_id: store.system.sse_status.last_event_id,
                            last_error: Some(SseError {
                                message: "setup required".to_string(),
                                status_code: Some(409),
                            }),
                            auth_mode: None,
                        };
                    });
                    return cleanup;
                }
                let auth_state_value = (**auth_state_value).clone();
                if let Some(auth_state_value) = auth_state_value {
                    let auth_mode = auth_mode_label(&Some(auth_state_value.clone()));
                    let on_state = {
                        let dispatch = dispatch.clone();
                        Callback::from(move |state: SseStatus| {
                            if matches!(
                                state.last_error.as_ref().and_then(|err| err.status_code),
                                Some(409)
                            ) {
                                clear_auth_storage();
                            }
                            dispatch.reduce_mut(|store| {
                                if matches!(
                                    state.last_error.as_ref().and_then(|err| err.status_code),
                                    Some(409)
                                ) {
                                    store.auth.app_mode = AppModeState::Setup;
                                    store.auth.state = None;
                                }
                                store.system.sse_status = state;
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
                            store.system.sse_status = SseStatus {
                                state: SseConnectionState::Disconnected,
                                backoff_ms: None,
                                next_retry_at_ms: None,
                                last_event_id: store.system.sse_status.last_event_id,
                                last_error: Some(SseError {
                                    message: "SSE unavailable".to_string(),
                                    status_code: None,
                                }),
                                auth_mode: auth_mode.clone(),
                            };
                        });
                    }
                } else {
                    dispatch.reduce_mut(|store| {
                        store.system.sse_status = SseStatus {
                            state: SseConnectionState::Disconnected,
                            backoff_ms: None,
                            next_retry_at_ms: None,
                            last_event_id: store.system.sse_status.last_event_id,
                            last_error: Some(SseError {
                                message: "awaiting authentication".to_string(),
                                status_code: None,
                            }),
                            auth_mode: None,
                        };
                    });
                }
                cleanup
            },
            (auth_state.clone(), app_mode_value, sse_reset, sse_query),
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
        let mode = *mode;
        use_effect_with_deps(
            move |mode| {
                LocalStorage::set(
                    MODE_KEY,
                    match *mode {
                        UiMode::Simple => "simple",
                        UiMode::Advanced => "advanced",
                    },
                )
                .ok();
                || ()
            },
            mode,
        );
    }
    {
        let density = *density;
        use_effect_with_deps(
            move |density| {
                LocalStorage::set(
                    DENSITY_KEY,
                    match *density {
                        Density::Compact => "compact",
                        Density::Normal => "normal",
                        Density::Comfy => "comfy",
                    },
                )
                .ok();
                || ()
            },
            density,
        );
    }
    {
        let locale = *locale;
        use_effect_with_deps(
            move |locale| {
                LocalStorage::set(LOCALE_KEY, locale.code()).ok();
                apply_direction(TranslationBundle::new(*locale).rtl());
                || ()
            },
            locale,
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

    let set_density = {
        let dispatch = dispatch.clone();
        Callback::from(move |next: Density| {
            dispatch.reduce_mut(|store| {
                store.ui.density = next;
            });
        })
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
        Callback::from(move |values: Vec<String>| {
            dispatch.reduce_mut(|store| {
                store.torrents.filters.tags = values;
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
                store.system.sse_status = SseStatus {
                    state: SseConnectionState::Reconnecting,
                    backoff_ms: Some(0),
                    next_retry_at_ms: Some(Date::now() as u64),
                    last_event_id: store.system.sse_status.last_event_id,
                    last_error: Some(SseError {
                        message: "manual reconnect".to_string(),
                        status_code: None,
                    }),
                    auth_mode: auth_mode_label(&store.auth.state),
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
    let on_save_auth = {
        let dispatch = dispatch.clone();
        Callback::from(move |state: AuthState| {
            let next_mode = match state {
                AuthState::Local(_) => AuthMode::Local,
                _ => AuthMode::ApiKey,
            };
            persist_auth_state(&state);
            dispatch.reduce_mut(|store| {
                store.auth.mode = next_mode;
                store.auth.state = Some(state);
            });
        })
    };
    let on_test_connection = {
        let api_ctx = (*api_ctx).clone();
        let dispatch = dispatch.clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        let test_busy = test_busy.clone();
        Callback::from(move |_| {
            if *test_busy {
                return;
            }
            test_busy.set(true);
            let client = api_ctx.client.clone();
            let dispatch = dispatch.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            let test_busy = test_busy.clone();
            yew::platform::spawn_local(async move {
                match client.fetch_health().await {
                    Ok(health) => {
                        dispatch.reduce_mut(|store| {
                            store.health.basic = Some(HealthSnapshot {
                                status: health.status.clone(),
                                mode: health.mode.clone(),
                                database_status: Some(health.database.status),
                                database_revision: health.database.revision,
                            });
                        });
                        push_toast(
                            &dispatch,
                            &toast_id,
                            ToastKind::Success,
                            bundle.text("settings.test_success"),
                        );
                    }
                    Err(err) => {
                        let message = detail_or_fallback(
                            err.detail.clone(),
                            bundle.text("settings.test_failed"),
                        );
                        push_toast(&dispatch, &toast_id, ToastKind::Error, message);
                    }
                }
                test_busy.set(false);
            });
        })
    };
    let on_server_restart = {
        let dispatch = dispatch.clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |_| {
            push_toast(
                &dispatch,
                &toast_id,
                ToastKind::Info,
                bundle.text("toast.server_restart_unavailable"),
            );
        })
    };
    let on_refresh_config = {
        let api_ctx = (*api_ctx).clone();
        let dispatch = dispatch.clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        let config_snapshot = config_snapshot.clone();
        let config_error = config_error.clone();
        let config_busy = config_busy.clone();
        Callback::from(move |_| {
            if *config_busy {
                return;
            }
            config_busy.set(true);
            config_error.set(None);
            let client = api_ctx.client.clone();
            let dispatch = dispatch.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            let config_snapshot = config_snapshot.clone();
            let config_error = config_error.clone();
            let config_busy = config_busy.clone();
            yew::platform::spawn_local(async move {
                match client.fetch_config_snapshot().await {
                    Ok(snapshot) => {
                        config_snapshot.set(Some(snapshot));
                        config_error.set(None);
                    }
                    Err(err) => {
                        let message = detail_or_fallback(
                            err.detail.clone(),
                            bundle.text("settings.config_failed"),
                        );
                        config_error.set(Some(message.clone()));
                        push_toast(&dispatch, &toast_id, ToastKind::Error, message);
                    }
                }
                config_busy.set(false);
            });
        })
    };
    let on_apply_settings = {
        let api_ctx = (*api_ctx).clone();
        let dispatch = dispatch.clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        let config_snapshot = config_snapshot.clone();
        let config_error = config_error.clone();
        let config_save_busy = config_save_busy.clone();
        Callback::from(move |changeset: Value| {
            if *config_save_busy {
                return;
            }
            if changeset
                .as_object()
                .map(|map| map.is_empty())
                .unwrap_or(true)
            {
                return;
            }
            config_save_busy.set(true);
            config_error.set(None);
            let client = api_ctx.client.clone();
            let dispatch = dispatch.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            let config_snapshot = config_snapshot.clone();
            let config_error = config_error.clone();
            let config_save_busy = config_save_busy.clone();
            yew::platform::spawn_local(async move {
                match client.patch_settings(changeset).await {
                    Ok(snapshot) => {
                        config_snapshot.set(Some(snapshot));
                        config_error.set(None);
                        push_toast(
                            &dispatch,
                            &toast_id,
                            ToastKind::Success,
                            bundle.text("settings.saved"),
                        );
                    }
                    Err(err) => {
                        let detail = detail_or_fallback(
                            err.detail.clone(),
                            bundle.text("settings.save_failed"),
                        );
                        config_error.set(Some(detail.clone()));
                        push_toast(&dispatch, &toast_id, ToastKind::Error, detail);
                    }
                }
                config_save_busy.set(false);
            });
        })
    };
    let on_server_logs = {
        let navigator = navigator.clone();
        Callback::from(move |_| {
            if let Some(navigator) = navigator.clone() {
                navigator.push(&Route::Logs);
            }
        })
    };
    let on_logs_error = {
        let dispatch = dispatch.clone();
        let toast_id = toast_id.clone();
        Callback::from(move |message: String| {
            if message.trim().is_empty() {
                return;
            }
            push_toast(&dispatch, &toast_id, ToastKind::Error, message);
        })
    };
    let on_factory_reset = {
        let factory_reset_open = factory_reset_open.clone();
        Callback::from(move |_| factory_reset_open.set(true))
    };
    let on_factory_reset_close = {
        let factory_reset_open = factory_reset_open.clone();
        let factory_reset_busy = factory_reset_busy.clone();
        Callback::from(move |_| {
            if *factory_reset_busy {
                return;
            }
            factory_reset_open.set(false);
        })
    };
    let on_factory_reset_confirm = {
        let api_ctx = (*api_ctx).clone();
        let dispatch = dispatch.clone();
        let toast_id = toast_id.clone();
        let factory_reset_busy = factory_reset_busy.clone();
        let factory_reset_open = factory_reset_open.clone();
        Callback::from(move |confirm: String| {
            if *factory_reset_busy {
                return;
            }
            factory_reset_busy.set(true);
            let client = api_ctx.client.clone();
            let dispatch = dispatch.clone();
            let toast_id = toast_id.clone();
            let factory_reset_busy = factory_reset_busy.clone();
            let factory_reset_open = factory_reset_open.clone();
            yew::platform::spawn_local(async move {
                match client.factory_reset(&confirm).await {
                    Ok(()) => {
                        clear_auth_storage();
                        dispatch.reduce_mut(|store| {
                            store.auth.app_mode = AppModeState::Setup;
                            store.auth.state = None;
                            store.auth.setup_token = None;
                            store.auth.setup_expires_at = None;
                            store.auth.setup_error = None;
                        });
                        factory_reset_busy.set(false);
                        factory_reset_open.set(false);
                        if window().location().reload().is_err() {
                            push_toast(
                                &dispatch,
                                &toast_id,
                                ToastKind::Error,
                                "Factory reset completed but reload failed.".to_string(),
                            );
                        }
                    }
                    Err(err) => {
                        let detail = detail_or_fallback(
                            err.detail.clone(),
                            "Factory reset failed.".to_string(),
                        );
                        push_toast(&dispatch, &toast_id, ToastKind::Error, detail);
                        factory_reset_busy.set(false);
                        factory_reset_open.set(true);
                    }
                }
            });
        })
    };
    let on_logout = {
        let dispatch = dispatch.clone();
        let auth_prompt_dismissed = auth_prompt_dismissed.clone();
        Callback::from(move |_| {
            clear_auth_storage();
            auth_prompt_dismissed.set(false);
            dispatch.reduce_mut(|store| {
                store.auth.state = None;
            });
        })
    };
    {
        let on_refresh_config = on_refresh_config.clone();
        let auth_state_value = auth_state_value.clone();
        let current_route = current_route.clone();
        use_effect_with_deps(
            move |(route, _auth_state)| {
                if matches!(route, Route::Settings) {
                    on_refresh_config.emit(());
                }
                || ()
            },
            (current_route, auth_state_value),
        );
    }
    let on_copy_payload = {
        let dispatch = dispatch.clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |(kind, value): (CopyKind, String)| {
            let dispatch = dispatch.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            yew::platform::spawn_local(async move {
                match copy_text_to_clipboard(value).await {
                    Ok(()) => {
                        let message = match kind {
                            CopyKind::Magnet => bundle.text("toast.magnet_copied"),
                            CopyKind::Metainfo => bundle.text("toast.metainfo_copied"),
                        };
                        push_toast(&dispatch, &toast_id, ToastKind::Success, message);
                    }
                    Err(err) => {
                        push_toast(&dispatch, &toast_id, ToastKind::Error, err);
                    }
                }
            });
        })
    };
    let on_copy_value = {
        let dispatch = dispatch.clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |value: String| {
            let dispatch = dispatch.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            yew::platform::spawn_local(async move {
                match copy_text_to_clipboard(value).await {
                    Ok(()) => push_toast(
                        &dispatch,
                        &toast_id,
                        ToastKind::Success,
                        bundle.text("toast.copied"),
                    ),
                    Err(err) => {
                        push_toast(&dispatch, &toast_id, ToastKind::Error, err);
                    }
                }
            });
        })
    };
    let on_error_toast = {
        let dispatch = dispatch.clone();
        let toast_id = toast_id.clone();
        Callback::from(move |message: String| {
            push_toast(&dispatch, &toast_id, ToastKind::Error, message);
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
                            bundle.text("toast.add_success"),
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
                    Err(err) => {
                        let message =
                            detail_or_fallback(err.detail.clone(), bundle.text("toast.add_failed"));
                        push_toast(&dispatch, &toast_id, ToastKind::Error, message);
                    }
                }
                dispatch.reduce_mut(|store| {
                    store.ui.busy.add_torrent = false;
                });
            });
        })
    };
    let on_reset_create = {
        let dispatch = dispatch.clone();
        Callback::from(move |_| {
            dispatch.reduce_mut(|store| {
                store.torrents.create_result = None;
                store.torrents.create_error = None;
            });
        })
    };
    let on_create_torrent = {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |request: TorrentAuthorRequest| {
            let client = api_ctx.client.clone();
            let dispatch = dispatch.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            dispatch.reduce_mut(|store| {
                store.ui.busy.create_torrent = true;
                store.torrents.create_error = None;
                store.torrents.create_result = None;
            });
            yew::platform::spawn_local(async move {
                match client.create_torrent(&request).await {
                    Ok(response) => {
                        dispatch.reduce_mut(|store| {
                            store.torrents.create_result = Some(response);
                        });
                        push_toast(
                            &dispatch,
                            &toast_id,
                            ToastKind::Success,
                            bundle.text("toast.create_success"),
                        );
                    }
                    Err(err) => {
                        let message = detail_or_fallback(
                            err.detail.clone(),
                            bundle.text("toast.create_failed"),
                        );
                        dispatch.reduce_mut(|store| {
                            store.torrents.create_error = Some(message.clone());
                        });
                        push_toast(&dispatch, &toast_id, ToastKind::Error, message);
                    }
                }
                dispatch.reduce_mut(|store| {
                    store.ui.busy.create_torrent = false;
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
                        format!("{} {id}", bundle.text("toast.torrent_placeholder"))
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
                            bundle.text("toast.action_failed")
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
            spawn_bulk_actions(
                client,
                dispatch.clone(),
                toast_id.clone(),
                bundle.clone(),
                action,
                ids,
            );
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
            let request = match change {
                FileSelectionChange::Toggle {
                    index,
                    path,
                    selected,
                } => {
                    dispatch.reduce_mut(|store| {
                        update_detail_file_selection(&mut store.torrents, id, index, selected);
                    });
                    TorrentSelectionRequest {
                        include: if selected {
                            vec![path.clone()]
                        } else {
                            Vec::new()
                        },
                        exclude: if selected {
                            Vec::new()
                        } else {
                            vec![path.clone()]
                        },
                        skip_fluff: None,
                        priorities: Vec::new(),
                    }
                }
                FileSelectionChange::Priority { index, priority } => {
                    dispatch.reduce_mut(|store| {
                        update_detail_file_priority(&mut store.torrents, id, index, priority);
                    });
                    TorrentSelectionRequest {
                        include: Vec::new(),
                        exclude: Vec::new(),
                        skip_fluff: None,
                        priorities: vec![FilePriorityOverride { index, priority }],
                    }
                }
                FileSelectionChange::SkipFluff { enabled } => {
                    dispatch.reduce_mut(|store| {
                        update_detail_skip_fluff(&mut store.torrents, id, enabled);
                    });
                    TorrentSelectionRequest {
                        include: Vec::new(),
                        exclude: Vec::new(),
                        skip_fluff: Some(enabled),
                        priorities: Vec::new(),
                    }
                }
            };
            yew::platform::spawn_local(async move {
                if let Err(err) = client
                    .update_torrent_selection(&id.to_string(), &request)
                    .await
                {
                    let message = detail_or_fallback(
                        err.detail.clone(),
                        bundle.text("toast.file_selection_failed"),
                    );
                    push_toast(&dispatch, &toast_id, ToastKind::Error, message);
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
    let on_update_options = {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |(id, request): (Uuid, TorrentOptionsRequest)| {
            let client = api_ctx.client.clone();
            let dispatch = dispatch.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            dispatch.reduce_mut(|store| {
                update_detail_options(&mut store.torrents, id, &request);
            });
            yew::platform::spawn_local(async move {
                if let Err(err) = client
                    .update_torrent_options(&id.to_string(), &request)
                    .await
                {
                    let message =
                        detail_or_fallback(err.detail.clone(), bundle.text("toast.options_failed"));
                    push_toast(&dispatch, &toast_id, ToastKind::Error, message);
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
        let locale = *locale;
        let dispatch = dispatch.clone();
        let active_flag = locale_flag(locale);
        let active_flag_src = format!("https://flagcdn.com/{active_flag}.svg");
        html! {
            <div class="dropdown dropdown-bottom dropdown-center">
                <div tabindex="0" class="btn btn-ghost btn-circle btn-sm cursor-pointer">
                    <img
                        src={active_flag_src}
                        alt="Locale"
                        class="rounded-box size-4.5 object-cover" />
                </div>
                <div tabindex="0" class="dropdown-content bg-base-100 rounded-box mt-2 w-40 shadow">
                    <ul class="menu w-full p-2">
                        {for LocaleCode::all().iter().map(|lc| {
                            let flag = locale_flag(*lc);
                            let flag_src = format!("https://flagcdn.com/{flag}.svg");
                            let label = lc.label();
                            let next = *lc;
                                let onclick = {
                                    let dispatch = dispatch.clone();
                                    Callback::from(move |event: MouseEvent| {
                                        event.prevent_default();
                                        dispatch.reduce_mut(|store| {
                                            store.ui.locale = next;
                                        });
                                    })
                                };
                            html! {
                                <li>
                                    <a class="flex items-center gap-2" href="#" onclick={onclick}>
                                        <img
                                            src={flag_src}
                                            alt="Locale"
                                            class="rounded-box size-4.5 cursor-pointer object-cover" />
                                        <span>{label}</span>
                                    </a>
                                </li>
                            }
                        })}
                    </ul>
                </div>
            </div>
        }
    };

    let bundle_ctx = bundle.clone();
    let bundle_routes = bundle.clone();
    let auth_state_for_routes = auth_state_value.clone();

    html! {
        <ContextProvider<ApiCtx> context={(*api_ctx).clone()}>
            <ContextProvider<TranslationBundle> context={(*bundle_ctx).clone()}>
                <AppShell
                    theme={theme_value}
                    on_toggle_theme={toggle_theme}
                    active={current_route.clone()}
                    locale_selector={locale_selector}
                    nav={nav_labels}
                    on_sse_retry={trigger_sse_reconnect.clone()}
                    on_server_restart={on_server_restart.clone()}
                    on_server_logs={on_server_logs.clone()}
                    on_factory_reset={on_factory_reset.clone()}
                    on_logout={on_logout.clone()}
                >
                    <Switch<Route> render={move |route| {
                        let bundle = (*bundle_routes).clone();
                        match route {
                            Route::Dashboard => html! {
                                <DashboardPage
                                    snapshot={(*dashboard).clone()}
                                    system_rates={system_rates_value}
                                />
                            },
                            Route::Logs => html! {
                                <LogsPage
                                    base_url={settings_base_url.clone()}
                                    auth_state={auth_state_for_routes.clone()}
                                    on_error_toast={on_logs_error.clone()}
                                />
                            },
                            Route::Torrents => html! {
                            <div class="space-y-4">
                                    <TorrentView
                                        visible_ids={visible_ids.clone()}
                                        density={density_value}
                                        mode={mode_value}
                                        on_density_change={set_density.clone()}
                                        on_bulk_action={on_bulk_action.clone()}
                                        on_action={on_action.clone()}
                                        on_add={on_add_torrent.clone()}
                                        add_busy={add_busy_value}
                                        create_result={create_result_value.clone()}
                                        create_error={create_error_value.clone()}
                                        create_busy={create_busy_value}
                                        on_create={on_create_torrent.clone()}
                                        on_reset_create={on_reset_create.clone()}
                                        on_copy_payload={on_copy_payload.clone()}
                                        search={search.clone()}
                                        on_search={set_search.clone()}
                                        state_filter={state_filter_value.clone()}
                                        tags_filter={tags_filter_value.clone()}
                                        tag_options={tag_options_value.clone()}
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
                                        on_update_options={on_update_options.clone()}
                                    />
                                </div>
                            },
                            Route::TorrentDetail { id } => html! {
                            <div class="space-y-4">
                                    <TorrentView
                                        visible_ids={visible_ids.clone()}
                                        density={density_value}
                                        mode={mode_value}
                                        on_density_change={set_density.clone()}
                                        on_bulk_action={on_bulk_action.clone()}
                                        on_action={on_action.clone()}
                                        on_add={on_add_torrent.clone()}
                                        add_busy={add_busy_value}
                                        create_result={create_result_value.clone()}
                                        create_error={create_error_value.clone()}
                                        create_busy={create_busy_value}
                                        on_create={on_create_torrent.clone()}
                                        on_reset_create={on_reset_create.clone()}
                                        on_copy_payload={on_copy_payload.clone()}
                                        search={search.clone()}
                                        on_search={set_search.clone()}
                                        state_filter={state_filter_value.clone()}
                                        tags_filter={tags_filter_value.clone()}
                                        tag_options={tag_options_value.clone()}
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
                                        on_update_options={on_update_options.clone()}
                                    />
                                </div>
                            },
                            Route::Settings => html! {
                                <SettingsPage
                                    base_url={settings_base_url.clone()}
                                    allow_anonymous={allow_anon}
                                    auth_mode={auth_mode}
                                    auth_state={auth_state_for_routes.clone()}
                                    bypass_local={bypass_local_value}
                                    on_toggle_bypass_local={on_toggle_bypass_local.clone()}
                                    on_save_auth={on_save_auth.clone()}
                                    on_test_connection={on_test_connection.clone()}
                                    test_busy={test_busy_value}
                                    on_server_restart={on_server_restart.clone()}
                                    on_server_logs={on_server_logs.clone()}
                                    config_snapshot={config_snapshot_value.clone()}
                                    config_error={config_error_value.clone()}
                                    config_busy={config_busy_value}
                                    config_save_busy={config_save_busy_value}
                                    on_refresh_config={on_refresh_config.clone()}
                                    on_apply_settings={on_apply_settings.clone()}
                                    on_copy_value={on_copy_value.clone()}
                                    on_error_toast={on_error_toast.clone()}
                                />
                            },
                            Route::NotFound => html! { <Placeholder title={bundle.text("placeholder.not_found_title")} body={bundle.text("placeholder.not_found_body")} /> },
                        }
                    }} />
                </AppShell>
                <ToastHost toasts={toasts_value.clone()} on_dismiss={dismiss_toast.clone()} />
                <FactoryResetModal
                    open={*factory_reset_open}
                    busy={*factory_reset_busy}
                    on_close={on_factory_reset_close.clone()}
                    on_confirm={on_factory_reset_confirm.clone()}
                />
                {if app_mode_value == AppModeState::Setup {
                    html! {
                        <SetupPrompt
                            token={setup_token_value.clone()}
                            expires_at={setup_expires_value.clone()}
                            busy={setup_busy_value}
                            error={setup_error_value.clone()}
                            allow_no_auth={allow_anon}
                            auth_mode={setup_auth_mode_value}
                            on_auth_mode_change={on_setup_auth_mode_change.clone()}
                            on_request_token={request_setup_token.clone()}
                            on_complete={complete_setup.clone()}
                        />
                    }
                } else if auth_state_value.is_none()
                    && !matches!(current_route, Route::Settings)
                    && !*auth_prompt_dismissed
                {
                    html! {
                        <AuthPrompt
                            allow_anonymous={allow_anon}
                            default_mode={if bypass_local_value { AuthMode::ApiKey } else { auth_mode }}
                            on_dismiss={dismiss_auth_prompt}
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
            </ContextProvider<TranslationBundle>>
        </ContextProvider<ApiCtx>>
    }
}

#[function_component(Placeholder)]
fn placeholder(props: &PlaceholderProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    html! {
        <div class="card bg-base-100 border border-base-200 shadow">
            <div class="card-body gap-2">
                <h2 class="text-lg font-semibold">{&props.title}</h2>
                <p class="text-sm text-base-content/60">{&props.body}</p>
                <span class="badge badge-ghost badge-sm">{bundle.text("placeholder.badge")}</span>
            </div>
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
                    format!("{} {}s", bundle.text("toast.rate_limited"), delay),
                );
                TimeoutFuture::new(retry_delay_ms(delay)).await;
                match client.fetch_torrents(&filters, &paging).await {
                    Ok(list) => apply_torrent_list(&dispatch, list, append),
                    Err(err) => push_toast(
                        &dispatch,
                        &toast_id,
                        ToastKind::Error,
                        detail_or_fallback(err.detail.clone(), bundle.text("toast.list_failed")),
                    ),
                }
            } else {
                push_toast(
                    &dispatch,
                    &toast_id,
                    ToastKind::Error,
                    detail_or_fallback(err.detail.clone(), bundle.text("toast.list_failed")),
                );
            }
        }
        Err(err) => push_toast(
            &dispatch,
            &toast_id,
            ToastKind::Error,
            detail_or_fallback(err.detail.clone(), bundle.text("toast.list_failed")),
        ),
    }
}

async fn fetch_torrent_detail_with_retry(
    client: std::rc::Rc<crate::services::api::ApiClient>,
    dispatch: Dispatch<AppStore>,
    toast_id: UseStateHandle<u64>,
    bundle: TranslationBundle,
    id: Uuid,
) -> Option<crate::models::TorrentDetail> {
    let id_str = id.to_string();
    match client.fetch_torrent_detail(&id_str).await {
        Ok(detail) => Some(detail),
        Err(err) if err.is_rate_limited() => {
            if let Some(delay) = err.retry_after_secs {
                push_toast(
                    &dispatch,
                    &toast_id,
                    ToastKind::Info,
                    format!("{} {}s", bundle.text("toast.rate_limited"), delay),
                );
                TimeoutFuture::new(retry_delay_ms(delay)).await;
                match client.fetch_torrent_detail(&id_str).await {
                    Ok(detail) => Some(detail),
                    Err(err) => {
                        push_toast(
                            &dispatch,
                            &toast_id,
                            ToastKind::Error,
                            detail_or_fallback(
                                err.detail.clone(),
                                bundle.text("toast.detail_failed"),
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
                    detail_or_fallback(err.detail.clone(), bundle.text("toast.detail_failed")),
                );
                None
            }
        }
        Err(err) => {
            push_toast(
                &dispatch,
                &toast_id,
                ToastKind::Error,
                detail_or_fallback(err.detail.clone(), bundle.text("toast.detail_failed")),
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

struct BulkActionState {
    queue: VecDeque<Uuid>,
    in_flight: usize,
    completed: usize,
    total: usize,
    successes: Vec<Uuid>,
    failures: Vec<String>,
}

fn spawn_bulk_actions(
    client: Rc<crate::services::api::ApiClient>,
    dispatch: Dispatch<AppStore>,
    toast_id: UseStateHandle<u64>,
    bundle: TranslationBundle,
    action: TorrentAction,
    ids: Vec<Uuid>,
) {
    const BULK_CONCURRENCY: usize = 4;
    if ids.is_empty() {
        return;
    }
    let action_is_delete = matches!(action, TorrentAction::Delete { .. });
    let total = ids.len();
    let queue = VecDeque::from(ids);
    let state = Rc::new(RefCell::new(BulkActionState {
        queue,
        in_flight: 0,
        completed: 0,
        total,
        successes: Vec::new(),
        failures: Vec::new(),
    }));
    let runner: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let runner_handle = runner.clone();
    let schedule: Rc<dyn Fn()> = Rc::new(move || {
        let mut batch = Vec::new();
        {
            let mut current = state.borrow_mut();
            while current.in_flight < BULK_CONCURRENCY {
                if let Some(id) = current.queue.pop_front() {
                    current.in_flight += 1;
                    batch.push(id);
                } else {
                    break;
                }
            }
        }
        for id in batch {
            let client = client.clone();
            let dispatch = dispatch.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            let action = action.clone();
            let state = state.clone();
            let runner_handle = runner_handle.clone();
            yew::platform::spawn_local(async move {
                let id_str = id.to_string();
                let display_name = dispatch
                    .get()
                    .torrents
                    .by_id
                    .get(&id)
                    .map(|row| row.name.clone())
                    .unwrap_or_else(|| {
                        format!("{} {id}", bundle.text("toast.torrent_placeholder"))
                    });
                let result = client.perform_action(&id_str, action.clone()).await;
                let finished = {
                    let mut current = state.borrow_mut();
                    current.in_flight = current.in_flight.saturating_sub(1);
                    current.completed = current.completed.saturating_add(1);
                    match result {
                        Ok(_) => current.successes.push(id),
                        Err(err) => {
                            let message = detail_or_fallback(
                                err.detail.clone(),
                                bundle.text("toast.detail_failed"),
                            );
                            current.failures.push(format!("{display_name}: {message}"));
                        }
                    }
                    current.completed == current.total
                };
                if finished {
                    finalize_bulk_action(&dispatch, &toast_id, &bundle, action_is_delete, &state);
                    return;
                }
                if let Some(next) = runner_handle.borrow().clone() {
                    next();
                }
            });
        }
    });
    *runner.borrow_mut() = Some(schedule.clone());
    schedule();
}

fn finalize_bulk_action(
    dispatch: &Dispatch<AppStore>,
    toast_id: &UseStateHandle<u64>,
    bundle: &TranslationBundle,
    action_is_delete: bool,
    state: &Rc<RefCell<BulkActionState>>,
) {
    let (successes, failures, total) = {
        let current = state.borrow();
        (
            current.successes.clone(),
            current.failures.clone(),
            current.total,
        )
    };
    dispatch.reduce_mut(|store| {
        if action_is_delete {
            for id in &successes {
                remove_row(&mut store.torrents, *id);
            }
        }
        let selected_len = store.torrents.selected.len();
        if selected_len > 1 {
            set_selected_id(&mut store.torrents, None);
        } else if selected_len == 1 {
            let only_id = store.torrents.selected.iter().next().copied();
            if store.torrents.selected_id != only_id {
                set_selected_id(&mut store.torrents, only_id);
            }
        } else {
            set_selected_id(&mut store.torrents, None);
        }
    });
    let failure_count = failures.len();
    let success_count = total.saturating_sub(failure_count);
    let message = if failure_count == 0 {
        format!("{} {}", bundle.text("toast.bulk_done"), total)
    } else {
        let first_error = failures.first().cloned().unwrap_or_default();
        format!(
            "{} {success_count}/{total} ({} failed). {first_error}",
            bundle.text("toast.bulk_done"),
            failure_count
        )
    };
    let kind = if failure_count == 0 {
        ToastKind::Success
    } else {
        ToastKind::Error
    };
    push_toast(dispatch, toast_id, kind, message);
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
        if let Err(err) = history.replace_state_with_url(&JsValue::NULL, "", Some(&url)) {
            log_dom_error("history.replace_state_with_url", err);
        }
    }
}

fn retry_delay_ms(delay_secs: u64) -> u32 {
    let millis = delay_secs.saturating_mul(1_000);
    match u32::try_from(millis) {
        Ok(value) => value,
        Err(_) => u32::MAX,
    }
}

fn auth_mode_label(auth: &Option<AuthState>) -> Option<String> {
    match auth {
        Some(AuthState::ApiKey(_)) => Some("API key".to_string()),
        Some(AuthState::Local(_)) => Some("Local auth".to_string()),
        Some(AuthState::Anonymous) => Some("Anonymous".to_string()),
        None => None,
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

fn detail_or_fallback(detail: Option<String>, fallback: String) -> String {
    match detail {
        Some(value) if !value.trim().is_empty() => value,
        _ => fallback,
    }
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
            if let Err(err) = body.set_attribute("data-bp", bp.name) {
                log_dom_error("body.set_attribute", err);
            }
        }
    }
}

fn apply_theme(theme: ThemeMode) {
    if let Some(document) = window().document() {
        if let Some(root) = document.document_element() {
            if let Err(err) = root.set_attribute("data-theme", theme.as_str()) {
                log_dom_error("root.set_attribute", err);
            }
        }
    }
}

fn apply_direction(is_rtl: bool) {
    if let Some(document) = window().document() {
        if let Some(root) = document.document_element() {
            if let Err(err) = root.set_attribute("dir", if is_rtl { "rtl" } else { "ltr" }) {
                log_dom_error("root.set_attribute", err);
            }
        }
    }
}

fn log_dom_error(operation: &'static str, err: JsValue) {
    console::error!("dom operation failed", operation, err);
}

fn locale_flag(locale: LocaleCode) -> &'static str {
    match locale {
        LocaleCode::Ar => "sa",
        LocaleCode::De => "de",
        LocaleCode::Es => "es",
        LocaleCode::Hi => "in",
        LocaleCode::It => "it",
        LocaleCode::Jv => "id",
        LocaleCode::Mr => "in",
        LocaleCode::Pt => "pt",
        LocaleCode::Ta => "in",
        LocaleCode::Tr => "tr",
        LocaleCode::Bn => "bd",
        LocaleCode::En => "us",
        LocaleCode::Fr => "fr",
        LocaleCode::Id => "id",
        LocaleCode::Ja => "jp",
        LocaleCode::Ko => "kr",
        LocaleCode::Pa => "in",
        LocaleCode::Ru => "ru",
        LocaleCode::Te => "in",
        LocaleCode::Zh => "cn",
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
        yew::Renderer::<RevaerRoot>::with_root(root).render();
    } else {
        yew::Renderer::<RevaerRoot>::new().render();
    }
}

#[function_component(RevaerRoot)]
fn revaer_root() -> Html {
    html! {
        <BrowserRouter>
            <RevaerApp />
        </BrowserRouter>
    }
}
