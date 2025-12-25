use crate::UiMode;
use crate::app::Route;
use crate::breakpoints::Breakpoint;
use crate::components::atoms::IconButton;
use crate::components::atoms::icons::{
    CategoriesIcon, HealthIcon, NotFoundIcon, RevaerLogoIcon, SettingsIcon, TagsIcon, TorrentsIcon,
};
use crate::components::status::SseBadge;
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::{NavLabels, SseState};
use crate::theme::ThemeMode;
use yew::prelude::*;
use yew_router::prelude::Link;

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
    #[prop_or_default]
    pub class: Classes,
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
    let close_nav = {
        let nav_open = nav_open.clone();
        Callback::from(move |_| nav_open.set(false))
    };

    let theme_label = match props.theme {
        ThemeMode::Light => t("shell.theme.light"),
        ThemeMode::Dark => t("shell.theme.dark"),
    };
    let current_label = match props.active {
        Route::Home | Route::Torrents | Route::TorrentDetail { .. } => props.nav.torrents.clone(),
        Route::Categories => props.nav.categories.clone(),
        Route::Tags => props.nav.tags.clone(),
        Route::Settings => props.nav.settings.clone(),
        Route::Health => props.nav.health.clone(),
        Route::NotFound => "Not found".into(),
    };
    let nav_active = match props.active {
        Route::Home | Route::TorrentDetail { .. } => Route::Torrents,
        _ => props.active.clone(),
    };

    html! {
        <div
            class={classes!(
                "app-shell",
                "reaver-shell",
                format!("theme-{}", props.theme.as_str()),
                props.class.clone()
            )}
        >
            <aside class={classes!("sidebar", if *nav_open { "open" } else { "closed" })}>
                <div class="brand">
                    <button class="ghost mobile-only" onclick={toggle_nav.clone()} aria-label={t("shell.close_nav")}>{"✕"}</button>
                    <div class="logo-mark">{reaver_mark()}</div>
                    <div class="brand-copy">
                        <strong>{t("shell.brand")}</strong>
                        <span class="muted">{t("shell.phase")}</span>
                    </div>
                </div>
                <ul class="menu nav-list">
                    {nav_item(Route::Torrents, &props.nav.torrents, nav_active.clone(), close_nav.clone())}
                    {nav_item(Route::Categories, &props.nav.categories, nav_active.clone(), close_nav.clone())}
                    {nav_item(Route::Tags, &props.nav.tags, nav_active.clone(), close_nav.clone())}
                    {nav_item(Route::Settings, &props.nav.settings, nav_active.clone(), close_nav.clone())}
                    {nav_item(Route::Health, &props.nav.health, nav_active, close_nav)}
                </ul>
                <div class="sidebar-footer">
                    <div class="sidebar-group">
                        <small>{t("shell.mode")}</small>
                        <div class="chip-group">
                            <button class={classes!("chip", if props.mode == UiMode::Simple { "active" } else { "ghost" })} onclick={{
                                let cb = props.on_mode_change.clone();
                                Callback::from(move |_| cb.emit(UiMode::Simple))
                            }}>{t("mode.simple")}</button>
                            <button class={classes!("chip", if props.mode == UiMode::Advanced { "active" } else { "ghost" })} onclick={{
                                let cb = props.on_mode_change.clone();
                                Callback::from(move |_| cb.emit(UiMode::Advanced))
                            }}>{t("mode.advanced")}</button>
                        </div>
                    </div>
                    <div class="sidebar-group">
                        <small>{t("shell.theme.label")}</small>
                        <button class="chip ghost" onclick={{
                            let cb = props.on_toggle_theme.clone();
                            Callback::from(move |_| cb.emit(()))
                        }}>{theme_label}</button>
                    </div>
                    <div class="sidebar-group">
                        <small>{t("shell.locale")}</small>
                        <div class="locale-select">{props.locale_selector.clone()}</div>
                    </div>
                </div>
            </aside>
            <div class="main">
                <header class="topbar glass">
                    <button class="ghost mobile-only" aria-label={t("shell.open_nav")} onclick={toggle_nav}>{"☰"}</button>
                    <div class="page-title">
                        <p class="eyebrow">{t("shell.phase")}</p>
                        <h2>{current_label}</h2>
                    </div>
                    <div class="top-actions">
                        <SseBadge state={props.sse_state.clone()} />
                        <IconButton
                            aria_label={t("shell.simulate_sse")}
                            onclick={{
                            let cb = props.on_sse_retry.clone();
                            Callback::from(move |_| cb.emit(()))
                            }}
                        >
                            {"↻"}
                        </IconButton>
                        <span class="pill subtle">{format!("BP {}", props.breakpoint.name)}</span>
                        <span class="pill subtle">{props.network_mode.clone()}</span>
                        <IconButton
                            aria_label={t("shell.toggle_theme")}
                            onclick={{
                            let cb = props.on_toggle_theme.clone();
                            Callback::from(move |_| cb.emit(()))
                            }}
                        >
                            {"🌓"}
                        </IconButton>
                    </div>
                </header>
                <main>
                    {for props.children.iter()}
                </main>
            </div>
        </div>
    }
}

fn nav_item(route: Route, label: &str, active: Route, on_select: Callback<()>) -> Html {
    let is_active = active == route;
    let classes = classes!("nav-item", if is_active { "active" } else { "" });
    let close = {
        let on_select = on_select.clone();
        Callback::from(move |_| on_select.emit(()))
    };
    html! {
        <li onclick={close}>
            <Link<Route> to={route.clone()} classes={classes}>
                <span class="nav-icon">{nav_icon(&route)}</span>
                <span class="nav-label">{label}</span>
            </Link<Route>>
        </li>
    }
}

fn nav_icon(route: &Route) -> Html {
    match route {
        Route::Home | Route::Torrents | Route::TorrentDetail { .. } => {
            html! { <TorrentsIcon size={18} /> }
        }
        Route::Categories => html! { <CategoriesIcon size={18} /> },
        Route::Tags => html! { <TagsIcon size={18} /> },
        Route::Settings => html! { <SettingsIcon size={18} /> },
        Route::Health => html! { <HealthIcon size={18} /> },
        Route::NotFound => html! { <NotFoundIcon size={18} /> },
    }
}

fn reaver_mark() -> Html {
    html! {
        <RevaerLogoIcon class={classes!("reaver-logo")} size={28} />
    }
}
