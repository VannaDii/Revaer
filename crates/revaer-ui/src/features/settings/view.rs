//! Settings page view.
//!
//! # Design
//! - Keep the view stateless and driven by AppStore-provided values.
//! - Emit preference changes via callbacks to avoid touching persistence here.
//! - Maintain a local draft for config edits to keep the UI responsive.

use crate::app::api::ApiCtx;
use crate::components::daisy::{Input, Modal, Select, Toggle};
use crate::core::auth::{AuthMode, AuthState, LocalAuth};
use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::{FsEntry, FsEntryKind};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashSet};
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
    pub config_save_busy: bool,
    pub on_refresh_config: Callback<()>,
    pub on_apply_settings: Callback<Value>,
    pub on_copy_value: Callback<String>,
    pub on_error_toast: Callback<String>,
}

#[derive(Properties, PartialEq)]
struct SettingsConnectionProps {
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
}

#[derive(Properties, PartialEq)]
struct SettingsConfigProps {
    pub active_tab: SettingsTab,
    pub config_snapshot: Option<Value>,
    pub config_error: Option<String>,
    pub config_busy: bool,
    pub config_save_busy: bool,
    pub on_refresh_config: Callback<()>,
    pub on_apply_settings: Callback<Value>,
    pub on_copy_value: Callback<String>,
    pub on_error_toast: Callback<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Connection,
    Downloads,
    Seeding,
    Network,
    Storage,
    Labels,
    System,
}

impl SettingsTab {
    const fn all() -> [Self; 7] {
        [
            Self::Connection,
            Self::Downloads,
            Self::Seeding,
            Self::Network,
            Self::Storage,
            Self::Labels,
            Self::System,
        ]
    }

    const fn label_key(self) -> &'static str {
        match self {
            Self::Connection => "settings.tabs.connection",
            Self::Downloads => "settings.tabs.downloads",
            Self::Seeding => "settings.tabs.seeding",
            Self::Network => "settings.tabs.network",
            Self::Storage => "settings.tabs.storage",
            Self::Labels => "settings.tabs.labels",
            Self::System => "settings.tabs.system",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsSection {
    AppProfile,
    EngineProfile,
    FsPolicy,
}

impl SettingsSection {
    const fn all() -> [Self; 3] {
        [Self::AppProfile, Self::EngineProfile, Self::FsPolicy]
    }

    const fn key(self) -> &'static str {
        match self {
            Self::AppProfile => "app_profile",
            Self::EngineProfile => "engine_profile",
            Self::FsPolicy => "fs_policy",
        }
    }
}

#[derive(Clone, PartialEq)]
struct FieldDraft {
    value: Value,
    raw: String,
    error: Option<String>,
}

#[derive(Clone, PartialEq, Default)]
struct SettingsDraft {
    fields: BTreeMap<String, FieldDraft>,
}

#[derive(Clone, PartialEq, Eq)]
struct SettingsField {
    section: SettingsSection,
    key: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LabelKind {
    Category,
    Tag,
}

impl LabelKind {
    const fn key(self) -> &'static str {
        match self {
            Self::Category => "category",
            Self::Tag => "tag",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "category" => Some(Self::Category),
            "tag" => Some(Self::Tag),
            _ => None,
        }
    }

