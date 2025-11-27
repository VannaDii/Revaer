use crate::breakpoints::Breakpoint;
use crate::components::auth::AuthPrompt;
use crate::components::dashboard::{DashboardPanel, demo_snapshot};
use crate::components::shell::{AppShell, NavLabels};
use crate::components::status::{SseOverlay, SseState};
use crate::components::toast::{Toast, ToastHost, ToastKind};
use crate::components::torrents::{AddTorrentInput, TorrentView, demo_rows};
use crate::i18n::{DEFAULT_LOCALE, LocaleCode, TranslationBundle};
use crate::logic::backoff_delay_ms;
use crate::services::ApiClient;
use crate::state::{
    TorrentAction, apply_progress, apply_rates, apply_remove, apply_status, success_message,
};
use crate::theme::ThemeMode;
use crate::{Density, UiMode};
use gloo::events::EventListener;
use gloo::storage::{LocalStorage, Storage};
use gloo::utils::window;
use gloo_timers::callback::Timeout;
use wasm_bindgen::JsCast;
use web_sys::{EventSource, MediaQueryList};
use yew::prelude::*;
use yew_router::prelude::*;

const THEME_KEY: &str = "revaer.theme";
const MODE_KEY: &str = "revaer.mode";
const LOCALE_KEY: &str = "revaer.locale";
const DENSITY_KEY: &str = "revaer.density";
const API_KEY_KEY: &str = "revaer.api_key";
const ALLOW_ANON: bool = false;

#[derive(Clone, Routable, PartialEq, Eq, Debug)]
pub enum Route {
    #[at("/")]
    Dashboard,
    #[at("/torrents")]
    Torrents,
    #[at("/search")]
    Search,
    #[at("/jobs")]
    Jobs,
    #[at("/settings")]
    Settings,
    #[at("/logs")]
    Logs,
    #[not_found]
    #[at("/404")]
    NotFound,
}

