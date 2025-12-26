//! Settings page view.
//!
//! # Design
//! - Keep the view stateless and driven by AppStore-provided values.
//! - Emit preference changes via callbacks to avoid touching persistence here.

use crate::components::daisy::{Input, Select, Toggle};
use crate::core::auth::{AuthMode, AuthState, LocalAuth};
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use serde_json::Value;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct SettingsPageProps {
    pub base_url: String,
    pub allow_anonymous: bool,
    pub auth_mode: AuthMode,
    pub auth_state: Option<AuthState>,
    pub bypass_local: bool,
    pub on_toggle_bypass_local: Callback<bool>,
    pub on_save_auth: Callback<AuthState>,
    pub on_test_connection: Callback<()>,
    pub test_busy: bool,
    pub on_server_restart: Callback<()>,
    pub on_server_logs: Callback<()>,
    pub config_snapshot: Option<Value>,
    pub config_error: Option<String>,
    pub config_busy: bool,
    pub on_refresh_config: Callback<()>,
}

#[function_component(SettingsPage)]
pub(crate) fn settings_page(props: &SettingsPageProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = {
        let bundle = bundle.clone();
        move |key: &str, fallback: &str| bundle.text(key, fallback)
    };
    let auth_mode = use_state(|| props.auth_mode);
    let api_key = use_state(String::new);
    let local_user = use_state(String::new);
    let local_pass = use_state(String::new);
    let auth_error = use_state(|| None as Option<String>);
    let auth_mode_options = vec![
        (
            AttrValue::from("api_key"),
            AttrValue::from(t("settings.auth_api", "API key")),
        ),
        (
            AttrValue::from("local"),
            AttrValue::from(t("settings.auth_local", "Local auth")),
        ),
    ];
    let on_toggle = {
        let on_toggle = props.on_toggle_bypass_local.clone();
        Callback::from(move |value: bool| on_toggle.emit(value))
    };
    {
        let auth_state = props.auth_state.clone();
        let auth_mode = auth_mode.clone();
        let api_key = api_key.clone();
        let local_user = local_user.clone();
        let local_pass = local_pass.clone();
        let default_mode = props.auth_mode;
        use_effect_with_deps(
            move |(auth_state, default_mode)| {
                match auth_state {
                    Some(AuthState::ApiKey(value)) => {
                        auth_mode.set(AuthMode::ApiKey);
                        api_key.set(value.clone());
                    }
                    Some(AuthState::Local(auth)) => {
                        auth_mode.set(AuthMode::Local);
                        local_user.set(auth.username.clone());
                        local_pass.set(auth.password.clone());
                    }
                    Some(AuthState::Anonymous) => {
                        auth_mode.set(AuthMode::ApiKey);
                        api_key.set(String::new());
                    }
                    None => {
                        auth_mode.set(*default_mode);
                        api_key.set(String::new());
                        local_user.set(String::new());
                        local_pass.set(String::new());
                    }
                }
                || ()
            },
            (auth_state, default_mode),
        );
    }

    let save_auth = {
        let auth_mode = auth_mode.clone();
        let api_key = api_key.clone();
        let local_user = local_user.clone();
        let local_pass = local_pass.clone();
        let allow_anonymous = props.allow_anonymous;
        let on_save_auth = props.on_save_auth.clone();
        let auth_error = auth_error.clone();
        let t = t.clone();
        Callback::from(move |_| match *auth_mode {
            AuthMode::ApiKey => {
                let value = (*api_key).trim().to_string();
                if value.is_empty() && !allow_anonymous {
                    auth_error.set(Some(t("settings.auth_required", "API key is required.")));
                    return;
                }
                auth_error.set(None);
                let state = if value.is_empty() {
                    AuthState::Anonymous
                } else {
                    AuthState::ApiKey(value)
                };
                on_save_auth.emit(state);
            }
            AuthMode::Local => {
                if local_user.trim().is_empty() || local_pass.trim().is_empty() {
                    auth_error.set(Some(t(
                        "settings.auth_local_required",
                        "Username and password are required.",
                    )));
                    return;
                }
                auth_error.set(None);
                on_save_auth.emit(AuthState::Local(LocalAuth {
                    username: (*local_user).clone(),
                    password: (*local_pass).clone(),
                }));
            }
        })
    };

    let on_auth_mode_change = {
        let auth_mode = auth_mode.clone();
        Callback::from(move |value: AttrValue| {
            let next = if value.as_str() == "local" {
                AuthMode::Local
            } else {
                AuthMode::ApiKey
            };
            auth_mode.set(next);
        })
    };
    let test_label = if props.test_busy {
        t("settings.test_busy", "Testing...")
    } else {
        t("settings.test", "Test connection")
    };
    let on_test_connection = {
        let on_test_connection = props.on_test_connection.clone();
        Callback::from(move |_| on_test_connection.emit(()))
    };
    let on_server_restart = {
        let on_server_restart = props.on_server_restart.clone();
        Callback::from(move |_| on_server_restart.emit(()))
    };
    let on_server_logs = {
        let on_server_logs = props.on_server_logs.clone();
        Callback::from(move |_| on_server_logs.emit(()))
    };
    let on_refresh_config = {
        let on_refresh_config = props.on_refresh_config.clone();
        Callback::from(move |_| on_refresh_config.emit(()))
    };

    let config_sections = build_config_sections(props.config_snapshot.as_ref());

    html! {
        <section class="space-y-6">
            <div>
                <p class="text-lg font-medium">{t("settings.title", "Settings")}</p>
                <p class="text-sm text-base-content/60">
                    {t("settings.subtitle", "Tune connectivity and engine preferences.")}
                </p>
            </div>

            <div class="grid gap-6 xl:grid-cols-2">
                <div class="card bg-base-100 shadow">
                    <div class="card-body gap-4">
                        <div>
                            <h3 class="text-base font-semibold">
                                {t("settings.connection_title", "Connection")}
                            </h3>
                            <p class="text-sm text-base-content/60">
                                {t("settings.connection_body", "Manage API endpoint and credentials.")}
                            </p>
                        </div>
                        <div class="grid gap-3">
                            <div class="form-control w-full">
                                <label class="label pb-1">
                                    <span class="label-text text-xs">{t("settings.base_url", "Base URL")}</span>
                                </label>
                                <Input
                                    value={AttrValue::from(props.base_url.clone())}
                                    disabled={true}
                                    class="w-full"
                                />
                            </div>
                            <div class="form-control w-full">
                                <label class="label pb-1">
                                    <span class="label-text text-xs">{t("settings.auth_mode", "Auth mode")}</span>
                                </label>
                                <Select
                                    value={Some(AttrValue::from(match *auth_mode {
                                        AuthMode::ApiKey => "api_key",
                                        AuthMode::Local => "local",
                                    }))}
                                    options={auth_mode_options}
                                    class="w-full"
                                    onchange={on_auth_mode_change}
                                />
                            </div>
                            {if *auth_mode == AuthMode::ApiKey {
                                html! {
                                    <div class="form-control w-full">
                                        <label class="label pb-1">
                                            <span class="label-text text-xs">{t("settings.api_key", "API key")}</span>
                                        </label>
                                        <Input
                                            value={AttrValue::from((*api_key).clone())}
                                            input_type={Some(AttrValue::from("password"))}
                                            placeholder={Some(AttrValue::from(t("settings.api_key_placeholder", "Paste your API key")))}
                                            class="w-full"
                                            oninput={{
                                                let api_key = api_key.clone();
                                                Callback::from(move |value: String| api_key.set(value))
                                            }}
                                        />
                                        {if props.allow_anonymous {
                                            html! { <p class="text-xs text-base-content/60 mt-1">{t("settings.allow_anon", "Leave blank to use anonymous access on localhost.")}</p> }
                                        } else { html! {} }}
                                    </div>
                                }
                            } else { html! {} }}
                            {if *auth_mode == AuthMode::Local {
                                html! {
                                    <div class="grid gap-3 sm:grid-cols-2">
                                        <div class="form-control w-full">
                                            <label class="label pb-1">
                                                <span class="label-text text-xs">{t("settings.local_user", "Username")}</span>
                                            </label>
                                            <Input
                                                value={AttrValue::from((*local_user).clone())}
                                                placeholder={Some(AttrValue::from(t("settings.local_user_placeholder", "username")))}
                                                class="w-full"
                                                oninput={{
                                                    let local_user = local_user.clone();
                                                    Callback::from(move |value: String| local_user.set(value))
                                                }}
                                            />
                                        </div>
                                        <div class="form-control w-full">
                                            <label class="label pb-1">
                                                <span class="label-text text-xs">{t("settings.local_pass", "Password")}</span>
                                            </label>
                                            <Input
                                                value={AttrValue::from((*local_pass).clone())}
                                                input_type={Some(AttrValue::from("password"))}
                                                placeholder={Some(AttrValue::from(t("settings.local_pass_placeholder", "password")))}
                                                class="w-full"
                                                oninput={{
                                                    let local_pass = local_pass.clone();
                                                    Callback::from(move |value: String| local_pass.set(value))
                                                }}
                                            />
                                        </div>
                                    </div>
                                }
                            } else { html! {} }}
                            <div class="flex flex-wrap items-center gap-3">
                                <Toggle
                                    label={Some(AttrValue::from(t("settings.bypass_toggle", "Prefer API key by default")))}
                                    checked={props.bypass_local}
                                    onchange={on_toggle}
                                />
                                <span class="badge badge-ghost badge-sm">
                                    {t("settings.bypass_badge", "Default")}
                                </span>
                            </div>
                            {if let Some(err) = &*auth_error {
                                html! {
                                    <div role="alert" class="alert alert-error">
                                        <span>{err}</span>
                                    </div>
                                }
                            } else { html! {} }}
                        </div>
                        <div class="flex flex-wrap items-center gap-2">
                            <button class="btn btn-primary btn-sm" onclick={save_auth}>
                                {t("settings.save", "Save")}
                            </button>
                            <button
                                class="btn btn-outline btn-sm"
                                disabled={props.test_busy}
                                onclick={on_test_connection}>
                                {test_label}
                            </button>
                        </div>
                    </div>
                </div>

                <div class="card bg-base-100 shadow">
                    <div class="card-body gap-4">
                        <div>
                            <h3 class="text-base font-semibold">
                                {t("settings.server_title", "Server controls")}
                            </h3>
                            <p class="text-sm text-base-content/60">
                                {t("settings.server_body", "Maintenance actions for the Revaer backend.")}
                            </p>
                        </div>
                        <div class="flex flex-wrap items-center gap-2">
                            <button
                                class="btn btn-outline btn-sm"
                                onclick={on_server_restart}>
                                {t("settings.server_restart", "Restart server")}
                            </button>
                            <button
                                class="btn btn-outline btn-sm"
                                onclick={on_server_logs}>
                                {t("settings.server_logs", "View logs")}
                            </button>
                        </div>
                    </div>
                </div>
            </div>

            <div class="card bg-base-100 shadow">
                <div class="card-body gap-4">
                    <div class="flex flex-wrap items-center justify-between gap-3">
                        <div>
                            <h3 class="text-base font-semibold">
                                {t("settings.engine_title", "Engine settings")}
                            </h3>
                            <p class="text-sm text-base-content/60">
                                {t("settings.engine_body", "Read-only snapshot of server configuration.")}
                            </p>
                        </div>
                        <button
                            class="btn btn-outline btn-sm"
                            disabled={props.config_busy}
                            onclick={on_refresh_config}>
                            {if props.config_busy {
                                t("settings.refreshing", "Refreshing...")
                            } else {
                                t("settings.refresh", "Refresh")
                            }}
                        </button>
                    </div>
                    {if let Some(err) = props.config_error.clone() {
                        html! {
                            <div role="alert" class="alert alert-error">
                                <span>{err}</span>
                            </div>
                        }
                    } else if config_sections.is_empty() {
                        html! {
                            <div class="rounded-box border border-base-200 p-4 text-sm text-base-content/60">
                                {t("settings.engine_empty", "No configuration loaded yet.")}
                            </div>
                        }
                    } else {
                        html! {
                            <div class="grid gap-4 lg:grid-cols-2">
                                {for config_sections.into_iter().map(|section| render_config_section(section))}
                            </div>
                        }
                    }}
                </div>
            </div>
        </section>
    }
}