    const fn label_key(self) -> &'static str {
        match self {
            Self::Category => "settings.labels.categories",
            Self::Tag => "settings.labels.tags",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
enum PathPickerTarget {
    Single(String),
    AllowPaths(String),
    LabelPolicy { kind: LabelKind, name: String },
}

#[derive(Clone, PartialEq)]
struct PathBrowserState {
    open: bool,
    target: Option<PathPickerTarget>,
    path: String,
    input: String,
    entries: Vec<FsEntry>,
    parent: Option<String>,
    busy: bool,
    error: Option<String>,
}

impl Default for PathBrowserState {
    fn default() -> Self {
        Self {
            open: false,
            target: None,
            path: String::new(),
            input: String::new(),
            entries: Vec::new(),
            parent: None,
            busy: false,
            error: None,
        }
    }
}

struct PathBrowserCallbacks {
    on_open: Callback<PathPickerTarget>,
    on_close: Callback<()>,
    on_confirm: Callback<()>,
    on_input: Callback<String>,
    on_navigate: Callback<String>,
    on_parent: Callback<()>,
    on_go: Callback<()>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NumericKind {
    Integer,
    Float,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NumericError {
    Integer,
    Float,
}

#[derive(Clone, PartialEq, Eq)]
struct SelectOptions {
    allow_empty: bool,
    options: Vec<(String, &'static str)>,
}

#[derive(Clone, PartialEq, Eq)]
struct StringListOptions {
    placeholder_key: &'static str,
    add_label_key: &'static str,
    empty_label_key: &'static str,
}

#[derive(Clone, PartialEq, Eq)]
enum FieldControl {
    Toggle,
    Select(SelectOptions),
    Number(NumericKind),
    Text,
    Path,
    PathList,
    StringList(StringListOptions),
    Telemetry,
    LabelPolicies,
    AltSpeed,
    Tracker,
    IpFilter,
    PeerClasses,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
struct SettingsStatus {
    dirty_count: usize,
    has_errors: bool,
}

#[function_component(SettingsPage)]
pub(crate) fn settings_page(props: &SettingsPageProps) -> Html {
    let active_tab = use_state(|| SettingsTab::Connection);
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = {
        let bundle = bundle.clone();
        move |key: &str| bundle.text(key)
    };
    let config_props = SettingsConfigProps {
        active_tab: *active_tab,
        config_snapshot: props.config_snapshot.clone(),
        config_error: props.config_error.clone(),
        config_busy: props.config_busy,
        config_save_busy: props.config_save_busy,
        on_refresh_config: props.on_refresh_config.clone(),
        on_apply_settings: props.on_apply_settings.clone(),
        on_copy_value: props.on_copy_value.clone(),
        on_error_toast: props.on_error_toast.clone(),
    };
    let connection_props = SettingsConnectionProps {
        base_url: props.base_url.clone(),
        allow_anonymous: props.allow_anonymous,
        auth_mode: props.auth_mode,
        auth_state: props.auth_state.clone(),
        bypass_local: props.bypass_local,
        on_toggle_bypass_local: props.on_toggle_bypass_local.clone(),
        on_save_auth: props.on_save_auth.clone(),
        on_test_connection: props.on_test_connection.clone(),
        test_busy: props.test_busy,
        on_server_restart: props.on_server_restart.clone(),
        on_server_logs: props.on_server_logs.clone(),
    };

    html! {
        <section class="space-y-6">
            <div>
                <p class="text-lg font-medium">{t("settings.title")}</p>
                <p class="text-sm text-base-content/60">
                    {t("settings.subtitle")}
                </p>
            </div>
            <div role="tablist" class="tabs tabs-bordered tabs-sm">
                {for SettingsTab::all().into_iter().map(|tab| {
                    let active_tab = active_tab.clone();
                    let is_active = *active_tab == tab;
                    let label = t(tab.label_key());
                    let class = classes!("tab", is_active.then_some("tab-active"));
                    html! {
                        <button
                            role="tab"
                            class={class}
                            aria-selected={is_active.then_some(AttrValue::from("true"))}
                            onclick={Callback::from(move |_| active_tab.set(tab))}
                        >
                            {label}
                        </button>
                    }
                })}
            </div>
            {match *active_tab {
                SettingsTab::Connection => html! {
                    <SettingsConnectionTab ..connection_props />
                },
                _ => html! {
                    <SettingsConfigTabs ..config_props />
                },
            }}
        </section>
    }
}

#[function_component(SettingsConnectionTab)]
fn settings_connection_tab(props: &SettingsConnectionProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let auth_mode = use_state(|| props.auth_mode);
    let api_key = use_state(String::new);
    let local_user = use_state(String::new);
    let local_pass = use_state(String::new);
    let auth_error = use_state(|| None as Option<String>);

    let auth_mode_options = build_auth_mode_options(&bundle);
    let on_toggle = toggle_callback(props.on_toggle_bypass_local.clone());
    {
        let auth_state = props.auth_state.clone();
        let auth_mode = auth_mode.clone();
        let api_key = api_key.clone();
        let local_user = local_user.clone();
        let local_pass = local_pass.clone();
        let default_mode = props.auth_mode;
        use_effect_with((auth_state, default_mode), move |deps| {
            let (auth_state, default_mode) = deps;
            apply_auth_state(
                auth_state.clone(),
                *default_mode,
                &auth_mode,
                &api_key,
                &local_user,
                &local_pass,
            );
            || ()
        });
    }

    let save_auth = build_save_auth_callback(
        &bundle,
        auth_mode.clone(),
        api_key.clone(),
        local_user.clone(),
        local_pass.clone(),
        auth_error.clone(),
        props.allow_anonymous,
        props.on_save_auth.clone(),
    );
    let on_auth_mode_change = auth_mode_change_callback(auth_mode.clone());
    let test_label = test_button_label(&bundle, props.test_busy);
    let on_test_connection = emit_callback(props.on_test_connection.clone());
    let on_server_restart = emit_callback(props.on_server_restart.clone());
    let on_server_logs = emit_callback(props.on_server_logs.clone());

    html! {
        <div class="grid gap-6 xl:grid-cols-2">
            {render_connection_card(
                &bundle,
                props,
                *auth_mode,
                api_key.clone(),
                local_user.clone(),
                local_pass.clone(),
                auth_error.clone(),
                auth_mode_options,
                on_auth_mode_change,
                on_toggle,
                save_auth,
                on_test_connection,
                test_label,
            )}
            {render_server_card(
                &bundle,
                on_server_restart,
                on_server_logs,
            )}
        </div>
    }
}

fn build_auth_mode_options(bundle: &TranslationBundle) -> Vec<(AttrValue, AttrValue)> {
    vec![
        (
            AttrValue::from("api_key"),
            AttrValue::from(bundle.text("settings.auth_api")),
        ),
        (
            AttrValue::from("local"),
            AttrValue::from(bundle.text("settings.auth_local")),
        ),
    ]
}

fn toggle_callback(callback: Callback<bool>) -> Callback<bool> {
    Callback::from(move |value: bool| callback.emit(value))
}

fn emit_callback(callback: Callback<()>) -> Callback<MouseEvent> {
    Callback::from(move |_| callback.emit(()))
}

fn apply_auth_state(
    auth_state: Option<AuthState>,
    default_mode: AuthMode,
    auth_mode: &UseStateHandle<AuthMode>,
    api_key: &UseStateHandle<String>,
    local_user: &UseStateHandle<String>,
    local_pass: &UseStateHandle<String>,
) {
    match auth_state {
        Some(AuthState::ApiKey(value)) => {
            auth_mode.set(AuthMode::ApiKey);
            api_key.set(value);
        }
        Some(AuthState::Local(auth)) => {
            auth_mode.set(AuthMode::Local);
            local_user.set(auth.username);
            local_pass.set(auth.password);
        }
        Some(AuthState::Anonymous) => {
            auth_mode.set(AuthMode::ApiKey);
            api_key.set(String::new());
        }
        None => {
            auth_mode.set(default_mode);
            api_key.set(String::new());
            local_user.set(String::new());
            local_pass.set(String::new());
        }
    }
}

fn build_save_auth_callback(
    bundle: &TranslationBundle,
    auth_mode: UseStateHandle<AuthMode>,
    api_key: UseStateHandle<String>,
    local_user: UseStateHandle<String>,
    local_pass: UseStateHandle<String>,
    auth_error: UseStateHandle<Option<String>>,
    allow_anonymous: bool,
    on_save_auth: Callback<AuthState>,
) -> Callback<MouseEvent> {
    let auth_required = bundle.text("settings.auth_required");
    let auth_local_required = bundle.text("settings.auth_local_required");
    Callback::from(move |_| match *auth_mode {
        AuthMode::ApiKey => {
            let value = (*api_key).trim().to_string();
            if value.is_empty() && !allow_anonymous {
                auth_error.set(Some(auth_required.clone()));
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
                auth_error.set(Some(auth_local_required.clone()));
                return;
            }
            auth_error.set(None);
            on_save_auth.emit(AuthState::Local(LocalAuth {
                username: (*local_user).clone(),
                password: (*local_pass).clone(),
            }));
        }
    })
}

fn auth_mode_change_callback(auth_mode: UseStateHandle<AuthMode>) -> Callback<AttrValue> {
    Callback::from(move |value: AttrValue| {
        let next = if value.as_str() == "local" {
            AuthMode::Local
        } else {
            AuthMode::ApiKey
        };
        auth_mode.set(next);
    })
}

fn test_button_label(bundle: &TranslationBundle, busy: bool) -> String {
    if busy {
        bundle.text("settings.test_busy")
    } else {
        bundle.text("settings.test")
    }
}

fn render_connection_card(
    bundle: &TranslationBundle,
    props: &SettingsConnectionProps,
    auth_mode: AuthMode,
    api_key: UseStateHandle<String>,
    local_user: UseStateHandle<String>,
    local_pass: UseStateHandle<String>,
    auth_error: UseStateHandle<Option<String>>,
    auth_mode_options: Vec<(AttrValue, AttrValue)>,
    on_auth_mode_change: Callback<AttrValue>,
    on_toggle: Callback<bool>,
    on_save: Callback<MouseEvent>,
    on_test: Callback<MouseEvent>,
    test_label: String,
) -> Html {
    html! {
        <div class="card bg-base-100 shadow">
            <div class="card-body gap-4">
                <div>
                    <h3 class="text-base font-semibold">
                        {bundle.text("settings.connection_title")}
                    </h3>
                    <p class="text-sm text-base-content/60">
                        {bundle.text("settings.connection_body")}
                    </p>
                </div>
                <div class="grid gap-3">
                    <div class="form-control w-full">
                        <label class="label pb-1">
                            <span class="label-text text-xs">{bundle.text("settings.base_url")}</span>
                        </label>
                        <Input
                            value={AttrValue::from(props.base_url.clone())}
                            disabled={true}
                            class="w-full"
                        />
                    </div>
                    <div class="form-control w-full">
                        <label class="label pb-1">
                            <span class="label-text text-xs">{bundle.text("settings.auth_mode")}</span>
                        </label>
                        <Select
                            value={Some(AttrValue::from(match auth_mode {
                                AuthMode::ApiKey => "api_key",
                                AuthMode::Local => "local",
                            }))}
                            options={auth_mode_options}
                            class="w-full"
                            onchange={on_auth_mode_change}
                        />
                    </div>
                    {render_auth_mode_fields(
                        bundle,
                        auth_mode,
                        api_key.clone(),
                        local_user.clone(),
                        local_pass.clone(),
                        props.allow_anonymous,
                    )}
                    <div class="flex flex-wrap items-center gap-3">
                        <Toggle
                            label={Some(AttrValue::from(bundle.text("settings.bypass_toggle")))}
                            checked={props.bypass_local}
                            onchange={on_toggle}
                        />
                        <span class="badge badge-ghost badge-sm">
                            {bundle.text("settings.bypass_badge")}
                        </span>
                    </div>
                    {render_auth_error(&auth_error)}
                </div>
                <div class="flex flex-wrap items-center gap-2">
                    <button class="btn btn-primary btn-sm" onclick={on_save}>
                        {bundle.text("settings.save")}
                    </button>
                    <button
                        class="btn btn-outline btn-sm"
                        disabled={props.test_busy}
                        onclick={on_test}>
                        {test_label}
                    </button>
                </div>
            </div>
        </div>
    }
}

fn render_auth_mode_fields(
    bundle: &TranslationBundle,
    auth_mode: AuthMode,
    api_key: UseStateHandle<String>,
    local_user: UseStateHandle<String>,
    local_pass: UseStateHandle<String>,
    allow_anonymous: bool,
) -> Html {
    match auth_mode {
        AuthMode::ApiKey => {
            let oninput = {
                let api_key = api_key.clone();
                Callback::from(move |value: String| api_key.set(value))
            };
            html! {
                <div class="form-control w-full">
                    <label class="label pb-1">
                        <span class="label-text text-xs">{bundle.text("settings.api_key")}</span>
                    </label>
                    <Input
                        value={AttrValue::from((*api_key).clone())}
                        input_type={Some(AttrValue::from("password"))}
                        placeholder={Some(AttrValue::from(bundle.text("settings.api_key_placeholder")))}
                        class="w-full"
                        oninput={oninput}
                    />
                    {if allow_anonymous {
                        html! { <p class="text-xs text-base-content/60 mt-1">{bundle.text("settings.allow_anon")}</p> }
                    } else { html! {} }}
                </div>
            }
        }
        AuthMode::Local => {
            let on_user = {
                let local_user = local_user.clone();
                Callback::from(move |value: String| local_user.set(value))
            };
            let on_pass = {
                let local_pass = local_pass.clone();
                Callback::from(move |value: String| local_pass.set(value))
            };
            html! {
                <div class="grid gap-3 sm:grid-cols-2">
                    <div class="form-control w-full">
                        <label class="label pb-1">
                            <span class="label-text text-xs">{bundle.text("settings.local_user")}</span>
                        </label>
                        <Input
                            value={AttrValue::from((*local_user).clone())}
                            placeholder={Some(AttrValue::from(bundle.text("settings.local_user_placeholder")))}
                            class="w-full"
                            oninput={on_user}
                        />
                    </div>
                    <div class="form-control w-full">
                        <label class="label pb-1">
                            <span class="label-text text-xs">{bundle.text("settings.local_pass")}</span>
                        </label>
                        <Input
                            value={AttrValue::from((*local_pass).clone())}
                            input_type={Some(AttrValue::from("password"))}
                            placeholder={Some(AttrValue::from(bundle.text("settings.local_pass_placeholder")))}
                            class="w-full"
                            oninput={on_pass}
                        />
                    </div>
                </div>
            }
        }
    }
}

fn render_auth_error(auth_error: &UseStateHandle<Option<String>>) -> Html {
    if let Some(err) = &**auth_error {
        html! {
            <div role="alert" class="alert alert-error">
                <span>{err.clone()}</span>
            </div>
        }
    } else {
        html! {}
    }
}

fn render_server_card(
    bundle: &TranslationBundle,
    on_server_restart: Callback<MouseEvent>,
    on_server_logs: Callback<MouseEvent>,
) -> Html {
    html! {
        <div class="card bg-base-100 shadow">
            <div class="card-body gap-4">
                <div>
                    <h3 class="text-base font-semibold">
                        {bundle.text("settings.server_title")}
                    </h3>
                    <p class="text-sm text-base-content/60">
                        {bundle.text("settings.server_body")}
                    </p>
                </div>
                <div class="flex flex-wrap items-center gap-2">
                    <button
                        class="btn btn-outline btn-sm"
                        onclick={on_server_restart}>
                        {bundle.text("settings.server_restart")}
                    </button>
                    <button
                        class="btn btn-outline btn-sm"
                        onclick={on_server_logs}>
                        {bundle.text("settings.server_logs")}
                    </button>
                </div>
            </div>
        </div>
    }
}

struct ConfigTabFields {
    app_groups: AppGroups,
    engine_groups: EngineGroups,
    fs_fields: Vec<SettingsField>,
}

impl ConfigTabFields {
    fn from_snapshot(snapshot: Option<&Value>) -> Self {
        let app_fields = collect_section_fields(snapshot, SettingsSection::AppProfile);
        let engine_fields = collect_section_fields(snapshot, SettingsSection::EngineProfile);
        let fs_fields = collect_section_fields(snapshot, SettingsSection::FsPolicy);
        let engine_groups = split_engine_fields(engine_fields);
        let app_groups = split_app_fields(app_fields);
        Self {
            app_groups,
            engine_groups,
            fs_fields,
        }
    }
}

#[function_component(SettingsConfigTabs)]
fn settings_config_tabs(props: &SettingsConfigProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let api_ctx = use_context::<ApiCtx>();
    let draft = use_state(SettingsDraft::default);
    let path_browser = use_state(PathBrowserState::default);
    {
        let draft = draft.clone();
        let snapshot = props.config_snapshot.clone();
        use_effect_with(snapshot, move |snapshot| {
            let next = snapshot
                .as_ref()
                .map(build_settings_draft)
                .unwrap_or_default();
            draft.set(next);
            || ()
        });
    }

    let path_callbacks = build_path_browser_callbacks(
        draft.clone(),
        path_browser.clone(),
        api_ctx,
        props.on_error_toast.clone(),
    );
    let config_snapshot = props.config_snapshot.as_ref();
    let immutable_keys = immutable_key_set(config_snapshot);
    let fields = ConfigTabFields::from_snapshot(config_snapshot);
    let config_error = render_config_error(props.config_error.clone());
    let status = settings_status(config_snapshot, &draft, &immutable_keys);
    let save_disabled = props.config_save_busy || status.has_errors || status.dirty_count == 0;
    let on_save = {
        let snapshot = props.config_snapshot.clone();
        let draft = draft.clone();
        let immutable_keys = immutable_keys.clone();
        let on_apply = props.on_apply_settings.clone();
        Callback::from(move |_| {
            let Some(snapshot) = snapshot.as_ref() else {
                return;
            };
            if let Some(payload) = build_changeset_from_snapshot(snapshot, &draft, &immutable_keys)
            {
                on_apply.emit(payload);
            }
        })
    };
    let save_bar = render_save_bar(
        status,
        save_disabled,
        props.config_save_busy,
        on_save,
        &bundle,
    );
    let tab_body = build_config_tab_body(
        props.active_tab,
        &fields,
        config_snapshot,
        draft.clone(),
        &immutable_keys,
        props,
        &bundle,
        path_callbacks.on_open.clone(),
    );

    html! {
        <div class="space-y-4">
            {config_error}
            {save_bar}
            {tab_body}
            {render_path_browser(&bundle, &path_browser, path_callbacks)}
        </div>
    }
}

fn build_path_browser_callbacks(
    draft: UseStateHandle<SettingsDraft>,
    path_browser: UseStateHandle<PathBrowserState>,
    api_ctx: Option<ApiCtx>,
    on_error_toast: Callback<String>,
) -> PathBrowserCallbacks {
    PathBrowserCallbacks {
        on_open: path_browser_open_callback(
            draft.clone(),
            path_browser.clone(),
            api_ctx.clone(),
            on_error_toast.clone(),
        ),
        on_close: path_browser_close_callback(path_browser.clone()),
        on_confirm: path_browser_confirm_callback(draft, path_browser.clone()),
        on_input: path_browser_input_callback(path_browser.clone()),
        on_navigate: path_browser_navigate_callback(
            path_browser.clone(),
            api_ctx.clone(),
            on_error_toast.clone(),
        ),
        on_parent: path_browser_parent_callback(
            path_browser.clone(),
            api_ctx.clone(),
            on_error_toast.clone(),
        ),
        on_go: path_browser_go_callback(path_browser, api_ctx, on_error_toast),
    }
}

fn path_browser_open_callback(
    draft: UseStateHandle<SettingsDraft>,
    path_browser: UseStateHandle<PathBrowserState>,
    api_ctx: Option<ApiCtx>,
    on_error_toast: Callback<String>,
) -> Callback<PathPickerTarget> {
    Callback::from(move |target: PathPickerTarget| {
        let initial = match &target {
            PathPickerTarget::Single(field_key) => draft
                .fields
                .get(field_key)
                .map(|field| field.raw.clone())
                .unwrap_or_default(),
            PathPickerTarget::AllowPaths(_) => String::new(),
            PathPickerTarget::LabelPolicy { kind, name } => {
                label_policy_download_dir(&draft, *kind, name).unwrap_or_default()
            }
        };
        let path = if initial.trim().is_empty() {
            "/".to_string()
        } else {
            initial
        };
        update_browser_state(&path_browser, |state| {
            state.open = true;
            state.target = Some(target);
            state.path = path.clone();
            state.input = path.clone();
            state.entries.clear();
            state.parent = None;
            state.busy = true;
            state.error = None;
        });
        fetch_browser_entries(
            api_ctx.clone(),
            path_browser.clone(),
            path,
            on_error_toast.clone(),
        );
    })
}

fn path_browser_close_callback(path_browser: UseStateHandle<PathBrowserState>) -> Callback<()> {
    Callback::from(move |_| {
        path_browser.set(PathBrowserState::default());
    })
}

fn path_browser_confirm_callback(
    draft: UseStateHandle<SettingsDraft>,
    path_browser: UseStateHandle<PathBrowserState>,
) -> Callback<()> {
    Callback::from(move |_| {
        let browser = (*path_browser).clone();
        let value = browser.input.trim().to_string();
        let Some(target) = browser.target else {
            return;
        };
        if value.is_empty() {
            return;
        }
        match target {
            PathPickerTarget::Single(field_key) => {
                update_field(&draft, &field_key, |field| {
                    field.value = Value::String(value.clone());
                    field.raw = value;
                    field.error = None;
                });
            }
            PathPickerTarget::AllowPaths(field_key) => {
                update_field(&draft, &field_key, |field| {
                    let mut entries = value_array_as_strings(&field.value);
                    if !entries.iter().any(|item| item == &value) {
                        entries.push(value.clone());
                        field.value = Value::Array(
                            entries.into_iter().map(Value::String).collect::<Vec<_>>(),
                        );
                        field.raw = value_to_raw(&field.value);
                        field.error = None;
                    }
                });
            }
            PathPickerTarget::LabelPolicy { kind, name } => {
                update_label_policy_download_dir(&draft, kind, &name, value);
            }
        }
        path_browser.set(PathBrowserState::default());
    })
}

fn path_browser_input_callback(path_browser: UseStateHandle<PathBrowserState>) -> Callback<String> {
    Callback::from(move |value: String| {
        update_browser_state(&path_browser, |state| {
            state.input = value;
        });
    })
}

fn path_browser_navigate_callback(
    path_browser: UseStateHandle<PathBrowserState>,
    api_ctx: Option<ApiCtx>,
    on_error_toast: Callback<String>,
) -> Callback<String> {
    Callback::from(move |path: String| {
        update_browser_state(&path_browser, |state| {
            state.busy = true;
            state.error = None;
            state.path = path.clone();
            state.input = path.clone();
            state.entries.clear();
            state.parent = None;
        });
        fetch_browser_entries(
            api_ctx.clone(),
            path_browser.clone(),
            path,
            on_error_toast.clone(),
        );
    })
}

fn path_browser_parent_callback(
    path_browser: UseStateHandle<PathBrowserState>,
    api_ctx: Option<ApiCtx>,
    on_error_toast: Callback<String>,
) -> Callback<()> {
    Callback::from(move |_| {
        let Some(parent) = (*path_browser).parent.clone() else {
            return;
        };
        update_browser_state(&path_browser, |state| {
            state.busy = true;
            state.error = None;
            state.path = parent.clone();
            state.input = parent.clone();
            state.entries.clear();
            state.parent = None;
        });
        fetch_browser_entries(
            api_ctx.clone(),
            path_browser.clone(),
            parent,
            on_error_toast.clone(),
        );
    })
}

fn path_browser_go_callback(
    path_browser: UseStateHandle<PathBrowserState>,
    api_ctx: Option<ApiCtx>,
    on_error_toast: Callback<String>,
) -> Callback<()> {
    Callback::from(move |_| {
        let path = (*path_browser).input.trim().to_string();
        if path.is_empty() {
            return;
        }
        update_browser_state(&path_browser, |state| {
            state.busy = true;
            state.error = None;
            state.path = path.clone();
            state.input = path.clone();
            state.entries.clear();
            state.parent = None;
        });
        fetch_browser_entries(
            api_ctx.clone(),
            path_browser.clone(),
            path,
            on_error_toast.clone(),
        );
    })
}

fn fetch_browser_entries(
    api_ctx: Option<ApiCtx>,
    path_browser: UseStateHandle<PathBrowserState>,
    path: String,
    on_error_toast: Callback<String>,
) {
    let Some(api_ctx) = api_ctx else {
        let message = "Missing API client.".to_string();
        update_browser_state(&path_browser, |state| {
            state.busy = false;
            state.error = Some(message.clone());
        });
        on_error_toast.emit(message);
        return;
    };

    let client = api_ctx.client.clone();
    let on_error_toast = on_error_toast.clone();
    yew::platform::spawn_local(async move {
        match client.browse_filesystem(&path).await {
            Ok(response) => update_browser_state(&path_browser, |state| {
                state.busy = false;
                state.error = None;
                state.path = response.path.clone();
                state.input = response.path;
                state.entries = response.entries;
                state.parent = response.parent;
            }),
            Err(err) => {
                let detail = err
                    .detail
                    .clone()
                    .unwrap_or_else(|| "Filesystem lookup failed.".to_string());
                on_error_toast.emit(detail.clone());
                update_browser_state(&path_browser, |state| {
                    state.busy = false;
                    state.error = Some(detail);
                    state.entries.clear();
                    state.parent = None;
                });
            }
        }
    });
}

fn render_config_error(config_error: Option<String>) -> Html {
    if let Some(err) = config_error {
        html! {
            <div role="alert" class="alert alert-error">
                <span>{err}</span>
            </div>
        }
    } else {
        html! {}
    }
}

fn render_save_bar(
    status: SettingsStatus,
    save_disabled: bool,
    save_busy: bool,
    on_save: Callback<MouseEvent>,
    bundle: &TranslationBundle,
) -> Html {
    let change_label = if status.dirty_count == 0 {
        bundle.text("settings.changes_none")
    } else {
        format!("{} {}", status.dirty_count, bundle.text("settings.changes"))
    };
    let save_label = if save_busy {
        bundle.text("settings.saving")
    } else {
        bundle.text("settings.save_all")
    };
    html! {
        <div class="alert bg-base-100 shadow">
            <div class="flex items-center gap-3">
                <span class="badge badge-outline">{change_label}</span>
                {if status.has_errors {
                    html! { <span class="text-xs text-error">{bundle.text("settings.fix_errors")}</span> }
                } else {
                    html! {}
                }}
            </div>
            <div class="flex items-center gap-2">
                <button
                    class="btn btn-primary btn-sm"
                    disabled={save_disabled}
                    onclick={on_save}
                >
                    {save_label}
                </button>
            </div>
        </div>
    }
}

fn build_config_tab_body(
    active_tab: SettingsTab,
    fields: &ConfigTabFields,
    config_snapshot: Option<&Value>,
    draft: UseStateHandle<SettingsDraft>,
    immutable_keys: &HashSet<String>,
    props: &SettingsConfigProps,
    bundle: &TranslationBundle,
    on_open_path_picker: Callback<PathPickerTarget>,
) -> Html {
    match active_tab {
        SettingsTab::Downloads => render_engine_group_tab(
            "settings.group.downloads",
            "settings.group.downloads_body",
            &fields.engine_groups.downloads,
            true,
            config_snapshot,
            draft,
            immutable_keys,
            props,
            bundle,
            on_open_path_picker,
        ),
        SettingsTab::Seeding => render_engine_group_tab(
            "settings.group.seeding",
            "settings.group.seeding_body",
            &fields.engine_groups.seeding,
            false,
            config_snapshot,
            draft,
            immutable_keys,
            props,
            bundle,
            on_open_path_picker,
        ),
        SettingsTab::Network => render_engine_group_tab(
            "settings.group.network",
            "settings.group.network_body",
            &fields.engine_groups.network,
            false,
            config_snapshot,
            draft,
            immutable_keys,
            props,
            bundle,
            on_open_path_picker,
        ),
        SettingsTab::Storage => render_storage_tab(
            bundle.text("settings.group.storage"),
            Some(bundle.text("settings.group.storage_body")),
            &fields.engine_groups.storage,
            &fields.fs_fields,
            config_snapshot,
            draft,
            immutable_keys,
            props,
            props.on_refresh_config.clone(),
            bundle,
            on_open_path_picker,
        ),
        SettingsTab::Labels => render_labels_tab(
            &fields.app_groups.labels,
            config_snapshot,
            draft,
            immutable_keys,
            props,
            bundle,
            on_open_path_picker,
        ),
        SettingsTab::System => render_system_tab(
            &fields.app_groups.info,
            &fields.app_groups.telemetry,
            &fields.app_groups.other,
            &fields.engine_groups.advanced,
            config_snapshot,
            draft,
            immutable_keys,
            props,
            props.on_refresh_config.clone(),
            bundle,
            on_open_path_picker,
        ),
        SettingsTab::Connection => html! {},
    }
}

fn render_engine_group_tab(
    title_key: &str,
    body_key: &str,
    fields: &[SettingsField],
    show_refresh: bool,
    config_snapshot: Option<&Value>,
    draft: UseStateHandle<SettingsDraft>,
    immutable_keys: &HashSet<String>,
    props: &SettingsConfigProps,
    bundle: &TranslationBundle,
    on_open_path_picker: Callback<PathPickerTarget>,
) -> Html {
    render_engine_tab(
        bundle.text(title_key),
        Some(bundle.text(body_key)),
        fields,
        config_snapshot,
        draft,
        immutable_keys,
        props,
        props.on_refresh_config.clone(),
        show_refresh,
        bundle,
        on_open_path_picker,
    )
}

fn render_engine_tab(
    title: String,
    description: Option<String>,
    fields: &[SettingsField],
    snapshot: Option<&Value>,
    draft: UseStateHandle<SettingsDraft>,
    immutable_keys: &HashSet<String>,
    props: &SettingsConfigProps,
    on_refresh: Callback<()>,
    show_refresh: bool,
    bundle: &TranslationBundle,
    on_open_path_picker: Callback<PathPickerTarget>,
) -> Html {
    if snapshot.is_none() {
        return render_config_placeholder(bundle, props.config_busy);
    }

    let header_action = if show_refresh {
        Some((
            emit_callback(on_refresh),
            props.config_busy,
            if props.config_busy {
                bundle.text("settings.refreshing")
            } else {
                bundle.text("settings.refresh")
            },
        ))
    } else {
        None
    };

    html! {
        <div class="space-y-4">
            {render_settings_group(
                title,
                description,
                fields.to_vec(),
                snapshot,
                draft,
                immutable_keys,
                props.on_copy_value.clone(),
                on_open_path_picker,
                bundle,
                header_action,
            )}
        </div>
    }
}

fn render_storage_tab(
    title: String,
    description: Option<String>,
    engine_fields: &[SettingsField],
    fs_fields: &[SettingsField],
    snapshot: Option<&Value>,
    draft: UseStateHandle<SettingsDraft>,
    immutable_keys: &HashSet<String>,
    props: &SettingsConfigProps,
    on_refresh: Callback<()>,
    bundle: &TranslationBundle,
    on_open_path_picker: Callback<PathPickerTarget>,
) -> Html {
    if snapshot.is_none() {
        return render_config_placeholder(bundle, props.config_busy);
    }

    html! {
        <div class="space-y-4">
            {render_settings_group(
                title,
                description,
                engine_fields.to_vec(),
                snapshot,
                draft.clone(),
                immutable_keys,
                props.on_copy_value.clone(),
                on_open_path_picker.clone(),
                bundle,
                Some((
                    emit_callback(on_refresh),
                    props.config_busy,
                    if props.config_busy {
                        bundle.text("settings.refreshing")
                    } else {
                        bundle.text("settings.refresh")
                    },
                )),
            )}
            {render_settings_group(
                bundle.text("settings.group.fs_policy"),
                Some(bundle.text("settings.group.fs_policy_body")),
                fs_fields.to_vec(),
                snapshot,
                draft,
                immutable_keys,
                props.on_copy_value.clone(),
                on_open_path_picker,
                bundle,
                None,
            )}
        </div>
    }
}

fn render_labels_tab(
    fields: &[SettingsField],
    snapshot: Option<&Value>,
    draft: UseStateHandle<SettingsDraft>,
    immutable_keys: &HashSet<String>,
    props: &SettingsConfigProps,
    bundle: &TranslationBundle,
    on_open_path_picker: Callback<PathPickerTarget>,
) -> Html {
    if snapshot.is_none() {
        return render_config_placeholder(bundle, props.config_busy);
    }

    html! {
        <div class="space-y-4">
            {render_settings_group(
                bundle.text("settings.group.labels"),
                Some(bundle.text("settings.group.labels_body")),
                fields.to_vec(),
                snapshot,
                draft,
                immutable_keys,
                props.on_copy_value.clone(),
                on_open_path_picker,
                bundle,
                None,
            )}
        </div>
    }
}

fn render_system_tab(
    app_info_fields: &[SettingsField],
    telemetry_fields: &[SettingsField],
    app_other_fields: &[SettingsField],
    engine_extra_fields: &[SettingsField],
    snapshot: Option<&Value>,
    draft: UseStateHandle<SettingsDraft>,
    immutable_keys: &HashSet<String>,
    props: &SettingsConfigProps,
    on_refresh: Callback<()>,
    bundle: &TranslationBundle,
    on_open_path_picker: Callback<PathPickerTarget>,
) -> Html {
    if snapshot.is_none() {
        return render_config_placeholder(bundle, props.config_busy);
    }

    let app_fields = [app_info_fields, app_other_fields].concat();

    html! {
        <div class="space-y-4">
            {render_settings_group(
                bundle.text("settings.group.system"),
                Some(bundle.text("settings.group.system_body")),
                app_fields.to_vec(),
                snapshot,
                draft.clone(),
                immutable_keys,
                props.on_copy_value.clone(),
                on_open_path_picker.clone(),
                bundle,
                Some((
                    emit_callback(on_refresh),
                    props.config_busy,
                    if props.config_busy {
                        bundle.text("settings.refreshing")
                    } else {
                        bundle.text("settings.refresh")
                    },
                )),
            )}
            {if telemetry_fields.is_empty() {
                html! {}
            } else {
                render_settings_group(
                    bundle.text("settings.group.telemetry"),
                    Some(bundle.text("settings.group.telemetry_body")),
                    telemetry_fields.to_vec(),
                    snapshot,
                    draft.clone(),
                    immutable_keys,
                    props.on_copy_value.clone(),
                    on_open_path_picker.clone(),
                    bundle,
                    None,
                )
            }}
            {if engine_extra_fields.is_empty() {
                html! {}
            } else {
                render_settings_group(
                    bundle.text("settings.group.engine_extra"),
                    Some(bundle.text("settings.group.engine_extra_body")),
                    engine_extra_fields.to_vec(),
                    snapshot,
                    draft.clone(),
                    immutable_keys,
                    props.on_copy_value.clone(),
                    on_open_path_picker.clone(),
                    bundle,
                    None,
                )
            }}
        </div>
    }
}

fn render_settings_group(
    title: String,
    description: Option<String>,
    fields: Vec<SettingsField>,
    snapshot: Option<&Value>,
    draft: UseStateHandle<SettingsDraft>,
    immutable_keys: &HashSet<String>,
    on_copy_value: Callback<String>,
    on_open_path_picker: Callback<PathPickerTarget>,
    bundle: &TranslationBundle,
    header_action: Option<(Callback<MouseEvent>, bool, String)>,
) -> Html {
    if fields.is_empty() {
        return html! {};
    }

    html! {
        <div class="card bg-base-100 shadow">
            <div class="card-body gap-4">
                <div class="flex flex-wrap items-start justify-between gap-3">
                    <div>
                        <h3 class="text-base font-semibold">{title}</h3>
                        {description.map(|body| html! {
                            <p class="text-sm text-base-content/60">{body}</p>
                        }).unwrap_or_default()}
                    </div>
                    <div class="flex flex-wrap items-center gap-2">
                        {header_action.map(|(callback, busy, label)| html! {
                            <button
                                class="btn btn-outline btn-sm"
                                disabled={busy}
                                onclick={callback}
                            >
                                {label}
                            </button>
                        }).unwrap_or_default()}
                    </div>
                </div>
                <div class="grid gap-4 lg:grid-cols-2">
                    {for fields.iter().map(|field| render_setting_field(
                        field,
                        snapshot,
                        draft.clone(),
                        immutable_keys,
                        on_copy_value.clone(),
                        on_open_path_picker.clone(),
                        bundle,
                    ))}
                </div>
            </div>
        </div>
    }
}

struct FieldContext {
    field_key: String,
    label: String,
    raw_value: String,
    display_value: String,
    error: Option<String>,
    bool_value: bool,
    list_value: Vec<String>,
    read_only: bool,
}

fn render_setting_field(
    field: &SettingsField,
    snapshot: Option<&Value>,
    draft: UseStateHandle<SettingsDraft>,
    immutable_keys: &HashSet<String>,
    on_copy_value: Callback<String>,
    on_open_path_picker: Callback<PathPickerTarget>,
    bundle: &TranslationBundle,
) -> Html {
    let (context, control) = build_field_context(field, snapshot, &draft, immutable_keys, bundle);
    if context.read_only {
        return render_readonly_field(context.label, context.display_value, on_copy_value, bundle);
    }
    let Some(control) = control else {
        return html! {};
    };
    render_editable_field(
        context,
        control,
        draft,
        on_copy_value,
        on_open_path_picker,
        bundle,
    )
}

fn build_field_context(
    field: &SettingsField,
    snapshot: Option<&Value>,
    draft: &UseStateHandle<SettingsDraft>,
    immutable_keys: &HashSet<String>,
    bundle: &TranslationBundle,
) -> (FieldContext, Option<FieldControl>) {
    let field_key = format!("{}.{}", field.section.key(), field.key);
    let field_label = field_label(bundle, field.section, &field.key);
    let value = snapshot
        .and_then(|snapshot| snapshot.get(field.section.key()))
        .and_then(|section| section.get(&field.key));
    let control = value.map(|value| control_for_field(field.section, &field.key, value));
    let field_state = draft.fields.get(&field_key);
    let display_value = field_state
        .map(|field| value_to_display(&field.value))
        .or_else(|| value.map(value_to_display))
        .unwrap_or_default();
    let raw_value = field_state
        .map(|field| field.raw.clone())
        .or_else(|| value.map(value_to_raw))
        .unwrap_or_default();
    let error = field_state.and_then(|field| field.error.clone());
    let bool_value = field_state
        .and_then(|field| field.value.as_bool())
        .or_else(|| value.and_then(Value::as_bool))
        .unwrap_or(false);
    let list_value = field_state
        .map(|field| value_array_as_strings(&field.value))
        .or_else(|| value.map(value_array_as_strings))
        .unwrap_or_default();
    let read_only = is_field_read_only(field.section, &field.key, immutable_keys);
    (
        FieldContext {
            field_key,
            label: field_label,
            raw_value,
            display_value,
            error,
            bool_value,
            list_value,
            read_only,
        },
        control,
    )
}

fn render_editable_field(
    context: FieldContext,
    control: FieldControl,
    draft: UseStateHandle<SettingsDraft>,
    on_copy_value: Callback<String>,
    on_open_path_picker: Callback<PathPickerTarget>,
    bundle: &TranslationBundle,
) -> Html {
    match control {
        FieldControl::Toggle => {
            render_toggle_field(context.label, context.field_key, context.bool_value, draft)
        }
        FieldControl::Select(options) => render_select_field(
            context.label,
            context.field_key,
            context.raw_value,
            options,
            draft,
            bundle,
        ),
        FieldControl::Number(kind) => render_number_field(
            context.label,
            context.field_key,
            context.raw_value,
            context.error,
            kind,
            draft,
            bundle,
        ),
        FieldControl::Text => {
            render_text_field(context.label, context.field_key, context.raw_value, draft)
        }
        FieldControl::Path => render_path_field(
            context.label,
            context.field_key,
            context.raw_value,
            draft,
            on_open_path_picker,
            bundle,
        ),
        FieldControl::PathList => render_path_list_field(
            context.label,
            context.field_key,
            context.list_value,
            draft,
            on_open_path_picker,
            bundle,
        ),
        FieldControl::StringList(options) => render_string_list_field(
            context.label,
            context.field_key,
            context.list_value,
            options,
            draft,
            bundle,
        ),
        FieldControl::Telemetry => render_telemetry_field(context.field_key, draft, bundle),
        FieldControl::LabelPolicies => {
            render_label_policies_field(context.field_key, draft, bundle, on_open_path_picker)
        }
        FieldControl::AltSpeed => render_alt_speed_field(context.field_key, draft, bundle),
        FieldControl::Tracker => render_tracker_field(context.field_key, draft, bundle),
        FieldControl::IpFilter => {
            render_ip_filter_field(context.field_key, draft, bundle, on_copy_value)
        }
        FieldControl::PeerClasses => render_peer_classes_field(context.field_key, draft, bundle),
    }
}

fn render_readonly_field(
    field_label: String,
    display_value: String,
    on_copy_value: Callback<String>,
    bundle: &TranslationBundle,
) -> Html {
    let on_copy = {
        let on_copy_value = on_copy_value.clone();
        let payload = display_value.clone();
        Callback::from(move |_| on_copy_value.emit(payload.clone()))
    };
    html! {
        <div class="form-control w-full">
            <label class="label pb-1">
                <span class="label-text text-xs">{field_label}</span>
            </label>
            <div class="flex items-start justify-between gap-2 rounded-box border border-base-200 bg-base-200/40 p-2">
                <span class="text-xs font-mono break-all whitespace-pre-wrap">{display_value}</span>
                <button class="btn btn-ghost btn-xs" onclick={on_copy}>
                    {bundle.text("settings.copy")}
                </button>
            </div>
        </div>
    }
}

fn render_toggle_field(
    field_label: String,
    field_key: String,
    checked: bool,
    draft: UseStateHandle<SettingsDraft>,
) -> Html {
    let onchange = {
        let draft = draft.clone();
        Callback::from(move |value: bool| {
            update_field(&draft, &field_key, |field| {
                field.value = Value::Bool(value);
                field.raw = value.to_string();
                field.error = None;
            });
        })
    };
    html! {
        <div class="form-control w-full">
            <Toggle
                label={Some(AttrValue::from(field_label))}
                checked={checked}
                onchange={onchange}
            />
        </div>
    }
}

fn render_select_field(
    field_label: String,
    field_key: String,
    selected: String,
    options: SelectOptions,
    draft: UseStateHandle<SettingsDraft>,
    bundle: &TranslationBundle,
) -> Html {
    let options_list = options
        .options
        .iter()
        .map(|(value, key)| {
            (
                AttrValue::from(value.clone()),
                AttrValue::from(bundle.text(key)),
            )
        })
        .collect::<Vec<_>>();
    let placeholder = options
        .allow_empty
        .then(|| AttrValue::from(bundle.text("settings.option.auto")));
    let selected_value = if selected.is_empty() && options.allow_empty {
        None
    } else {
        Some(AttrValue::from(selected))
    };
    let onchange = {
        let draft = draft.clone();
        Callback::from(move |value: AttrValue| {
            let raw = value.to_string();
            update_field(&draft, &field_key, |field| {
                if raw.is_empty() {
                    field.value = Value::Null;
                } else {
                    field.value = Value::String(raw.clone());
                }
                field.raw = raw;
                field.error = None;
            });
        })
    };
    html! {
        <div class="form-control w-full">
            <label class="label pb-1">
                <span class="label-text text-xs">{field_label}</span>
            </label>
            <Select
                value={selected_value}
                options={options_list}
                placeholder={placeholder}
                class="w-full"
                onchange={onchange}
            />
        </div>
    }
}

fn render_number_field(
    field_label: String,
    field_key: String,
    raw_value: String,
    error: Option<String>,
    kind: NumericKind,
    draft: UseStateHandle<SettingsDraft>,
    bundle: &TranslationBundle,
) -> Html {
    let input_class = classes!("w-full", error.as_ref().map(|_| "input-error"));
    let error_integer = bundle.text("settings.error_integer");
    let error_number = bundle.text("settings.error_number");
    let oninput = {
        let draft = draft.clone();
        Callback::from(move |value: String| {
            let trimmed = value.trim().to_string();
            update_field(&draft, &field_key, |field| {
                field.raw = value.clone();
                if trimmed.is_empty() {
                    field.value = Value::Null;
                    field.error = None;
                    return;
                }
                match parse_numeric(kind, &trimmed) {
                    Ok(parsed) => {
                        field.value = parsed;
                        field.error = None;
                    }
                    Err(NumericError::Integer) => {
                        field.error = Some(error_integer.clone());
                    }
                    Err(NumericError::Float) => {
                        field.error = Some(error_number.clone());
                    }
                }
            });
        })
    };
    html! {
        <div class="form-control w-full">
            <label class="label pb-1">
                <span class="label-text text-xs">{field_label}</span>
            </label>
            <Input
                value={AttrValue::from(raw_value)}
                input_type={Some(AttrValue::from("number"))}
                class={input_class}
                oninput={oninput}
            />
            {error.map(|message| html! {
                <span class="text-xs text-error">{message}</span>
            }).unwrap_or_default()}
        </div>
    }
}

fn render_text_field(
    field_label: String,
    field_key: String,
    raw_value: String,
    draft: UseStateHandle<SettingsDraft>,
) -> Html {
    let oninput = {
        let draft = draft.clone();
        Callback::from(move |value: String| {
            update_field(&draft, &field_key, |field| {
                field.value = Value::String(value.clone());
                field.raw = value;
                field.error = None;
            });
        })
    };
    html! {
        <div class="form-control w-full">
            <label class="label pb-1">
                <span class="label-text text-xs">{field_label}</span>
            </label>
            <Input
                value={AttrValue::from(raw_value)}
                class="w-full"
                oninput={oninput}
            />
        </div>
    }
}

fn render_path_field(
    field_label: String,
    field_key: String,
    raw_value: String,
    draft: UseStateHandle<SettingsDraft>,
    on_open_path_picker: Callback<PathPickerTarget>,
    bundle: &TranslationBundle,
) -> Html {
    let field_key_input = field_key.clone();
    let oninput = {
        let draft = draft.clone();
        Callback::from(move |value: String| {
            update_field(&draft, &field_key_input, |field| {
                field.value = Value::String(value.clone());
                field.raw = value;
                field.error = None;
            });
        })
    };
    let on_browse = {
        let on_open_path_picker = on_open_path_picker.clone();
        let field_key = field_key.clone();
        Callback::from(move |_| {
            on_open_path_picker.emit(PathPickerTarget::Single(field_key.clone()));
        })
    };
    html! {
        <div class="form-control w-full">
            <label class="label pb-1">
                <span class="label-text text-xs">{field_label}</span>
            </label>
            <div class="flex gap-2">
                <Input
                    value={AttrValue::from(raw_value)}
                    class="w-full"
                    oninput={oninput}
                />
                <button class="btn btn-outline btn-sm" onclick={on_browse}>
                    {bundle.text("settings.browse")}
                </button>
            </div>
        </div>
    }
}

fn render_path_list_field(
    field_label: String,
    field_key: String,
    entries: Vec<String>,
    draft: UseStateHandle<SettingsDraft>,
    on_open_path_picker: Callback<PathPickerTarget>,
    bundle: &TranslationBundle,
) -> Html {
    let on_add = {
        let on_open_path_picker = on_open_path_picker.clone();
        let field_key = field_key.clone();
        Callback::from(move |_| {
            on_open_path_picker.emit(PathPickerTarget::AllowPaths(field_key.clone()));
        })
    };
    let remove_entry = |entry: String| {
        let draft = draft.clone();
        let field_key = field_key.clone();
        Callback::from(move |_| {
            update_field(&draft, &field_key, |field| {
                let mut next = value_array_as_strings(&field.value);
                next.retain(|item| item != &entry);
                field.value = Value::Array(next.into_iter().map(Value::String).collect());
                field.raw = value_to_raw(&field.value);
                field.error = None;
            });
        })
    };

    html! {
        <div class="form-control w-full">
            <label class="label pb-1">
                <span class="label-text text-xs">{field_label}</span>
            </label>
            <div class="space-y-2">
                <div class="space-y-2">
                    {for entries.iter().map(|entry| {
                        let on_remove = remove_entry(entry.clone());
                        html! {
                            <div class="flex items-center justify-between gap-2 rounded-box border border-base-200 bg-base-200/30 px-2 py-1">
                                <span class="text-xs font-mono break-all">{entry.clone()}</span>
                                <button class="btn btn-ghost btn-xs" onclick={on_remove}>
                                    {bundle.text("settings.remove")}
                                </button>
                            </div>
                        }
                    })}
                    {if entries.is_empty() {
                        html! {
                            <div class="rounded-box border border-base-200 bg-base-200/30 px-2 py-2 text-xs text-base-content/60">
                                {bundle.text("settings.paths_empty")}
                            </div>
                        }
                    } else {
                        html! {}
                    }}
                </div>
                <button class="btn btn-outline btn-sm" onclick={on_add}>
                    {bundle.text("settings.add_path")}
                </button>
            </div>
        </div>
    }
}

fn render_string_list_field(
    field_label: String,
    field_key: String,
    entries: Vec<String>,
    options: StringListOptions,
    draft: UseStateHandle<SettingsDraft>,
    bundle: &TranslationBundle,
) -> Html {
    let input_value = draft
        .fields
        .get(&field_key)
        .map(|field| field.raw.clone())
        .unwrap_or_default();
    let oninput = {
        let draft = draft.clone();
        let field_key = field_key.clone();
        Callback::from(move |value: String| {
            update_field(&draft, &field_key, |field| {
                field.raw = value;
            });
        })
    };
    let on_add = {
        let draft = draft.clone();
        let field_key = field_key.clone();
        Callback::from(move |_| {
            let raw = draft
                .fields
                .get(&field_key)
                .map(|field| field.raw.clone())
                .unwrap_or_default();
            let value = raw.trim().to_string();
            if value.is_empty() {
                return;
            }
            update_field(&draft, &field_key, |field| {
                let mut next = value_array_as_strings(&field.value);
                if !next.iter().any(|entry| entry == &value) {
                    next.push(value.clone());
                }
                field.value = Value::Array(next.into_iter().map(Value::String).collect());
                field.raw = String::new();
                field.error = None;
            });
        })
    };
    let remove_entry = |entry: String| {
        let draft = draft.clone();
        let field_key = field_key.clone();
        Callback::from(move |_| {
            update_field(&draft, &field_key, |field| {
                let mut next = value_array_as_strings(&field.value);
                next.retain(|item| item != &entry);
                field.value = Value::Array(next.into_iter().map(Value::String).collect());
                field.error = None;
            });
        })
    };
    let placeholder = AttrValue::from(bundle.text(options.placeholder_key));
    html! {
        <div class="form-control w-full">
            <label class="label pb-1">
                <span class="label-text text-xs">{field_label}</span>
            </label>
            <div class="space-y-2">
                <div class="flex flex-wrap gap-2">
                    <Input
                        value={AttrValue::from(input_value)}
                        placeholder={Some(placeholder)}
                        class="w-full"
                        oninput={oninput}
                    />
                    <button class="btn btn-outline btn-sm" onclick={on_add}>
                        {bundle.text(options.add_label_key)}
                    </button>
                </div>
                <div class="space-y-2">
                    {for entries.iter().map(|entry| {
                        let on_remove = remove_entry(entry.clone());
                        html! {
                            <div class="flex items-center justify-between gap-2 rounded-box border border-base-200 bg-base-200/30 px-2 py-1">
                                <span class="text-xs font-mono break-all">{entry.clone()}</span>
                                <button class="btn btn-ghost btn-xs" onclick={on_remove}>
                                    {bundle.text("settings.remove")}
                                </button>
                            </div>
                        }
                    })}
                    {if entries.is_empty() {
                        html! {
                            <div class="rounded-box border border-base-200 bg-base-200/30 px-2 py-2 text-xs text-base-content/60">
                                {bundle.text(options.empty_label_key)}
                            </div>
                        }
                    } else {
                        html! {}
                    }}
                </div>
            </div>
        </div>
    }
}

fn render_telemetry_field(
    field_key: String,
    draft: UseStateHandle<SettingsDraft>,
    bundle: &TranslationBundle,
) -> Html {
    let map = field_object_value(&draft, &field_key);
    let level_value = map_string(&map, "level");
    let format_value = map_string(&map, "format");
    let otel_enabled = map_bool(&map, "otel_enabled");
    let otel_service = map_string(&map, "otel_service_name");
    let otel_endpoint = map_string(&map, "otel_endpoint");

    let level_options = telemetry_level_options(bundle);
    let format_options = telemetry_format_options(bundle);
    let on_level_change = {
        let draft = draft.clone();
        let field_key = field_key.clone();
        Callback::from(move |value: AttrValue| {
            let value = value.to_string();
            update_object_field(&draft, &field_key, |map| {
                set_optional_string(map, "level", &value);
            });
        })
    };
    let on_format_change = {
        let draft = draft.clone();
        let field_key = field_key.clone();
        Callback::from(move |value: AttrValue| {
            let value = value.to_string();
            update_object_field(&draft, &field_key, |map| {
                set_optional_string(map, "format", &value);
            });
        })
    };
    let on_otel_toggle = {
        let draft = draft.clone();
        let field_key = field_key.clone();
        Callback::from(move |value: bool| {
            update_object_field(&draft, &field_key, |map| {
                map.insert("otel_enabled".to_string(), Value::Bool(value));
            });
        })
    };
    let on_service_input = {
        let draft = draft.clone();
        let field_key = field_key.clone();
        Callback::from(move |value: String| {
            update_object_field(&draft, &field_key, |map| {
                set_optional_string(map, "otel_service_name", &value);
            });
        })
    };
    let on_endpoint_input = {
        let draft = draft.clone();
        let field_key = field_key.clone();
        Callback::from(move |value: String| {
            update_object_field(&draft, &field_key, |map| {
                set_optional_string(map, "otel_endpoint", &value);
            });
        })
    };

    html! {
        <div class="form-control w-full lg:col-span-2">
            <label class="label pb-1">
                <span class="label-text text-xs">{bundle.text("settings.fields.app_profile.telemetry")}</span>
            </label>
            <div class="space-y-3">
                <div class="grid gap-3 sm:grid-cols-2">
                    {render_telemetry_select(
                        "settings.telemetry.level.label",
                        level_value,
                        level_options,
                        on_level_change,
                        bundle,
                    )}
                    {render_telemetry_select(
                        "settings.telemetry.format.label",
                        format_value,
                        format_options,
                        on_format_change,
                        bundle,
                    )}
                </div>
                <Toggle
                    label={Some(AttrValue::from(bundle.text("settings.telemetry.otel_enabled")))}
                    checked={otel_enabled}
                    onchange={on_otel_toggle}
                />
                <div class="grid gap-3 sm:grid-cols-2">
                    <div class="form-control w-full">
                        <label class="label pb-1">
                            <span class="label-text text-xs">{bundle.text("settings.telemetry.otel_service")}</span>
                        </label>
                        <Input
                            value={AttrValue::from(otel_service)}
                            class="w-full"
                            disabled={!otel_enabled}
                            oninput={on_service_input}
                        />
                    </div>
                    <div class="form-control w-full">
                        <label class="label pb-1">
                            <span class="label-text text-xs">{bundle.text("settings.telemetry.otel_endpoint")}</span>
                        </label>
                        <Input
                            value={AttrValue::from(otel_endpoint)}
                            class="w-full"
                            disabled={!otel_enabled}
                            oninput={on_endpoint_input}
                        />
                    </div>
                </div>
            </div>
        </div>
    }
}

fn telemetry_level_options(bundle: &TranslationBundle) -> Vec<(AttrValue, AttrValue)> {
    vec![
        ("trace", "settings.telemetry.level.trace"),
        ("debug", "settings.telemetry.level.debug"),
        ("info", "settings.telemetry.level.info"),
        ("warn", "settings.telemetry.level.warn"),
        ("error", "settings.telemetry.level.error"),
    ]
    .into_iter()
    .map(|(value, label)| (AttrValue::from(value), AttrValue::from(bundle.text(label))))
    .collect::<Vec<_>>()
}

fn telemetry_format_options(bundle: &TranslationBundle) -> Vec<(AttrValue, AttrValue)> {
    vec![
        ("pretty", "settings.telemetry.format.pretty"),
        ("json", "settings.telemetry.format.json"),
    ]
    .into_iter()
    .map(|(value, label)| (AttrValue::from(value), AttrValue::from(bundle.text(label))))
    .collect::<Vec<_>>()
}

fn render_telemetry_select(
    label_key: &'static str,
    value: String,
    options: Vec<(AttrValue, AttrValue)>,
    onchange: Callback<AttrValue>,
    bundle: &TranslationBundle,
) -> Html {
    html! {
        <div class="form-control w-full">
            <label class="label pb-1">
                <span class="label-text text-xs">{bundle.text(label_key)}</span>
            </label>
            <Select
                value={(!value.is_empty()).then(|| AttrValue::from(value))}
                options={options}
                placeholder={Some(AttrValue::from(bundle.text("settings.option.auto")))}
                class="w-full"
                onchange={onchange}
            />
        </div>
    }
}

fn render_alt_speed_field(
    field_key: String,
    draft: UseStateHandle<SettingsDraft>,
    bundle: &TranslationBundle,
) -> Html {
    let map = field_object_value(&draft, &field_key);
    let values = alt_speed_values(&map);
    let error_integer = bundle.text("settings.error_integer");
    let field_error = draft
        .fields
        .get(&field_key)
        .and_then(|field| field.error.clone());

    let on_download = alt_speed_numeric_callback(
        draft.clone(),
        field_key.clone(),
        "download_bps",
        error_integer.clone(),
    );
    let on_upload = alt_speed_numeric_callback(
        draft.clone(),
        field_key.clone(),
        "upload_bps",
        error_integer.clone(),
    );
    let on_schedule_toggle = alt_speed_schedule_toggle_callback(draft.clone(), field_key.clone());

    html! {
        <div class="form-control w-full lg:col-span-2">
            <label class="label pb-1">
                <span class="label-text text-xs">{bundle.text("settings.fields.engine_profile.alt_speed")}</span>
            </label>
            <div class="space-y-3">
                {render_alt_speed_limits(&values, on_download, on_upload, bundle)}
                <Toggle
                    label={Some(AttrValue::from(bundle.text("settings.alt_speed.schedule")))}
                    checked={values.schedule_enabled}
                    onchange={on_schedule_toggle}
                />
                {render_alt_speed_schedule(draft, field_key, &values, bundle)}
                {field_error.map(|message| html! {
                    <span class="text-xs text-error">{message}</span>
                }).unwrap_or_default()}
            </div>
        </div>
    }
}

struct AltSpeedValues {
    download_bps: String,
    upload_bps: String,
    schedule_enabled: bool,
    days: Vec<String>,
    start_time: String,
    end_time: String,
}

fn alt_speed_values(map: &Map<String, Value>) -> AltSpeedValues {
    let download_bps = map
        .get("download_bps")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let upload_bps = map
        .get("upload_bps")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let schedule = map.get("schedule").and_then(Value::as_object).cloned();
    let schedule_enabled = schedule.is_some();
    let schedule_map = schedule.unwrap_or_default();
    let days = schedule_map
        .get("days")
        .map(value_array_as_strings)
        .unwrap_or_default();
    let start_time = schedule_map
        .get("start")
        .and_then(Value::as_str)
        .unwrap_or("00:00")
        .to_string();
    let end_time = schedule_map
        .get("end")
        .and_then(Value::as_str)
        .unwrap_or("23:59")
        .to_string();
    AltSpeedValues {
        download_bps,
        upload_bps,
        schedule_enabled,
        days,
        start_time,
        end_time,
    }
}

fn alt_speed_numeric_callback(
    draft: UseStateHandle<SettingsDraft>,
    field_key: String,
    key: &'static str,
    error_integer: String,
) -> Callback<String> {
    Callback::from(move |value: String| {
        update_object_field_with_error(&draft, &field_key, |map| {
            match apply_optional_numeric(&value, NumericKind::Integer) {
                Ok(Some(number)) => {
                    map.insert(key.to_string(), number);
                    None
                }
                Ok(None) => {
                    map.remove(key);
                    None
                }
                Err(_) => Some(error_integer.clone()),
            }
        });
    })
}

fn alt_speed_schedule_toggle_callback(
    draft: UseStateHandle<SettingsDraft>,
    field_key: String,
) -> Callback<bool> {
    Callback::from(move |value: bool| {
        update_object_field(&draft, &field_key, |map| {
            if value {
                let mut schedule = map
                    .get("schedule")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                if !schedule.contains_key("days") {
                    schedule.insert(
                        "days".to_string(),
                        Value::Array(
                            WEEKDAYS
                                .iter()
                                .take(5)
                                .map(|(day, _)| Value::String((*day).to_string()))
                                .collect(),
                        ),
                    );
                }
                schedule
                    .entry("start".to_string())
                    .or_insert_with(|| Value::String("00:00".to_string()));
                schedule
                    .entry("end".to_string())
                    .or_insert_with(|| Value::String("23:59".to_string()));
                map.insert("schedule".to_string(), Value::Object(schedule));
            } else {
                map.remove("schedule");
            }
        });
    })
}

fn render_alt_speed_limits(
    values: &AltSpeedValues,
    on_download: Callback<String>,
    on_upload: Callback<String>,
    bundle: &TranslationBundle,
) -> Html {
    html! {
        <div class="grid gap-3 sm:grid-cols-2">
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{bundle.text("settings.alt_speed.download")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.download_bps.clone())}
                    input_type={Some(AttrValue::from("number"))}
                    class="w-full"
                    oninput={on_download}
                />
            </div>
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{bundle.text("settings.alt_speed.upload")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.upload_bps.clone())}
                    input_type={Some(AttrValue::from("number"))}
                    class="w-full"
                    oninput={on_upload}
                />
            </div>
        </div>
    }
}

fn render_alt_speed_schedule(
    draft: UseStateHandle<SettingsDraft>,
    field_key: String,
    values: &AltSpeedValues,
    bundle: &TranslationBundle,
) -> Html {
    let schedule_enabled = values.schedule_enabled;
    let days = values.days.clone();
    let start_time = values.start_time.clone();
    let end_time = values.end_time.clone();
    let on_day_toggle = alt_speed_day_toggle_callback(draft.clone(), field_key.clone());
    let on_start = alt_speed_schedule_time_callback(draft.clone(), field_key.clone(), "start");
    let on_end = alt_speed_schedule_time_callback(draft, field_key, "end");

    html! {
        <div class={classes!("grid", "gap-3", "md:grid-cols-2", (!schedule_enabled).then_some("opacity-60"))}>
            <div class="space-y-2">
                <p class="text-xs text-base-content/60">{bundle.text("settings.alt_speed.days")}</p>
                <div class="grid grid-cols-2 gap-2 sm:grid-cols-4">
                    {for WEEKDAYS.iter().map(|(day, label)| {
                        let checked = days.iter().any(|entry| entry == day);
                        let onchange = on_day_toggle(day);
                        html! {
                            <label class="flex items-center gap-2 text-xs">
                                <input
                                    type="checkbox"
                                    class="checkbox checkbox-xs"
                                    checked={checked}
                                    disabled={!schedule_enabled}
                                    onchange={onchange}
                                />
                                <span>{bundle.text(label)}</span>
                            </label>
                        }
                    })}
                </div>
            </div>
            <div class="grid gap-3 sm:grid-cols-2">
                <div class="form-control w-full">
                    <label class="label pb-1">
                        <span class="label-text text-xs">{bundle.text("settings.alt_speed.start")}</span>
                    </label>
                    <Input
                        value={AttrValue::from(start_time)}
                        input_type={Some(AttrValue::from("time"))}
                        class="w-full"
                        disabled={!schedule_enabled}
                        oninput={on_start}
                    />
                </div>
                <div class="form-control w-full">
                    <label class="label pb-1">
                        <span class="label-text text-xs">{bundle.text("settings.alt_speed.end")}</span>
                    </label>
                    <Input
                        value={AttrValue::from(end_time)}
                        input_type={Some(AttrValue::from("time"))}
                        class="w-full"
                        disabled={!schedule_enabled}
                        oninput={on_end}
                    />
                </div>
            </div>
        </div>
    }
}

fn alt_speed_day_toggle_callback(
    draft: UseStateHandle<SettingsDraft>,
    field_key: String,
) -> impl Fn(&'static str) -> Callback<Event> + Clone {
    move |day: &'static str| {
        let draft = draft.clone();
        let field_key = field_key.clone();
        Callback::from(move |event: Event| {
            let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() else {
                return;
            };
            let checked = input.checked();
            update_object_field(&draft, &field_key, |map| {
                let mut schedule = map
                    .get("schedule")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                let mut current = schedule
                    .get("days")
                    .map(value_array_as_strings)
                    .unwrap_or_default();
                if checked && !current.iter().any(|entry| entry == day) {
                    current.push(day.to_string());
                }
                if !checked {
                    current.retain(|entry| entry != day);
                }
                let ordered = ordered_weekdays(&current);
                schedule.insert(
                    "days".to_string(),
                    Value::Array(ordered.into_iter().map(Value::String).collect()),
                );
                schedule
                    .entry("start".to_string())
                    .or_insert_with(|| Value::String("00:00".to_string()));
                schedule
                    .entry("end".to_string())
                    .or_insert_with(|| Value::String("23:59".to_string()));
                map.insert("schedule".to_string(), Value::Object(schedule));
            });
        })
    }
}

fn alt_speed_schedule_time_callback(
    draft: UseStateHandle<SettingsDraft>,
    field_key: String,
    key: &'static str,
) -> Callback<String> {
    Callback::from(move |value: String| {
        update_object_field(&draft, &field_key, |map| {
            let mut schedule = map
                .get("schedule")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            schedule.insert(key.to_string(), Value::String(value));
            map.insert("schedule".to_string(), Value::Object(schedule));
        });
    })
}

#[derive(Properties, PartialEq)]
struct LabelPoliciesFieldProps {
    field_key: String,
    draft: UseStateHandle<SettingsDraft>,
    bundle: TranslationBundle,
    on_open_path_picker: Callback<PathPickerTarget>,
}

#[function_component(LabelPoliciesField)]
fn label_policies_field(props: &LabelPoliciesFieldProps) -> Html {
    let new_category = use_state(String::new);
    let new_tag = use_state(String::new);
    let label_policies_value = props
        .draft
        .fields
        .get(&props.field_key)
        .map(|field| field.value.clone())
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let categories = label_policy_entries(&label_policies_value, LabelKind::Category);
    let tags = label_policy_entries(&label_policies_value, LabelKind::Tag);
    let field_error = props
        .draft
        .fields
        .get(&props.field_key)
        .and_then(|field| field.error.clone());

    let context = LabelPolicyContext {
        draft: props.draft.clone(),
        field_key: props.field_key.clone(),
        bundle: props.bundle.clone(),
        on_open_path_picker: props.on_open_path_picker.clone(),
        error_integer: props.bundle.text("settings.error_integer"),
        error_number: props.bundle.text("settings.error_number"),
    };

    let on_category_input = label_policy_input_callback(new_category.clone());
    let on_tag_input = label_policy_input_callback(new_tag.clone());
    let on_add_category =
        label_policy_add_callback(context.clone(), new_category.clone(), LabelKind::Category);
    let on_add_tag = label_policy_add_callback(context.clone(), new_tag.clone(), LabelKind::Tag);

    html! {
        <div class="form-control w-full lg:col-span-2">
            <label class="label pb-1">
                <span class="label-text text-xs">{context.bundle.text("settings.fields.app_profile.label_policies")}</span>
            </label>
            <div class="space-y-4">
                {render_label_policy_list(
                    LabelKind::Category,
                    categories,
                    new_category,
                    on_category_input,
                    on_add_category,
                    &context,
                )}
                {render_label_policy_list(
                    LabelKind::Tag,
                    tags,
                    new_tag,
                    on_tag_input,
                    on_add_tag,
                    &context,
                )}
                {field_error.map(|message| html! {
                    <div role="alert" class="alert alert-error">
                        <span>{message}</span>
                    </div>
                }).unwrap_or_default()}
            </div>
        </div>
    }
}

#[derive(Clone)]
struct LabelPolicyContext {
    draft: UseStateHandle<SettingsDraft>,
    field_key: String,
    bundle: TranslationBundle,
    on_open_path_picker: Callback<PathPickerTarget>,
    error_integer: String,
    error_number: String,
}

fn label_policy_input_callback(state: UseStateHandle<String>) -> Callback<String> {
    Callback::from(move |value: String| state.set(value))
}

fn label_policy_add_callback(
    context: LabelPolicyContext,
    state: UseStateHandle<String>,
    kind: LabelKind,
) -> Callback<MouseEvent> {
    Callback::from(move |_| {
        let name = state.trim().to_string();
        if name.is_empty() {
            return;
        }
        insert_label_policy_entry(&context.draft, &context.field_key, kind, &name);
        state.set(String::new());
    })
}

fn render_label_policy_list(
    kind: LabelKind,
    entries: Vec<(String, Map<String, Value>)>,
    new_value: UseStateHandle<String>,
    on_new_input: Callback<String>,
    on_add: Callback<MouseEvent>,
    context: &LabelPolicyContext,
) -> Html {
    let title = context.bundle.text(kind.label_key());
    let placeholder = context.bundle.text("settings.labels.name_placeholder");
    let add_label = match kind {
        LabelKind::Category => context.bundle.text("settings.labels.add_category"),
        LabelKind::Tag => context.bundle.text("settings.labels.add_tag"),
    };
    html! {
        <div class="space-y-3">
            <div class="flex items-center justify-between gap-2">
                <h4 class="text-sm font-semibold">{title}</h4>
                <div class="flex items-center gap-2">
                    <Input
                        value={AttrValue::from((*new_value).clone())}
                        placeholder={Some(AttrValue::from(placeholder))}
                        class="w-44"
                        oninput={on_new_input}
                    />
                    <button class="btn btn-outline btn-xs" onclick={on_add}>
                        {add_label}
                    </button>
                </div>
            </div>
            {if entries.is_empty() {
                html! {
                    <div class="rounded-box border border-base-200 bg-base-200/30 px-3 py-2 text-xs text-base-content/60">
                        {context.bundle.text("settings.labels.empty")}
                    </div>
                }
            } else {
                html! {
                    <div class="space-y-3">
                        {for entries.into_iter().map(|(name, policy)| {
                            render_label_policy_entry(kind, name, policy, context)
                        })}
                    </div>
                }
            }}
        </div>
    }
}

struct LabelPolicyEntryValues {
    download_dir: String,
    queue_position: String,
    auto_managed: bool,
    seed_ratio_limit: String,
    seed_time_limit: String,
    rate_download: String,
    rate_upload: String,
    cleanup_seed_ratio: String,
    cleanup_seed_time: String,
    cleanup_remove: bool,
}

fn label_policy_entry_values(policy: &Map<String, Value>) -> LabelPolicyEntryValues {
    let download_dir = policy
        .get("download_dir")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let queue_position = policy
        .get("queue_position")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let auto_managed = policy
        .get("auto_managed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let seed_ratio_limit = policy
        .get("seed_ratio_limit")
        .and_then(Value::as_f64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let seed_time_limit = policy
        .get("seed_time_limit")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let rate_download = policy
        .get("rate_limit_download_bps")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let rate_upload = policy
        .get("rate_limit_upload_bps")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let cleanup_seed_ratio = policy
        .get("cleanup_seed_ratio_limit")
        .and_then(Value::as_f64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let cleanup_seed_time = policy
        .get("cleanup_seed_time_limit")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let cleanup_remove = policy
        .get("cleanup_remove_data")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    LabelPolicyEntryValues {
        download_dir,
        queue_position,
        auto_managed,
        seed_ratio_limit,
        seed_time_limit,
        rate_download,
        rate_upload,
        cleanup_seed_ratio,
        cleanup_seed_time,
        cleanup_remove,
    }
}

struct LabelPolicyCallbacks {
    on_remove: Callback<MouseEvent>,
    on_download_dir: Callback<String>,
    on_browse: Callback<MouseEvent>,
    on_queue_position: Callback<String>,
    on_auto_managed: Callback<bool>,
    on_seed_ratio: Callback<String>,
    on_seed_time: Callback<String>,
    on_rate_download: Callback<String>,
    on_rate_upload: Callback<String>,
    on_cleanup_seed_ratio: Callback<String>,
    on_cleanup_seed_time: Callback<String>,
    on_cleanup_remove: Callback<bool>,
}

fn label_policy_entry_callbacks(
    context: &LabelPolicyContext,
    kind: LabelKind,
    name: &str,
) -> LabelPolicyCallbacks {
    LabelPolicyCallbacks {
        on_remove: label_policy_remove_callback(context, kind, name.to_string()),
        on_download_dir: label_policy_string_callback(
            context,
            kind,
            name.to_string(),
            "download_dir",
        ),
        on_browse: label_policy_browse_callback(context, kind, name.to_string()),
        on_queue_position: label_policy_numeric_callback(
            context,
            kind,
            name.to_string(),
            "queue_position",
            NumericKind::Integer,
            context.error_integer.clone(),
        ),
        on_auto_managed: label_policy_bool_callback(
            context,
            kind,
            name.to_string(),
            "auto_managed",
        ),
        on_seed_ratio: label_policy_numeric_callback(
            context,
            kind,
            name.to_string(),
            "seed_ratio_limit",
            NumericKind::Float,
            context.error_number.clone(),
        ),
        on_seed_time: label_policy_numeric_callback(
            context,
            kind,
            name.to_string(),
            "seed_time_limit",
            NumericKind::Integer,
            context.error_integer.clone(),
        ),
        on_rate_download: label_policy_numeric_callback(
            context,
            kind,
            name.to_string(),
            "rate_limit_download_bps",
            NumericKind::Integer,
            context.error_integer.clone(),
        ),
        on_rate_upload: label_policy_numeric_callback(
            context,
            kind,
            name.to_string(),
            "rate_limit_upload_bps",
            NumericKind::Integer,
            context.error_integer.clone(),
        ),
        on_cleanup_seed_ratio: label_policy_numeric_callback(
            context,
            kind,
            name.to_string(),
            "cleanup_seed_ratio_limit",
            NumericKind::Float,
            context.error_number.clone(),
        ),
        on_cleanup_seed_time: label_policy_numeric_callback(
            context,
            kind,
            name.to_string(),
            "cleanup_seed_time_limit",
            NumericKind::Integer,
            context.error_integer.clone(),
        ),
        on_cleanup_remove: label_policy_bool_callback(
            context,
            kind,
            name.to_string(),
            "cleanup_remove_data",
        ),
    }
}

fn render_label_policy_entry(
    kind: LabelKind,
    name: String,
    policy: Map<String, Value>,
    context: &LabelPolicyContext,
) -> Html {
    let values = label_policy_entry_values(&policy);
    let callbacks = label_policy_entry_callbacks(context, kind, &name);

    html! {
        <div class="rounded-box border border-base-200 bg-base-200/20 p-3 space-y-3">
            {render_label_policy_header(&name, callbacks.on_remove.clone(), &context.bundle)}
            {render_label_policy_primary(&values, &callbacks, &context.bundle)}
            {render_label_policy_limits(&values, &callbacks, &context.bundle)}
        </div>
    }
}

fn render_label_policy_header(
    name: &str,
    on_remove: Callback<MouseEvent>,
    bundle: &TranslationBundle,
) -> Html {
    html! {
        <div class="flex items-center justify-between">
            <p class="text-sm font-semibold">{name}</p>
            <button class="btn btn-ghost btn-xs" onclick={on_remove}>
                {bundle.text("settings.remove")}
            </button>
        </div>
    }
}

fn render_label_policy_primary(
    values: &LabelPolicyEntryValues,
    callbacks: &LabelPolicyCallbacks,
    bundle: &TranslationBundle,
) -> Html {
    html! {
        <div class="grid gap-3 sm:grid-cols-2">
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{bundle.text("settings.labels.download_dir")}</span>
                </label>
                <div class="flex gap-2">
                    <Input
                        value={AttrValue::from(values.download_dir.clone())}
                        class="w-full"
                        oninput={callbacks.on_download_dir.clone()}
                    />
                    <button class="btn btn-outline btn-xs" onclick={callbacks.on_browse.clone()}>
                        {bundle.text("settings.browse")}
                    </button>
                </div>
            </div>
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{bundle.text("settings.labels.queue_position")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.queue_position.clone())}
                    input_type={Some(AttrValue::from("number"))}
                    class="w-full"
                    oninput={callbacks.on_queue_position.clone()}
                />
            </div>
            <div class="form-control w-full">
                <Toggle
                    label={Some(AttrValue::from(bundle.text("settings.labels.auto_managed")))}
                    checked={values.auto_managed}
                    onchange={callbacks.on_auto_managed.clone()}
                />
            </div>
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{bundle.text("settings.labels.seed_ratio_limit")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.seed_ratio_limit.clone())}
                    input_type={Some(AttrValue::from("number"))}
                    class="w-full"
                    oninput={callbacks.on_seed_ratio.clone()}
                />
            </div>
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{bundle.text("settings.labels.seed_time_limit")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.seed_time_limit.clone())}
                    input_type={Some(AttrValue::from("number"))}
                    class="w-full"
                    oninput={callbacks.on_seed_time.clone()}
                />
            </div>
        </div>
    }
}

fn render_label_policy_limits(
    values: &LabelPolicyEntryValues,
    callbacks: &LabelPolicyCallbacks,
    bundle: &TranslationBundle,
) -> Html {
    html! {
        <div class="grid gap-3 sm:grid-cols-2">
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{bundle.text("settings.labels.rate_limit_download")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.rate_download.clone())}
                    input_type={Some(AttrValue::from("number"))}
                    class="w-full"
                    oninput={callbacks.on_rate_download.clone()}
                />
            </div>
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{bundle.text("settings.labels.rate_limit_upload")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.rate_upload.clone())}
                    input_type={Some(AttrValue::from("number"))}
                    class="w-full"
                    oninput={callbacks.on_rate_upload.clone()}
                />
            </div>
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{bundle.text("settings.labels.cleanup_seed_ratio")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.cleanup_seed_ratio.clone())}
                    input_type={Some(AttrValue::from("number"))}
                    class="w-full"
                    oninput={callbacks.on_cleanup_seed_ratio.clone()}
                />
            </div>
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{bundle.text("settings.labels.cleanup_seed_time")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.cleanup_seed_time.clone())}
                    input_type={Some(AttrValue::from("number"))}
                    class="w-full"
                    oninput={callbacks.on_cleanup_seed_time.clone()}
                />
            </div>
            <div class="form-control w-full">
                <Toggle
                    label={Some(AttrValue::from(bundle.text("settings.labels.cleanup_remove_data")))}
                    checked={values.cleanup_remove}
                    onchange={callbacks.on_cleanup_remove.clone()}
                />
            </div>
        </div>
    }
}

