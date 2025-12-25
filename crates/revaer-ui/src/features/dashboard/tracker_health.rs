use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::DashboardSnapshot;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct DashboardTrackerHealthProps {
    pub snapshot: DashboardSnapshot,
}

#[function_component(DashboardTrackerHealth)]
pub(crate) fn dashboard_tracker_health(props: &DashboardTrackerHealthProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str, fallback: &str| bundle.text(key, fallback);

    let ok = props.snapshot.tracker_health.ok;
    let warn = props.snapshot.tracker_health.warn;
    let error = props.snapshot.tracker_health.error;
    let error_label = format!("{} {}", t("dashboard.error", "Error"), error);

    html! {
        <div class="xl:col-span-5">
            <div class="card bg-base-100 shadow">
                <div class="card-body p-0">
                    <div class="flex items-center justify-between px-5 pt-5">
                        <span class="font-medium">
                            {t("dashboard.tracker_health", "Tracker Health")}
                        </span>
                        <div class="inline-flex items-center gap-2">
                            <div class="text-base-content/60 w-6 border border-dashed"></div>
                            <span class="text-base-content/80 text-xs">
                                {error_label}
                            </span>
                        </div>
                    </div>
                    <div class="mt-4 py-3">
                        <div
                            class="divide-base-300 grid grid-cols-2 gap-5 px-5 sm:grid-cols-3 sm:divide-x">
                            <div class="text-center">
                                <p>{t("dashboard.ok", "OK")}</p>
                                <p class="mt-0.5 text-xl font-medium">{ok}</p>
                                <div class="text-success mt-0.5 inline-flex items-center gap-1">
                                    <span class="iconify lucide--arrow-up size-3"></span>
                                    <p class="text-xs">{"Stable"}</p>
                                </div>
                            </div>
                            <div class="hidden text-center sm:block">
                                <p>{t("dashboard.warn", "Warn")}</p>
                                <p class="mt-0.5 text-xl font-medium">{warn}</p>
                                <div class="text-success mt-0.5 inline-flex items-center gap-1">
                                    <span class="iconify lucide--arrow-up size-3"></span>
                                    <p class="text-xs">{"Watch"}</p>
                                </div>
                            </div>
                        </div>
                    </div>
                    <div class="-mt-25 sm:mx-5">
                        <div id="customer-acquisition-chart"></div>
                    </div>
                </div>
            </div>
        </div>
    }
}
