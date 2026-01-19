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
    }
}
