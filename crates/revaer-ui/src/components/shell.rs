use crate::UiMode;
use crate::app::Route;
use crate::breakpoints::Breakpoint;
use crate::components::status::{SseBadge, SseState};
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::theme::ThemeMode;
use yew::prelude::*;
use yew_router::prelude::Link;

#[derive(Clone, PartialEq)]
pub(crate) struct NavLabels {
    pub dashboard: String,
    pub torrents: String,
    pub search: String,
    pub jobs: String,
    pub settings: String,
    pub logs: String,
}

#[derive(Properties, PartialEq)]
pub(crate) struct ShellProps {
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
    pub network_mode: String,
}

#[function_component(AppShell)]
pub(crate) fn app_shell(props: &ShellProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
    let nav_open = use_state(|| false);
    let toggle_nav = {
        let nav_open = nav_open.clone();
        Callback::from(move |_| nav_open.set(!*nav_open))
    };

    let theme_label = match props.theme {
        ThemeMode::Light => t("shell.theme.light"),
        ThemeMode::Dark => t("shell.theme.dark"),
    };

    html! {
        <div class={classes!("app-shell", format!("theme-{}", props.theme.as_str()))}>
            <aside class={classes!("sidebar", if *nav_open { "open" } else { "closed" })}>
                <div class="brand">
                    <button class="ghost mobile-only" onclick={toggle_nav.clone()} aria-label={t("shell.close_nav")}>{"✕"}</button>
                    <strong>{t("shell.brand")}</strong>
                    <span class="muted">{t("shell.phase")}</span>
                </div>
                <nav>
                    {nav_item(Route::Dashboard, &props.nav.dashboard, props.active.clone())}
                    {nav_item(Route::Torrents, &props.nav.torrents, props.active.clone())}
                    {nav_item(Route::Search, &props.nav.search, props.active.clone())}
                    {nav_item(Route::Jobs, &props.nav.jobs, props.active.clone())}
                    {nav_item(Route::Settings, &props.nav.settings, props.active.clone())}
                    {nav_item(Route::Logs, &props.nav.logs, props.active.clone())}
                </nav>
                <div class="sidebar-footer">
                    <div class="mode-toggle">
                        <small>{t("shell.mode")}</small>
                        <div class="segmented">
                            <button class={classes!(if props.mode == UiMode::Simple { "active" } else { "" })} onclick={{
                                let cb = props.on_mode_change.clone();
                                Callback::from(move |_| cb.emit(UiMode::Simple))
                            }}>{t("mode.simple")}</button>
                            <button class={classes!(if props.mode == UiMode::Advanced { "active" } else { "" })} onclick={{
                                let cb = props.on_mode_change.clone();
                                Callback::from(move |_| cb.emit(UiMode::Advanced))
                            }}>{t("mode.advanced")}</button>
                        </div>
                    </div>
                    <div class="theme-toggle">
                        <small>{t("shell.theme.label")}</small>
                        <button class="ghost" onclick={{
                            let cb = props.on_toggle_theme.clone();
                            Callback::from(move |_| cb.emit(()))
                        }}>{theme_label}</button>
                    </div>
                    <div class="locale-toggle">
                        <small>{t("shell.locale")}</small>
                        {props.locale_selector.clone()}
                    </div>
                </div>
            </aside>
            <div class="main">
                <header class="topbar">
                    <button class="ghost mobile-only" aria-label={t("shell.open_nav")} onclick={toggle_nav}>{"☰"}</button>
                    <div class="searchbar">
                        <input placeholder={t("shell.search_placeholder")} aria-label={t("shell.search_label")} />
                    </div>
                    <div class="top-actions">
                        <SseBadge state={props.sse_state} />
                        <button class="ghost" onclick={{
                            let cb = props.on_sse_retry.clone();
                            Callback::from(move |_| cb.emit(()))
                        }}>{t("shell.simulate_sse")}</button>
                        <span class="pill subtle">{format!("BP: {}", props.breakpoint.name)}</span>
                        <span class="pill subtle">{format!("VPN: {}", props.network_mode)}</span>
                        <button class="ghost" onclick={{
                            let cb = props.on_toggle_theme.clone();
                            Callback::from(move |_| cb.emit(()))
                        }} aria-label={t("shell.toggle_theme")}>{"🌓"}</button>
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
