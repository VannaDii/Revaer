use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::{DashboardSnapshot, EventKind};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct DashboardRecentEventsProps {
    pub snapshot: DashboardSnapshot,
}

#[function_component(DashboardRecentEvents)]
pub(crate) fn dashboard_recent_events(props: &DashboardRecentEventsProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str, fallback: &str| bundle.text(key, fallback);

    let rows = if props.snapshot.recent_events.is_empty() {
        html! {
            <tr>
                <td colspan="6" class="text-base-content/60 text-sm">
                    {t("dashboard.no_events", "No recent events")}
                </td>
            </tr>
        }
    } else {
        html! {
            {for props
                .snapshot
                .recent_events
                .iter()
                .take(6)
                .enumerate()
                .map(|(idx, event)| {
                    let badge_class = match event.kind {
                        EventKind::Info => "badge-info",
                        EventKind::Warning => "badge-warning",
                        EventKind::Error => "badge-error",
                    };
                    let badge_label = match event.kind {
                        EventKind::Info => t("dashboard.event_info", "Info"),
                        EventKind::Warning => t("dashboard.event_warn", "Warn"),
                        EventKind::Error => t("dashboard.event_error", "Error"),
                    };
                    html! {
                        <tr>
                            <th>
                                <input
                                    aria-label="checked-order"
                                    class="checkbox checkbox-sm"
                                    type="checkbox" />
                            </th>
                            <td class="flex items-center space-x-3 truncate">
                                <img
                                    alt="order image"
                                    class="mask mask-squircle bg-base-200 size-7.5"
                                    src={event_icon_src(idx)} />
                                <p>{event.label}</p>
                            </td>
                            <td class="font-medium">{event.detail}</td>
                            <td class="text-xs">{"Just now"}</td>
                            <td>
                                <div class={classes!("badge", badge_class, "badge-sm", "badge-soft")}>{badge_label}</div>
                            </td>
                            <td>
                                <div class="flex items-center gap-1">
                                    <button
                                        aria-label="Show product"
                                        class="btn btn-square btn-ghost btn-xs">
                                        <span
                                            class="iconify lucide--eye text-base-content/60 size-4"></span>
                                    </button>
                                    <button
                                        aria-label="Show product"
                                        class="btn btn-square btn-error btn-outline btn-xs border-transparent">
                                        <span
                                            class="iconify lucide--trash size-4"></span>
                                    </button>
                                </div>
                            </td>
                        </tr>
                    }
                })}
        }
    };

    html! {
        <div class="xl:col-span-3 2xl:col-span-5">
            <div aria-label="Card" class="card bg-base-100 shadow">
                <div class="card-body p-0">
                    <div class="flex items-center gap-3 px-5 pt-5">
                        <span class="iconify lucide--shopping-bag size-4.5"></span>
                        <span class="font-medium">{t("dashboard.events", "Recent Events")}</span>
                        <button class="btn btn-outline border-base-300 btn-sm ms-auto">
                            <span class="iconify lucide--download size-3.5"></span>
                            {"Report"}
                        </button>
                    </div>
                    <div class="mt-2 overflow-auto">
                        <table class="table *:text-nowrap">
                            <thead>
                                <tr>
                                    <th>
                                        <input
                                            aria-label="checked-all-order"
                                            class="checkbox checkbox-sm"
                                            type="checkbox" />
                                    </th>
                                    <th>{"Event"}</th>
                                    <th>{"Detail"}</th>
                                    <th>{"When"}</th>
                                    <th>{"Severity"}</th>
                                    <th>{"Action"}</th>
                                </tr>
                            </thead>
                            <tbody>
                                {rows}
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>
        </div>
    }
}

fn event_icon_src(index: usize) -> String {
    let product = (index % 10) + 1;
    format!("static/nexus/images/apps/ecommerce/products/{product}.jpg")
}
