use crate::components::atoms::IconButton;
use crate::components::atoms::icons::{IconDownload, IconEye, IconShoppingBag, IconTrash};
use crate::components::daisy::{DaisyColor, DaisySize, DaisyVariant, List};
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
    let t = |key: &str| bundle.text(key);

    let rows = if props.snapshot.recent_events.is_empty() {
        html! {
            <li class="px-5 py-4 text-base-content/60 text-sm">
                {t("dashboard.events_sub")}
            </li>
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
                        EventKind::Info => t("dashboard.event_info"),
                        EventKind::Warning => t("dashboard.event_warn"),
                        EventKind::Error => t("dashboard.event_error"),
                    };
                    html! {
                        <li class="row-hover px-5 py-3">
                            <div class="grid grid-cols-7 items-center gap-3">
                                <div>
                                    <input
                                        aria-label="checked-order"
                                        class="checkbox checkbox-sm"
                                        type="checkbox" />
                                </div>
                                <div class="col-span-2 flex items-center space-x-3 truncate">
                                    <img
                                        alt="order image"
                                        class="mask mask-squircle bg-base-200 size-7.5"
                                        src={event_icon_src(idx)} />
                                    <p>{event.label}</p>
                                </div>
                                <div class="font-medium">{event.detail}</div>
                                <div class="text-xs">{"Just now"}</div>
                                <div>
                                    <div class={classes!("badge", badge_class, "badge-sm", "badge-soft")}>{badge_label}</div>
                                </div>
                                <div class="flex items-center gap-1">
                                    <IconButton
                                        icon={html! { <IconEye class={classes!("text-base-content/60")} size={Some(AttrValue::from("4"))} /> }}
                                        label={AttrValue::from("Show product")}
                                        size={DaisySize::Xs}
                                        class={classes!("btn-square")}
                                    />
                                    <IconButton
                                        icon={html! { <IconTrash size={Some(AttrValue::from("4"))} /> }}
                                        label={AttrValue::from("Delete")}
                                        size={DaisySize::Xs}
                                        class={classes!("btn-square", "border-transparent")}
                                        tone={Some(DaisyColor::Error)}
                                        variant={DaisyVariant::Outline}
                                    />
                                </div>
                            </div>
                        </li>
                    }
                })}
        }
    };

    html! {
        <div class="xl:col-span-3 2xl:col-span-5">
            <div aria-label="Card" class="card bg-base-100 shadow">
                <div class="card-body p-0">
                    <div class="flex items-center gap-3 px-5 pt-5">
                        <IconShoppingBag size={Some(AttrValue::from("4.5"))} />
                        <span class="font-medium">{t("dashboard.events")}</span>
                        <button class="btn btn-outline border-base-300 btn-sm ms-auto">
                            <IconDownload size={Some(AttrValue::from("3.5"))} />
                            {"Report"}
                        </button>
                    </div>
                    <div class="mt-2 overflow-auto">
                        <div class="px-5 pb-2">
                            <div class="grid grid-cols-7 items-center gap-3 text-base-content/60 text-xs">
                                <div>
                                    <input
                                        aria-label="checked-all-order"
                                        class="checkbox checkbox-sm"
                                        type="checkbox" />
                                </div>
                                <div class="col-span-2">{"Event"}</div>
                                <div>{"Detail"}</div>
                                <div>{"When"}</div>
                                <div>{"Severity"}</div>
                                <div>{"Action"}</div>
                            </div>
                        </div>
                        <List class="bg-base-200 divide-base-200 divide-y *:text-nowrap">
                            {rows}
                        </List>
                    </div>
                </div>
            </div>
        </div>
    }
}

fn event_icon_src(index: usize) -> String {
    let product = (index % 10) + 1;
    format!("/static/nexus/images/apps/ecommerce/products/{product}.jpg")
}
