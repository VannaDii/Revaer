use crate::components::atoms::IconButton;
use crate::components::atoms::icons::IconX;
use crate::components::daisy::DaisySize;
use crate::core::auth::{AuthMode, AuthState, LocalAuth};
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct AuthPromptProps {
    pub allow_anonymous: bool,
    pub default_mode: AuthMode,
    pub on_submit: Callback<AuthState>,
    pub on_dismiss: Callback<()>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(AuthPrompt)]
pub(crate) fn auth_prompt(props: &AuthPromptProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let dismiss_label = bundle.text("auth.dismiss");
    let t = move |key: &str| bundle.text(key);
    let api_key = use_state(String::new);
    let local_user = use_state(String::new);
    let local_pass = use_state(String::new);
    let error = use_state(|| None as Option<String>);
    let mode = use_state(|| props.default_mode);

    let submit = {
        let api_key = api_key.clone();
        let local_user = local_user.clone();
        let local_pass = local_pass.clone();
        let error = error.clone();
        let on_submit = props.on_submit.clone();
        let allow_anonymous = props.allow_anonymous;
        let mode = mode.clone();
        let t = t.clone();
        Callback::from(move |_| match *mode {
            AuthMode::ApiKey => {
                if api_key.is_empty() && !allow_anonymous {
                    error.set(Some(t("auth.error_required")));
                    return;
                }
                error.set(None);
                if api_key.is_empty() {
                    on_submit.emit(AuthState::Anonymous);
                } else {
                    on_submit.emit(AuthState::ApiKey((*api_key).clone()));
                }
            }
            AuthMode::Local => {
                if local_user.is_empty() || local_pass.is_empty() {
                    error.set(Some(t("auth.error_local_required")));
                    return;
                }
                error.set(None);
                on_submit.emit(AuthState::Local(LocalAuth {
                    username: (*local_user).clone(),
                    password: (*local_pass).clone(),
                }));
            }
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
    let on_user_input = {
        let local_user = local_user.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                local_user.set(input.value());
            }
        })
    };
    let on_pass_input = {
        let local_pass = local_pass.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                local_pass.set(input.value());
            }
        })
    };
    let on_dismiss = {
        let on_dismiss = props.on_dismiss.clone();
        Callback::from(move |_| on_dismiss.emit(()))
    };

    html! {
        <div class={classes!("auth-overlay", props.class.clone())} role="dialog" aria-modal="false">
            <div class="card bg-base-100 shadow border border-base-200">
                <div class="card-body gap-4">
                    <div class="flex items-start justify-between gap-3">
                        <div>
                            <h3 class="text-lg font-semibold">{t("auth.title")}</h3>
                            <p class="text-sm text-base-content/60">{t("auth.body")}</p>
                        </div>
                        <IconButton
                            icon={html! { <IconX size={Some(AttrValue::from("4"))} /> }}
                            label={AttrValue::from(dismiss_label.clone())}
                            size={DaisySize::Xs}
                            circle={true}
                            button_type={Some(AttrValue::from("button"))}
                            onclick={on_dismiss.clone()}
                        />
                    </div>
                    <div role="tablist" class="tabs tabs-boxed">
                        <button
                            role="tab"
                            class={classes!("tab", if *mode == AuthMode::ApiKey { "tab-active" } else { "" })}
                            onclick={{
                                let mode = mode.clone();
                                Callback::from(move |_| mode.set(AuthMode::ApiKey))
                            }}>
                            {t("auth.tab.api_key")}
                        </button>
                        <button
                            role="tab"
                            class={classes!("tab", if *mode == AuthMode::Local { "tab-active" } else { "" })}
                            onclick={{
                                let mode = mode.clone();
                                Callback::from(move |_| mode.set(AuthMode::Local))
                            }}>
                            {t("auth.tab.local")}
                        </button>
                    </div>
                    {if *mode == AuthMode::ApiKey {
                        html! {
                            <label class="form-control gap-1">
                                <span class="label-text text-xs">{t("auth.label")}</span>
                                <input
                                    class="input input-bordered w-full"
                                    type="password"
                                    placeholder={t("auth.placeholder")}
                                    oninput={on_input} />
                            </label>
                        }
                    } else { html!{} }}
                    {if *mode == AuthMode::Local {
                        html! {
                            <div class="grid gap-3">
                                <label class="form-control gap-1">
                                    <span class="label-text text-xs">{t("auth.local_user")}</span>
                                    <input
                                        class="input input-bordered w-full"
                                        type="text"
                                        placeholder={t("auth.local_user_placeholder")}
                                        oninput={on_user_input} />
                                </label>
                                <label class="form-control gap-1">
                                    <span class="label-text text-xs">{t("auth.local_pass")}</span>
                                    <input
                                        class="input input-bordered w-full"
                                        type="password"
                                        placeholder={t("auth.local_pass_placeholder")}
                                        oninput={on_pass_input} />
                                </label>
                            </div>
                        }
                    } else { html!{} }}
                    {if props.allow_anonymous {
                        html! { <p class="text-xs text-base-content/60">{t("auth.allow_anon")}</p> }
                    } else { html!{} }}
                    {if let Some(err) = &*error {
                        html! { <p class="text-sm text-error">{err}</p> }
                    } else { html! {} }}
                    <div class="flex justify-end gap-2">
                        <button class="btn btn-ghost btn-sm" type="button" onclick={on_dismiss}>
                            {dismiss_label}
                        </button>
                        {if props.allow_anonymous {
                            html! {
                                <button
                                    class="btn btn-ghost btn-sm"
                                    type="button"
                                    onclick={{
                                        let on_submit = props.on_submit.clone();
                                        Callback::from(move |_| on_submit.emit(AuthState::Anonymous))
                                    }}>
                                    {t("auth.use_anon")}
                                </button>
                            }
                        } else { html!{} }}
                        <button class="btn btn-primary btn-sm" type="button" onclick={submit}>
                            {t("auth.submit")}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    }
}
