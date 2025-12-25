//! Settings page view.
//!
//! # Design
//! - Keep the view stateless and driven by AppStore-provided values.
//! - Emit preference changes via callbacks to avoid touching persistence here.

use crate::components::daisy::Toggle;
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct SettingsPageProps {
    pub bypass_local: bool,
    pub on_toggle_bypass_local: Callback<bool>,
}

#[function_component(SettingsPage)]
pub(crate) fn settings_page(props: &SettingsPageProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str, fallback: &str| bundle.text(key, fallback);
    let on_toggle = {
        let on_toggle = props.on_toggle_bypass_local.clone();
        Callback::from(move |value: bool| on_toggle.emit(value))
    };

    html! {
        <section class="settings-page">
            <div class="panel">
                <div class="panel-head">
                    <div>
                        <p class="eyebrow">{t("settings.title", "Settings")}</p>
                        <h3>{t("settings.auth_title", "Authentication")}</h3>
                        <p class="muted">{t("settings.auth_body", "Control how the UI prompts for credentials.")}</p>
                    </div>
                </div>
                <div class="stacked">
                    <div class="card">
                        <div class="panel-subhead">
                            <strong>{t("settings.bypass_title", "Bypass local auth")}</strong>
                            <span class="pill subtle">{t("settings.bypass_badge", "Default")}</span>
                        </div>
                        <p class="muted">{t("settings.bypass_body", "Prefer API keys in the auth prompt and avoid showing local auth first.")}</p>
                        <Toggle
                            label={Some(AttrValue::from(t("settings.bypass_toggle", "Prefer API key by default")))}
                            checked={props.bypass_local}
                            onchange={on_toggle}
                        />
                    </div>
                </div>
            </div>
        </section>
    }
}
