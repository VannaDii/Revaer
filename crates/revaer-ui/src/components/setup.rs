//! First-run setup prompt for the UI.
//!
//! # Design
//! - Keep setup state in the caller; this component is a pure view + event emitter.
//! - Token entry is local state so callers can rotate issued tokens without storing input.
//! - Surface errors verbatim to avoid masking backend setup failures.

use crate::components::daisy::Select;
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use yew::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SetupAuthMode {
    ApiKey,
    NoAuth,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SetupCompleteInput {
    pub token: String,
    pub auth_mode: SetupAuthMode,
}

#[derive(Properties, PartialEq)]
pub(crate) struct SetupPromptProps {
    pub token: Option<String>,
    pub expires_at: Option<String>,
    pub busy: bool,
    pub error: Option<String>,
    pub allow_no_auth: bool,
    pub auth_mode: SetupAuthMode,
    pub on_auth_mode_change: Callback<SetupAuthMode>,
    pub on_request_token: Callback<()>,
    pub on_complete: Callback<SetupCompleteInput>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(SetupPrompt)]
pub(crate) fn setup_prompt(props: &SetupPromptProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key);
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
        let auth_mode = props.auth_mode;
        Callback::from(move |_| {
            let value = token_input.trim().to_string();
            if !value.is_empty() {
                on_complete.emit(SetupCompleteInput {
                    token: value,
                    auth_mode,
                });
            }
        })
    };

    let on_auth_change = {
        let on_auth_mode_change = props.on_auth_mode_change.clone();
        Callback::from(move |value: AttrValue| {
            let next = match value.as_str() {
                "none" => SetupAuthMode::NoAuth,
                _ => SetupAuthMode::ApiKey,
            };
            on_auth_mode_change.emit(next);
        })
    };

    html! {
        <div class={classes!("setup-overlay", props.class.clone())} role="dialog" aria-modal="false">
            <div class="card bg-base-100 shadow border border-base-200">
                <div class="card-body gap-4">
                    <div>
                        <h3 class="text-lg font-semibold">{t("setup.title")}</h3>
                        <p class="text-sm text-base-content/60">{t("setup.body")}</p>
                    </div>
                    <div class="grid gap-3">
                        <label class="form-control gap-1">
                            <span class="label-text text-xs">{t("setup.token_label")}</span>
                            <input
                                class="input input-bordered w-full"
                                type="text"
                                placeholder={t("setup.token_placeholder")}
                                value={(*token_input).clone()}
                                oninput={on_input} />
                        </label>
                        {if let Some(expires) = props.expires_at.as_ref() {
                            html! { <p class="text-xs text-base-content/60">{format!("{} {expires}", t("setup.expires_prefix"))}</p> }
                        } else { html!{} }}
                        {if props.allow_no_auth {
                            html! {
                                <label class="form-control gap-1">
                                    <span class="label-text text-xs">{t("setup.auth_mode_label")}</span>
                                    <Select
                                        class="w-full"
                                        value={Some(AttrValue::from(match props.auth_mode {
                                            SetupAuthMode::ApiKey => "api_key",
                                            SetupAuthMode::NoAuth => "none",
                                        }))}
                                        options={vec![
                                            (AttrValue::from("api_key"), AttrValue::from(t("setup.auth_mode_api_key"))),
                                            (AttrValue::from("none"), AttrValue::from(t("setup.auth_mode_none"))),
                                        ]}
                                        onchange={on_auth_change}
                                    />
                                    <p class="text-xs text-base-content/60">{t("setup.auth_mode_hint")}</p>
                                </label>
                            }
                        } else { html!{} }}
                    </div>
                    {if let Some(err) = props.error.as_ref() {
                        html! { <p class="text-sm text-error">{err.clone()}</p> }
                    } else { html! {} }}
                    <div class="flex justify-end gap-2">
                        <button
                            class="btn btn-ghost btn-sm"
                            onclick={{
                                let on_request = props.on_request_token.clone();
                                Callback::from(move |_| on_request.emit(()))
                            }}
                            disabled={props.busy}>
                            {t("setup.issue_token")}
                        </button>
                        <button
                            class="btn btn-primary btn-sm"
                            onclick={submit}
                            disabled={props.busy}>
                            {t("setup.complete")}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    }
}
