//! Health page view.
//!
//! # Design
//! - Render cached health snapshots without issuing API calls.
//! - Surface degraded components and torrent metrics when available.

use crate::core::store::AppStore;
use crate::core::store::{FullHealthSnapshot, HealthSnapshot};
use crate::core::store::{HealthMetricsSnapshot, TorrentHealthSnapshot};
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use yew::prelude::*;
use yewdux::prelude::use_selector;

#[derive(Properties, PartialEq)]
pub(crate) struct HealthPageProps {
    pub on_copy_metrics: Callback<String>,
}

#[function_component(HealthPage)]
pub(crate) fn health_page(props: &HealthPageProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key);

    let basic = use_selector(|store: &AppStore| store.health.basic.clone());
    let full = use_selector(|store: &AppStore| store.health.full.clone());
    let metrics_text = use_selector(|store: &AppStore| store.health.metrics_text.clone());
    let metrics_text_value = (*metrics_text).clone();
    let can_copy = metrics_text_value
        .as_ref()
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false);
    let on_copy = {
        let on_copy_metrics = props.on_copy_metrics.clone();
        let metrics_text = metrics_text_value.clone();
        Callback::from(move |_| {
            if let Some(text) = metrics_text.clone() {
                if !text.trim().is_empty() {
                    on_copy_metrics.emit(text);
                }
            }
        })
    };

    html! {
        <section class="space-y-6">
            <div class="card bg-base-100 shadow">
                <div class="card-body gap-6">
                    <div class="space-y-1">
                        <p class="text-xs uppercase tracking-wide text-base-content/60">{t("nav.health")}</p>
                        <h3 class="text-lg font-semibold">{t("health.title")}</h3>
                        <p class="text-sm text-base-content/60">{t("health.body")}</p>
                    </div>
                    <div class="grid gap-4 lg:grid-cols-2">
                        {render_basic(&t, (*basic).clone())}
                        {render_full(&t, (*full).clone())}
                    </div>
                    <div class="rounded-box border border-base-200 bg-base-200/40 p-4 space-y-3">
                        <div class="flex items-center gap-2">
                            <strong class="text-sm">{t("health.metrics")}</strong>
                            <span class="badge badge-ghost badge-xs">{"/metrics"}</span>
                            <button
                                class="btn btn-ghost btn-xs ms-auto"
                                disabled={!can_copy}
                                onclick={on_copy}
                            >
                                {t("health.metrics_copy")}
                            </button>
                        </div>
                        {if let Some(text) = metrics_text_value {
                            html! { <pre class="rounded-box bg-base-200 p-3 text-xs text-base-content/80 overflow-auto">{text}</pre> }
                        } else {
                            html! { <p class="text-sm text-base-content/60">{t("health.metrics_empty")}</p> }
                        }}
                    </div>
                </div>
            </div>
        </section>
    }
}

fn render_basic(t: &impl Fn(&str) -> String, basic: Option<HealthSnapshot>) -> Html {
    let Some(snapshot) = basic else {
        return html! {
            <div class="card bg-base-200 border border-base-200">
                <div class="card-body gap-2">
                    <h4 class="text-sm font-semibold">{t("health.basic")}</h4>
                    <p class="text-sm text-base-content/60">{t("health.basic_empty")}</p>
                </div>
            </div>
        };
    };
    let db_status = snapshot.database_status.as_deref().unwrap_or("unknown");
    let db_revision = snapshot
        .database_revision
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());
    html! {
        <div class="card bg-base-200 border border-base-200">
            <div class="card-body gap-3">
                <h4 class="text-sm font-semibold">{t("health.basic")}</h4>
                <div class="flex items-center justify-between text-sm">
                    <span class="text-base-content/60">{t("health.status")}</span>
                    <span class={status_badge(snapshot.status.as_str())}>{snapshot.status.clone()}</span>
                </div>
                <div class="flex items-center justify-between text-sm">
                    <span class="text-base-content/60">{t("health.mode")}</span>
                    <span class="badge badge-ghost badge-sm">{snapshot.mode}</span>
                </div>
                <div class="flex items-center justify-between text-sm">
                    <span class="text-base-content/60">{t("health.db_status")}</span>
                    <span class={status_badge(db_status)}>{db_status.to_string()}</span>
                </div>
                <div class="flex items-center justify-between text-sm">
                    <span class="text-base-content/60">{t("health.db_revision")}</span>
                    <span class="badge badge-ghost badge-sm">{db_revision}</span>
                </div>
            </div>
        </div>
    }
}

