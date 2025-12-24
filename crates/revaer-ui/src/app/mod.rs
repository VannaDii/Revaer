use crate::app::api::ApiCtx;
use crate::app::sse::{SseHandle, connect_sse};
use crate::components::auth::AuthPrompt;
use crate::components::dashboard::DashboardPanel;
use crate::components::setup::SetupPrompt;
use crate::components::shell::AppShell;
use crate::components::status::SseOverlay;
use crate::components::toast::ToastHost;
use crate::components::torrents::{TorrentView, demo_rows};
use crate::core::auth::{AuthMode, AuthState};
use crate::core::breakpoints::Breakpoint;
use crate::core::events::UiEventEnvelope;
use crate::core::logic::{SseView, build_sse_query};
use crate::core::store::{AppModeState, AppStore, SseApplyOutcome, apply_sse_envelope};
use crate::core::theme::ThemeMode;
use crate::core::ui::{Density, UiMode};
use crate::features::torrents::actions::{TorrentAction, success_message};
use crate::features::torrents::state::{
    ProgressPatch, SelectionSet, apply_progress_patch, remove_row, select_selected_detail,
    select_visible_ids, select_visible_rows, set_rows, set_selected, set_selected_id,
    upsert_detail,
};
use crate::i18n::{DEFAULT_LOCALE, LocaleCode, TranslationBundle};
use crate::models::{
    AddTorrentInput, NavLabels, SseState, Toast, ToastKind, demo_detail, demo_snapshot,
};
use crate::services::sse::SseDecodeError;
use gloo::events::EventListener;
use gloo::storage::{LocalStorage, Storage};
use gloo::utils::window;
use gloo_timers::callback::{Interval, Timeout};
use preferences::{
    DENSITY_KEY, LOCALE_KEY, MODE_KEY, THEME_KEY, allow_anonymous, api_base_url, load_auth_mode,
    load_auth_state, load_density, load_locale, load_mode, load_theme, persist_auth_state,
};
pub(crate) use routes::Route;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use uuid::Uuid;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::{Dispatch, use_selector};

mod api;
mod preferences;
mod routes;
mod sse;

