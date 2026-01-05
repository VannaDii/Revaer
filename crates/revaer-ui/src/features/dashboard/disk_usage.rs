use crate::components::atoms::IconButton;
use crate::components::atoms::icons::IconMoreHorizontal;
use crate::components::daisy::DaisySize;
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::DashboardSnapshot;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct DashboardDiskUsageProps {
    pub snapshot: DashboardSnapshot,
}

#[function_component(DashboardDiskUsage)]
pub(crate) fn dashboard_disk_usage(props: &DashboardDiskUsageProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key);

    let percent_value =
        usage_percent(props.snapshot.disk_used_gb, props.snapshot.disk_total_gb).clamp(0.0, 100.0);
    let used_label = format_capacity(props.snapshot.disk_used_gb);
    let total_label = format_capacity(props.snapshot.disk_total_gb);
    let usage_label = format!("{used_label} / {total_label}");
    let percent_label = format!("{percent_value:.1}%");
    let action_placeholder = Callback::from(|_| {});

    html! {
        <div class="xl:col-span-7">
            <div class="card bg-base-100 shadow">
                <div class="card-body px-0 pb-0">
                    <div class="px-6">
                        <div class="flex items-start justify-between">
                            <span class="font-medium">{t("dashboard.disk")}</span>
                            <div class="flex items-center gap-2">
                                <div role="tablist" class="tabs tabs-boxed tabs-xs hidden sm:flex">
                                    <button
                                        type="button"
                                        role="tab"
                                        class="tab tab-disabled"
                                        aria-selected="false"
                                        disabled={true}
                                    >
                                        {"Day"}
                                    </button>
                                    <button
                                        type="button"
                                        role="tab"
                                        class="tab tab-disabled"
                                        aria-selected="false"
                                        disabled={true}
                                    >
                                        {"Month"}
                                    </button>
                                    <button
                                        type="button"
                                        role="tab"
                                        class="tab tab-active"
                                        aria-selected="true"
                                    >
                                        {"Year"}
                                    </button>
                                </div>
                                <div class="dropdown dropdown-end">
                                    <IconButton
                                        icon={html! { <IconMoreHorizontal size={Some(AttrValue::from("4"))} /> }}
                                        label={AttrValue::from("Storage actions")}
                                        size={DaisySize::Xs}
                                        class={classes!("btn-square")}
                                    />
                                    <ul class="dropdown-content menu bg-base-100 rounded-box mt-2 w-44 p-1 shadow z-10">
                                        <li><button type="button" onclick={action_placeholder.clone()}>{"Enhance"}</button></li>
                                        <li><button type="button" onclick={action_placeholder.clone()}>{"Insights"}</button></li>
                                        <li><button type="button" onclick={action_placeholder.clone()}>{"Auto Tag"}</button></li>
                                        <li><button type="button" class="text-error" onclick={action_placeholder}>{"Delete"}</button></li>
                                    </ul>
                                </div>
                            </div>
                        </div>
                        <div class="mt-3">
                            <div class="flex items-center gap-3">
                                <span class="text-4xl font-semibold">{usage_label}</span>
                                <span class="text-success font-medium">{percent_label}</span>
                            </div>
                            <span class="text-base-content/60 text-sm">
                                {t("dashboard.disk_sub")}
                            </span>
                        </div>
                    </div>
                    <div id="revenue-statics-chart"></div>
                </div>
            </div>
        </div>
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