fn label_policy_remove_callback(
    context: &LabelPolicyContext,
    kind: LabelKind,
    name: String,
) -> Callback<MouseEvent> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    Callback::from(move |_| {
        remove_label_policy_entry(&draft, &field_key, kind, &name);
    })
}

fn label_policy_string_callback(
    context: &LabelPolicyContext,
    kind: LabelKind,
    name: String,
    key: &'static str,
) -> Callback<String> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    Callback::from(move |value: String| {
        update_label_policy_string(&draft, &field_key, kind, &name, key, &value);
    })
}

fn label_policy_browse_callback(
    context: &LabelPolicyContext,
    kind: LabelKind,
    name: String,
) -> Callback<MouseEvent> {
    let on_open = context.on_open_path_picker.clone();
    Callback::from(move |_| {
        on_open.emit(PathPickerTarget::LabelPolicy {
            kind,
            name: name.clone(),
        });
    })
}

fn label_policy_numeric_callback(
    context: &LabelPolicyContext,
    kind: LabelKind,
    name: String,
    key: &'static str,
    kind_num: NumericKind,
    error_message: String,
) -> Callback<String> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    Callback::from(move |value: String| {
        update_label_policy_numeric(
            &draft,
            &field_key,
            kind,
            &name,
            key,
            &value,
            kind_num,
            &error_message,
        );
    })
}

