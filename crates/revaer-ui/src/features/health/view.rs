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
        <section class="health-page">
            <div class="panel">
                <div class="panel-head">
                    <div>
                        <p class="eyebrow">{t("nav.health")}</p>
                        <h3>{t("health.title")}</h3>
                        <p class="muted">{t("health.body")}</p>
                    </div>
                </div>
                <div class="health-grid">
                    {render_basic(&t, (*basic).clone())}
                    {render_full(&t, (*full).clone())}
                </div>
                <div class="metrics-panel">
                    <div class="panel-subhead">
                        <strong>{t("health.metrics")}</strong>
                        <span class="pill subtle">{"/metrics"}</span>
                        <button class="btn btn-ghost btn-xs" disabled={!can_copy} onclick={on_copy}>
                            {t("health.metrics_copy")}
                        </button>
                    </div>
                    {if let Some(text) = metrics_text_value {
                        html! { <pre class="metrics-output">{text}</pre> }
                    } else {
                        html! { <p class="muted">{t("health.metrics_empty")}</p> }
                    }}
                </div>
            </div>
        </section>
    }
}

fn render_basic(t: &impl Fn(&str) -> String, basic: Option<HealthSnapshot>) -> Html {
    let Some(snapshot) = basic else {
        return html! {
            <div class="card">
                <h4>{t("health.basic")}</h4>
                <p class="muted">{t("health.basic_empty")}</p>
            </div>
        };
    };
    let db_status = snapshot.database_status.as_deref().unwrap_or("unknown");
    let db_revision = snapshot
        .database_revision
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());
    html! {
        <div class="card">
            <h4>{t("health.basic")}</h4>
            <div class="health-row">
                <span class="muted">{t("health.status")}</span>
                <span class={status_pill(snapshot.status.as_str())}>{snapshot.status.clone()}</span>
            </div>
            <div class="health-row">
                <span class="muted">{t("health.mode")}</span>
                <span class="pill subtle">{snapshot.mode}</span>
            </div>
            <div class="health-row">
                <span class="muted">{t("health.db_status")}</span>
                <span class={status_pill(db_status)}>{db_status.to_string()}</span>
            </div>
            <div class="health-row">
                <span class="muted">{t("health.db_revision")}</span>
                <span class="pill subtle">{db_revision}</span>
            </div>
        </div>
    }
}

fn render_full(t: &impl Fn(&str) -> String, full: Option<FullHealthSnapshot>) -> Html {
    let Some(snapshot) = full else {
        return html! {
            <div class="card">
                <h4>{t("health.full")}</h4>
                <p class="muted">{t("health.full_empty")}</p>
            </div>
        };
    };
    let degraded = if snapshot.degraded.is_empty() {
        html! { <span class="pill subtle">{t("health.degraded_none")}</span> }
    } else {
        html! {
            <div class="pill-group">
                {for snapshot.degraded.iter().map(|name| html! { <span class="pill warn">{name.clone()}</span> })}
            </div>
        }
    };
    html! {
        <div class="card">
            <h4>{t("health.full")}</h4>
            <div class="health-row">
                <span class="muted">{t("health.status")}</span>
                <span class={status_pill(snapshot.status.as_str())}>{snapshot.status.clone()}</span>
            </div>
            <div class="health-row">
                <span class="muted">{t("health.mode")}</span>
                <span class="pill subtle">{snapshot.mode}</span>
            </div>
            <div class="health-row">
                <span class="muted">{t("health.revision")}</span>
                <span class="pill subtle">{snapshot.revision}</span>
            </div>
            <div class="health-row">
                <span class="muted">{t("health.build")}</span>
                <span class="pill subtle">{snapshot.build}</span>
            </div>
            <div class="health-row">
                <span class="muted">{t("health.degraded")}</span>
                {degraded}
            </div>
            <div class="health-block">
                <h5>{t("health.metrics_summary")}</h5>
                {render_metrics(&snapshot.metrics)}
            </div>
            <div class="health-block">
                <h5>{t("health.torrent")}</h5>
                {render_torrent_snapshot(&snapshot.torrent)}
            </div>
        </div>
    }
}

fn render_metrics(metrics: &HealthMetricsSnapshot) -> Html {
    html! {
        <div class="health-grid-compact">
            <div class="health-row">
                <span class="muted">{"Config watch (ms)"}</span>
                <span class="pill subtle">{metrics.config_watch_latency_ms}</span>
            </div>
            <div class="health-row">
                <span class="muted">{"Config apply (ms)"}</span>
                <span class="pill subtle">{metrics.config_apply_latency_ms}</span>
            </div>
            <div class="health-row">
                <span class="muted">{"Config failures"}</span>
                <span class="pill subtle">{metrics.config_update_failures_total}</span>
            </div>
            <div class="health-row">
                <span class="muted">{"Watch slow"}</span>
                <span class="pill subtle">{metrics.config_watch_slow_total}</span>
            </div>
            <div class="health-row">
                <span class="muted">{"Guardrail"}</span>
                <span class="pill subtle">{metrics.guardrail_violations_total}</span>
            </div>
            <div class="health-row">
                <span class="muted">{"Rate limit"}</span>
                <span class="pill subtle">{metrics.rate_limit_throttled_total}</span>
            </div>
        </div>
    }
}

fn render_torrent_snapshot(snapshot: &TorrentHealthSnapshot) -> Html {
    html! {
        <div class="health-grid-compact">
            <div class="health-row">
                <span class="muted">{"Active"}</span>
                <span class="pill subtle">{snapshot.active}</span>
            </div>
            <div class="health-row">
                <span class="muted">{"Queue depth"}</span>
                <span class="pill subtle">{snapshot.queue_depth}</span>
            </div>
        </div>
    }
}

fn status_pill(status: &str) -> Classes {
    let tone = match status {
        "ok" | "healthy" | "active" => "ok",
        "warn" | "warning" | "degraded" => "warn",
        "error" | "failed" => "error",
        _ => "subtle",
    };
    classes!("pill", tone)
}
