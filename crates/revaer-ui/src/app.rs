use crate::components::dashboard::{DashboardPanel, demo_snapshot};
use crate::components::shell::{AppShell, NavLabels};
use crate::components::status::{SseOverlay, SseState};
use crate::components::torrents::{TorrentView, demo_rows};
use crate::i18n::{DEFAULT_LOCALE, LocaleCode, TranslationBundle};
use crate::theme::ThemeMode;
use crate::{Density, UiMode};
use gloo::storage::{LocalStorage, Storage};
use gloo::utils::window;
use wasm_bindgen::JsCast;
use web_sys::MediaQueryList;
use yew::prelude::*;
use yew_router::prelude::*;

const THEME_KEY: &str = "revaer.theme";
const MODE_KEY: &str = "revaer.mode";
const LOCALE_KEY: &str = "revaer.locale";
const DENSITY_KEY: &str = "revaer.density";

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
    let sse_state = use_state(|| SseState::Reconnecting {
        retry_in_secs: 5,
        last_event: "12s ago",
        reason: "network: timeout",
    });
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
        <BrowserRouter>
            <AppShell
                theme={*theme}
                on_toggle_theme={toggle_theme}
                mode={*mode}
                on_mode_change={set_mode}
                active={current_route}
                locale_selector={locale_selector}
                nav={nav_labels}
                sse_state={*sse_state}
                on_sse_retry={simulate_sse_drop}
                network_mode={"connected"}
            >
                <Switch<Route> render={move |route| {
                    match route {
                        Route::Dashboard => html! { <DashboardPanel snapshot={demo_snapshot()} mode={*mode} density={*density} /> },
                        Route::Torrents | Route::Search => html! { <TorrentView torrents={demo_rows()} density={*density} mode={*mode} on_density_change={set_density.clone()} /> },
                        Route::Jobs => html! { <Placeholder title="Jobs / Post-processing" body="Job states, watch folder errors, SSE updates" /> },
                        Route::Settings => html! { <Placeholder title="Settings" body="Engine profile, paths, roles, remote mode" /> },
                        Route::Logs => html! { <Placeholder title="Logs" body="Streaming event log with filters" /> },
                        Route::NotFound => html! { <Placeholder title="Not found" body="Use the navigation to return to a supported view." /> },
                    }
                }} />
            </AppShell>
            <SseOverlay state={*sse_state} on_retry={clear_sse_overlay} network_mode={"remote (API key)"} />
        </BrowserRouter>
    }
}

#[function_component(Placeholder)]
fn placeholder(props: &PlaceholderProps) -> Html {
    html! {
        <div class="placeholder">
            <h2>{&props.title}</h2>
            <p class="muted">{&props.body}</p>
            <div class="pill subtle">{"Mobile + desktop responsive layout ready."}</div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct PlaceholderProps {
    pub title: String,
    pub body: String,
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
