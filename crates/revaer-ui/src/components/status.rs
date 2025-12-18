use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::SseState;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct SseProps {
    pub state: SseState,
    pub on_retry: Callback<()>,
    pub network_mode: String,
}

#[function_component(SseOverlay)]
pub(crate) fn sse_overlay(props: &SseProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
    if props.state == SseState::Connected {
        return html! {};
    }

    let (retry_in_secs, last_event, reason) = match props.state {
        SseState::Reconnecting {
            retry_in_secs,
            last_event,
            reason,
        } => (retry_in_secs, last_event, reason),
        SseState::Connected => (0, "", ""),
    };

    let retry_now = {
        let cb = props.on_retry.clone();
        Callback::from(move |_| cb.emit(()))
    };

    html! {
        <div class="sse-overlay" role="status" aria-live="polite">
            <div class="card">
                <header>
                    <strong>{t("sse.title")}</strong>
                    <span class="pill warn">{props.network_mode.clone()}</span>
                </header>
                <p>{t("sse.body")}</p>
                <ul>
                    <li>{format!("{} {}", t("sse.next_retry"), format!("{retry_in_secs}s"))}</li>
                    <li>{format!("{} {}", t("sse.last_event"), last_event)}</li>
                    <li>{format!("{} {}", t("sse.reason"), reason)}</li>
                </ul>
                <div class="actions">
                    <button class="ghost" onclick={retry_now.clone()}>{t("sse.retry")}</button>
                    <button class="solid" onclick={retry_now}>{t("sse.dismiss")}</button>
                </div>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub(crate) struct SseBadgeProps {
    pub state: SseState,
}

#[function_component(SseBadge)]
pub(crate) fn sse_badge(props: &SseBadgeProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
    match props.state {
        SseState::Connected => html! { <span class="pill live">{t("sse.badge")}</span> },
        SseState::Reconnecting { retry_in_secs, .. } => html! {
            <span class="pill warn">{format!("{} ({retry_in_secs}s)", t("sse.badge_reconnecting"))}</span>
        },
    }
}
