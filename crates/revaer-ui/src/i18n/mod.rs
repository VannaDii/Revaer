//! Lightweight JSON-backed translations with per-locale bundles.

use serde::Deserialize;
use serde_json::Value;
use std::sync::LazyLock;

/// Supported locale codes for Phase 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LocaleCode {
    /// Arabic.
    Ar,
    /// German.
    De,
    /// Spanish.
    Es,
    /// Hindi.
    Hi,
    /// Italian.
    It,
    /// Javanese.
    Jv,
    /// Marathi.
    Mr,
    /// Portuguese.
    Pt,
    /// Tamil.
    Ta,
    /// Turkish.
    Tr,
    /// Bengali.
    Bn,
    /// English.
    En,
    /// French.
    Fr,
    /// Indonesian.
    Id,
    /// Japanese.
    Ja,
    /// Korean.
    Ko,
    /// Punjabi.
    Pa,
    /// Russian.
    Ru,
    /// Telugu.
    Te,
    /// Chinese (Simplified).
    Zh,
}

impl LocaleCode {
    #[must_use]
    /// All supported locales in display order.
    pub const fn all() -> [Self; 20] {
        [
            Self::Ar,
            Self::De,
            Self::Es,
            Self::Hi,
            Self::It,
            Self::Jv,
            Self::Mr,
            Self::Pt,
            Self::Ta,
            Self::Tr,
            Self::Bn,
            Self::En,
            Self::Fr,
            Self::Id,
            Self::Ja,
            Self::Ko,
            Self::Pa,
            Self::Ru,
            Self::Te,
            Self::Zh,
        ]
    }

    /// RFC 5646 string for the locale (two-letter codes for Phase 1).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Ar => "ar",
            Self::De => "de",
            Self::Es => "es",
            Self::Hi => "hi",
            Self::It => "it",
            Self::Jv => "jv",
            Self::Mr => "mr",
            Self::Pt => "pt",
            Self::Ta => "ta",
            Self::Tr => "tr",
            Self::Bn => "bn",
            Self::En => "en",
            Self::Fr => "fr",
            Self::Id => "id",
            Self::Ja => "ja",
            Self::Ko => "ko",
            Self::Pa => "pa",
            Self::Ru => "ru",
            Self::Te => "te",
            Self::Zh => "zh",
        }
    }

    /// Human-friendly label for dropdowns.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ar => "العربية",
            Self::De => "Deutsch",
            Self::Es => "Español",
            Self::Hi => "हिन्दी",
            Self::It => "Italiano",
            Self::Jv => "Basa Jawa",
            Self::Mr => "मराठी",
            Self::Pt => "Português",
            Self::Ta => "தமிழ்",
            Self::Tr => "Türkçe",
            Self::Bn => "বাংলা",
            Self::En => "English",
            Self::Fr => "Français",
            Self::Id => "Bahasa Indonesia",
            Self::Ja => "日本語",
            Self::Ko => "한국어",
            Self::Pa => "ਪੰਜਾਬੀ",
            Self::Ru => "Русский",
            Self::Te => "తెలుగు",
            Self::Zh => "中文",
        }
    }

    /// Map an arbitrary browser language tag to a supported locale, falling back to None.
    #[must_use]
    pub fn from_lang_tag(tag: &str) -> Option<Self> {
        let lowered = tag.to_ascii_lowercase();
        let base = lowered.split('-').next().unwrap_or_default();
        Self::all()
            .iter()
            .copied()
            .find(|locale| locale.code() == base)
    }
}

/// Default fallback locale.
pub const DEFAULT_LOCALE: LocaleCode = LocaleCode::En;

/// Translation bundle containing a parsed JSON tree for the locale.
#[derive(Clone, Debug)]
pub struct TranslationBundle {
    /// Locale backing this bundle.
    pub locale: LocaleCode,
    tree: Value,
    rtl: bool,
}

