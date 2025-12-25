use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::DashboardSnapshot;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct DashboardQueueSummaryProps {
    pub snapshot: DashboardSnapshot,
}

#[function_component(DashboardQueueSummary)]
pub(crate) fn dashboard_queue_summary(props: &DashboardQueueSummaryProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str, fallback: &str| bundle.text(key, fallback);

    let entries = [
        (
            t("dashboard.active", "Active"),
            props.snapshot.queue.active,
            "Currently active",
        ),
        (
            t("dashboard.paused", "Paused"),
            props.snapshot.queue.paused,
            "Paused by policy",
        ),
        (
            t("dashboard.queued", "Queued"),
            props.snapshot.queue.queued,
            "Waiting in queue",
        ),
        (
            t("dashboard.depth", "Depth"),
            props.snapshot.queue.depth,
            "Depth remaining",
        ),
    ];

    html! {
        <>
            <div class="xl:col-span-2 2xl:col-span-3">
                <div class="card bg-base-100 shadow">
                    <div class="card-body pb-3">
                        <div class="flex items-center gap-3">
                            <span
                                class="iconify lucide--messages-square size-4.5"></span>
                            <span class="font-medium">{"Quick Chat"}</span>
                            <a
                                href="#"
                                class="btn btn-outline btn-sm border-base-300 ms-auto">
                                {"Go To Chat"}
                            </a>
                        </div>
                        <div class="-mx-2 mt-2 space-y-0.5">
                            {for entries.iter().enumerate().map(|(idx, (label, value, detail))| {
                                html! {
                                    <div
                                        class="rounded-box hover:bg-base-200 flex cursor-pointer items-center gap-3 px-2 py-2 transition-all active:scale-[.98]">
                                        <img
                                            alt="chat"
                                            class="bg-base-200 mask mask-squircle size-11"
                                            src={queue_icon_src(idx)} />
                                        <div class="grow">
                                            <div class="flex gap-1">
                                                <p class="grow">{label}</p>
                                                <span class="text-base-content/60 text-xs">
                                                    {value}
                                                </span>
                                            </div>
                                            <p
                                                class="text-base-content/80 line-clamp-1 text-sm text-ellipsis">
                                                {detail}
                                            </p>
                                        </div>
                                    </div>
                                }
                            })}
                        </div>
                    </div>
                </div>
            </div>
            <div class="xl:col-span-3 2xl:col-span-4">
                <div class="card bg-base-100 shadow">
                    <div class="card-body gap-0 p-0">
                        <div class="flex items-center gap-3 px-5 pt-5">
                            <span
                                class="iconify lucide--globe-2 size-4.5"></span>
                            <span class="font-medium">{"Global Sales (%)"}</span>
                            <button
                                class="btn btn-ghost btn-outline border-base-300 btn-sm z-1 ms-auto">
                                <span class="iconify lucide--eye size-4"></span>
                                {"Overview"}
                            </button>
                        </div>
                        <div class="me-5 -mt-5 mb-1">
                            <div id="global-sales-chart"></div>
                        </div>
                    </div>
                </div>
            </div>
        </>
    }
}

fn queue_icon_src(index: usize) -> String {
    let avatar = (index % 5) + 1;
    format!("/static/nexus/images/avatars/{avatar}.png")
}
