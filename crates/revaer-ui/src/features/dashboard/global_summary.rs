//! Nexus global summary card for the dashboard.
//!
//! # Design
//! - Mirror the Nexus "Global Sales" card structure for layout parity.
//! - Use snapshot-driven labels to avoid unused placeholder content.
//! - Failure mode: if snapshot values are missing, keep layout intact with key text.

use crate::app::Route;
use crate::components::atoms::icons::{IconEye, IconGlobe2};
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::DashboardSnapshot;
use yew::prelude::*;
use yew_router::prelude::Link;

#[derive(Properties, PartialEq)]
pub(crate) struct DashboardGlobalSummaryProps {
    pub snapshot: DashboardSnapshot,
}

#[function_component(DashboardGlobalSummary)]
pub(crate) fn dashboard_global_summary(props: &DashboardGlobalSummaryProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let label = bundle.text("dashboard.queue");
    let depth_label = format!("{label} ({})", props.snapshot.queue.depth);
    let overview_label = bundle.text("nav.torrents");

    html! {
        <div class="xl:col-span-3 2xl:col-span-4">
            <div class="card bg-base-100 shadow">
                <div class="card-body gap-0 p-0">
                    <div class="flex items-center gap-3 px-5 pt-5">
                        <IconGlobe2 size={Some(AttrValue::from("4.5"))} />
                        <span class="font-medium">{depth_label}</span>
                        <Link<Route>
                            to={Route::Torrents}
                            classes="btn btn-ghost btn-outline border-base-300 btn-sm z-1 ms-auto">
                            <IconEye size={Some(AttrValue::from("4"))} />
                            {overview_label}
                        </Link<Route>>
                    </div>
                    <div class="me-5 -mt-5 mb-1">
                        <div id="global-sales-chart"></div>
                    </div>
                </div>
            </div>
        </div>
    }
}