#[function_component(RevaerApp)]
pub fn revaer_app() -> Html {
    let theme = use_state(load_theme);
    let mode = use_state(load_mode);
    let density = use_state(load_density);
    let locale = use_state(load_locale);
    let breakpoint = use_state(current_breakpoint);
    let allow_anon = allow_anonymous();
    let dispatch = Dispatch::<AppStore>::new();
    let api_ctx = use_memo(|_| ApiCtx::new(api_base_url()), ());
    let dashboard = use_state(demo_snapshot);
    let toasts = use_state(Vec::<Toast>::new);
    let toast_id = use_state(|| 0u64);
    let add_busy = use_state(|| false);
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
    let setup_token = use_selector(|store: &AppStore| store.auth.setup_token.clone());
    let setup_expires = use_selector(|store: &AppStore| store.auth.setup_expires_at.clone());
    let setup_error = use_selector(|store: &AppStore| store.auth.setup_error.clone());
    let setup_busy = use_selector(|store: &AppStore| store.auth.setup_busy);
    let torrents_rows = use_selector(|store: &AppStore| select_visible_rows(&store.torrents));
    let visible_ids = use_selector(|store: &AppStore| select_visible_ids(&store.torrents));
    let selected_id = use_selector(|store: &AppStore| store.torrents.selected_id);
    let selected_ids = use_selector(|store: &AppStore| store.torrents.selected.clone());
    let selected_detail = use_selector(|store: &AppStore| select_selected_detail(&store.torrents));
    let filters = use_selector(|store: &AppStore| store.torrents.filters.clone());
    let sse_state = use_selector(|store: &AppStore| store.system.sse_state.clone());

    let auth_mode = *auth_mode;
    let auth_state_value = (*auth_state).clone();
    let app_mode_value = *app_mode;
    let setup_token_value = (*setup_token).clone();
    let setup_expires_value = (*setup_expires).clone();
    let setup_error_value = (*setup_error).clone();
    let setup_busy_value = *setup_busy;
    let torrents_rows = (*torrents_rows).clone();
    let visible_ids = (*visible_ids).clone();
    let selected_id_value = *selected_id;
    let selected_ids_value = (*selected_ids).clone();
    let selected_detail_value = (*selected_detail).clone();
    let filters_value = (*filters).clone();
    let search = filters_value.search.clone();
    let regex = filters_value.regex;
    let sse_state_value = (*sse_state).clone();

    let current_route = use_route::<Route>().unwrap_or(Route::Torrents);
    let selected_route_id = match current_route.clone() {
        Route::TorrentDetail { id } => Uuid::parse_str(&id).ok(),
        _ => None,
    };

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
                dispatch.reduce_mut(|store| {
                    store.auth.mode = mode;
                    store.auth.state = state;
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
                            });
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
        let api_ctx = (*api_ctx).clone();
        use_effect_with_deps(
            move |auth_state| {
                if auth_state.as_ref().is_some() {
                    let dashboard_client = api_ctx.client.clone();
                    yew::platform::spawn_local(async move {
                        if let Ok(snapshot) = dashboard_client.fetch_dashboard().await {
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
        let filters = filters.clone();
        let auth_state = auth_state.clone();
        use_effect_with_deps(
            move |(filters, auth_state)| {
                let auth_state = (**auth_state).clone();
                let filters = (**filters).clone();
                let dispatch = dispatch.clone();
                let client = api_ctx.client.clone();
                yew::platform::spawn_local(async move {
                    if auth_state.is_some() {
                        let search = if filters.search.trim().is_empty() {
                            None
                        } else {
                            Some(filters.search.clone())
                        };
                        match client.fetch_torrents(search, filters.regex).await {
                            Ok(list) if !list.is_empty() => {
                                dispatch.reduce_mut(|store| {
                                    set_rows(&mut store.torrents, list);
                                });
                            }
                            _ => dispatch.reduce_mut(|store| {
                                set_rows(&mut store.torrents, demo_rows());
                            }),
                        }
                    } else {
                        dispatch.reduce_mut(|store| {
                            set_rows(&mut store.torrents, demo_rows());
                        });
                    }
                });
                || ()
            },
            (filters.clone(), auth_state.clone()),
        );
    }

    let schedule_refresh = {
        let refresh_timer = refresh_timer.clone();
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        Callback::from(move |_| {
            if refresh_timer.borrow().is_some() {
                return;
            }
            let refresh_timer_handle = refresh_timer.clone();
            let dispatch = dispatch.clone();
            let client = api_ctx.client.clone();
            let handle = Timeout::new(1200, move || {
                refresh_timer_handle.borrow_mut().take();
                let state = dispatch.get();
                let auth_state = state.auth.state.clone();
                let filters = state.torrents.filters.clone();
                if auth_state.is_none() {
                    dispatch.reduce_mut(|store| {
                        set_rows(&mut store.torrents, demo_rows());
                    });
                    return;
                }
                let search = if filters.search.trim().is_empty() {
                    None
                } else {
                    Some(filters.search.clone())
                };
                yew::platform::spawn_local(async move {
                    if let Ok(list) = client.fetch_torrents(search, filters.regex).await {
                        dispatch.reduce_mut(|store| {
                            set_rows(&mut store.torrents, list);
                        });
                    }
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
        use_effect_with_deps(
            move |selected_id| {
                let cleanup = || ();
                if let Some(id) = **selected_id {
                    if !dispatch.get().torrents.details_by_id.contains_key(&id) {
                        let dispatch = dispatch.clone();
                        let client = api_ctx.client.clone();
                        yew::platform::spawn_local(async move {
                            match client.fetch_torrent_detail(&id.to_string()).await {
                                Ok(detail) => dispatch.reduce_mut(|store| {
                                    upsert_detail(&mut store.torrents, id, detail);
                                }),
                                Err(_) => {
                                    if let Some(detail) = demo_detail(&id.to_string()) {
                                        dispatch.reduce_mut(|store| {
                                            upsert_detail(&mut store.torrents, id, detail);
                                        });
                                    }
                                }
                            }
                        });
                    }
                }
                cleanup
            },
            selected_id.clone(),
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
        let dashboard_state = dashboard.clone();
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
                        let dashboard_state = dashboard_state.clone();
                        let schedule_refresh = schedule_refresh.clone();
                        Callback::from(move |envelope: UiEventEnvelope| {
                            handle_sse_envelope(
                                envelope,
                                &dispatch,
                                &progress_buffer,
                                &dashboard_state,
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
        let theme = theme.clone();
        Callback::from(move |_| {
            let next = if *theme == ThemeMode::Light {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            };
            theme.set(next);
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
                store.torrents.filters.search = value;
            });
        })
    };
    let toggle_regex = {
        let dispatch = dispatch.clone();
        Callback::from(move |_| {
            dispatch.reduce_mut(|store| {
                store.torrents.filters.regex = !store.torrents.filters.regex;
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
        let toasts = toasts.clone();
        Callback::from(move |id: u64| {
            toasts.set(
                (*toasts)
                    .iter()
                    .cloned()
                    .filter(|toast| toast.id != id)
                    .collect(),
            );
        })
    };
    let on_add_torrent = {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        let toasts = toasts.clone();
        let toast_id = toast_id.clone();
        let add_busy = add_busy.clone();
        let filters = filters.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |input: AddTorrentInput| {
            let client = api_ctx.client.clone();
            let dispatch = dispatch.clone();
            let toasts = toasts.clone();
            let toast_id = toast_id.clone();
            let add_busy = add_busy.clone();
            let filters = (*filters).clone();
            let bundle = bundle.clone();
            add_busy.set(true);
            yew::platform::spawn_local(async move {
                match client.add_torrent(input).await {
                    Ok(row) => {
                        push_toast(
                            &toasts,
                            &toast_id,
                            ToastKind::Success,
                            format!("{} {}", bundle.text("toast.add_success", ""), row.name),
                        );
                        let search = if filters.search.trim().is_empty() {
                            None
                        } else {
                            Some(filters.search.clone())
                        };
                        match client.fetch_torrents(search, filters.regex).await {
                            Ok(list) => dispatch.reduce_mut(|store| {
                                set_rows(&mut store.torrents, list);
                            }),
                            Err(err) => push_toast(
                                &toasts,
                                &toast_id,
                                ToastKind::Info,
                                format!("{} {err}", bundle.text("toast.add_refresh_failed", "")),
                            ),
                        }
                    }
                    Err(err) => push_toast(
                        &toasts,
                        &toast_id,
                        ToastKind::Error,
                        format!("{} {err}", bundle.text("toast.add_failed", "")),
                    ),
                }
                add_busy.set(false);
            });
        })
    };
    let on_action = {
        let dispatch = dispatch.clone();
        let api_ctx = (*api_ctx).clone();
        let toasts = toasts.clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |(action, id): (TorrentAction, Uuid)| {
            let client = api_ctx.client.clone();
            let dispatch = dispatch.clone();
            let toasts = toasts.clone();
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
                            &toasts,
                            &toast_id,
                            ToastKind::Success,
                            success_message(&bundle, &action, &display_name),
                        );
                    }
                    Err(err) => push_toast(
                        &toasts,
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
        let toasts = toasts.clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |(action, ids): (TorrentAction, Vec<Uuid>)| {
            let client = api_ctx.client.clone();
            let dispatch = dispatch.clone();
            let toasts = toasts.clone();
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
                            &toasts,
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
                    &toasts,
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
                        theme={*theme}
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
                                        <DashboardPanel snapshot={(*dashboard).clone()} mode={*mode} density={*density} torrents={torrents_rows.clone()} />
                                        <TorrentView breakpoint={*breakpoint} torrents={torrents_rows.clone()} density={*density} mode={*mode} on_density_change={set_density.clone()} on_bulk_action={on_bulk_action.clone()} on_action={on_action.clone()} on_add={on_add_torrent.clone()} add_busy={*add_busy} search={search.clone()} regex={regex} on_search={set_search.clone()} on_toggle_regex={toggle_regex.clone()} selected_id={selected_id_value} selected_ids={selected_ids_value.clone()} on_set_selected={on_set_selected.clone()} selected_detail={selected_detail_value.clone()} on_select_detail={on_select_detail.clone()} />
                                    </div>
                                },
                                Route::TorrentDetail { id } => html! {
                                    <div class="torrents-stack">
                                        <DashboardPanel snapshot={(*dashboard).clone()} mode={*mode} density={*density} torrents={torrents_rows.clone()} />
                                        <TorrentView breakpoint={*breakpoint} torrents={torrents_rows.clone()} density={*density} mode={*mode} on_density_change={set_density.clone()} on_bulk_action={on_bulk_action.clone()} on_action={on_action.clone()} on_add={on_add_torrent.clone()} add_busy={*add_busy} search={search.clone()} regex={regex} on_search={set_search.clone()} on_toggle_regex={toggle_regex.clone()} selected_id={Uuid::parse_str(&id).ok()} selected_ids={selected_ids_value.clone()} on_set_selected={on_set_selected.clone()} selected_detail={selected_detail_value.clone()} on_select_detail={on_select_detail.clone()} />
                                    </div>
                                },
                                Route::Categories => html! { <Placeholder title={bundle.text("placeholder.categories_title", "Categories")} body={bundle.text("placeholder.categories_body", "Category policy editor")} /> },
                                Route::Tags => html! { <Placeholder title={bundle.text("placeholder.tags_title", "Tags")} body={bundle.text("placeholder.tags_body", "Tag management")} /> },
                                Route::Settings => html! { <Placeholder title={bundle.text("placeholder.settings_title", "Settings")} body={bundle.text("placeholder.settings_body", "Engine profile and paths")} /> },
                                Route::Health => html! { <Placeholder title={bundle.text("placeholder.health_title", "Health")} body={bundle.text("placeholder.health_body", "Service status and diagnostics")} /> },
                                Route::NotFound => html! { <Placeholder title={bundle.text("placeholder.not_found_title", "Not found")} body={bundle.text("placeholder.not_found_body", "Use navigation to return to a supported view.")} /> },
                            }
                        }} />
                    </AppShell>
                    <ToastHost toasts={(*toasts).clone()} on_dismiss={dismiss_toast.clone()} />
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
                                default_mode={auth_mode}
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

fn push_toast(
    toasts: &UseStateHandle<Vec<Toast>>,
    next_id: &UseStateHandle<u64>,
    kind: ToastKind,
    message: String,
) {
    let id = **next_id + 1;
    next_id.set(id);
    let mut list = (**toasts).clone();
    list.push(Toast { id, message, kind });
    if list.len() > 4 {
        let drain = list.len() - 4;
        list.drain(0..drain);
    }
    toasts.set(list);
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

fn update_system_rates(
    state: &UseStateHandle<crate::models::DashboardSnapshot>,
    download_bps: u64,
    upload_bps: u64,
) -> crate::models::DashboardSnapshot {
    let mut snapshot = (**state).clone();
    snapshot.download_bps = download_bps;
    snapshot.upload_bps = upload_bps;
    snapshot
}

fn handle_sse_envelope(
    envelope: UiEventEnvelope,
    dispatch: &Dispatch<AppStore>,
    progress_buffer: &Rc<RefCell<HashMap<Uuid, ProgressPatch>>>,
    dashboard: &UseStateHandle<crate::models::DashboardSnapshot>,
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
            dashboard.set(update_system_rates(dashboard, download_bps, upload_bps));
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