fn render_full(t: &impl Fn(&str) -> String, full: Option<FullHealthSnapshot>) -> Html {
    let Some(snapshot) = full else {
        return html! {
            <div class="card bg-base-200 border border-base-200">
                <div class="card-body gap-2">
                    <h4 class="text-sm font-semibold">{t("health.full")}</h4>
                    <p class="text-sm text-base-content/60">{t("health.full_empty")}</p>
                </div>
            </div>
        };
    };
    let degraded = if snapshot.degraded.is_empty() {
        html! { <span class="badge badge-ghost badge-sm">{t("health.degraded_none")}</span> }
    } else {
        html! {
            <div class="flex flex-wrap gap-1">
                {for snapshot.degraded.iter().map(|name| html! { <span class="badge badge-warning badge-soft badge-sm">{name.clone()}</span> })}
            </div>
        }
    };
    html! {
        <div class="card bg-base-200 border border-base-200">
            <div class="card-body gap-3">
                <h4 class="text-sm font-semibold">{t("health.full")}</h4>
                <div class="flex items-center justify-between text-sm">
                    <span class="text-base-content/60">{t("health.status")}</span>
                    <span class={status_badge(snapshot.status.as_str())}>{snapshot.status.clone()}</span>
                </div>
                <div class="flex items-center justify-between text-sm">
                    <span class="text-base-content/60">{t("health.mode")}</span>
                    <span class="badge badge-ghost badge-sm">{snapshot.mode}</span>
                </div>
                <div class="flex items-center justify-between text-sm">
                    <span class="text-base-content/60">{t("health.revision")}</span>
                    <span class="badge badge-ghost badge-sm">{snapshot.revision}</span>
                </div>
                <div class="flex items-center justify-between text-sm">
                    <span class="text-base-content/60">{t("health.build")}</span>
                    <span class="badge badge-ghost badge-sm">{snapshot.build}</span>
                </div>
                <div class="flex items-center justify-between text-sm">
                    <span class="text-base-content/60">{t("health.degraded")}</span>
                    {degraded}
                </div>
                <div class="rounded-box border border-base-200 bg-base-200/40 p-3 space-y-2">
                    <h5 class="text-sm font-semibold">{t("health.metrics_summary")}</h5>
                    {render_metrics(&snapshot.metrics)}
                </div>
                <div class="rounded-box border border-base-200 bg-base-200/40 p-3 space-y-2">
                    <h5 class="text-sm font-semibold">{t("health.torrent")}</h5>
                    {render_torrent_snapshot(&snapshot.torrent)}
                </div>
            </div>
        </div>
    }
}

fn render_metrics(metrics: &HealthMetricsSnapshot) -> Html {
    html! {
        <div class="grid gap-2 sm:grid-cols-2">
            <div class="flex items-center justify-between text-sm">
                <span class="text-base-content/60">{"Config watch (ms)"}</span>
                <span class="badge badge-ghost badge-sm">{metrics.config_watch_latency_ms}</span>
            </div>
            <div class="flex items-center justify-between text-sm">
                <span class="text-base-content/60">{"Config apply (ms)"}</span>
                <span class="badge badge-ghost badge-sm">{metrics.config_apply_latency_ms}</span>
            </div>
            <div class="flex items-center justify-between text-sm">
                <span class="text-base-content/60">{"Config failures"}</span>
                <span class="badge badge-ghost badge-sm">{metrics.config_update_failures_total}</span>
            </div>
            <div class="flex items-center justify-between text-sm">
                <span class="text-base-content/60">{"Watch slow"}</span>
                <span class="badge badge-ghost badge-sm">{metrics.config_watch_slow_total}</span>
            </div>
            <div class="flex items-center justify-between text-sm">
                <span class="text-base-content/60">{"Guardrail"}</span>
                <span class="badge badge-ghost badge-sm">{metrics.guardrail_violations_total}</span>
            </div>
            <div class="flex items-center justify-between text-sm">
                <span class="text-base-content/60">{"Rate limit"}</span>
                <span class="badge badge-ghost badge-sm">{metrics.rate_limit_throttled_total}</span>
            </div>
        </div>
    }
}

fn render_torrent_snapshot(snapshot: &TorrentHealthSnapshot) -> Html {
    html! {
        <div class="grid gap-2 sm:grid-cols-2">
            <div class="flex items-center justify-between text-sm">
                <span class="text-base-content/60">{"Active"}</span>
                <span class="badge badge-ghost badge-sm">{snapshot.active}</span>
            </div>
            <div class="flex items-center justify-between text-sm">
                <span class="text-base-content/60">{"Queue depth"}</span>
                <span class="badge badge-ghost badge-sm">{snapshot.queue_depth}</span>
            </div>
        </div>
    }
}

fn status_badge(status: &str) -> Classes {
    let tone = match status {
        "ok" | "healthy" | "active" => Some("badge-success"),
        "warn" | "warning" | "degraded" => Some("badge-warning"),
        "error" | "failed" => Some("badge-error"),
        _ => None,
    };
    let mut classes = classes!("badge", "badge-sm");
    if let Some(tone) = tone {
        classes.push(tone);
        classes.push("badge-soft");
    } else {
        classes.push("badge-ghost");
    }
    classes
}
