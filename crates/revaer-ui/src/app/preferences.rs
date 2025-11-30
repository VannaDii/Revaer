//! Persistence and environment helpers for the app shell.

use crate::core::theme::ThemeMode;
use crate::core::ui::{Density, UiMode};
use crate::i18n::{DEFAULT_LOCALE, LocaleCode};
use gloo::storage::{LocalStorage, Storage};
use gloo::utils::window;
use web_sys::Url;

pub const THEME_KEY: &str = "revaer.theme";
pub const MODE_KEY: &str = "revaer.mode";
pub const LOCALE_KEY: &str = "revaer.locale";
pub const DENSITY_KEY: &str = "revaer.density";
pub const API_KEY_KEY: &str = "revaer.api_key";

pub fn load_theme() -> ThemeMode {
    if let Ok(value) = LocalStorage::get::<String>(THEME_KEY) {
        return match value.as_str() {
            "dark" => ThemeMode::Dark,
            _ => ThemeMode::Light,
        };
    }
    prefers_dark()
        .unwrap_or(false)
        .then_some(ThemeMode::Dark)
        .unwrap_or(ThemeMode::Light)
}

pub fn load_mode() -> UiMode {
    if let Ok(value) = LocalStorage::get::<String>(MODE_KEY) {
        return match value.as_str() {
            "advanced" => UiMode::Advanced,
            _ => UiMode::Simple,
        };
    }
    UiMode::Simple
}

pub fn load_density() -> Density {
    if let Ok(value) = LocalStorage::get::<String>(DENSITY_KEY) {
        return match value.as_str() {
            "compact" => Density::Compact,
            "comfy" => Density::Comfy,
            _ => Density::Normal,
        };
    }
    Density::Normal
}

pub fn load_locale() -> LocaleCode {
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

pub fn load_api_key(allow_anon: bool) -> Option<String> {
    if let Ok(value) = LocalStorage::get::<String>(API_KEY_KEY) {
        if !value.trim().is_empty() {
            return Some(value);
        }
    }
    if allow_anon {
        Some("dev:revaer_dev".to_string())
    } else {
        None
    }
}

pub fn allow_anonymous() -> bool {
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

pub fn api_base_url() -> String {
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

pub fn prefers_dark() -> Option<bool> {
    let media: web_sys::MediaQueryList = window()
        .match_media("(prefers-color-scheme: dark)")
        .ok()
        .and_then(|m| m)?;
    Some(media.matches())
}
