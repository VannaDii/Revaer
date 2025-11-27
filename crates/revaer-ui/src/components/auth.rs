use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct AuthPromptProps {
    pub require_key: bool,
    pub allow_anonymous: bool,
    pub on_submit: Callback<Option<String>>,
}

#[function_component(AuthPrompt)]
pub fn auth_prompt(props: &AuthPromptProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
    let api_key = use_state(String::new);
    let error = use_state(|| None as Option<String>);

    let submit = {
        let api_key = api_key.clone();
        let error = error.clone();
        let on_submit = props.on_submit.clone();
        Callback::from(move |_| {
            if api_key.is_empty() && props.require_key && !props.allow_anonymous {
                error.set(Some(t("auth.error_required")));
                return;
            }
            error.set(None);
            let value = if api_key.is_empty() {
                None
            } else {
                Some((*api_key).clone())
            };
            on_submit.emit(value);
        })
    };

    let on_input = {
        let api_key = api_key.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                api_key.set(input.value());
            }
        })
    };

    html! {
        <div class="auth-overlay" role="dialog" aria-modal="true">
            <div class="card">
                <header>
                    <h3>{t("auth.title")}</h3>
                </header>
                <p class="muted">
                    {t("auth.body")}
                </p>
                <label class="stack">
                    <span>{t("auth.label")}</span>
                    <input type="password" placeholder={t("auth.placeholder")} onchange={on_input} />
                </label>
                {if props.allow_anonymous {
                    html! { <p class="muted">{t("auth.allow_anon")}</p> }
                } else { html!{} }}
                {if let Some(err) = &*error {
                    html! { <p class="error-text">{err}</p> }
                } else { html! {} }}
                <div class="actions">
                    <button class="ghost" onclick={{
                        let on_submit = props.on_submit.clone();
                        Callback::from(move |_| on_submit.emit(None))
                    }}>{t("auth.use_anon")}</button>
                    <button class="solid" onclick={submit}>{t("auth.submit")}</button>
                </div>
            </div>
        </div>
    }
}