#[function_component(RevaerApp)]
pub fn revaer_app() -> Html {
    let theme = use_state(load_theme);
    let mode = use_state(load_mode);
    let density = use_state(load_density);
    let locale = use_state(load_locale);
    let breakpoint = use_state(current_breakpoint);
    let api_key = use_state(load_api_key);
    let torrents = use_state(demo_rows);
    let dashboard = use_state(demo_snapshot);
    let search = use_state(String::new);
    let regex = use_state(|| false);
    let toasts = use_state(Vec::<Toast>::new);
    let toast_id = use_state(|| 0u64);
    let add_busy = use_state(|| false);
    let sse_handle = use_mut_ref(|| None as Option<EventSource>);
    let sse_state = use_state(|| SseState::Reconnecting {
        retry_in_secs: 5,
        last_event: "12s ago",
        reason: "network: timeout",
    });
    let sse_retry = use_state(|| 0u32);
    let bundle = {
        let locale = *locale;
        use_memo(move |_| TranslationBundle::new(locale), locale)
    };

    let nav_labels = {
        let bundle = (*bundle).clone();
        NavLabels {
            dashboard: bundle.text("nav.dashboard", "Dashboard"),
            torrents: bundle.text("nav.torrents", "Torrents"),
            search: bundle.text("nav.search", "Search"),
            jobs: bundle.text("nav.jobs", "Jobs / Post-processing"),
            settings: bundle.text("nav.settings", "Settings"),
            logs: bundle.text("nav.logs", "Logs"),
        }
    };
    let current_route = use_route::<Route>().unwrap_or(Route::Dashboard);

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
        let api_key = (*api_key).clone();
        let dashboard = dashboard.clone();
        use_effect(move || {
            let dashboard_client = ApiClient::new(api_base_url(), api_key.clone());
            yew::platform::spawn_local(async move {
                if let Ok(snapshot) = dashboard_client.fetch_dashboard().await {
                    dashboard.set(snapshot);
                }
            });
            || ()
        });
    }
    {
        let api_key = (*api_key).clone();
        let torrents = torrents.clone();
        let search = (*search).clone();
        let regex = *regex;
        use_effect_with_deps(
            move |_| {
                let client = ApiClient::new(api_base_url(), api_key.clone());
                yew::platform::spawn_local(async move {
                    match client.fetch_torrents(Some(search.clone()), regex).await {
                        Ok(list) if !list.is_empty() => torrents.set(list),
                        _ => torrents.set(demo_rows()),
                    }
                });
                || ()
            },
            (search.clone(), regex),
        );
    }
    {
        let api_key = (*api_key).clone();
        let torrents = torrents.clone();
        let search_value = (*search).clone();
        let regex_flag = *regex;
        let sse_handle = sse_handle.clone();
        let sse_state = sse_state.clone();
        let dashboard_state = dashboard.clone();
        let sse_retry = sse_retry.clone();
        use_effect(
            move || {
                if let Some(src) = sse_handle.borrow_mut().take() {
                    src.close();
                }
                let state_updater = torrents.clone();
                let sse_state = sse_state.clone();
                let mut cancel_timer: Option<Timeout> = None;
                let search = search_value.clone();
                let regex = regex_flag;
                if let Some(source) =
                    crate::services::connect_sse(&api_base_url(), api_key.clone(), move |event| {
                        match event {
                            crate::models::SseEvent::TorrentProgress {
                                torrent_id,
                                progress,
                                eta_seconds,
                                download_bps,
                                upload_bps,
                            } => {
                                state_updater.set(update_progress(
                                    &state_updater,
                                    torrent_id.to_string(),
                                    progress,
                                    eta_seconds,
                                    download_bps,
                                    upload_bps,
                                ));
                            }
                            crate::models::SseEvent::TorrentRates {
                                torrent_id,
                                download_bps,
                                upload_bps,
                            } => {
                                state_updater.set(update_rates(
                                    &state_updater,
                                    torrent_id.to_string(),
                                    download_bps,
                                    upload_bps,
                                ));
                            }
                            crate::models::SseEvent::TorrentState {
                                torrent_id, status, ..
                            } => {
                                state_updater.set(update_status(
                                    &state_updater,
                                    torrent_id.to_string(),
                                    status,
                                ));
                            }
                            crate::models::SseEvent::TorrentRemoved { torrent_id } => {
                                state_updater
                                    .set(remove_torrent(&state_updater, torrent_id.to_string()));
                            }
                            crate::models::SseEvent::TorrentAdded { .. } => {
                                let torrents = state_updater.clone();
                                let api_key = api_key.clone();
                                let search = search.clone();
                                yew::platform::spawn_local(async move {
                                    let client = ApiClient::new(api_base_url(), api_key.clone());
                                    if let Ok(list) =
                                        client.fetch_torrents(Some(search), regex).await
                                    {
                                        torrents.set(list);
                                    }
                                });
                            }
                            crate::models::SseEvent::SystemRates {
                                download_bps,
                                upload_bps,
                            } => {
                                dashboard_state.set(update_system_rates(
                                    &dashboard_state,
                                    download_bps,
                                    upload_bps,
                                ));
                            }
                            crate::models::SseEvent::QueueStatus {
                                active,
                                paused,
                                queued,
                                depth,
                            } => {
                                dashboard_state.set(update_queue(
                                    &dashboard_state,
                                    active,
                                    paused,
                                    queued,
                                    depth,
                                ));
                            }
                            crate::models::SseEvent::VpnState {
                                state,
                                message,
                                last_change,
                            } => {
                                dashboard_state.set(update_vpn(
                                    &dashboard_state,
                                    state,
                                    message,
                                    last_change,
                                ));
                            }
                            _ => {}
                        }
                        sse_state.set(SseState::Connected);
                        sse_retry.set(0);
                    })
                {
                    *sse_handle.borrow_mut() = Some(source);
                } else {
                    let attempt = *sse_retry;
                    let retry_ms = backoff_delay_ms(attempt);
                    sse_state.set(SseState::Reconnecting {
                        retry_in_secs: (retry_ms / 1000).min(30) as u8,
                        last_event: "connect failed",
                        reason: "eventsource unsupported",
                    });
                    let retry = sse_retry.clone();
                    cancel_timer = Some(Timeout::new(retry_ms, move || {
                        retry.set(attempt.saturating_add(1));
                    }));
                }
                move || {
                    if let Some(src) = sse_handle.borrow_mut().take() {
                        src.close();
                    }
                    if let Some(timer) = cancel_timer {
                        timer.cancel();
                    }
                }
            },
            (*api_key).clone(),
        );
    }
    {
        let breakpoint = breakpoint.clone();
        use_effect(move || {
            apply_breakpoint(*breakpoint);
            let handler = gloo::events::EventListener::new(&gloo::utils::window(), "resize", {
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
        let search = search.clone();
        Callback::from(move |value: String| search.set(value))
    };
    let toggle_regex = {
        let regex = regex.clone();
        Callback::from(move |_| regex.set(!*regex))
    };
    let simulate_sse_drop = {
        let sse_state = sse_state.clone();
        Callback::from(move |_| {
            sse_state.set(SseState::Reconnecting {
                retry_in_secs: 8,
                last_event: "25s ago",
                reason: "gateway timeout",
            });
        })
    };
    let clear_sse_overlay = {
        let sse_state = sse_state.clone();
        Callback::from(move |_| sse_state.set(SseState::Connected))
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
        let api_key = api_key.clone();
        let torrents = torrents.clone();
        let toasts = toasts.clone();
        let toast_id = toast_id.clone();
        let add_busy = add_busy.clone();
        let search = search.clone();
        let regex = regex.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |input: AddTorrentInput| {
            let client = ApiClient::new(api_base_url(), (*api_key).clone());
            let torrents = torrents.clone();
            let toasts = toasts.clone();
            let toast_id = toast_id.clone();
            let add_busy = add_busy.clone();
            let search = (*search).clone();
            let regex = *regex;
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
                        match client.fetch_torrents(Some(search), regex).await {
                            Ok(list) => torrents.set(list),
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
        let api_key = api_key.clone();
        let torrents = torrents.clone();
        let toasts = toasts.clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |(action, id): (TorrentAction, String)| {
            let client = ApiClient::new(api_base_url(), (*api_key).clone());
            let torrents = torrents.clone();
            let toasts = toasts.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            yew::platform::spawn_local(async move {
                let display_name = (*torrents)
                    .iter()
                    .find(|row| row.id == id)
                    .map(|row| row.name.clone())
                    .unwrap_or_else(|| {
                        format!("{} {id}", bundle.text("toast.torrent_placeholder", ""))
                    });
                match client.perform_action(&id, action.clone()).await {
                    Ok(_) => {
                        if matches!(action, TorrentAction::Delete { .. }) {
                            torrents.set(
                                torrents
                                    .iter()
                                    .cloned()
                                    .filter(|row| row.id != id)
                                    .collect(),
                            );
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
        let api_key = api_key.clone();
        let torrents = torrents.clone();
        let toasts = toasts.clone();
        let toast_id = toast_id.clone();
        let bundle = (*bundle).clone();
        Callback::from(move |(action, ids): (TorrentAction, Vec<String>)| {
            let client = ApiClient::new(api_base_url(), (*api_key).clone());
            let torrents = torrents.clone();
            let toasts = toasts.clone();
            let toast_id = toast_id.clone();
            let bundle = bundle.clone();
            yew::platform::spawn_local(async move {
                for id in ids.clone() {
                    let display_name = (*torrents)
                        .iter()
                        .find(|row| row.id == id)
                        .map(|row| row.name.clone())
                        .unwrap_or_else(|| {
                            format!("{} {id}", bundle.text("toast.torrent_placeholder", ""))
                        });
                    if let Err(err) = client.perform_action(&id, action.clone()).await {
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
                    torrents.set(
                        torrents
                            .iter()
                            .cloned()
                            .filter(|row| !ids.contains(&row.id))
                            .collect(),
                    );
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

    html! {
        <ContextProvider<TranslationBundle> context={(*bundle).clone()}>
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
                    sse_state={*sse_state}
                    on_sse_retry={simulate_sse_drop}
                    network_mode={bundle.text("shell.network_connected", "")}
                >
                    <Switch<Route> render={move |route| {
                        let bundle = (*bundle).clone();
                        match route {
                            Route::Dashboard => html! { <DashboardPanel snapshot={(*dashboard).clone()} mode={*mode} density={*density} /> },
                            Route::Torrents | Route::Search => html! { <TorrentView base_url={api_base_url()} api_key={(*api_key).clone()} breakpoint={*breakpoint} torrents={(*torrents).clone()} density={*density} mode={*mode} on_density_change={set_density.clone()} on_bulk_action={on_bulk_action.clone()} on_action={on_action.clone()} on_add={on_add_torrent.clone()} add_busy={*add_busy} search={(*search).clone()} regex={*regex} on_search={set_search.clone()} on_toggle_regex={toggle_regex.clone()} /> },
                            Route::Jobs => html! { <Placeholder title={bundle.text("placeholder.jobs_title", "")} body={bundle.text("placeholder.jobs_body", "")} /> },
                            Route::Settings => html! { <Placeholder title={bundle.text("placeholder.settings_title", "")} body={bundle.text("placeholder.settings_body", "")} /> },
                            Route::Logs => html! { <Placeholder title={bundle.text("placeholder.logs_title", "")} body={bundle.text("placeholder.logs_body", "")} /> },
                            Route::NotFound => html! { <Placeholder title={bundle.text("placeholder.not_found_title", "")} body={bundle.text("placeholder.not_found_body", "")} /> },
                        }
                    }} />
                </AppShell>
                <ToastHost toasts={(*toasts).clone()} on_dismiss={dismiss_toast.clone()} />
                <SseOverlay state={*sse_state} on_retry={clear_sse_overlay} network_mode={bundle.text("shell.network_remote", "")} />
                {if api_key.is_none() {
                    html! {
                        <AuthPrompt
                            require_key={true}
                            allow_anonymous={ALLOW_ANON}
                            on_submit={{
                                let api_key = api_key.clone();
                                Callback::from(move |value: Option<String>| {
                                    if let Some(key) = value.clone() {
                                        let _ = LocalStorage::set(API_KEY_KEY, &key);
                                    }
                                    api_key.set(value);
                                })
                            }}
                        />
                    }
                } else {
                    html!{}
                }}
            </BrowserRouter>
        </ContextProvider<TranslationBundle>>
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
    let id = *next_id + 1;
    next_id.set(id);
    let mut list = (*toasts).clone();
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

fn load_theme() -> ThemeMode {
    if let Ok(value) = LocalStorage::get::<String>(THEME_KEY) {
        return match value.as_str() {
            "dark" => ThemeMode::Dark,
            _ => ThemeMode::Light,
        };
    }
    prefers_dark()
        .unwrap_or(false)
        .then_some(ThemeMode::Dark)
        .unwrap_or(ThemeMode::Light)
}

fn load_mode() -> UiMode {
    if let Ok(value) = LocalStorage::get::<String>(MODE_KEY) {
        return match value.as_str() {
            "advanced" => UiMode::Advanced,
            _ => UiMode::Simple,
        };
    }
    UiMode::Simple
}

fn load_density() -> Density {
    if let Ok(value) = LocalStorage::get::<String>(DENSITY_KEY) {
        return match value.as_str() {
            "compact" => Density::Compact,
            "comfy" => Density::Comfy,
            _ => Density::Normal,
        };
    }
    Density::Normal
}

fn load_locale() -> LocaleCode {
    if let Ok(value) = LocalStorage::get::<String>(LOCALE_KEY) {
        if let Some(locale) = LocaleCode::from_lang_tag(&value) {
            return locale;
        }
    }
    if let Some(nav) = window().navigator().language() {
        if let Some(locale) = LocaleCode::from_lang_tag(&nav) {
            return locale;
        }
    }
    DEFAULT_LOCALE
}

fn load_api_key() -> Option<String> {
    LocalStorage::get::<String>(API_KEY_KEY).ok()
}

fn api_base_url() -> String {
    window()
        .location()
        .origin()
        .unwrap_or_else(|_| "http://localhost:7878".to_string())
}

fn update_progress(
    state: &UseStateHandle<Vec<crate::state::TorrentRow>>,
    id: String,
    progress: f32,
    eta_seconds: Option<u64>,
    download_bps: u64,
    upload_bps: u64,
) -> Vec<crate::state::TorrentRow> {
    apply_progress(
        &(*state),
        &id,
        progress,
        eta_seconds,
        download_bps,
        upload_bps,
    )
}

fn update_rates(
    state: &UseStateHandle<Vec<crate::state::TorrentRow>>,
    id: String,
    download_bps: u64,
    upload_bps: u64,
) -> Vec<crate::state::TorrentRow> {
    apply_rates(&(*state), &id, download_bps, upload_bps)
}

fn update_status(
    state: &UseStateHandle<Vec<crate::state::TorrentRow>>,
    id: String,
    status: String,
) -> Vec<crate::state::TorrentRow> {
    apply_status(&(*state), &id, &status)
}

fn remove_torrent(
    state: &UseStateHandle<Vec<crate::state::TorrentRow>>,
    id: String,
) -> Vec<crate::state::TorrentRow> {
    apply_remove(&(*state), &id)
}

fn update_system_rates(
    state: &UseStateHandle<crate::components::dashboard::DashboardSnapshot>,
    download_bps: u64,
    upload_bps: u64,
) -> crate::components::dashboard::DashboardSnapshot {
    let mut snapshot = (*state).clone();
    snapshot.download_bps = download_bps;
    snapshot.upload_bps = upload_bps;
    snapshot
}

fn update_queue(
    state: &UseStateHandle<crate::components::dashboard::DashboardSnapshot>,
    active: u32,
    paused: u32,
    queued: u32,
    depth: u32,
) -> crate::components::dashboard::DashboardSnapshot {
    let mut snapshot = (*state).clone();
    snapshot.queue = crate::components::dashboard::QueueStatus {
        active: active as u16,
        paused: paused as u16,
        queued: queued as u16,
        depth: depth as u16,
    };
    snapshot
}

fn update_vpn(
    state: &UseStateHandle<crate::components::dashboard::DashboardSnapshot>,
    status: String,
    message: String,
    last_change: String,
) -> crate::components::dashboard::DashboardSnapshot {
    let mut snapshot = (*state).clone();
    snapshot.vpn = crate::components::dashboard::VpnState {
        state: status,
        message,
        last_change,
    };
    snapshot
}

fn prefers_dark() -> Option<bool> {
    let media: MediaQueryList = window()
        .match_media("(prefers-color-scheme: dark)")
        .ok()?
        .flatten()?;
    Some(media.matches())
}

/// Entrypoint invoked by Trunk for wasm32 builds.
pub fn run_app() {
    yew::Renderer::<RevaerApp>::new().render();
}