impl PartialEq for TranslationBundle {
    fn eq(&self, other: &Self) -> bool {
        self.locale == other.locale
    }
}

impl TranslationBundle {
    /// Build a translation bundle for the given locale, falling back to English.
    ///
    /// The bundle will gracefully degrade to English strings when a key is missing.
    #[must_use]
    pub fn new(locale: LocaleCode) -> Self {
        let raw = raw_locale(locale);
        let tree: Value = serde_json::from_str(raw).unwrap_or(Value::Null);
        let rtl = tree
            .get("meta")
            .and_then(|meta| meta.get("rtl"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Self { locale, tree, rtl }
    }

    /// Resolve a dotted path (`section.key`) with English fallback and caller default.
    #[must_use]
    pub fn text(&self, path: &str, default: &str) -> String {
        resolve(&self.tree, path)
            .or_else(|| resolve(&EN_FALLBACK.tree, path))
            .unwrap_or_else(|| default.to_string())
    }

    /// Whether the locale prefers RTL layout (bidi).
    #[must_use]
    pub const fn rtl(&self) -> bool {
        self.rtl
    }

    #[cfg(test)]
    #[must_use]
    /// Locale backing this bundle.
    pub const fn locale(&self) -> LocaleCode {
        self.locale
    }
}

static EN_FALLBACK: LazyLock<TranslationBundle> =
    LazyLock::new(|| TranslationBundle::new(LocaleCode::En));

fn resolve(tree: &Value, path: &str) -> Option<String> {
    let mut node = tree;
    for segment in path.split('.') {
        node = node.get(segment)?;
    }
    node.as_str().map(ToString::to_string)
}

const fn raw_locale(locale: LocaleCode) -> &'static str {
    match locale {
        LocaleCode::Ar => include_str!("../../i18n/ar.json"),
        LocaleCode::De => include_str!("../../i18n/de.json"),
        LocaleCode::Es => include_str!("../../i18n/es.json"),
        LocaleCode::Hi => include_str!("../../i18n/hi.json"),
        LocaleCode::It => include_str!("../../i18n/it.json"),
        LocaleCode::Jv => include_str!("../../i18n/jv.json"),
        LocaleCode::Mr => include_str!("../../i18n/mr.json"),
        LocaleCode::Pt => include_str!("../../i18n/pt.json"),
        LocaleCode::Ta => include_str!("../../i18n/ta.json"),
        LocaleCode::Tr => include_str!("../../i18n/tr.json"),
        LocaleCode::Bn => include_str!("../../i18n/bn.json"),
        LocaleCode::En => include_str!("../../i18n/en.json"),
        LocaleCode::Fr => include_str!("../../i18n/fr.json"),
        LocaleCode::Id => include_str!("../../i18n/id.json"),
        LocaleCode::Ja => include_str!("../../i18n/ja.json"),
        LocaleCode::Ko => include_str!("../../i18n/ko.json"),
        LocaleCode::Pa => include_str!("../../i18n/pa.json"),
        LocaleCode::Ru => include_str!("../../i18n/ru.json"),
        LocaleCode::Te => include_str!("../../i18n/te.json"),
        LocaleCode::Zh => include_str!("../../i18n/zh.json"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_key_falls_back_to_default() {
        let bundle = TranslationBundle::new(LocaleCode::Fr);
        assert_eq!(bundle.text("nonexistent.key", "fallback"), "fallback");
    }

    #[test]
    fn rtl_flag_respects_meta() {
        assert!(TranslationBundle::new(LocaleCode::Ar).rtl());
        assert!(!TranslationBundle::new(LocaleCode::En).rtl());
    }

    #[test]
    fn bundles_load_all_locales() {
        for locale in LocaleCode::all() {
            let bundle = TranslationBundle::new(locale);
            assert_eq!(bundle.locale(), locale);
            assert!(!bundle.text("nav.dashboard", "Dash").is_empty());
        }
    }
}
