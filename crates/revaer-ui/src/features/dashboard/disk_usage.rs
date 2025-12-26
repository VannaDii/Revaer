use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::{DashboardSnapshot, PathUsage};
use yew::prelude::*;
use yew_router::prelude::use_navigator;

#[derive(Properties, PartialEq)]
pub(crate) struct DashboardDiskUsageProps {
    pub snapshot: DashboardSnapshot,
}

#[function_component(DashboardDiskUsage)]
pub(crate) fn dashboard_disk_usage(props: &DashboardDiskUsageProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str, fallback: &str| bundle.text(key, fallback);

    let percent_value = usage_percent(props.snapshot.disk_used_gb, props.snapshot.disk_total_gb);
    let status_label = usage_status(percent_value, &bundle);
    let navigator = use_navigator();
    let on_open_settings = {
        let navigator = navigator.clone();
        Callback::from(move |_| {
            if let Some(navigator) = navigator.clone() {
                navigator.push(&crate::app::Route::Settings);
            }
        })
    };

    let rows = if props.snapshot.paths.is_empty() {
        let placeholder = PathUsage {
            label: "No paths reported",
            used_gb: 0,
            total_gb: 0,
        };
        html! { {render_path_usage(&placeholder)} }
    } else {
        html! {
            {for props.snapshot.paths.iter().map(|path| render_path_usage(path))}
        }
    };

    html! {
        <div class="xl:col-span-7">
            <div class="card bg-base-100 shadow">
                <div class="flex items-center gap-3 px-4 py-2.5">
                    <span class="iconify lucide--hard-drive size-4"></span>
                    <p class="grow font-medium">{t("dashboard.disk", "Storage Status")}</p>
                    <p class="text-base-content/40 text-xs font-medium max-sm:hidden">
                        {t("dashboard.disk_checked", "Just checked")}
                    </p>
                    <div class="dropdown dropdown-bottom dropdown-end">
                        <div
                            tabindex="0"
                            role="button"
                            class="btn btn-ghost btn-circle btn-xs"
                            aria-label="Menu">
                            <span class="iconify lucide--more-vertical size-3.5"></span>
                        </div>
                        <div
                            tabindex="0"
                            class="dropdown-content bg-base-100 rounded-box mt-2 w-40 shadow transition-all hover:shadow-lg">
                            <ul class="menu w-full p-1.5">
                                <li>
                                    <div onclick={on_open_settings.clone()}>
                                        <span class="iconify lucide--settings size-4"></span>
                                        {t("dashboard.disk_settings", "Open settings")}
                                    </div>
                                </li>
                                <li>
                                    <div>
                                        <span class="iconify lucide--brain size-4"></span>
                                        {t("dashboard.disk_insights", "Insights")}
                                    </div>
                                </li>
                                <li>
                                    <div>
                                        <span class="iconify lucide--wand-2 size-4"></span>
                                        {t("dashboard.disk_auto", "Auto tag")}
                                    </div>
                                </li>
                            </ul>
                            <hr class="border-base-300" />
                            <ul class="menu w-full p-1.5">
                                <li>
                                    <div class="text-error hover:bg-error/10">
                                        <span class="iconify lucide--trash size-4"></span>
                                        {t("dashboard.disk_cleanup", "Cleanup")}
                                    </div>
                                </li>
                            </ul>
                        </div>
                    </div>
                </div>
                <div class="border-base-300 border-t border-dashed px-4 py-2.5">
                    <p class="text-base-content/60 text-sm">
                        {t("dashboard.disk_hint", "How much space is left?")}
                    </p>
                    <p class="mt-3 font-medium">{status_label}</p>
                    {rows}
                </div>
                <div class="mt-auto flex items-end justify-end gap-2 px-4 pt-2 pb-4">
                    <div
                        class="bg-success/10 tooltip text-success flex items-center rounded-full p-0.5"
                        data-tip={t("dashboard.disk_tooltip", "Storage checked")}>
                        <span class="iconify lucide--check size-3.5"></span>
                    </div>
                    <button
                        class="btn btn-sm from-primary to-secondary text-primary-content ms-auto gap-2 border-none bg-linear-to-br"
                        onclick={on_open_settings}>
                        <span class="iconify lucide--wand-2 size-4"></span>
                        {t("dashboard.disk_manage", "Manage storage")}
                    </button>
                </div>
            </div>
        </div>
    }
}

fn render_path_usage(path: &PathUsage) -> Html {
    let percent = usage_percent(path.used_gb, path.total_gb);
    let used_label = format_capacity(path.used_gb);
    let total_label = format_capacity(path.total_gb);
    let max_gb = path.total_gb.max(1);
    let progress_class = usage_progress_class(percent);

    html! {
        <div class="border-base-200 rounded-box mt-2 border p-3">
            <div class="flex items-center justify-between">
                <p class="text-sm font-medium">{path.label}</p>
                <span class="text-base-content/80 text-xs">{format!("{percent:.0}%")}</span>
            </div>
            <progress
                max={max_gb.to_string()}
                value={path.used_gb.to_string()}
                class={classes!("progress", progress_class, "mt-1.5", "h-1.5", "align-super")}></progress>
            <div class="-mt-1.5 flex items-center justify-between">
                <span class="text-sm font-medium">{used_label}</span>
                <span class="text-base-content/80 text-xs">{total_label}</span>
            </div>
        </div>
    }
}

fn usage_progress_class(percent: f64) -> &'static str {
    if percent >= 90.0 {
        "progress-error"
    } else if percent >= 75.0 {
        "progress-warning"
    } else {
        "progress-success"
    }
}

fn usage_status(percent: f64, bundle: &TranslationBundle) -> String {
    if percent >= 90.0 {
        bundle.text("dashboard.disk_full", "Drive is almost full")
    } else if percent >= 75.0 {
        bundle.text("dashboard.disk_warn", "Storage is filling up")
    } else {
        bundle.text("dashboard.disk_ok", "Plenty of space available")
    }
}

fn format_capacity(gb: u32) -> String {
    if gb >= 1024 {
        let tb = f64::from(gb) / 1024.0;
        format!("{tb:.1} TB")
    } else {
        format!("{gb} GB")
    }
}

fn usage_percent(used: u32, total: u32) -> f64 {
    if total == 0 {
        0.0
    } else {
        f64::from(used) / f64::from(total) * 100.0
    }
}