fn label_policy_bool_callback(
    context: &LabelPolicyContext,
    kind: LabelKind,
    name: String,
    key: &'static str,
) -> Callback<bool> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    Callback::from(move |value: bool| {
        update_label_policy_bool(&draft, &field_key, kind, &name, key, value);
    })
}

fn render_label_policies_field(
    field_key: String,
    draft: UseStateHandle<SettingsDraft>,
    bundle: &TranslationBundle,
    on_open_path_picker: Callback<PathPickerTarget>,
) -> Html {
    html! {
        <LabelPoliciesField
            field_key={field_key}
            draft={draft}
            bundle={bundle.clone()}
            on_open_path_picker={on_open_path_picker}
        />
    }
}

#[derive(Properties, PartialEq)]
struct TrackerFieldProps {
    field_key: String,
    draft: UseStateHandle<SettingsDraft>,
    bundle: TranslationBundle,
}

#[function_component(TrackerField)]
fn tracker_field(props: &TrackerFieldProps) -> Html {
    let default_input = use_state(String::new);
    let extra_input = use_state(String::new);
    let map = field_object_value(&props.draft, &props.field_key);
    let field_error = props
        .draft
        .fields
        .get(&props.field_key)
        .and_then(|field| field.error.clone());

    let context = TrackerContext {
        draft: props.draft.clone(),
        field_key: props.field_key.clone(),
        bundle: props.bundle.clone(),
        error_integer: props.bundle.text("settings.error_integer"),
    };
    let values = tracker_values(&map);

    html! {
        <div class="form-control w-full lg:col-span-2 space-y-3">
            <label class="label pb-1">
                <span class="label-text text-xs">{context.bundle.text("settings.fields.engine_profile.tracker")}</span>
            </label>
            {render_tracker_lists(&context, &values, default_input, extra_input)}
            {render_tracker_behavior(&context, &values)}
            {render_tracker_ssl(&context, &values)}
            {render_tracker_proxy(&context, &values)}
            {render_tracker_auth(&context, &values)}
            {field_error.map(|message| html! {
                <div role="alert" class="alert alert-error">
                    <span>{message}</span>
                </div>
            }).unwrap_or_default()}
        </div>
    }
}

