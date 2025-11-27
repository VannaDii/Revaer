use crate::UiMode;
use crate::app::Route;
use crate::breakpoints::Breakpoint;
use crate::components::status::{SseBadge, SseState};
use crate::theme::ThemeMode;
use yew::prelude::*;
use yew_router::prelude::Link;

#[derive(Clone, PartialEq)]
pub struct NavLabels {
    pub dashboard: String,
    pub torrents: String,
    pub search: String,
    pub jobs: String,
    pub settings: String,
    pub logs: String,
}

#[derive(Properties, PartialEq)]
pub struct ShellProps {
    pub children: Children,
    pub theme: ThemeMode,
    pub on_toggle_theme: Callback<()>,
    pub mode: UiMode,
    pub on_mode_change: Callback<UiMode>,
    pub active: Route,
    pub locale_selector: Html,
    pub nav: NavLabels,
    pub breakpoint: Breakpoint,
    pub sse_state: SseState,
    pub on_sse_retry: Callback<()>,
    pub network_mode: &'static str,
}

#[function_component(AppShell)]
pub fn app_shell(props: &ShellProps) -> Html {
    let nav_open = use_state(|| false);
    let toggle_nav = {
        let nav_open = nav_open.clone();
        Callback::from(move |_| nav_open.set(!*nav_open))
    };

    let theme_label = match props.theme {
        ThemeMode::Light => "Light",
        ThemeMode::Dark => "Dark",
    };

    html! {
        <div class={classes!("app-shell", format!("theme-{}", props.theme.as_str()))}>
            <aside class={classes!("sidebar", if *nav_open { "open" } else { "closed" })}>
                <div class="brand">
                    <button class="ghost mobile-only" onclick={toggle_nav.clone()} aria-label="Close navigation">{"✕"}</button>
                    <strong>{"Revaer"}</strong>
                    <span class="muted">{"Phase 1"}</span>
                </div>
                <nav>
                    {nav_item(Route::Dashboard, &props.nav.dashboard, props.active)}
                    {nav_item(Route::Torrents, &props.nav.torrents, props.active)}
                    {nav_item(Route::Search, &props.nav.search, props.active)}
                    {nav_item(Route::Jobs, &props.nav.jobs, props.active)}
                    {nav_item(Route::Settings, &props.nav.settings, props.active)}
                    {nav_item(Route::Logs, &props.nav.logs, props.active)}
                </nav>
                <div class="sidebar-footer">
                    <div class="mode-toggle">
                        <small>{"Mode"}</small>
                        <div class="segmented">
                            <button class={classes!(if props.mode == UiMode::Simple { "active" } else { "" })} onclick={{
                                let cb = props.on_mode_change.clone();
                                Callback::from(move |_| cb.emit(UiMode::Simple))
                            }}>{"Simple"}</button>
                            <button class={classes!(if props.mode == UiMode::Advanced { "active" } else { "" })} onclick={{
                                let cb = props.on_mode_change.clone();
                                Callback::from(move |_| cb.emit(UiMode::Advanced))
                            }}>{"Advanced"}</button>
                        </div>
                    </div>
                    <div class="theme-toggle">
                        <small>{"Theme"}</small>
                        <button class="ghost" onclick={props.on_toggle_theme.clone()}>{theme_label}</button>
                    </div>
                    <div class="locale-toggle">
                        <small>{"Locale"}</small>
                        {props.locale_selector.clone()}
                    </div>
                </div>
            </aside>
            <div class="main">
                <header class="topbar">
                    <button class="ghost mobile-only" aria-label="Open navigation" onclick={toggle_nav}>{"☰"}</button>
                    <div class="searchbar">
                        <input placeholder="Search torrents (/ to focus)" aria-label="Global search" />
                    </div>
                    <div class="top-actions">
                        <SseBadge state={props.sse_state} />
                        <button class="ghost" onclick={props.on_sse_retry.clone()}>{"Simulate SSE drop"}</button>
                        <span class="pill subtle">{format!("BP: {}", props.breakpoint.name)}</span>
                        <span class="pill subtle">{format!("VPN: {}", props.network_mode)}</span>
                        <button class="ghost" onclick={props.on_toggle_theme.clone()} aria-label="Toggle theme">{"🌓"}</button>
                    </div>
                </header>
                <main>
                    {for props.children.iter()}
                </main>
            </div>
        </div>
    }
}

fn nav_item(route: Route, label: &str, active: Route) -> Html {
    let classes = classes!(
        "nav-item",
        if active == route {
            Some("active")
        } else {
            None
        }
    );
    html! {
        <Link<Route> to={route} classes={classes}>{label}</Link<Route>>
    }
}
