use crate::app::Route;
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::DashboardSnapshot;
use yew::prelude::*;
use yew_router::prelude::Link;

#[derive(Properties, PartialEq)]
pub(crate) struct DashboardQueueSummaryProps {
    pub snapshot: DashboardSnapshot,
}

#[function_component(DashboardQueueSummary)]
pub(crate) fn dashboard_queue_summary(props: &DashboardQueueSummaryProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key);

    let entries = [
        (
            t("dashboard.active"),
            props.snapshot.queue.active,
            t("dashboard.torrents_sub"),
        ),
        (
            t("dashboard.paused"),
            props.snapshot.queue.paused,
            t("dashboard.torrents_sub"),
        ),
        (
            t("dashboard.queued"),
            props.snapshot.queue.queued,
            t("dashboard.queue_sub"),
        ),
        (
            t("dashboard.depth"),
            props.snapshot.queue.depth,
            t("dashboard.queue_sub"),
        ),
    ];

    html! {
        <div class="xl:col-span-2 2xl:col-span-3">
            <div class="card bg-base-100 shadow">
                <div class="card-body pb-3">
                    <div class="flex items-center gap-3">
                        <span class="iconify lucide--messages-square size-4.5"></span>
                        <span class="font-medium">{t("dashboard.queue")}</span>
                        <Link<Route>
                            to={Route::Torrents}
                            classes="btn btn-outline btn-sm border-base-300 ms-auto">
                            {t("nav.torrents")}
                        </Link<Route>>
                    </div>
                    <div class="-mx-2 mt-2 space-y-0.5">
                        {for entries.iter().enumerate().map(|(idx, (label, value, detail))| {
                            html! {
                                <div
                                    class="rounded-box hover:bg-base-200 flex cursor-pointer items-center gap-3 px-2 py-2 transition-all active:scale-[.98]">
                                    <img
                                        alt="queue"
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
    }
}

fn queue_icon_src(index: usize) -> String {
    let avatar = (index % 5) + 1;
    format!("/static/nexus/images/avatars/{avatar}.png")
}