#[derive(Clone)]
struct TrackerContext {
    draft: UseStateHandle<SettingsDraft>,
    field_key: String,
    bundle: TranslationBundle,
    error_integer: String,
}

struct TrackerValues {
    default_list: Vec<String>,
    extra_list: Vec<String>,
    replace: bool,
    announce_to_all: bool,
    user_agent: String,
    announce_ip: String,
    listen_interface: String,
    request_timeout: String,
    ssl_cert: String,
    ssl_private_key: String,
    ssl_ca_cert: String,
    ssl_tracker_verify: bool,
    proxy_enabled: bool,
    proxy_host: String,
    proxy_port: String,
    proxy_kind: String,
    proxy_user: String,
    proxy_pass: String,
    proxy_peers: bool,
    auth_enabled: bool,
    auth_user: String,
    auth_pass: String,
    auth_cookie: String,
}

fn tracker_values(map: &Map<String, Value>) -> TrackerValues {
    let default_list = map_array_strings(map, "default");
    let extra_list = map_array_strings(map, "extra");
    let replace = map_bool(map, "replace");
    let announce_to_all = map_bool(map, "announce_to_all");
    let user_agent = map_string(map, "user_agent");
    let announce_ip = map_string(map, "announce_ip");
    let listen_interface = map_string(map, "listen_interface");
    let request_timeout = map
        .get("request_timeout_ms")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let ssl_cert = map_string(map, "ssl_cert");
    let ssl_private_key = map_string(map, "ssl_private_key");
    let ssl_ca_cert = map_string(map, "ssl_ca_cert");
    let ssl_tracker_verify = map
        .get("ssl_tracker_verify")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let proxy_value = map.get("proxy");
    let proxy_enabled = proxy_value.map(|value| !value.is_null()).unwrap_or(false);
    let proxy_map = proxy_value
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let proxy_host = map_string(&proxy_map, "host");
    let proxy_port = proxy_map
        .get("port")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let proxy_kind = map_string(&proxy_map, "kind");
    let proxy_user = map_string(&proxy_map, "username_secret");
    let proxy_pass = map_string(&proxy_map, "password_secret");
    let proxy_peers = map_bool(&proxy_map, "proxy_peers");

    let auth_value = map.get("auth");
    let auth_enabled = auth_value.map(|value| !value.is_null()).unwrap_or(false);
    let auth_map = auth_value
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let auth_user = map_string(&auth_map, "username_secret");
    let auth_pass = map_string(&auth_map, "password_secret");
    let auth_cookie = map_string(&auth_map, "cookie_secret");

    TrackerValues {
        default_list,
        extra_list,
        replace,
        announce_to_all,
        user_agent,
        announce_ip,
        listen_interface,
        request_timeout,
        ssl_cert,
        ssl_private_key,
        ssl_ca_cert,
        ssl_tracker_verify,
        proxy_enabled,
        proxy_host,
        proxy_port,
        proxy_kind,
        proxy_user,
        proxy_pass,
        proxy_peers,
        auth_enabled,
        auth_user,
        auth_pass,
        auth_cookie,
    }
}

fn render_tracker_lists(
    context: &TrackerContext,
    values: &TrackerValues,
    default_input: UseStateHandle<String>,
    extra_input: UseStateHandle<String>,
) -> Html {
    let on_default_input = tracker_list_input_callback(default_input.clone());
    let on_extra_input = tracker_list_input_callback(extra_input.clone());
    let default_value = (*default_input).clone();
    let extra_value = (*extra_input).clone();
    let on_default_add = tracker_list_add_callback(context, "default", default_input);
    let on_extra_add = tracker_list_add_callback(context, "extra", extra_input);

    html! {
        <div class="grid gap-3 sm:grid-cols-2">
            {render_tracker_list(
                context,
                "default",
                "settings.tracker.default",
                "settings.tracker.default_placeholder",
                values.default_list.clone(),
                default_value,
                on_default_input,
                on_default_add,
            )}
            {render_tracker_list(
                context,
                "extra",
                "settings.tracker.extra",
                "settings.tracker.extra_placeholder",
                values.extra_list.clone(),
                extra_value,
                on_extra_input,
                on_extra_add,
            )}
        </div>
    }
}

fn render_tracker_list(
    context: &TrackerContext,
    list_key: &'static str,
    label_key: &'static str,
    placeholder_key: &'static str,
    entries: Vec<String>,
    input_value: String,
    on_input: Callback<String>,
    on_add: Callback<MouseEvent>,
) -> Html {
    html! {
        <div class="form-control w-full">
            <label class="label pb-1">
                <span class="label-text text-xs">{context.bundle.text(label_key)}</span>
            </label>
            <div class="space-y-2">
                <div class="flex gap-2">
                    <Input
                        value={AttrValue::from(input_value)}
                        placeholder={Some(AttrValue::from(context.bundle.text(placeholder_key)))}
                        class="w-full"
                        oninput={on_input}
                    />
                    <button class="btn btn-outline btn-xs" onclick={on_add}>
                        {context.bundle.text("settings.add")}
                    </button>
                </div>
                {for entries.iter().map(|entry| {
                    let on_remove = tracker_list_remove_callback(context, list_key, entry.clone());
                    html! {
                        <div class="flex items-center justify-between gap-2 rounded-box border border-base-200 bg-base-200/30 px-2 py-1">
                            <span class="text-xs font-mono break-all">{entry.clone()}</span>
                            <button class="btn btn-ghost btn-xs" onclick={on_remove}>
                                {context.bundle.text("settings.remove")}
                            </button>
                        </div>
                    }
                })}
            </div>
        </div>
    }
}

fn render_tracker_behavior(context: &TrackerContext, values: &TrackerValues) -> Html {
    let on_replace = tracker_bool_callback(context, "replace");
    let on_announce_to_all = tracker_bool_callback(context, "announce_to_all");
    let on_user_agent = tracker_string_callback(context, "user_agent");
    let on_announce_ip = tracker_string_callback(context, "announce_ip");
    let on_listen_interface = tracker_string_callback(context, "listen_interface");
    let on_request_timeout =
        tracker_numeric_callback(context, "request_timeout_ms", context.error_integer.clone());

    html! {
        <div class="grid gap-3 sm:grid-cols-2">
            <Toggle
                label={Some(AttrValue::from(context.bundle.text("settings.tracker.replace")))}
                checked={values.replace}
                onchange={on_replace}
            />
            <Toggle
                label={Some(AttrValue::from(context.bundle.text("settings.tracker.announce_to_all")))}
                checked={values.announce_to_all}
                onchange={on_announce_to_all}
            />
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{context.bundle.text("settings.tracker.user_agent")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.user_agent.clone())}
                    class="w-full"
                    oninput={on_user_agent}
                />
            </div>
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{context.bundle.text("settings.tracker.announce_ip")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.announce_ip.clone())}
                    class="w-full"
                    oninput={on_announce_ip}
                />
            </div>
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{context.bundle.text("settings.tracker.listen_interface")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.listen_interface.clone())}
                    class="w-full"
                    oninput={on_listen_interface}
                />
            </div>
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{context.bundle.text("settings.tracker.request_timeout")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.request_timeout.clone())}
                    input_type={Some(AttrValue::from("number"))}
                    class="w-full"
                    oninput={on_request_timeout}
                />
            </div>
        </div>
    }
}

fn render_tracker_ssl(context: &TrackerContext, values: &TrackerValues) -> Html {
    let on_ssl_cert = tracker_string_callback(context, "ssl_cert");
    let on_ssl_key = tracker_string_callback(context, "ssl_private_key");
    let on_ssl_ca = tracker_string_callback(context, "ssl_ca_cert");
    let on_ssl_verify = tracker_bool_callback(context, "ssl_tracker_verify");

    html! {
        <div class="grid gap-3 sm:grid-cols-2">
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{context.bundle.text("settings.tracker.ssl_cert")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.ssl_cert.clone())}
                    class="w-full"
                    oninput={on_ssl_cert}
                />
            </div>
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{context.bundle.text("settings.tracker.ssl_private_key")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.ssl_private_key.clone())}
                    class="w-full"
                    oninput={on_ssl_key}
                />
            </div>
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{context.bundle.text("settings.tracker.ssl_ca_cert")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.ssl_ca_cert.clone())}
                    class="w-full"
                    oninput={on_ssl_ca}
                />
            </div>
            <Toggle
                label={Some(AttrValue::from(context.bundle.text("settings.tracker.ssl_verify")))}
                checked={values.ssl_tracker_verify}
                onchange={on_ssl_verify}
            />
        </div>
    }
}

