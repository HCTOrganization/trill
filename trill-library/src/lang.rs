//! The locale set, shared by the app's own Fluent bundles and the
//! per-family game-name bundles in [`crate::game`].
//!
//! Only the identifiers live here. The app's translated strings stay
//! with the frontend that renders them; the game families carry their
//! own bundles.

/// Strings a game family doesn't translate fall back to this one. The
/// per-game bundles still ship `en-US` fragments even though the app UI
/// does not, because this is also what a game name resolves to when the
/// host's language has no fragment.
pub const FALLBACK_LANG: unic_langid::LanguageIdentifier = unic_langid::langid!("en-US");

/// Locale the app's own strings fall back to, and the default UI
/// language. `locales/` ships Chinese and Japanese only, so an
/// untranslated key and a stored choice this build no longer ships both
/// land here.
pub const UI_FALLBACK_LANG: unic_langid::LanguageIdentifier = unic_langid::langid!("zh-CN");

/// Locales the app exposes in the language picker.
pub const SUPPORTED_LANGS: &[unic_langid::LanguageIdentifier] = &[
    unic_langid::langid!("ja-JP"),
    unic_langid::langid!("zh-CN"),
    unic_langid::langid!("zh-TW"),
];

/// Locale a stored choice migrates to when this build no longer ships
/// it (e.g. a config written by a build that offered more languages).
/// Landing on a shipped locale keeps the UI translated instead of
/// rendering the missing-key sentinel.
pub const REMOVED_LANGUAGE_FALLBACK: unic_langid::LanguageIdentifier = UI_FALLBACK_LANG;

/// Clamp a stored language choice to one this build ships.
pub fn normalize(lang: unic_langid::LanguageIdentifier) -> unic_langid::LanguageIdentifier {
    if SUPPORTED_LANGS.contains(&lang) {
        lang
    } else {
        REMOVED_LANGUAGE_FALLBACK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removed_languages_migrate_to_zh_cn() {
        assert_eq!(normalize(unic_langid::langid!("de-DE")), REMOVED_LANGUAGE_FALLBACK);
        assert_eq!(normalize(unic_langid::langid!("en-US")), REMOVED_LANGUAGE_FALLBACK);
    }

    #[test]
    fn shipped_languages_are_unchanged() {
        for lang in SUPPORTED_LANGS {
            assert_eq!(&normalize(lang.clone()), lang);
        }
    }
}
