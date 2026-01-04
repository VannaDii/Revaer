use crate::components::atoms::icons::{
    IconArrowDown, IconArrowUp, IconCircleDollarSign, IconEraser, IconPackage, IconUsers,
    IconVariant,
};
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
    let t = |key: &str| bundle.text(key);

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
                                {t("dashboard.down")}
                            </p>
                            <div class="mt-3 flex items-center gap-2">
                                <p class="inline text-2xl font-semibold">
                                    {download_rate}
                                </p>
                                <div
                                    class="badge badge-soft badge-success badge-sm gap-0.5 px-1 font-medium">
                                    <IconArrowUp size={Some(AttrValue::from("3.5"))} />
                                    {"10.8%"}
                                </div>
                            </div>
                        </div>
                        <div class="bg-base-200 rounded-box flex items-center p-2">
                            <IconCircleDollarSign
                                size={Some(AttrValue::from("5"))}
                                variant={IconVariant::Solid}
                            />
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
                                {t("dashboard.up")}
                            </p>
                            <div class="mt-3 flex items-center gap-2">
                                <p class="inline text-2xl font-semibold">
                                    {upload_rate}
                                </p>
                                <div
                                    class="badge badge-soft badge-success badge-sm gap-0.5 px-1 font-medium">
                                    <IconArrowUp size={Some(AttrValue::from("3.5"))} />
                                    {"21.2%"}
                                </div>
                            </div>
                        </div>
                        <div class="bg-base-200 rounded-box flex items-center p-2">
                            <IconPackage size={Some(AttrValue::from("5"))} />
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
                                {t("dashboard.active")}
                            </p>
                            <div class="mt-3 flex items-center gap-2">
                                <p class="inline text-2xl font-semibold">
                                    {active_count}
                                </p>
                                <div
                                    class="badge badge-soft badge-error badge-sm gap-0.5 px-1 font-medium">
                                    <IconArrowDown size={Some(AttrValue::from("3.5"))} />
                                    {"-6.8%"}
                                </div>
                            </div>
                        </div>
                        <div class="bg-base-200 rounded-box flex items-center p-2">
                            <IconUsers size={Some(AttrValue::from("5"))} />
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
                                {t("dashboard.completed")}
                            </p>
                            <div class="mt-3 flex items-center gap-2">
                                <p class="inline text-2xl font-semibold">
                                    {completed_count}
                                </p>
                                <div
                                    class="badge badge-soft badge-success badge-sm gap-0.5 px-1 font-medium">
                                    <IconArrowUp size={Some(AttrValue::from("3.5"))} />
                                    {"8.5%"}
                                </div>
                            </div>
                        </div>
                        <div class="bg-base-200 rounded-box flex items-center p-2">
                            <IconEraser size={Some(AttrValue::from("5"))} />
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
