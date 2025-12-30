//! Persistence and environment helpers for the app shell.

use crate::core::auth::{AuthMode, AuthState, LocalAuth};
use crate::core::theme::ThemeMode;
use crate::core::ui::{Density, UiMode};
use crate::i18n::{DEFAULT_LOCALE, LocaleCode};
use gloo::console;
use gloo::storage::{LocalStorage, Storage};
use gloo::utils::window;
use js_sys::Date;
use serde::Serialize;
use web_sys::Url;

pub(crate) const THEME_KEY: &str = "revaer.theme";
pub(crate) const MODE_KEY: &str = "revaer.mode";
pub(crate) const LOCALE_KEY: &str = "revaer.locale";
pub(crate) const DENSITY_KEY: &str = "revaer.density";
pub(crate) const API_KEY_KEY: &str = "revaer.api_key";
pub(crate) const API_KEY_EXPIRES_AT_KEY: &str = "revaer.api_key_expires_at";
pub(crate) const AUTH_MODE_KEY: &str = "revaer.auth.mode";
pub(crate) const LOCAL_AUTH_USER_KEY: &str = "revaer.auth.user";
pub(crate) const LOCAL_AUTH_PASS_KEY: &str = "revaer.auth.pass";
pub(crate) const AUTH_BYPASS_LOCAL_KEY: &str = "revaer.auth.bypass_local";
pub(crate) const AUTH_ANONYMOUS_KEY: &str = "revaer.auth.anonymous";
pub(crate) const SSE_LAST_EVENT_ID_KEY: &str = "revaer.sse.last_event_id";

pub(crate) fn load_theme() -> ThemeMode {
    if let Ok(value) = LocalStorage::get::<String>(THEME_KEY) {
        return match value.as_str() {
            "revaer-dark" | "dark" => ThemeMode::Dark,
            "revaer-light" | "light" => ThemeMode::Light,
            _ => ThemeMode::Dark,
        };
    }
    ThemeMode::Dark
}

pub(crate) fn load_mode() -> UiMode {
    if let Ok(value) = LocalStorage::get::<String>(MODE_KEY) {
        return match value.as_str() {
            "advanced" => UiMode::Advanced,
            _ => UiMode::Simple,
        };
    }
    UiMode::Simple
}

pub(crate) fn load_density() -> Density {
    if let Ok(value) = LocalStorage::get::<String>(DENSITY_KEY) {
        return match value.as_str() {
            "compact" => Density::Compact,
            "comfy" => Density::Comfy,
            _ => Density::Normal,
        };
    }
    Density::Normal
}

pub(crate) fn load_locale() -> LocaleCode {
    if let Ok(value) = LocalStorage::get::<String>(LOCALE_KEY) {
        if let Some(locale) = LocaleCode::from_lang_tag(&value) {
            return locale;
        }
    }
    if let Some(nav) = window().navigator().language() {
        if let Some(locale) = LocaleCode::from_lang_tag(&nav) {
            return locale;
        }
    }
    DEFAULT_LOCALE
}

pub(crate) fn load_api_key(_allow_anon: bool) -> Option<String> {
    let value = LocalStorage::get::<String>(API_KEY_KEY).ok()?;
    if value.trim().is_empty() {
        return None;
    }
    if let Some(expires_at_ms) = load_api_key_expires_at_ms() {
        let now = Date::now() as i64;
        if expires_at_ms <= now {
            clear_api_key_storage();
            return None;
        }
    }
    Some(value)
}

pub(crate) fn load_auth_mode() -> AuthMode {
    if let Ok(value) = LocalStorage::get::<String>(AUTH_MODE_KEY) {
        return match value.as_str() {
            "local" => AuthMode::Local,
            _ => AuthMode::ApiKey,
        };
    }
    AuthMode::ApiKey
}

pub(crate) fn load_bypass_local() -> bool {
    LocalStorage::get::<bool>(AUTH_BYPASS_LOCAL_KEY).unwrap_or(false)
}

pub(crate) fn load_local_auth() -> Option<LocalAuth> {
    let username = LocalStorage::get::<String>(LOCAL_AUTH_USER_KEY).ok()?;
    let password = LocalStorage::get::<String>(LOCAL_AUTH_PASS_KEY).ok()?;
    if username.trim().is_empty() || password.trim().is_empty() {
        return None;
    }
    Some(LocalAuth { username, password })
}

pub(crate) fn load_auth_state(mode: AuthMode, allow_anon: bool) -> Option<AuthState> {
    if allow_anon && load_anonymous_auth() {
        return Some(AuthState::Anonymous);
    }
    match mode {
        AuthMode::ApiKey => load_api_key(allow_anon).map(AuthState::ApiKey),
        AuthMode::Local => load_local_auth().map(AuthState::Local),
    }
}

