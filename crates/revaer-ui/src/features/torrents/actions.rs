//! Torrent actions and display helpers.

use crate::i18n::TranslationBundle;

/// Torrent actions emitted from UI controls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TorrentAction {
    /// Pause the torrent.
    Pause,
    /// Resume the torrent.
    Resume,
    /// Force a recheck.
    Recheck,
    /// Delete the torrent, optionally removing data.
    Delete {
        /// Whether payload data should also be removed.
        with_data: bool,
    },
}

/// Format a toast message for a successful action.
#[must_use]
pub fn success_message(bundle: &TranslationBundle, action: &TorrentAction, name: &str) -> String {
    match action {
        TorrentAction::Pause => format!("{} {name}", bundle.text("toast.pause", "")),
        TorrentAction::Resume => format!("{} {name}", bundle.text("toast.resume", "")),
        TorrentAction::Recheck => format!("{} {name}", bundle.text("toast.recheck", "")),
        TorrentAction::Delete { with_data } => {
            if *with_data {
                format!("{} {name}", bundle.text("toast.delete_data", ""))
            } else {
                format!("{} {name}", bundle.text("toast.delete", ""))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{LocaleCode, TranslationBundle};

    #[test]
    fn success_messages_switch_on_action() {
        let bundle = TranslationBundle::new(LocaleCode::En);
        let pause = success_message(&bundle, &TorrentAction::Pause, "x");
        let resume = success_message(&bundle, &TorrentAction::Resume, "x");
        let recheck = success_message(&bundle, &TorrentAction::Recheck, "x");
        let delete_meta =
            success_message(&bundle, &TorrentAction::Delete { with_data: false }, "x");
        let delete_data = success_message(&bundle, &TorrentAction::Delete { with_data: true }, "x");

        assert!(!pause.is_empty());
        assert_ne!(pause, resume);
        assert_ne!(recheck, delete_meta);
        assert_ne!(delete_meta, delete_data);
    }
}
