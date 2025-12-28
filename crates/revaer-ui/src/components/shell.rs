use crate::app::Route;
use crate::components::connectivity::{ConnectivityIndicator, ConnectivityModal};
use crate::core::store::{select_sse_status, select_sse_status_summary};
use crate::core::theme::ThemeMode;
use crate::models::NavLabels;
use yew::prelude::*;
use yew_router::prelude::Link;
use yewdux::prelude::use_selector;

#[derive(Properties, PartialEq)]
pub(crate) struct ShellProps {
    pub children: Children,
    pub theme: ThemeMode,
    pub on_toggle_theme: Callback<()>,
    pub active: Route,
    pub locale_selector: Html,
    pub nav: NavLabels,
    pub on_sse_retry: Callback<()>,
    pub on_server_restart: Callback<()>,
    pub on_server_logs: Callback<()>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(AppShell)]
pub(crate) fn app_shell(props: &ShellProps) -> Html {
    let home_active = matches!(props.active, Route::Dashboard);
    let torrents_active = matches!(props.active, Route::Torrents | Route::TorrentDetail { .. });
    let settings_active = matches!(props.active, Route::Settings);

    let page_label = match props.active {
        Route::Dashboard => props.nav.dashboard.clone(),
        Route::Torrents | Route::TorrentDetail { .. } => props.nav.torrents.clone(),
        Route::Settings => props.nav.settings.clone(),
        Route::NotFound => "Not Found".to_string(),
    };
    let connectivity_summary = use_selector(select_sse_status_summary);
    let connectivity_status = use_selector(select_sse_status);
    let show_connectivity = use_state(|| false);
    let open_connectivity = {
        let show_connectivity = show_connectivity.clone();
        Callback::from(move |_| show_connectivity.set(true))
    };
    let close_connectivity = {
        let show_connectivity = show_connectivity.clone();
        Callback::from(move |_| show_connectivity.set(false))
    };

    html! {
        <div class={classes!("size-full", props.class.clone())}>
            <div class="flex">
                <input
                    type="checkbox"
                    id="layout-sidebar-toggle-trigger"
                    class="hidden"
                    aria-label="Toggle layout sidebar" />
                <input
                    type="checkbox"
                    id="layout-sidebar-hover-trigger"
                    class="hidden"
                    aria-label="Dense layout sidebar" />
                <div id="layout-sidebar-hover" class="bg-base-300 h-screen w-1"></div>

                <div id="layout-sidebar" class="sidebar-menu sidebar-menu-activation">
                    <div class="flex min-h-16 items-center justify-between gap-3 ps-5 pe-4">
                        <Link<Route> to={Route::Dashboard}>
                            {if props.theme == ThemeMode::Dark {
                                html! {
                                    <img
                                        alt="logo-dark"
                                        class="h-5.5"
                                        src="/static/revaer-logo.png" />
                                }
                            } else {
                                html! {
                                    <img
                                        alt="logo-light"
                                        class="h-5.5"
                                        src="/static/revaer-logo.png" />
                                }
                            }}
                        </Link<Route>>
                        <label
                            for="layout-sidebar-hover-trigger"
                            title="Toggle sidebar hover"
                            class="btn btn-circle btn-ghost btn-sm text-base-content/50 relative max-lg:hidden">
                            <span
                                class="iconify lucide--panel-left-close absolute size-4.5 opacity-100 transition-all duration-300 group-has-[[id=layout-sidebar-hover-trigger]:checked]/html:opacity-0"></span>
                            <span
                                class="iconify lucide--panel-left-dashed absolute size-4.5 opacity-0 transition-all duration-300 group-has-[[id=layout-sidebar-hover-trigger]:checked]/html:opacity-100"></span>
                        </label>
                    </div>
                    <div class="relative min-h-0 grow">
                        <div data-simplebar="" class="size-full">
                            <div class="mb-3 space-y-0.5 px-2.5">
                                <p class="menu-label px-2.5 pt-3 pb-1.5 first:pt-0">{"Overview"}</p>
                                <Link<Route>
                                    to={Route::Dashboard}
                                    classes={menu_item_class(home_active)}>
                                    <span class="iconify lucide--home size-4"></span>
                                    <span class="grow">{props.nav.dashboard.clone()}</span>
                                </Link<Route>>
                                <Link<Route>
                                    to={Route::Torrents}
                                    classes={menu_item_class(torrents_active)}>
                                    <span class="iconify lucide--download size-4"></span>
                                    <span class="grow">{props.nav.torrents.clone()}</span>
                                </Link<Route>>
                                <Link<Route>
                                    to={Route::Settings}
                                    classes={menu_item_class(settings_active)}>
                                    <span class="iconify lucide--settings size-4"></span>
                                    <span class="grow">{props.nav.settings.clone()}</span>
                                </Link<Route>>
                            </div>
                        </div>
                        <div
                            class="from-base-100/60 pointer-events-none absolute start-0 end-0 bottom-0 h-7 bg-linear-to-t to-transparent"></div>
                    </div>
                    <div class="mb-2">
                        <ConnectivityIndicator
                            summary={(*connectivity_summary).clone()}
                            on_open={open_connectivity.clone()}
                        />
                    </div>
                </div>

                <label for="layout-sidebar-toggle-trigger" id="layout-sidebar-backdrop"></label>

                <div class="flex h-screen min-w-0 grow flex-col overflow-auto">
                    <div
                        role="navigation"
                        aria-label="Navbar"
                        class="flex items-center justify-between px-3"
                        id="layout-topbar">
                        <div class="inline-flex items-center gap-3">
                            <label
                                class="btn btn-square btn-ghost btn-sm group-has-[[id=layout-sidebar-hover-trigger]:checked]/html:hidden"
                                aria-label="Leftmenu toggle"
                                for="layout-sidebar-toggle-trigger">
                                <span class="iconify lucide--menu size-5"></span>
                            </label>
                            <label
                                class="btn btn-square btn-ghost btn-sm hidden group-has-[[id=layout-sidebar-hover-trigger]:checked]/html:flex"
                                aria-label="Leftmenu toggle"
                                for="layout-sidebar-hover-trigger">
                                <span class="iconify lucide--menu size-5"></span>
                            </label>
                            <div class="breadcrumbs p-0 text-sm">
                                <ul>
                                    <li class="opacity-80">{page_label.clone()}</li>
                                </ul>
                            </div>
                        </div>
                        <div class="inline-flex items-center gap-1.5">
                            <button
                                aria-label="Toggle Theme"
                                class="btn btn-sm btn-circle btn-ghost"
                                onclick={{
                                    let cb = props.on_toggle_theme.clone();
                                    Callback::from(move |_| cb.emit(()))
                                }}>
                                {if props.theme == ThemeMode::Dark {
                                    html! { <span class="iconify lucide--sun size-4.5"></span> }
                                } else {
                                    html! { <span class="iconify lucide--moon size-4.5"></span> }
                                }}
                            </button>
                            {props.locale_selector.clone()}
                            <div class="dropdown dropdown-bottom dropdown-end">
                                <div
                                    tabindex="0"
                                    role="button"
                                    class="btn btn-circle btn-ghost btn-sm"
                                    aria-label="Server menu">
                                    <span class="iconify lucide--server size-4.5"></span>
                                </div>
                                <ul
                                    tabindex="0"
                                    role="menu"
                                    class="dropdown-content menu bg-base-100 rounded-box mt-2 w-44 p-1 shadow">
                                    <li>
                                        <button
                                            onclick={{
                                                let cb = props.on_server_restart.clone();
                                                Callback::from(move |_| cb.emit(()))
                                            }}>
                                            <span class="iconify lucide--refresh-cw size-4"></span>
                                            <span>{"Restart server"}</span>
                                        </button>
                                    </li>
                                    <li>
                                        <button
                                            onclick={{
                                                let cb = props.on_server_logs.clone();
                                                Callback::from(move |_| cb.emit(()))
                                            }}>
                                            <span class="iconify lucide--file-text size-4"></span>
                                            <span>{"View logs"}</span>
                                        </button>
                                    </li>
                                </ul>
                            </div>
                        </div>
                    </div>
                    <div id="layout-content">
                        {for props.children.iter()}
                    </div>
                </div>
            </div>

            {if *show_connectivity {
                html! {
                    <ConnectivityModal
                        status={(*connectivity_status).clone()}
                        on_retry={props.on_sse_retry.clone()}
                        on_dismiss={close_connectivity.clone()}
                    />
                }
            } else {
                html! {}
            }}
        </div>
    }
}

fn menu_item_class(active: bool) -> Classes {
    classes!("menu-item", if active { "active" } else { "false" })
}
