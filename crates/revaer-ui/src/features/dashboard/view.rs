use crate::core::store::SystemRates;
use crate::models::DashboardSnapshot;
use yew::prelude::*;

use super::disk_usage::DashboardDiskUsage;
use super::global_summary::DashboardGlobalSummary;
use super::queue_summary::DashboardQueueSummary;
use super::recent_events::DashboardRecentEvents;
use super::shell::DashboardShell;
use super::stats_cards::DashboardStatsCards;
use super::tracker_health::DashboardTrackerHealth;

#[derive(Properties, PartialEq)]
pub(crate) struct DashboardPageProps {
    pub snapshot: DashboardSnapshot,
    pub system_rates: SystemRates,
}

#[function_component(DashboardPage)]
pub(crate) fn dashboard_page(props: &DashboardPageProps) -> Html {
    html! {
        <DashboardShell>
            <DashboardStatsCards
                snapshot={props.snapshot.clone()}
                system_rates={props.system_rates}
            />
            <div class="mt-6 grid grid-cols-1 gap-6 xl:grid-cols-12">
                <DashboardDiskUsage snapshot={props.snapshot.clone()} />
                <DashboardTrackerHealth snapshot={props.snapshot.clone()} />
            </div>
            <div class="mt-6 grid grid-cols-1 gap-6 xl:grid-cols-5 2xl:grid-cols-12">
                <DashboardRecentEvents snapshot={props.snapshot.clone()} />
                <DashboardQueueSummary snapshot={props.snapshot.clone()} />
                <DashboardGlobalSummary snapshot={props.snapshot.clone()} />
            </div>
        </DashboardShell>
    }
}
