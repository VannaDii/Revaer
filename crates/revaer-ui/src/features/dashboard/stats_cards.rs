use crate::core::logic::format_rate;
use crate::core::store::SystemRates;
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::DashboardSnapshot;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct DashboardStatsCardsProps {
    pub snapshot: DashboardSnapshot,
    pub system_rates: SystemRates,
}

#[function_component(DashboardStatsCards)]
pub(crate) fn dashboard_stats_cards(props: &DashboardStatsCardsProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str, fallback: &str| bundle.text(key, fallback);

    let download_rate = format_rate(props.system_rates.download_bps);
    let upload_rate = format_rate(props.system_rates.upload_bps);
    let active_count = props.snapshot.active;
    let completed_count = props.snapshot.completed;

    html! {
        <div class="grid gap-5 lg:grid-cols-2 xl:grid-cols-4">
            <div class="card bg-base-100 shadow">
                <div class="card-body gap-2">
                    <div class="flex items-start justify-between gap-2 text-sm">
                        <div>
                            <p class="text-base-content/80 font-medium">
                                {t("dashboard.down", "Down")}
                            </p>
                            <div class="mt-3 flex items-center gap-2">
                                <p class="inline text-2xl font-semibold">
                                    {download_rate}
                                </p>
                                <div
                                    class="badge badge-soft badge-success badge-sm gap-0.5 px-1 font-medium">
                                    <span
                                        class="iconify lucide--arrow-up size-3.5"></span>
                                    {"10.8%"}
                                </div>
                            </div>
                        </div>
                        <div class="bg-base-200 rounded-box flex items-center p-2">
                            <span
                                class="iconify lucide--circle-dollar-sign size-5"></span>
                        </div>
                    </div>
                    <p class="text-base-content/60 text-sm">
                        {"vs."}
                        <span class="mx-1">{"$494.16"}</span>
                        {"last period"}
                    </p>
                </div>
            </div>
            <div class="card bg-base-100 shadow">
                <div class="card-body gap-2">
                    <div class="flex items-start justify-between gap-2 text-sm">
                        <div>
                            <p class="text-base-content/80 font-medium">
                                {t("dashboard.up", "Up")}
                            </p>
                            <div class="mt-3 flex items-center gap-2">
                                <p class="inline text-2xl font-semibold">
                                    {upload_rate}
                                </p>
                                <div
                                    class="badge badge-soft badge-success badge-sm gap-0.5 px-1 font-medium">
                                    <span
                                        class="iconify lucide--arrow-up size-3.5"></span>
                                    {"21.2%"}
                                </div>
                            </div>
                        </div>
                        <div class="bg-base-200 rounded-box flex items-center p-2">
                            <span class="iconify lucide--package size-5"></span>
                        </div>
                    </div>
                    <p class="text-base-content/60 text-sm">
                        {"vs."}
                        <span class="mx-1">{"3845"}</span>
                        {"last period"}
                    </p>
                </div>
            </div>
            <div class="card bg-base-100 shadow">
                <div class="card-body gap-2">
                    <div class="flex items-start justify-between gap-2 text-sm">
                        <div>
                            <p class="text-base-content/80 font-medium">
                                {t("dashboard.active", "Active")}
                            </p>
                            <div class="mt-3 flex items-center gap-2">
                                <p class="inline text-2xl font-semibold">
                                    {active_count}
                                </p>
                                <div
                                    class="badge badge-soft badge-error badge-sm gap-0.5 px-1 font-medium">
                                    <span
                                        class="iconify lucide--arrow-down size-3.5"></span>
                                    {"-6.8%"}
                                </div>
                            </div>
                        </div>
                        <div class="bg-base-200 rounded-box flex items-center p-2">
                            <span class="iconify lucide--users size-5"></span>
                        </div>
                    </div>
                    <p class="text-base-content/60 text-sm">
                        {"vs."}
                        <span class="mx-1">{"2448"}</span>
                        {"last period"}
                    </p>
                </div>
            </div>
            <div class="card bg-base-100 shadow">
                <div class="card-body gap-2">
                    <div class="flex items-start justify-between gap-2 text-sm">
                        <div>
                            <p class="text-base-content/80 font-medium">
                                {t("dashboard.completed", "Completed")}
                            </p>
                            <div class="mt-3 flex items-center gap-2">
                                <p class="inline text-2xl font-semibold">
                                    {completed_count}
                                </p>
                                <div
                                    class="badge badge-soft badge-success badge-sm gap-0.5 px-1 font-medium">
                                    <span
                                        class="iconify lucide--arrow-up size-3.5"></span>
                                    {"8.5%"}
                                </div>
                            </div>
                        </div>
                        <div class="bg-base-200 rounded-box flex items-center p-2">
                            <span class="iconify lucide--eraser size-5"></span>
                        </div>
                    </div>
                    <p class="text-base-content/60 text-sm">
                        {"vs."}
                        <span class="mx-1">{"$98.14"}</span>
                        {"last period"}
                    </p>
                </div>
            </div>
        </div>
    }
}