fn render_tracker_proxy(context: &TrackerContext, values: &TrackerValues) -> Html {
    let on_proxy_toggle = tracker_nested_toggle_callback(context, "proxy");
    let on_proxy_host = tracker_nested_string_callback(context, "proxy", "host");
    let on_proxy_port =
        tracker_nested_numeric_callback(context, "proxy", "port", context.error_integer.clone());
    let on_proxy_kind = tracker_nested_select_callback(context, "proxy", "kind");
    let on_proxy_user = tracker_nested_string_callback(context, "proxy", "username_secret");
    let on_proxy_pass = tracker_nested_string_callback(context, "proxy", "password_secret");
    let on_proxy_peers = tracker_nested_bool_callback(context, "proxy", "proxy_peers");
    let proxy_kind_options = tracker_proxy_kind_options(&context.bundle);

    html! {
        <div class="rounded-box border border-base-200 bg-base-200/20 p-3 space-y-3">
            <Toggle
                label={Some(AttrValue::from(context.bundle.text("settings.tracker.proxy_enabled")))}
                checked={values.proxy_enabled}
                onchange={on_proxy_toggle}
            />
            {if values.proxy_enabled {
                html! {
                    <div class="grid gap-3 sm:grid-cols-2">
                        <div class="form-control w-full">
                            <label class="label pb-1">
                                <span class="label-text text-xs">{context.bundle.text("settings.tracker.proxy_host")}</span>
                            </label>
                            <Input
                                value={AttrValue::from(values.proxy_host.clone())}
                                class="w-full"
                                oninput={on_proxy_host}
                            />
                        </div>
                        <div class="form-control w-full">
                            <label class="label pb-1">
                                <span class="label-text text-xs">{context.bundle.text("settings.tracker.proxy_port")}</span>
                            </label>
                            <Input
                                value={AttrValue::from(values.proxy_port.clone())}
                                input_type={Some(AttrValue::from("number"))}
                                class="w-full"
                                oninput={on_proxy_port}
                            />
                        </div>
                        <div class="form-control w-full">
                            <label class="label pb-1">
                                <span class="label-text text-xs">{context.bundle.text("settings.tracker.proxy_kind")}</span>
                            </label>
                            <Select
                                value={(!values.proxy_kind.is_empty()).then(|| AttrValue::from(values.proxy_kind.clone()))}
                                options={proxy_kind_options}
                                placeholder={Some(AttrValue::from(context.bundle.text("settings.option.auto")))}
                                class="w-full"
                                onchange={on_proxy_kind}
                            />
                        </div>
                        <div class="form-control w-full">
                            <label class="label pb-1">
                                <span class="label-text text-xs">{context.bundle.text("settings.tracker.proxy_user")}</span>
                            </label>
                            <Input
                                value={AttrValue::from(values.proxy_user.clone())}
                                class="w-full"
                                oninput={on_proxy_user}
                            />
                        </div>
                        <div class="form-control w-full">
                            <label class="label pb-1">
                                <span class="label-text text-xs">{context.bundle.text("settings.tracker.proxy_pass")}</span>
                            </label>
                            <Input
                                value={AttrValue::from(values.proxy_pass.clone())}
                                class="w-full"
                                oninput={on_proxy_pass}
                            />
                        </div>
                        <Toggle
                            label={Some(AttrValue::from(context.bundle.text("settings.tracker.proxy_peers")))}
                            checked={values.proxy_peers}
                            onchange={on_proxy_peers}
                        />
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}

fn render_tracker_auth(context: &TrackerContext, values: &TrackerValues) -> Html {
    let on_auth_toggle = tracker_nested_toggle_callback(context, "auth");
    let on_auth_user = tracker_nested_string_callback(context, "auth", "username_secret");
    let on_auth_pass = tracker_nested_string_callback(context, "auth", "password_secret");
    let on_auth_cookie = tracker_nested_string_callback(context, "auth", "cookie_secret");

    html! {
        <div class="rounded-box border border-base-200 bg-base-200/20 p-3 space-y-3">
            <Toggle
                label={Some(AttrValue::from(context.bundle.text("settings.tracker.auth_enabled")))}
                checked={values.auth_enabled}
                onchange={on_auth_toggle}
            />
            {if values.auth_enabled {
                html! {
                    <div class="grid gap-3 sm:grid-cols-2">
                        <div class="form-control w-full">
                            <label class="label pb-1">
                                <span class="label-text text-xs">{context.bundle.text("settings.tracker.auth_user")}</span>
                            </label>
                            <Input
                                value={AttrValue::from(values.auth_user.clone())}
                                class="w-full"
                                oninput={on_auth_user}
                            />
                        </div>
                        <div class="form-control w-full">
                            <label class="label pb-1">
                                <span class="label-text text-xs">{context.bundle.text("settings.tracker.auth_pass")}</span>
                            </label>
                            <Input
                                value={AttrValue::from(values.auth_pass.clone())}
                                class="w-full"
                                oninput={on_auth_pass}
                            />
                        </div>
                        <div class="form-control w-full">
                            <label class="label pb-1">
                                <span class="label-text text-xs">{context.bundle.text("settings.tracker.auth_cookie")}</span>
                            </label>
                            <Input
                                value={AttrValue::from(values.auth_cookie.clone())}
                                class="w-full"
                                oninput={on_auth_cookie}
                            />
                        </div>
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}

fn tracker_list_input_callback(state: UseStateHandle<String>) -> Callback<String> {
    Callback::from(move |value: String| state.set(value))
}

fn tracker_list_add_callback(
    context: &TrackerContext,
    list_key: &'static str,
    input: UseStateHandle<String>,
) -> Callback<MouseEvent> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    let bundle = context.bundle.clone();
    Callback::from(move |_| {
        let value = input.trim().to_string();
        if value.is_empty() {
            return;
        }
        update_tracker_field(&draft, &field_key, &bundle, |map| {
            let mut list = map_array_strings(map, list_key);
            if !list.iter().any(|entry| entry == &value) {
                list.push(value.clone());
            }
            map.insert(
                list_key.to_string(),
                Value::Array(list.into_iter().map(Value::String).collect()),
            );
            None
        });
        input.set(String::new());
    })
}

fn tracker_list_remove_callback(
    context: &TrackerContext,
    list_key: &'static str,
    entry: String,
) -> Callback<MouseEvent> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    let bundle = context.bundle.clone();
    Callback::from(move |_| {
        update_tracker_field(&draft, &field_key, &bundle, |map| {
            let mut list = map_array_strings(map, list_key);
            list.retain(|item| item != &entry);
            map.insert(
                list_key.to_string(),
                Value::Array(list.into_iter().map(Value::String).collect()),
            );
            None
        });
    })
}

fn tracker_bool_callback(context: &TrackerContext, key: &'static str) -> Callback<bool> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    let bundle = context.bundle.clone();
    Callback::from(move |value: bool| {
        update_tracker_field(&draft, &field_key, &bundle, |map| {
            map.insert(key.to_string(), Value::Bool(value));
            None
        });
    })
}

fn tracker_string_callback(context: &TrackerContext, key: &'static str) -> Callback<String> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    let bundle = context.bundle.clone();
    Callback::from(move |value: String| {
        update_tracker_field(&draft, &field_key, &bundle, |map| {
            set_optional_string(map, key, &value);
            None
        });
    })
}

fn tracker_numeric_callback(
    context: &TrackerContext,
    key: &'static str,
    error_message: String,
) -> Callback<String> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    let bundle = context.bundle.clone();
    Callback::from(move |value: String| {
        update_tracker_field(
            &draft,
            &field_key,
            &bundle,
            |map| match apply_optional_numeric(&value, NumericKind::Integer) {
                Ok(Some(number)) => {
                    map.insert(key.to_string(), number);
                    None
                }
                Ok(None) => {
                    map.remove(key);
                    None
                }
                Err(_) => Some(error_message.clone()),
            },
        );
    })
}

fn tracker_nested_toggle_callback(
    context: &TrackerContext,
    parent: &'static str,
) -> Callback<bool> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    let bundle = context.bundle.clone();
    Callback::from(move |value: bool| {
        update_tracker_field(&draft, &field_key, &bundle, |map| {
            if value {
                map.insert(parent.to_string(), Value::Object(Map::new()));
            } else {
                map.remove(parent);
            }
            None
        });
    })
}

fn tracker_nested_string_callback(
    context: &TrackerContext,
    parent: &'static str,
    key: &'static str,
) -> Callback<String> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    let bundle = context.bundle.clone();
    Callback::from(move |value: String| {
        update_tracker_field(&draft, &field_key, &bundle, |map| {
            let mut nested = map
                .get(parent)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            set_optional_string(&mut nested, key, &value);
            map.insert(parent.to_string(), Value::Object(nested));
            None
        });
    })
}

fn tracker_nested_bool_callback(
    context: &TrackerContext,
    parent: &'static str,
    key: &'static str,
) -> Callback<bool> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    let bundle = context.bundle.clone();
    Callback::from(move |value: bool| {
        update_tracker_field(&draft, &field_key, &bundle, |map| {
            let mut nested = map
                .get(parent)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            nested.insert(key.to_string(), Value::Bool(value));
            map.insert(parent.to_string(), Value::Object(nested));
            None
        });
    })
}

fn tracker_nested_numeric_callback(
    context: &TrackerContext,
    parent: &'static str,
    key: &'static str,
    error_message: String,
) -> Callback<String> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    let bundle = context.bundle.clone();
    Callback::from(move |value: String| {
        update_tracker_field(&draft, &field_key, &bundle, |map| {
            let mut nested = map
                .get(parent)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            match apply_optional_numeric(&value, NumericKind::Integer) {
                Ok(Some(number)) => {
                    nested.insert(key.to_string(), number);
                    map.insert(parent.to_string(), Value::Object(nested));
                    None
                }
                Ok(None) => {
                    nested.remove(key);
                    map.insert(parent.to_string(), Value::Object(nested));
                    None
                }
                Err(_) => Some(error_message.clone()),
            }
        });
    })
}

fn tracker_nested_select_callback(
    context: &TrackerContext,
    parent: &'static str,
    key: &'static str,
) -> Callback<AttrValue> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    let bundle = context.bundle.clone();
    Callback::from(move |value: AttrValue| {
        let value = value.to_string();
        update_tracker_field(&draft, &field_key, &bundle, |map| {
            let mut nested = map
                .get(parent)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            set_optional_string(&mut nested, key, &value);
            map.insert(parent.to_string(), Value::Object(nested));
            None
        });
    })
}

fn tracker_proxy_kind_options(bundle: &TranslationBundle) -> Vec<(AttrValue, AttrValue)> {
    vec![
        ("http", "settings.tracker.proxy_http"),
        ("socks5", "settings.tracker.proxy_socks5"),
    ]
    .into_iter()
    .map(|(value, label)| (AttrValue::from(value), AttrValue::from(bundle.text(label))))
    .collect::<Vec<_>>()
}

fn render_tracker_field(
    field_key: String,
    draft: UseStateHandle<SettingsDraft>,
    bundle: &TranslationBundle,
) -> Html {
    html! {
        <TrackerField field_key={field_key} draft={draft} bundle={bundle.clone()} />
    }
}

#[derive(Properties, PartialEq)]
struct IpFilterFieldProps {
    field_key: String,
    draft: UseStateHandle<SettingsDraft>,
    bundle: TranslationBundle,
    on_copy_value: Callback<String>,
}

#[function_component(IpFilterField)]
fn ip_filter_field(props: &IpFilterFieldProps) -> Html {
    let cidr_input = use_state(String::new);
    let map = field_object_value(&props.draft, &props.field_key);
    let values = ip_filter_values(&map);

    let context = IpFilterContext {
        draft: props.draft.clone(),
        field_key: props.field_key.clone(),
        bundle: props.bundle.clone(),
        on_copy_value: props.on_copy_value.clone(),
    };

    let on_cidr_input = ip_filter_input_callback(cidr_input.clone());
    let on_add_cidr = ip_filter_add_callback(&context, cidr_input.clone());
    let on_blocklist = ip_filter_blocklist_callback(&context);

    html! {
        <div class="form-control w-full lg:col-span-2 space-y-3">
            <label class="label pb-1">
                <span class="label-text text-xs">{context.bundle.text("settings.fields.engine_profile.ip_filter")}</span>
            </label>
            {render_ip_filter_inputs(&context, &values, cidr_input, on_cidr_input, on_add_cidr, on_blocklist)}
            {render_ip_filter_cidrs(&context, &values)}
            {render_ip_filter_meta(&context, &values)}
        </div>
    }
}

#[derive(Clone)]
struct IpFilterContext {
    draft: UseStateHandle<SettingsDraft>,
    field_key: String,
    bundle: TranslationBundle,
    on_copy_value: Callback<String>,
}

struct IpFilterValues {
    cidrs: Vec<String>,
    blocklist_url: String,
    etag: String,
    last_updated: String,
    last_error: String,
}

fn ip_filter_values(map: &Map<String, Value>) -> IpFilterValues {
    IpFilterValues {
        cidrs: map_array_strings(map, "cidrs"),
        blocklist_url: map_string(map, "blocklist_url"),
        etag: map_string(map, "etag"),
        last_updated: map_string(map, "last_updated_at"),
        last_error: map_string(map, "last_error"),
    }
}

fn ip_filter_input_callback(state: UseStateHandle<String>) -> Callback<String> {
    Callback::from(move |value: String| state.set(value))
}

fn ip_filter_add_callback(
    context: &IpFilterContext,
    state: UseStateHandle<String>,
) -> Callback<MouseEvent> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    Callback::from(move |_| {
        let value = state.trim().to_string();
        if value.is_empty() {
            return;
        }
        update_object_field(&draft, &field_key, |map| {
            let mut list = map_array_strings(map, "cidrs");
            if !list.iter().any(|entry| entry == &value) {
                list.push(value.clone());
            }
            map.insert(
                "cidrs".to_string(),
                Value::Array(list.into_iter().map(Value::String).collect()),
            );
        });
        state.set(String::new());
    })
}

fn ip_filter_blocklist_callback(context: &IpFilterContext) -> Callback<String> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    Callback::from(move |value: String| {
        update_object_field(&draft, &field_key, |map| {
            set_optional_string(map, "blocklist_url", &value);
        });
    })
}

fn render_ip_filter_inputs(
    context: &IpFilterContext,
    values: &IpFilterValues,
    cidr_input: UseStateHandle<String>,
    on_cidr_input: Callback<String>,
    on_add_cidr: Callback<MouseEvent>,
    on_blocklist: Callback<String>,
) -> Html {
    html! {
        <div class="grid gap-3 sm:grid-cols-2">
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{context.bundle.text("settings.ip_filter.blocklist_url")}</span>
                </label>
                <Input
                    value={AttrValue::from(values.blocklist_url.clone())}
                    class="w-full"
                    oninput={on_blocklist}
                />
            </div>
            <div class="form-control w-full">
                <label class="label pb-1">
                    <span class="label-text text-xs">{context.bundle.text("settings.ip_filter.inline_cidrs")}</span>
                </label>
                <div class="flex gap-2">
                    <Input
                        value={AttrValue::from((*cidr_input).clone())}
                        placeholder={Some(AttrValue::from(context.bundle.text("settings.ip_filter.cidr_placeholder")))}
                        class="w-full"
                        oninput={on_cidr_input}
                    />
                    <button class="btn btn-outline btn-xs" onclick={on_add_cidr}>
                        {context.bundle.text("settings.add")}
                    </button>
                </div>
            </div>
        </div>
    }
}

fn render_ip_filter_cidrs(context: &IpFilterContext, values: &IpFilterValues) -> Html {
    html! {
        <div class="space-y-2">
            {for values.cidrs.iter().map(|entry| {
                let on_remove = ip_filter_remove_callback(context, entry.clone());
                html! {
                    <div class="flex items-center justify-between gap-2 rounded-box border border-base-200 bg-base-200/30 px-2 py-1">
                        <span class="text-xs font-mono break-all">{entry.clone()}</span>
                        <button class="btn btn-ghost btn-xs" onclick={on_remove}>
                            {context.bundle.text("settings.remove")}
                        </button>
                    </div>
                }
            })}
            {if values.cidrs.is_empty() {
                html! {
                    <div class="rounded-box border border-base-200 bg-base-200/30 px-2 py-2 text-xs text-base-content/60">
                        {context.bundle.text("settings.ip_filter.empty")}
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}

fn render_ip_filter_meta(context: &IpFilterContext, values: &IpFilterValues) -> Html {
    html! {
        <div class="grid gap-3 sm:grid-cols-2">
            {if !values.etag.is_empty() {
                render_readonly_field(
                    context.bundle.text("settings.ip_filter.etag"),
                    values.etag.clone(),
                    context.on_copy_value.clone(),
                    &context.bundle,
                )
            } else {
                html! {}
            }}
            {if !values.last_updated.is_empty() {
                render_readonly_field(
                    context.bundle.text("settings.ip_filter.last_updated"),
                    values.last_updated.clone(),
                    context.on_copy_value.clone(),
                    &context.bundle,
                )
            } else {
                html! {}
            }}
            {if !values.last_error.is_empty() {
                render_readonly_field(
                    context.bundle.text("settings.ip_filter.last_error"),
                    values.last_error.clone(),
                    context.on_copy_value.clone(),
                    &context.bundle,
                )
            } else {
                html! {}
            }}
        </div>
    }
}

fn ip_filter_remove_callback(context: &IpFilterContext, entry: String) -> Callback<MouseEvent> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    Callback::from(move |_| {
        update_object_field(&draft, &field_key, |map| {
            let mut list = map_array_strings(map, "cidrs");
            list.retain(|item| item != &entry);
            map.insert(
                "cidrs".to_string(),
                Value::Array(list.into_iter().map(Value::String).collect()),
            );
        });
    })
}

fn render_ip_filter_field(
    field_key: String,
    draft: UseStateHandle<SettingsDraft>,
    bundle: &TranslationBundle,
    on_copy_value: Callback<String>,
) -> Html {
    html! {
        <IpFilterField
            field_key={field_key}
            draft={draft}
            bundle={bundle.clone()}
            on_copy_value={on_copy_value}
        />
    }
}

fn render_peer_classes_field(
    field_key: String,
    draft: UseStateHandle<SettingsDraft>,
    bundle: &TranslationBundle,
) -> Html {
    let map = field_object_value(&draft, &field_key);
    let mut entries = peer_classes_from_value(&map);
    entries.sort_by_key(|entry| entry.id);

    let context = PeerClassContext {
        draft,
        field_key,
        bundle: bundle.clone(),
    };
    let on_add = peer_class_add_callback(&context);

    html! {
        <div class="form-control w-full lg:col-span-2 space-y-3">
            <label class="label pb-1">
                <span class="label-text text-xs">{context.bundle.text("settings.fields.engine_profile.peer_classes")}</span>
            </label>
            <div class="flex justify-between items-center">
                <span class="text-xs text-base-content/60">{context.bundle.text("settings.peer_classes.subtitle")}</span>
                <button class="btn btn-outline btn-xs" onclick={on_add}>
                    {context.bundle.text("settings.peer_classes.add")}
                </button>
            </div>
            {if entries.is_empty() {
                html! {
                    <div class="rounded-box border border-base-200 bg-base-200/30 px-2 py-2 text-xs text-base-content/60">
                        {context.bundle.text("settings.peer_classes.empty")}
                    </div>
                }
            } else {
                html! {
                    <div class="space-y-3">
                        {for entries.into_iter().map(|entry| render_peer_class_entry(entry, &context))}
                    </div>
                }
            }}
        </div>
    }
}

#[derive(Clone)]
struct PeerClassContext {
    draft: UseStateHandle<SettingsDraft>,
    field_key: String,
    bundle: TranslationBundle,
}

fn peer_class_add_callback(context: &PeerClassContext) -> Callback<MouseEvent> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    Callback::from(move |_| {
        update_peer_classes(&draft, &field_key, |classes| {
            if let Some(id) = next_peer_class_id(classes) {
                classes.push(PeerClassEntry {
                    id,
                    label: format!("class_{id}"),
                    download_priority: 1,
                    upload_priority: 1,
                    connection_limit_factor: 100,
                    ignore_unchoke_slots: false,
                    is_default: false,
                });
            }
        });
    })
}

fn render_peer_class_entry(entry: PeerClassEntry, context: &PeerClassContext) -> Html {
    let id_value = entry.id.to_string();
    let label_value = entry.label.clone();
    let download_priority = entry.download_priority.to_string();
    let upload_priority = entry.upload_priority.to_string();
    let connection_factor = entry.connection_limit_factor.to_string();
    let ignore_unchoke = entry.ignore_unchoke_slots;
    let is_default = entry.is_default;

    let on_remove = peer_class_remove_callback(context, entry.id);
    let on_id = peer_class_id_callback(context, entry.id);
    let on_label = peer_class_label_callback(context, entry.id);
    let on_download_priority = peer_class_download_priority_callback(context, entry.id);
    let on_upload_priority = peer_class_upload_priority_callback(context, entry.id);
    let on_connection_factor = peer_class_connection_factor_callback(context, entry.id);
    let on_ignore = peer_class_ignore_callback(context, entry.id);
    let on_default = peer_class_default_callback(context, entry.id);

    html! {
        <div class="rounded-box border border-base-200 bg-base-200/20 p-3 space-y-3">
            <div class="flex items-center justify-between">
                <p class="text-sm font-semibold">{format!("{} {}", context.bundle.text("settings.peer_classes.class"), entry.id)}</p>
                <button class="btn btn-ghost btn-xs" onclick={on_remove}>
                    {context.bundle.text("settings.remove")}
                </button>
            </div>
            <div class="grid gap-3 sm:grid-cols-3">
                <div class="form-control w-full">
                    <label class="label pb-1">
                        <span class="label-text text-xs">{context.bundle.text("settings.peer_classes.id")}</span>
                    </label>
                    <Input
                        value={AttrValue::from(id_value)}
                        input_type={Some(AttrValue::from("number"))}
                        class="w-full"
                        oninput={on_id}
                    />
                </div>
                <div class="form-control w-full sm:col-span-2">
                    <label class="label pb-1">
                        <span class="label-text text-xs">{context.bundle.text("settings.peer_classes.label")}</span>
                    </label>
                    <Input
                        value={AttrValue::from(label_value)}
                        class="w-full"
                        oninput={on_label}
                    />
                </div>
                <div class="form-control w-full">
                    <label class="label pb-1">
                        <span class="label-text text-xs">{context.bundle.text("settings.peer_classes.download_priority")}</span>
                    </label>
                    <Input
                        value={AttrValue::from(download_priority)}
                        input_type={Some(AttrValue::from("number"))}
                        class="w-full"
                        oninput={on_download_priority}
                    />
                </div>
                <div class="form-control w-full">
                    <label class="label pb-1">
                        <span class="label-text text-xs">{context.bundle.text("settings.peer_classes.upload_priority")}</span>
                    </label>
                    <Input
                        value={AttrValue::from(upload_priority)}
                        input_type={Some(AttrValue::from("number"))}
                        class="w-full"
                        oninput={on_upload_priority}
                    />
                </div>
                <div class="form-control w-full">
                    <label class="label pb-1">
                        <span class="label-text text-xs">{context.bundle.text("settings.peer_classes.connection_factor")}</span>
                    </label>
                    <Input
                        value={AttrValue::from(connection_factor)}
                        input_type={Some(AttrValue::from("number"))}
                        class="w-full"
                        oninput={on_connection_factor}
                    />
                </div>
                <Toggle
                    label={Some(AttrValue::from(context.bundle.text("settings.peer_classes.ignore_unchoke")))}
                    checked={ignore_unchoke}
                    onchange={on_ignore}
                />
                <Toggle
                    label={Some(AttrValue::from(context.bundle.text("settings.peer_classes.default")))}
                    checked={is_default}
                    onchange={on_default}
                />
            </div>
        </div>
    }
}

fn peer_class_remove_callback(context: &PeerClassContext, id: u8) -> Callback<MouseEvent> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    Callback::from(move |_| {
        update_peer_classes(&draft, &field_key, |classes| {
            classes.retain(|class| class.id != id);
        });
    })
}

fn peer_class_id_callback(context: &PeerClassContext, id: u8) -> Callback<String> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    Callback::from(move |value: String| {
        update_peer_classes(&draft, &field_key, |classes| {
            if let Some(class) = classes.iter_mut().find(|class| class.id == id) {
                if let Ok(parsed) = value.trim().parse::<u8>() {
                    if parsed <= 31 {
                        class.id = parsed;
                    }
                }
            }
        });
    })
}

