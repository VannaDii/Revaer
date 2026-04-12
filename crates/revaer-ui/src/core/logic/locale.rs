//! Locale mapping helpers for UI flags.
//!
//! # Design
//! - Keep locale-to-flag mapping pure and deterministic for UI rendering.
//! - Invariants: returned codes are stable ISO-style slugs for the flag CDN.
//! - Failure modes: unknown locales fall back to the caller's default mapping.

use crate::i18n::LocaleCode;

/// Map a locale code to the flag CDN slug.
#[must_use]
pub(crate) const fn locale_flag(locale: LocaleCode) -> &'static str {
    match locale {
        LocaleCode::Ar => "sa",
        LocaleCode::De => "de",
        LocaleCode::Es => "es",
        LocaleCode::Hi | LocaleCode::Mr | LocaleCode::Ta | LocaleCode::Pa | LocaleCode::Te => "in",
        LocaleCode::It => "it",
        LocaleCode::Jv | LocaleCode::Id => "id",
        LocaleCode::Pt => "pt",
        LocaleCode::Tr => "tr",
        LocaleCode::Bn => "bd",
        LocaleCode::En => "gb",
        LocaleCode::Fr => "fr",
        LocaleCode::Ja => "jp",
        LocaleCode::Ko => "kr",
        LocaleCode::Ru => "ru",
        LocaleCode::Zh => "cn",
    }
}

#[cfg(test)]
mod tests {
    use super::locale_flag;
    use crate::i18n::LocaleCode;

    #[test]
    fn locale_flag_maps_known_codes() {
        assert_eq!(locale_flag(LocaleCode::En), "gb");
        assert_eq!(locale_flag(LocaleCode::Fr), "fr");
        assert_eq!(locale_flag(LocaleCode::Ja), "jp");
        assert_eq!(locale_flag(LocaleCode::Ar), "sa");
        assert_eq!(locale_flag(LocaleCode::Bn), "bd");
        assert_eq!(locale_flag(LocaleCode::De), "de");
        assert_eq!(locale_flag(LocaleCode::Es), "es");
        assert_eq!(locale_flag(LocaleCode::Hi), "in");
        assert_eq!(locale_flag(LocaleCode::Mr), "in");
        assert_eq!(locale_flag(LocaleCode::Ta), "in");
        assert_eq!(locale_flag(LocaleCode::Pa), "in");
        assert_eq!(locale_flag(LocaleCode::Te), "in");
        assert_eq!(locale_flag(LocaleCode::It), "it");
        assert_eq!(locale_flag(LocaleCode::Jv), "id");
        assert_eq!(locale_flag(LocaleCode::Id), "id");
        assert_eq!(locale_flag(LocaleCode::Pt), "pt");
        assert_eq!(locale_flag(LocaleCode::Tr), "tr");
        assert_eq!(locale_flag(LocaleCode::Ko), "kr");
        assert_eq!(locale_flag(LocaleCode::Ru), "ru");
        assert_eq!(locale_flag(LocaleCode::Zh), "cn");
    }
}