pub(crate) fn load_last_event_id() -> Option<u64> {
    LocalStorage::get::<String>(SSE_LAST_EVENT_ID_KEY)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
}

pub(crate) fn persist_last_event_id(id: u64) {
    set_storage(SSE_LAST_EVENT_ID_KEY, id.to_string());
}

pub(crate) fn persist_auth_state(state: &AuthState) {
    match state {
        AuthState::ApiKey(value) => {
            set_storage(AUTH_MODE_KEY, "api_key");
            set_storage(API_KEY_KEY, value);
            delete_storage(AUTH_ANONYMOUS_KEY);
            delete_storage(API_KEY_EXPIRES_AT_KEY);
        }
        AuthState::Local(auth) => {
            set_storage(AUTH_MODE_KEY, "local");
            set_storage(LOCAL_AUTH_USER_KEY, &auth.username);
            set_storage(LOCAL_AUTH_PASS_KEY, &auth.password);
            delete_storage(AUTH_ANONYMOUS_KEY);
            delete_storage(API_KEY_EXPIRES_AT_KEY);
        }
        AuthState::Anonymous => {
            set_storage(AUTH_MODE_KEY, "api_key");
            set_storage(AUTH_ANONYMOUS_KEY, true);
            delete_storage(API_KEY_KEY);
            delete_storage(LOCAL_AUTH_USER_KEY);
            delete_storage(LOCAL_AUTH_PASS_KEY);
            delete_storage(API_KEY_EXPIRES_AT_KEY);
        }
    }
}

pub(crate) fn persist_api_key_with_expiry(api_key: &str, expires_at: &str) {
    set_storage(AUTH_MODE_KEY, "api_key");
    set_storage(API_KEY_KEY, api_key);
    delete_storage(AUTH_ANONYMOUS_KEY);
    if let Some(expires_at_ms) = parse_expires_at_ms(expires_at) {
        set_storage(API_KEY_EXPIRES_AT_KEY, expires_at_ms);
    } else {
        delete_storage(API_KEY_EXPIRES_AT_KEY);
    }
}

pub(crate) fn clear_auth_storage() {
    clear_api_key_storage();
    delete_storage(AUTH_ANONYMOUS_KEY);
    delete_storage(LOCAL_AUTH_USER_KEY);
    delete_storage(LOCAL_AUTH_PASS_KEY);
}

pub(crate) fn clear_api_key_storage() {
    delete_storage(API_KEY_KEY);
    delete_storage(API_KEY_EXPIRES_AT_KEY);
}

fn parse_expires_at_ms(value: &str) -> Option<i64> {
    let parsed = Date::parse(value);
    if parsed.is_nan() {
        None
    } else {
        Some(parsed as i64)
    }
}

pub(crate) fn load_api_key_expires_at_ms() -> Option<i64> {
    LocalStorage::get::<i64>(API_KEY_EXPIRES_AT_KEY).ok()
}

fn load_anonymous_auth() -> bool {
    LocalStorage::get::<bool>(AUTH_ANONYMOUS_KEY).unwrap_or(false)
}

pub(crate) fn persist_bypass_local(value: bool) {
    set_storage(AUTH_BYPASS_LOCAL_KEY, value);
}

pub(crate) fn allow_anonymous() -> bool {
    is_local_host()
}

fn is_local_host() -> bool {
    let host = window()
        .location()
        .hostname()
        .unwrap_or_else(|_| String::new())
        .to_ascii_lowercase();
    if host.is_empty()
        || host == "localhost"
        || host == "127.0.0.1"
        || host == "::1"
        || host.starts_with("127.")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || (host.starts_with("172.")
            && host
                .split('.')
                .nth(1)
                .and_then(|b| b.parse::<u8>().ok())
                .map_or(false, |b| (16..=31).contains(&b)))
        || host.ends_with(".local")
    {
        return true;
    }
    false
}

pub(crate) fn api_base_url() -> String {
    let href = window()
        .location()
        .href()
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    if let Ok(url) = Url::new(&href) {
        let protocol = url.protocol();
        let host = url.hostname();
        let port = url.port();
        let mapped_port = match port.as_str() {
            "" => None,
            "8080" => Some("7070"),
            other => Some(other),
        };

        let mut base = format!("{}//{}", protocol, host);
        if let Some(port) = mapped_port {
            base.push(':');
            base.push_str(port);
        }
        return base;
    }

    "http://localhost:7070".to_string()
}

fn set_storage<T: Serialize>(key: &'static str, value: T) {
    if let Err(err) = LocalStorage::set(key, value) {
        log_storage_error("set", key, &err.to_string());
    }
}

fn delete_storage(key: &'static str) {
    LocalStorage::delete(key);
}

fn log_storage_error(operation: &'static str, key: &'static str, detail: &str) {
    console::error!("storage operation failed", operation, key, detail);
}