fn peer_class_label_callback(context: &PeerClassContext, id: u8) -> Callback<String> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    Callback::from(move |value: String| {
        update_peer_classes(&draft, &field_key, |classes| {
            if let Some(class) = classes.iter_mut().find(|class| class.id == id) {
                class.label = value;
            }
        });
    })
}

fn peer_class_download_priority_callback(context: &PeerClassContext, id: u8) -> Callback<String> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    Callback::from(move |value: String| {
        update_peer_classes(&draft, &field_key, |classes| {
            if let Some(class) = classes.iter_mut().find(|class| class.id == id) {
                if let Ok(parsed) = value.trim().parse::<u16>() {
                    class.download_priority = parsed.max(1);
                }
            }
        });
    })
}

fn peer_class_upload_priority_callback(context: &PeerClassContext, id: u8) -> Callback<String> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    Callback::from(move |value: String| {
        update_peer_classes(&draft, &field_key, |classes| {
            if let Some(class) = classes.iter_mut().find(|class| class.id == id) {
                if let Ok(parsed) = value.trim().parse::<u16>() {
                    class.upload_priority = parsed.max(1);
                }
            }
        });
    })
}

fn peer_class_connection_factor_callback(context: &PeerClassContext, id: u8) -> Callback<String> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    Callback::from(move |value: String| {
        update_peer_classes(&draft, &field_key, |classes| {
            if let Some(class) = classes.iter_mut().find(|class| class.id == id) {
                if let Ok(parsed) = value.trim().parse::<u16>() {
                    class.connection_limit_factor = parsed.max(1);
                }
            }
        });
    })
}

fn peer_class_ignore_callback(context: &PeerClassContext, id: u8) -> Callback<bool> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    Callback::from(move |value: bool| {
        update_peer_classes(&draft, &field_key, |classes| {
            if let Some(class) = classes.iter_mut().find(|class| class.id == id) {
                class.ignore_unchoke_slots = value;
            }
        });
    })
}

fn peer_class_default_callback(context: &PeerClassContext, id: u8) -> Callback<bool> {
    let draft = context.draft.clone();
    let field_key = context.field_key.clone();
    Callback::from(move |value: bool| {
        update_peer_classes(&draft, &field_key, |classes| {
            if let Some(class) = classes.iter_mut().find(|class| class.id == id) {
                class.is_default = value;
            }
        });
    })
}

fn render_path_browser(
    bundle: &TranslationBundle,
    state: &UseStateHandle<PathBrowserState>,
    callbacks: PathBrowserCallbacks,
) -> Html {
    let disabled = state.input.trim().is_empty() || state.busy;
    let on_close_click = emit_callback(callbacks.on_close.clone());
    let on_confirm_click = emit_callback(callbacks.on_confirm.clone());
    let on_go = emit_callback(callbacks.on_go.clone());
    let entries = state.entries.clone();
    let has_parent = state.parent.is_some();

    html! {
        <Modal open={state.open} on_close={callbacks.on_close.clone()}>
            <div class="space-y-4">
                <div>
                    <h3 class="text-lg font-semibold">{bundle.text("settings.path_picker_title")}</h3>
                    <p class="text-sm text-base-content/60">
                        {bundle.text("settings.path_picker_body")}
                    </p>
                </div>
                <div class="flex flex-wrap items-center gap-2">
                    <button
                        class="btn btn-outline btn-xs"
                        onclick={emit_callback(callbacks.on_parent.clone())}
                        disabled={!has_parent || state.busy}
                    >
                        {bundle.text("settings.path_picker_up")}
                    </button>
                    <div class="form-control grow">
                        <Input
                            value={AttrValue::from(state.input.clone())}
                            placeholder={Some(AttrValue::from(bundle.text("settings.path_picker_placeholder")))}
                            class="w-full"
                            oninput={callbacks.on_input.clone()}
                        />
                    </div>
                    <button
                        class="btn btn-outline btn-xs"
                        onclick={on_go}
                        disabled={state.busy}
                    >
                        {bundle.text("settings.path_picker_go")}
                    </button>
                </div>
                {state.error.clone().map(|message| html! {
                    <div role="alert" class="alert alert-error">
                        <span>{message}</span>
                    </div>
                }).unwrap_or_default()}
                <div class="rounded-box border border-base-200 bg-base-200/40 p-2">
                    {if state.busy {
                        html! { <p class="text-xs text-base-content/60">{bundle.text("settings.path_picker_loading")}</p> }
                    } else if entries.is_empty() {
                        html! { <p class="text-xs text-base-content/60">{bundle.text("settings.path_picker_empty")}</p> }
                    } else {
                        html! {
                            <ul class="menu menu-sm">
                                {for entries.into_iter().map(|entry| {
                                    let path = entry.path.clone();
                                    let is_dir = matches!(entry.kind, FsEntryKind::Directory | FsEntryKind::Symlink);
                                    let icon = if is_dir { "lucide--folder" } else { "lucide--file" };
                                    let on_click = {
                                        let callback = callbacks.on_navigate.clone();
                                        Callback::from(move |_| {
                                            if is_dir {
                                                callback.emit(path.clone());
                                            }
                                        })
                                    };
                                    html! {
                                        <li>
                                            <button class={classes!((!is_dir).then_some("text-base-content/40"))} onclick={on_click}>
                                                <span class={classes!("iconify", icon, "size-4")}></span>
                                                <span class="truncate">{entry.name.clone()}</span>
                                            </button>
                                        </li>
                                    }
                                })}
                            </ul>
                        }
                    }}
                </div>
                <div class="flex justify-end gap-2">
                    <button class="btn btn-ghost btn-sm" onclick={on_close_click}>
                        {bundle.text("settings.path_picker_cancel")}
                    </button>
                    <button
                        class="btn btn-primary btn-sm"
                        onclick={on_confirm_click}
                        disabled={disabled}
                    >
                        {bundle.text("settings.path_picker_confirm")}
                    </button>
                </div>
            </div>
        </Modal>
    }
}

fn render_config_placeholder(bundle: &TranslationBundle, busy: bool) -> Html {
    html! {
        <div class="space-y-4">
            <div class="card bg-base-100 shadow">
                <div class="card-body gap-3">
                    <div class="flex items-center justify-between">
                        <p class="text-sm font-semibold">{bundle.text("settings.engine_title")}</p>
                        {if busy {
                            html! { <span class="text-xs text-base-content/60">{bundle.text("settings.refreshing")}</span> }
                        } else { html! {} }}
                    </div>
                    <div class="rounded-box border border-base-200 p-4 text-sm text-base-content/60">
                        {bundle.text("settings.engine_empty")}
                    </div>
                </div>
            </div>
        </div>
    }
}

fn settings_status(
    snapshot: Option<&Value>,
    draft: &SettingsDraft,
    immutable_keys: &HashSet<String>,
) -> SettingsStatus {
    let Some(snapshot) = snapshot else {
        return SettingsStatus::default();
    };
    let mut status = SettingsStatus::default();
    for section in SettingsSection::all() {
        let Some(section_value) = snapshot.get(section.key()) else {
            continue;
        };
        let Some(map) = section_value.as_object() else {
            continue;
        };
        for (key, original) in map {
            let field_key = format!("{}.{}", section.key(), key);
            if let Some(state) = draft.fields.get(&field_key) {
                if state.error.is_some() {
                    status.has_errors = true;
                }
                if is_field_read_only(section, key, immutable_keys) {
                    continue;
                }
                if original != &state.value {
                    status.dirty_count = status.dirty_count.saturating_add(1);
                }
            }
        }
    }
    status
}

fn build_changeset_from_snapshot(
    snapshot: &Value,
    draft: &SettingsDraft,
    immutable_keys: &HashSet<String>,
) -> Option<Value> {
    let mut app_patch = None;
    let mut engine_patch = None;
    let mut fs_patch = None;
    for section in SettingsSection::all() {
        let Some(section_value) = snapshot.get(section.key()) else {
            continue;
        };
        let Some(map) = section_value.as_object() else {
            continue;
        };
        let mut updated_map = map.clone();
        let mut dirty = false;
        for (key, original) in map {
            if is_field_read_only(section, key, immutable_keys) {
                continue;
            }
            let field_key = format!("{}.{}", section.key(), key);
            let Some(current) = draft.fields.get(&field_key) else {
                continue;
            };
            if original == &current.value {
                continue;
            }
            updated_map.insert(key.clone(), current.value.clone());
            dirty = true;
        }
        if !dirty {
            continue;
        }
        match section {
            SettingsSection::AppProfile => app_patch = Some(updated_map),
            SettingsSection::EngineProfile => engine_patch = Some(updated_map),
            SettingsSection::FsPolicy => fs_patch = Some(updated_map),
        }
    }

    if app_patch.is_none() && engine_patch.is_none() && fs_patch.is_none() {
        return None;
    }

    let mut root = Map::new();
    if let Some(app_patch) = app_patch {
        root.insert("app_profile".to_string(), Value::Object(app_patch));
    }
    if let Some(engine_patch) = engine_patch {
        root.insert("engine_profile".to_string(), Value::Object(engine_patch));
    }
    if let Some(fs_patch) = fs_patch {
        root.insert("fs_policy".to_string(), Value::Object(fs_patch));
    }

    Some(Value::Object(root))
}

fn build_settings_draft(snapshot: &Value) -> SettingsDraft {
    let mut fields = BTreeMap::new();
    for section in SettingsSection::all() {
        let Some(section_value) = snapshot.get(section.key()) else {
            continue;
        };
        let Some(map) = section_value.as_object() else {
            continue;
        };
        for (key, value) in map {
            let field_key = format!("{}.{}", section.key(), key);
            fields.insert(
                field_key,
                FieldDraft {
                    value: value.clone(),
                    raw: value_to_raw(value),
                    error: None,
                },
            );
        }
    }
    SettingsDraft { fields }
}

fn collect_section_fields(
    snapshot: Option<&Value>,
    section: SettingsSection,
) -> Vec<SettingsField> {
    let Some(snapshot) = snapshot else {
        return Vec::new();
    };
    let Some(section_value) = snapshot.get(section.key()) else {
        return Vec::new();
    };
    let Some(map) = section_value.as_object() else {
        return Vec::new();
    };
    let mut fields = map
        .keys()
        .map(|key| SettingsField {
            section,
            key: key.clone(),
        })
        .collect::<Vec<_>>();
    fields.sort_by(|a, b| a.key.cmp(&b.key));
    fields
}

fn split_engine_fields(fields: Vec<SettingsField>) -> EngineGroups {
    let (downloads, remaining) = split_fields(fields, DOWNLOAD_FIELDS);
    let (seeding, remaining) = split_fields(remaining, SEEDING_FIELDS);
    let (network, remaining) = split_fields(remaining, NETWORK_FIELDS);
    let (storage, remaining) = split_fields(remaining, STORAGE_FIELDS);

    EngineGroups {
        downloads,
        seeding,
        network,
        storage,
        advanced: remaining,
    }
}

struct EngineGroups {
    downloads: Vec<SettingsField>,
    seeding: Vec<SettingsField>,
    network: Vec<SettingsField>,
    storage: Vec<SettingsField>,
    advanced: Vec<SettingsField>,
}

struct AppGroups {
    info: Vec<SettingsField>,
    telemetry: Vec<SettingsField>,
    labels: Vec<SettingsField>,
    other: Vec<SettingsField>,
}

const APP_INFO_FIELDS: &[&str] = &[
    "id",
    "instance_name",
    "mode",
    "auth_mode",
    "version",
    "http_port",
    "bind_addr",
    "immutable_keys",
];

const APP_TELEMETRY_FIELDS: &[&str] = &["telemetry"];

const APP_LABEL_FIELDS: &[&str] = &["label_policies"];

const LABEL_POLICIES_FIELD_KEY: &str = "app_profile.label_policies";

const DOWNLOAD_FIELDS: &[&str] = &[
    "download_root",
    "resume_dir",
    "sequential_default",
    "auto_managed",
    "max_active",
    "max_download_bps",
    "max_upload_bps",
    "connections_limit",
    "connections_limit_per_torrent",
    "unchoke_slots",
    "half_open_limit",
    "stats_interval_ms",
    "alt_speed",
];

const SEEDING_FIELDS: &[&str] = &[
    "seed_ratio_limit",
    "seed_time_limit",
    "auto_manage_prefer_seeds",
    "dont_count_slow_torrents",
    "super_seeding",
    "strict_super_seeding",
    "choking_algorithm",
    "seed_choking_algorithm",
    "optimistic_unchoke_slots",
];

const NETWORK_FIELDS: &[&str] = &[
    "listen_port",
    "listen_interfaces",
    "ipv6_mode",
    "dht",
    "dht_bootstrap_nodes",
    "dht_router_nodes",
    "encryption",
    "enable_lsd",
    "enable_upnp",
    "enable_natpmp",
    "enable_pex",
    "enable_outgoing_utp",
    "enable_incoming_utp",
    "force_proxy",
    "prefer_rc4",
    "allow_multiple_connections_per_ip",
    "outgoing_port_min",
    "outgoing_port_max",
    "peer_dscp",
    "tracker",
    "ip_filter",
    "peer_classes",
];

const STORAGE_FIELDS: &[&str] = &[
    "storage_mode",
    "use_partfile",
    "disk_read_mode",
    "disk_write_mode",
    "verify_piece_hashes",
    "cache_size",
    "cache_expiry",
    "coalesce_reads",
    "coalesce_writes",
    "use_disk_cache_pool",
    "max_queued_disk_bytes",
];

const WEEKDAYS: [(&str, &str); 7] = [
    ("mon", "settings.weekday.mon"),
    ("tue", "settings.weekday.tue"),
    ("wed", "settings.weekday.wed"),
    ("thu", "settings.weekday.thu"),
    ("fri", "settings.weekday.fri"),
    ("sat", "settings.weekday.sat"),
    ("sun", "settings.weekday.sun"),
];

fn split_fields(
    fields: Vec<SettingsField>,
    names: &[&str],
) -> (Vec<SettingsField>, Vec<SettingsField>) {
    let name_set = names.iter().copied().collect::<HashSet<_>>();
    let mut selected = Vec::new();
    let mut remaining = Vec::new();
    for field in fields {
        if name_set.contains(field.key.as_str()) {
            selected.push(field);
        } else {
            remaining.push(field);
        }
    }
    (selected, remaining)
}

fn split_app_fields(fields: Vec<SettingsField>) -> AppGroups {
    let (labels, remaining) = split_fields(fields, APP_LABEL_FIELDS);
    let (telemetry, remaining) = split_fields(remaining, APP_TELEMETRY_FIELDS);
    let (info, remaining) = split_fields(remaining, APP_INFO_FIELDS);
    AppGroups {
        info,
        telemetry,
        labels,
        other: remaining,
    }
}

fn immutable_key_set(snapshot: Option<&Value>) -> HashSet<String> {
    let mut keys = HashSet::new();
    let Some(snapshot) = snapshot else {
        return keys;
    };
    let Some(app) = snapshot.get(SettingsSection::AppProfile.key()) else {
        return keys;
    };
    let Some(value) = app.get("immutable_keys") else {
        return keys;
    };
    let Some(entries) = value.as_array() else {
        return keys;
    };
    for entry in entries {
        if let Some(item) = entry.as_str() {
            keys.insert(item.to_string());
        }
    }
    keys
}

fn update_field(
    draft: &UseStateHandle<SettingsDraft>,
    field_key: &str,
    update: impl FnOnce(&mut FieldDraft),
) {
    let mut next = (**draft).clone();
    if let Some(field) = next.fields.get_mut(field_key) {
        update(field);
        draft.set(next);
    }
}

fn update_browser_state(
    browser: &UseStateHandle<PathBrowserState>,
    update: impl FnOnce(&mut PathBrowserState),
) {
    let mut next = (**browser).clone();
    update(&mut next);
    browser.set(next);
}

fn field_object_value(
    draft: &UseStateHandle<SettingsDraft>,
    field_key: &str,
) -> Map<String, Value> {
    draft
        .fields
        .get(field_key)
        .and_then(|field| field.value.as_object().cloned())
        .unwrap_or_default()
}

fn map_string(map: &Map<String, Value>, key: &str) -> String {
    map.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn map_bool(map: &Map<String, Value>, key: &str) -> bool {
    map.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn map_array_strings(map: &Map<String, Value>, key: &str) -> Vec<String> {
    map.get(key).map(value_array_as_strings).unwrap_or_default()
}

fn set_optional_string(map: &mut Map<String, Value>, key: &str, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        map.remove(key);
    } else {
        map.insert(key.to_string(), Value::String(trimmed.to_string()));
    }
}

fn update_object_field(
    draft: &UseStateHandle<SettingsDraft>,
    field_key: &str,
    update: impl FnOnce(&mut Map<String, Value>),
) {
    update_field(draft, field_key, |field| {
        let mut map = field.value.as_object().cloned().unwrap_or_default();
        update(&mut map);
        field.value = Value::Object(map);
        field.error = None;
        field.raw = String::new();
    });
}

fn update_object_field_with_error(
    draft: &UseStateHandle<SettingsDraft>,
    field_key: &str,
    update: impl FnOnce(&mut Map<String, Value>) -> Option<String>,
) {
    update_field(draft, field_key, |field| {
        let mut map = field.value.as_object().cloned().unwrap_or_default();
        let error = update(&mut map);
        field.value = Value::Object(map);
        field.error = error;
        field.raw = String::new();
    });
}

fn ordered_weekdays(days: &[String]) -> Vec<String> {
    WEEKDAYS
        .iter()
        .filter_map(|(day, _)| {
            days.iter()
                .any(|entry| entry == day)
                .then(|| (*day).to_string())
        })
        .collect()
}

fn apply_optional_numeric(raw: &str, kind: NumericKind) -> Result<Option<Value>, NumericError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    parse_numeric(kind, trimmed).map(Some)
}