struct ConfigSection {
    title: String,
    entries: Vec<(String, String)>,
}

fn build_config_sections(snapshot: Option<&Value>) -> Vec<ConfigSection> {
    let Some(snapshot) = snapshot else {
        return Vec::new();
    };
    let mut sections = Vec::new();
    for (title, key) in [
        ("App profile", "app_profile"),
        ("Engine profile", "engine_profile"),
        ("Effective engine profile", "engine_profile_effective"),
        ("Filesystem policy", "fs_policy"),
    ] {
        if let Some(section) = snapshot.get(key) {
            let mut entries = Vec::new();
            flatten_json("", section, &mut entries);
            if !entries.is_empty() {
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                sections.push(ConfigSection {
                    title: title.to_string(),
                    entries,
                });
            }
        }
    }
    sections
}

fn flatten_json(prefix: &str, value: &Value, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let next = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_json(&next, value, out);
            }
        }
        Value::Array(values) => {
            for (idx, value) in values.iter().enumerate() {
                let next = format!("{prefix}[{idx}]");
                flatten_json(&next, value, out);
            }
        }
        _ => {
            let key = if prefix.is_empty() {
                "(value)".to_string()
            } else {
                prefix.to_string()
            };
            out.push((key, value_to_string(value)));
        }
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

fn render_config_section(section: ConfigSection) -> Html {
    html! {
        <div class="card bg-base-200/30 border border-base-200 shadow-sm">
            <div class="card-body gap-3 p-4">
                <h4 class="text-sm font-semibold">{section.title}</h4>
                <div class="overflow-x-auto">
                    <table class="table table-sm">
                        <thead>
                            <tr>
                                <th class="text-xs">{"Key"}</th>
                                <th class="text-xs">{"Value"}</th>
                            </tr>
                        </thead>
                        <tbody>
                            {for section.entries.into_iter().map(|(key, value)| {
                                html! {
                                    <tr>
                                        <td class="text-xs font-mono">{key}</td>
                                        <td class="text-xs">{value}</td>
                                    </tr>
                                }
                            })}
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    }
}
