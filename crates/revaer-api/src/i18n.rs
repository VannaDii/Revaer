//! # Design
//!
//! - Centralize API-facing localization for error payloads based on `Accept-Language`.
//! - Unsupported locales fall back to the default locale with identity translations.
//! - Translation parse failures degrade to empty bundles and log once at load time.

use std::collections::HashMap;
use std::sync::OnceLock;

use axum::{
    body::Body,
    http::{HeaderMap, Request, header::ACCEPT_LANGUAGE},
    middleware::Next,
    response::Response,
};
use serde::Deserialize;
use tracing::error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LocaleCode {
    En,
}

impl LocaleCode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
        }
    }

    fn from_tag(tag: &str) -> Option<Self> {
        let mut parts = tag.split('-');
        let primary = parts.next().unwrap_or(tag).trim();
        if primary.eq_ignore_ascii_case("en") {
            return Some(Self::En);
        }
        None
    }
}

const DEFAULT_LOCALE: LocaleCode = LocaleCode::En;

tokio::task_local! {
    static REQUEST_LOCALE: LocaleCode;
}

#[derive(Debug, Default)]
struct TranslationBundle {
    messages: HashMap<String, String>,
}

impl TranslationBundle {
    fn lookup(&self, message: &str) -> Option<&str> {
        self.messages.get(message).map(String::as_str)
    }
}

#[derive(Debug, Deserialize)]
struct TranslationFile {
    #[serde(default)]
    messages: HashMap<String, String>,
}

pub(crate) fn localize_message(locale: LocaleCode, message: &str) -> String {
    translations_for(locale)
        .lookup(message)
        .map_or_else(|| message.to_string(), ToString::to_string)
}

pub(crate) fn current_locale() -> LocaleCode {
    REQUEST_LOCALE
        .try_with(|locale| *locale)
        .map_or(DEFAULT_LOCALE, |locale| locale)
}

pub(crate) async fn with_locale(req: Request<Body>, next: Next) -> Response {
    let locale = parse_locale(req.headers());
    REQUEST_LOCALE
        .scope(locale, async move { next.run(req).await })
        .await
}

fn parse_locale(headers: &HeaderMap) -> LocaleCode {
    let value = headers
        .get(ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok());
    if let Some(value) = value
        && let Some(locale) = parse_accept_language(value)
    {
        return locale;
    }
    DEFAULT_LOCALE
}

fn parse_accept_language(value: &str) -> Option<LocaleCode> {
    for part in value.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut components = trimmed.split(';');
        if let Some(tag) = components.next()
            && let Some(locale) = LocaleCode::from_tag(tag.trim())
        {
            return Some(locale);
        }
    }
    None
}

fn translations_for(locale: LocaleCode) -> &'static TranslationBundle {
    static EN_TRANSLATIONS: OnceLock<TranslationBundle> = OnceLock::new();
    match locale {
        LocaleCode::En => EN_TRANSLATIONS.get_or_init(|| load_translations(LocaleCode::En)),
    }
}

fn load_translations(locale: LocaleCode) -> TranslationBundle {
    let raw = match locale {
        LocaleCode::En => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/i18n/en.json")),
    };
    match serde_json::from_str::<TranslationFile>(raw) {
        Ok(file) => TranslationBundle {
            messages: file.messages,
        },
        Err(err) => {
            error!(
                error = %err,
                locale = locale.as_str(),
                "failed to parse API i18n bundle"
            );
            TranslationBundle::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_language_prefers_first_supported_locale() {
        let selected = parse_accept_language("en-US,en;q=0.9");
        assert_eq!(selected, Some(LocaleCode::En));
    }

    #[test]
    fn translations_load_for_default_locale() {
        let bundle = translations_for(LocaleCode::En);
        assert!(bundle.lookup("bad request").is_some());
    }

    #[test]
    fn localize_message_falls_back_when_missing() {
        let translated = localize_message(LocaleCode::En, "missing-key");
        assert_eq!(translated, "missing-key");
    }
}
