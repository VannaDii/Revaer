use yew::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SseState {
    Connected,
    Reconnecting {
        retry_in_secs: u8,
        last_event: &'static str,
        reason: &'static str,
    },
}

#[derive(Properties, PartialEq)]
pub struct SseProps {
    pub state: SseState,
    pub on_retry: Callback<()>,
    pub network_mode: &'static str,
}

#[function_component(SseOverlay)]
pub fn sse_overlay(props: &SseProps) -> Html {
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
                    <strong>{"SSE disconnected"}</strong>
                    <span class="pill warn">{props.network_mode}</span>
                </header>
                <p>{"Reconnecting with exponential backoff and jitter."}</p>
                <ul>
                    <li>{format!("Next retry in {}s", retry_in_secs)}</li>
                    <li>{format!("Last event {}", last_event)}</li>
                    <li>{format!("Reason: {}", reason)}</li>
                </ul>
                <div class="actions">
                    <button class="ghost" onclick={retry_now}>{"Retry now"}</button>
                    <button class="solid" onclick={retry_now}>{"Dismiss overlay"}</button>
                </div>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct SseBadgeProps {
    pub state: SseState,
}

#[function_component(SseBadge)]
pub fn sse_badge(props: &SseBadgeProps) -> Html {
    match props.state {
        SseState::Connected => html! { <span class="pill live">{"SSE"}</span> },
        SseState::Reconnecting { retry_in_secs, .. } => html! {
            <span class="pill warn">{format!("SSE reconnecting ({retry_in_secs}s)")}</span>
        },
    }
}
