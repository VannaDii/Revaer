//! First-run setup prompt for the UI.
//!
//! # Design
//! - Keep setup state in the caller; this component is a pure view + event emitter.
//! - Token entry is local state so callers can rotate issued tokens without storing input.
//! - Surface errors verbatim to avoid masking backend setup failures.

use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct SetupPromptProps {
    pub token: Option<String>,
    pub expires_at: Option<String>,
    pub busy: bool,
    pub error: Option<String>,
    pub on_request_token: Callback<()>,
    pub on_complete: Callback<String>,
}

#[function_component(SetupPrompt)]
pub(crate) fn setup_prompt(props: &SetupPromptProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
    let token_input = use_state(String::new);

    {
        let token = props.token.clone();
        let token_input = token_input.clone();
        use_effect_with_deps(
            move |token| {
                if let Some(value) = token {
                    token_input.set(value.clone());
                }
                || ()
            },
            token,
        );
    }

    let on_input = {
        let token_input = token_input.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                token_input.set(input.value());
            }
        })
    };

    let submit = {
        let token_input = token_input.clone();
        let on_complete = props.on_complete.clone();
        Callback::from(move |_| {
            let value = token_input.trim().to_string();
            if !value.is_empty() {
                on_complete.emit(value);
            }
        })
    };

    html! {
        <div class="setup-overlay" role="dialog" aria-modal="true">
            <div class="card">
                <header>
                    <h3>{t("setup.title")}</h3>
                </header>
                <p class="muted">{t("setup.body")}</p>
                <div class="stacked">
                    <label class="stack">
                        <span>{t("setup.token_label")}</span>
                        <input type="text" placeholder={t("setup.token_placeholder")} value={(*token_input).clone()} oninput={on_input} />
                    </label>
                    {if let Some(expires) = props.expires_at.as_ref() {
                        html! { <p class="muted">{format!("{} {expires}", t("setup.expires_prefix"))}</p> }
                    } else { html!{} }}
                </div>
                {if let Some(err) = props.error.as_ref() {
                    html! { <p class="error-text">{err.clone()}</p> }
                } else { html! {} }}
                <div class="actions">
                    <button class="ghost" onclick={{
                        let on_request = props.on_request_token.clone();
                        Callback::from(move |_| on_request.emit(()))
                    }} disabled={props.busy}>{t("setup.issue_token")}</button>
                    <button class="solid" onclick={submit} disabled={props.busy}>{t("setup.complete")}</button>
                </div>
            </div>
        </div>
    }
}