fn label_policy_entries(value: &Value, kind: LabelKind) -> Vec<(String, Map<String, Value>)> {
    let mut entries = Vec::new();
    let Some(list) = value.as_array() else {
        return entries;
    };
    for entry in list {
        let Some(map) = entry.as_object() else {
            continue;
        };
        let entry_kind = map
            .get("kind")
            .and_then(Value::as_str)
            .and_then(LabelKind::from_str);
        let Some(name) = map.get("name").and_then(Value::as_str) else {
            continue;
        };
        if entry_kind == Some(kind) {
            entries.push((name.to_string(), map.clone()));
        }
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

fn insert_label_policy_entry(
    draft: &UseStateHandle<SettingsDraft>,
    field_key: &str,
    kind: LabelKind,
    name: &str,
) {
    update_label_policy_entry(draft, field_key, kind, name, |_| {});
}

fn remove_label_policy_entry(
    draft: &UseStateHandle<SettingsDraft>,
    field_key: &str,
    kind: LabelKind,
    name: &str,
) {
    update_field(draft, field_key, |field| {
        let mut entries = field.value.as_array().cloned().unwrap_or_default();
        entries.retain(|entry| !label_policy_matches(entry, kind, name));
        field.value = Value::Array(entries);
        field.error = None;
        field.raw = String::new();
    });
}

fn update_label_policy_entry(
    draft: &UseStateHandle<SettingsDraft>,
    field_key: &str,
    kind: LabelKind,
    name: &str,
    mut update: impl FnMut(&mut Map<String, Value>),
) {
    update_label_policy_entry_with_error(draft, field_key, kind, name, |policy| {
        update(policy);
        None
    });
}

fn update_label_policy_entry_with_error(
    draft: &UseStateHandle<SettingsDraft>,
    field_key: &str,
    kind: LabelKind,
    name: &str,
    mut update: impl FnMut(&mut Map<String, Value>) -> Option<String>,
) {
    update_field(draft, field_key, |field| {
        let mut entries = field.value.as_array().cloned().unwrap_or_default();
        let mut error = None;
        let mut matched = false;

        for entry in &mut entries {
            if !label_policy_matches(entry, kind, name) {
                continue;
            }
            let mut policy = entry.as_object().cloned().unwrap_or_default();
            error = update(&mut policy);
            normalize_label_policy_entry(kind, name, &mut policy);
            *entry = Value::Object(policy);
            matched = true;
            break;
        }

        if !matched {
            let mut policy = Map::new();
            error = update(&mut policy);
            normalize_label_policy_entry(kind, name, &mut policy);
            entries.push(Value::Object(policy));
        }

        field.value = Value::Array(entries);
        field.error = error;
        field.raw = String::new();
    });
}

fn label_policy_matches(entry: &Value, kind: LabelKind, name: &str) -> bool {
    let Some(map) = entry.as_object() else {
        return false;
    };
    let entry_kind = map
        .get("kind")
        .and_then(Value::as_str)
        .and_then(LabelKind::from_str);
    let entry_name = map.get("name").and_then(Value::as_str);
    entry_kind == Some(kind) && entry_name == Some(name)
}

fn normalize_label_policy_entry(kind: LabelKind, name: &str, policy: &mut Map<String, Value>) {
    policy.insert("kind".to_string(), Value::String(kind.key().to_string()));
    policy.insert("name".to_string(), Value::String(name.to_string()));
}

fn update_label_policy_string(
    draft: &UseStateHandle<SettingsDraft>,
    field_key: &str,
    kind: LabelKind,
    name: &str,
    key: &str,
    value: &str,
) {
    update_label_policy_entry(draft, field_key, kind, name, |policy| {
        set_optional_string(policy, key, value);
    });
}

fn update_label_policy_bool(
    draft: &UseStateHandle<SettingsDraft>,
    field_key: &str,
    kind: LabelKind,
    name: &str,
    key: &str,
    value: bool,
) {
    update_label_policy_entry(draft, field_key, kind, name, |policy| {
        policy.insert(key.to_string(), Value::Bool(value));
    });
}

fn update_label_policy_numeric(
    draft: &UseStateHandle<SettingsDraft>,
    field_key: &str,
    kind: LabelKind,
    name: &str,
    key: &str,
    raw: &str,
    kind_num: NumericKind,
    error_message: &str,
) {
    update_label_policy_entry_with_error(draft, field_key, kind, name, |policy| {
        match apply_optional_numeric(raw, kind_num) {
            Ok(Some(number)) => {
                policy.insert(key.to_string(), number);
                None
            }
            Ok(None) => {
                policy.remove(key);
                None
            }
            Err(_) => Some(error_message.to_string()),
        }
    });
}

fn label_policy_download_dir(
    draft: &UseStateHandle<SettingsDraft>,
    kind: LabelKind,
    name: &str,
) -> Option<String> {
    draft
        .fields
        .get(LABEL_POLICIES_FIELD_KEY)
        .and_then(|field| field.value.as_array())
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| label_policy_matches(entry, kind, name))
        })
        .and_then(Value::as_object)
        .and_then(|policy| policy.get("download_dir"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn update_label_policy_download_dir(
    draft: &UseStateHandle<SettingsDraft>,
    kind: LabelKind,
    name: &str,
    value: String,
) {
    update_label_policy_string(
        draft,
        LABEL_POLICIES_FIELD_KEY,
        kind,
        name,
        "download_dir",
        &value,
    );
}

fn update_tracker_field(
    draft: &UseStateHandle<SettingsDraft>,
    field_key: &str,
    bundle: &TranslationBundle,
    update: impl FnOnce(&mut Map<String, Value>) -> Option<String>,
) {
    update_field(draft, field_key, |field| {
        let mut map = field.value.as_object().cloned().unwrap_or_default();
        let error = update(&mut map).or_else(|| validate_tracker_map(&map, bundle));
        field.value = Value::Object(map);
        field.error = error;
        field.raw = String::new();
    });
}

fn validate_tracker_map(map: &Map<String, Value>, bundle: &TranslationBundle) -> Option<String> {
    if let Some(proxy_value) = map.get("proxy") {
        if proxy_value.is_null() {
            return None;
        }
        let Some(proxy) = proxy_value.as_object() else {
            return Some(bundle.text("settings.tracker.error_proxy"));
        };
        let host = proxy
            .get("host")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let port = proxy.get("port").and_then(Value::as_i64).unwrap_or(0);
        if host.is_empty() || !(1..=65_535).contains(&port) {
            return Some(bundle.text("settings.tracker.error_proxy"));
        }
    }

    if let Some(auth_value) = map.get("auth") {
        if auth_value.is_null() {
            return None;
        }
        let Some(auth) = auth_value.as_object() else {
            return Some(bundle.text("settings.tracker.error_auth"));
        };
        let has_secret = ["username_secret", "password_secret", "cookie_secret"]
            .iter()
            .any(|key| {
                auth.get(*key)
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty())
            });
        if !has_secret {
            return Some(bundle.text("settings.tracker.error_auth"));
        }
    }

    None
}

#[derive(Clone, PartialEq, Eq)]
struct PeerClassEntry {
    id: u8,
    label: String,
    download_priority: u16,
    upload_priority: u16,
    connection_limit_factor: u16,
    ignore_unchoke_slots: bool,
    is_default: bool,
}

fn peer_classes_from_value(map: &Map<String, Value>) -> Vec<PeerClassEntry> {
    let defaults = map
        .get("default")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(Value::as_u64)
                .filter_map(|value| u8::try_from(value).ok())
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let mut classes = Vec::new();
    if let Some(entries) = map.get("classes").and_then(Value::as_array) {
        for entry in entries {
            let Some(obj) = entry.as_object() else {
                continue;
            };
            let Some(id_value) = obj.get("id").and_then(Value::as_u64) else {
                continue;
            };
            let Ok(id) = u8::try_from(id_value) else {
                continue;
            };
            if id > 31 {
                continue;
            }
            let label = obj
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let download_priority = obj
                .get("download_priority")
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(1)
                .max(1);
            let upload_priority = obj
                .get("upload_priority")
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(1)
                .max(1);
            let connection_limit_factor = obj
                .get("connection_limit_factor")
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(100)
                .max(1);
            let ignore_unchoke_slots = obj
                .get("ignore_unchoke_slots")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let label = if label.is_empty() {
                format!("class_{id}")
            } else {
                label
            };
            classes.push(PeerClassEntry {
                id,
                label,
                download_priority,
                upload_priority,
                connection_limit_factor,
                ignore_unchoke_slots,
                is_default: defaults.contains(&id),
            });
        }
    }
    classes
}

fn update_peer_classes(
    draft: &UseStateHandle<SettingsDraft>,
    field_key: &str,
    update: impl FnOnce(&mut Vec<PeerClassEntry>),
) {
    update_field(draft, field_key, |field| {
        let mut map = field.value.as_object().cloned().unwrap_or_default();
        let mut classes = peer_classes_from_value(&map);
        update(&mut classes);
        classes.sort_by_key(|entry| entry.id);

        let class_values = classes
            .iter()
            .map(|entry| {
                json!({
                    "id": entry.id,
                    "label": entry.label,
                    "download_priority": entry.download_priority,
                    "upload_priority": entry.upload_priority,
                    "connection_limit_factor": entry.connection_limit_factor,
                    "ignore_unchoke_slots": entry.ignore_unchoke_slots
                })
            })
            .collect::<Vec<_>>();
        let defaults = classes
            .iter()
            .filter(|entry| entry.is_default)
            .map(|entry| Value::from(entry.id))
            .collect::<Vec<_>>();
        map.insert("classes".to_string(), Value::Array(class_values));
        map.insert("default".to_string(), Value::Array(defaults));
        field.value = Value::Object(map);
        field.error = None;
        field.raw = String::new();
    });
}

fn next_peer_class_id(classes: &[PeerClassEntry]) -> Option<u8> {
    (0..=31).find(|id| classes.iter().all(|entry| entry.id != *id))
}

fn parse_numeric(kind: NumericKind, value: &str) -> Result<Value, NumericError> {
    match kind {
        NumericKind::Integer => value
            .parse::<i64>()
            .map(Value::from)
            .map_err(|_| NumericError::Integer),
        NumericKind::Float => {
            let parsed = value.parse::<f64>().map_err(|_| NumericError::Float)?;
            serde_json::Number::from_f64(parsed)
                .map(Value::Number)
                .ok_or(NumericError::Float)
        }
    }
}

fn value_to_raw(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => String::new(),
    }
}

fn value_to_display(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(entries) => entries
            .iter()
            .map(value_to_display)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(map) => {
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort();
            keys.iter()
                .map(|key| {
                    let value = map.get(*key).map(value_to_display).unwrap_or_default();
                    format!("{key}: {value}")
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

fn value_array_as_strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn field_label(bundle: &TranslationBundle, section: SettingsSection, key: &str) -> String {
    let translation_key = format!("settings.fields.{}.{}", section.key(), key);
    let translated = bundle.text(&translation_key);
    if translated == translation_key {
        humanize_key(key)
    } else {
        translated
    }
}

fn humanize_key(key: &str) -> String {
    let mut out = String::new();
    for (idx, segment) in key.split('_').enumerate() {
        if segment.is_empty() {
            continue;
        }
        if idx > 0 {
            out.push(' ');
        }
        let mut chars = segment.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            for ch in chars {
                out.push(ch);
            }
        }
    }
    out
}

fn is_field_read_only(
    section: SettingsSection,
    key: &str,
    immutable_keys: &HashSet<String>,
) -> bool {
    if matches!(
        (section, key),
        (SettingsSection::AppProfile, "id")
            | (SettingsSection::AppProfile, "version")
            | (SettingsSection::AppProfile, "mode")
            | (SettingsSection::AppProfile, "bind_addr")
            | (SettingsSection::AppProfile, "http_port")
            | (SettingsSection::AppProfile, "immutable_keys")
            | (SettingsSection::EngineProfile, "id")
            | (SettingsSection::EngineProfile, "implementation")
            | (SettingsSection::FsPolicy, "id")
    ) {
        return true;
    }
    let scoped = format!("{}.{}", section.key(), key);
    let scoped_wildcard = format!("{}.*", section.key());
    immutable_keys.contains(section.key())
        || immutable_keys.contains(key)
        || immutable_keys.contains(&scoped)
        || immutable_keys.contains(&scoped_wildcard)
}

fn control_for_field(section: SettingsSection, key: &str, value: &Value) -> FieldControl {
    if matches!((section, key), (SettingsSection::AppProfile, "telemetry")) {
        return FieldControl::Telemetry;
    }
    if matches!(
        (section, key),
        (SettingsSection::AppProfile, "label_policies")
    ) {
        return FieldControl::LabelPolicies;
    }
    if matches!(
        (section, key),
        (SettingsSection::EngineProfile, "alt_speed")
    ) {
        return FieldControl::AltSpeed;
    }
    if matches!((section, key), (SettingsSection::EngineProfile, "tracker")) {
        return FieldControl::Tracker;
    }
    if matches!(
        (section, key),
        (SettingsSection::EngineProfile, "ip_filter")
    ) {
        return FieldControl::IpFilter;
    }
    if matches!(
        (section, key),
        (SettingsSection::EngineProfile, "peer_classes")
    ) {
        return FieldControl::PeerClasses;
    }
    if section == SettingsSection::FsPolicy && key == "allow_paths" {
        return FieldControl::PathList;
    }
    if is_directory_field(section, key) {
        return FieldControl::Path;
    }
    if let Some(options) = select_options(section, key, value) {
        return FieldControl::Select(options);
    }
    if let Some(kind) = numeric_kind(section, key) {
        return FieldControl::Number(kind);
    }
    if value.is_boolean() {
        return FieldControl::Toggle;
    }
    if value.is_array() {
        return FieldControl::StringList(
            string_list_options(section, key).unwrap_or(default_list_options()),
        );
    }
    FieldControl::Text
}

fn string_list_options(section: SettingsSection, key: &str) -> Option<StringListOptions> {
    let options = match (section, key) {
        (SettingsSection::EngineProfile, "listen_interfaces") => StringListOptions {
            placeholder_key: "settings.list.listen_interfaces_placeholder",
            add_label_key: "settings.add",
            empty_label_key: "settings.list.empty",
        },
        (SettingsSection::EngineProfile, "dht_bootstrap_nodes") => StringListOptions {
            placeholder_key: "settings.list.dht_bootstrap_placeholder",
            add_label_key: "settings.add",
            empty_label_key: "settings.list.empty",
        },
        (SettingsSection::EngineProfile, "dht_router_nodes") => StringListOptions {
            placeholder_key: "settings.list.dht_router_placeholder",
            add_label_key: "settings.add",
            empty_label_key: "settings.list.empty",
        },
        (SettingsSection::FsPolicy, "cleanup_keep") => StringListOptions {
            placeholder_key: "settings.list.cleanup_keep_placeholder",
            add_label_key: "settings.add",
            empty_label_key: "settings.list.empty",
        },
        (SettingsSection::FsPolicy, "cleanup_drop") => StringListOptions {
            placeholder_key: "settings.list.cleanup_drop_placeholder",
            add_label_key: "settings.add",
            empty_label_key: "settings.list.empty",
        },
        _ => return None,
    };
    Some(options)
}

fn default_list_options() -> StringListOptions {
    StringListOptions {
        placeholder_key: "settings.list.placeholder",
        add_label_key: "settings.add",
        empty_label_key: "settings.list.empty",
    }
}

fn is_directory_field(section: SettingsSection, key: &str) -> bool {
    matches!(
        (section, key),
        (SettingsSection::EngineProfile, "download_root")
            | (SettingsSection::EngineProfile, "resume_dir")
            | (SettingsSection::FsPolicy, "library_root")
    )
}

fn select_options(section: SettingsSection, key: &str, value: &Value) -> Option<SelectOptions> {
    let allow_empty = value.is_null()
        || matches!(
            (section, key),
            (SettingsSection::EngineProfile, "disk_read_mode")
                | (SettingsSection::EngineProfile, "disk_write_mode")
        );
    let options = match (section, key) {
        (SettingsSection::AppProfile, "auth_mode") => vec![
            ("api_key".to_string(), "settings.option.api_key"),
            ("none".to_string(), "settings.option.no_auth"),
        ],
        (SettingsSection::EngineProfile, "ipv6_mode") => vec![
            ("disabled".to_string(), "settings.option.disabled"),
            ("enabled".to_string(), "settings.option.enabled"),
            ("prefer_v6".to_string(), "settings.option.prefer_v6"),
        ],
        (SettingsSection::EngineProfile, "encryption") => vec![
            ("require".to_string(), "settings.option.require"),
            ("prefer".to_string(), "settings.option.prefer"),
            ("disable".to_string(), "settings.option.disable"),
        ],
        (SettingsSection::EngineProfile, "choking_algorithm") => vec![
            ("fixed_slots".to_string(), "settings.option.fixed_slots"),
            ("rate_based".to_string(), "settings.option.rate_based"),
        ],
        (SettingsSection::EngineProfile, "seed_choking_algorithm") => vec![
            ("round_robin".to_string(), "settings.option.round_robin"),
            (
                "fastest_upload".to_string(),
                "settings.option.fastest_upload",
            ),
            ("anti_leech".to_string(), "settings.option.anti_leech"),
        ],
        (SettingsSection::EngineProfile, "storage_mode") => vec![
            ("sparse".to_string(), "settings.option.sparse"),
            ("allocate".to_string(), "settings.option.allocate"),
        ],
        (SettingsSection::EngineProfile, "disk_read_mode") => vec![
            (
                "enable_os_cache".to_string(),
                "settings.option.enable_os_cache",
            ),
            (
                "disable_os_cache".to_string(),
                "settings.option.disable_os_cache",
            ),
            ("write_through".to_string(), "settings.option.write_through"),
        ],
        (SettingsSection::EngineProfile, "disk_write_mode") => vec![
            (
                "enable_os_cache".to_string(),
                "settings.option.enable_os_cache",
            ),
            (
                "disable_os_cache".to_string(),
                "settings.option.disable_os_cache",
            ),
            ("write_through".to_string(), "settings.option.write_through"),
        ],
        (SettingsSection::FsPolicy, "par2") => vec![
            ("off".to_string(), "settings.option.off"),
            ("verify".to_string(), "settings.option.verify"),
            ("repair".to_string(), "settings.option.repair"),
        ],
        (SettingsSection::FsPolicy, "move_mode") => vec![
            ("copy".to_string(), "settings.option.copy"),
            ("move".to_string(), "settings.option.move"),
            ("hardlink".to_string(), "settings.option.hardlink"),
        ],
        _ => return None,
    };
    Some(SelectOptions {
        allow_empty,
        options,
    })
}

fn numeric_kind(section: SettingsSection, key: &str) -> Option<NumericKind> {
    if section != SettingsSection::EngineProfile {
        return None;
    }
    match key {
        "listen_port"
        | "outgoing_port_min"
        | "outgoing_port_max"
        | "peer_dscp"
        | "max_active"
        | "connections_limit"
        | "connections_limit_per_torrent"
        | "unchoke_slots"
        | "half_open_limit"
        | "optimistic_unchoke_slots"
        | "cache_size"
        | "cache_expiry" => Some(NumericKind::Integer),
        "max_download_bps"
        | "max_upload_bps"
        | "seed_time_limit"
        | "stats_interval_ms"
        | "max_queued_disk_bytes" => Some(NumericKind::Integer),
        "seed_ratio_limit" => Some(NumericKind::Float),
        _ => None,
    }
}
