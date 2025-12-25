//! Torrent actions and display helpers.

use crate::i18n::TranslationBundle;

/// Torrent actions emitted from UI controls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TorrentAction {
    /// Pause the torrent.
    Pause,
    /// Resume the torrent.
    Resume,
    /// Force a reannounce to trackers.
    Reannounce,
    /// Force a recheck.
    Recheck,
    /// Toggle sequential download mode.
    Sequential {
        /// Enables sequential mode when true.
        enable: bool,
    },
    /// Update per-torrent rate limits.
    Rate {
        /// Optional download cap in bytes per second.
        download_bps: Option<u64>,
        /// Optional upload cap in bytes per second.
        upload_bps: Option<u64>,
    },
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
        TorrentAction::Reannounce => format!("{} {name}", bundle.text("toast.reannounce", "")),
        TorrentAction::Recheck => format!("{} {name}", bundle.text("toast.recheck", "")),
        TorrentAction::Sequential { enable } => {
            if *enable {
                format!("{} {name}", bundle.text("toast.sequential_on", ""))
            } else {
                format!("{} {name}", bundle.text("toast.sequential_off", ""))
            }
        }
        TorrentAction::Rate { .. } => {
            format!("{} {name}", bundle.text("toast.rate", ""))
        }
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
        let reannounce = success_message(&bundle, &TorrentAction::Reannounce, "x");
        let recheck = success_message(&bundle, &TorrentAction::Recheck, "x");
        let sequential_on =
            success_message(&bundle, &TorrentAction::Sequential { enable: true }, "x");
        let delete_meta =
            success_message(&bundle, &TorrentAction::Delete { with_data: false }, "x");
        let delete_data = success_message(&bundle, &TorrentAction::Delete { with_data: true }, "x");
        let rate = success_message(
            &bundle,
            &TorrentAction::Rate {
                download_bps: Some(1024),
                upload_bps: None,
            },
            "x",
        );

        assert!(!pause.is_empty());
        assert_ne!(pause, resume);
        assert_ne!(resume, reannounce);
        assert_ne!(recheck, delete_meta);
        assert_ne!(delete_meta, delete_data);
        assert_ne!(sequential_on, rate);
    }
}
