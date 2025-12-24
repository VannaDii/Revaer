//! Label policy feature state.
//!
//! # Design
//! - Keep form inputs as strings for lossless editing.
//! - Convert to shared API types only on save.
//! - Avoid storing derived values that can be recomputed.

use crate::features::labels::logic::{parse_optional_f64, parse_optional_i32, parse_optional_u64};
use crate::models::{
    TorrentCleanupPolicy, TorrentLabelEntry, TorrentLabelPolicy, TorrentRateLimit,
};

/// Label type for policy management.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelKind {
    /// Category label policies.
    Category,
    /// Tag label policies.
    Tag,
}

impl LabelKind {
    /// Human-readable singular label.
    #[must_use]
    pub const fn singular(self) -> &'static str {
        match self {
            Self::Category => "Category",
            Self::Tag => "Tag",
        }
    }

    /// Human-readable plural label.
    #[must_use]
    pub const fn plural(self) -> &'static str {
        match self {
            Self::Category => "Categories",
            Self::Tag => "Tags",
        }
    }
}

/// Tri-state selector for auto-managed overrides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoManagedChoice {
    /// Do not override the default auto-managed value.
    Default,
    /// Force auto-managed on.
    Enabled,
    /// Force auto-managed off.
    Disabled,
}

impl AutoManagedChoice {
    /// Map optional overrides into a tri-state selector value.
    #[must_use]
    pub const fn from_option(value: Option<bool>) -> Self {
        match value {
            Some(true) => Self::Enabled,
            Some(false) => Self::Disabled,
            None => Self::Default,
        }
    }

    /// Convert the selector into an optional override.
    #[must_use]
    pub const fn as_option(self) -> Option<bool> {
        match self {
            Self::Default => None,
            Self::Enabled => Some(true),
            Self::Disabled => Some(false),
        }
    }

    /// String value used by the select control.
    #[must_use]
    pub const fn as_value(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }

    /// Parse a select control value into a selector choice.
    #[must_use]
    pub fn from_value(value: &str) -> Self {
        match value {
            "enabled" => Self::Enabled,
            "disabled" => Self::Disabled,
            _ => Self::Default,
        }
    }
}

/// Mutable label policy form state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LabelFormState {
    /// Label name entered in the editor.
    pub name: String,
    /// Optional download directory override.
    pub download_dir: String,
    /// Download rate cap in bytes/sec.
    pub rate_limit_download: String,
    /// Upload rate cap in bytes/sec.
    pub rate_limit_upload: String,
    /// Queue position override.
    pub queue_position: String,
    /// Auto-managed override choice.
    pub auto_managed: AutoManagedChoice,
    /// Seed ratio limit override.
    pub seed_ratio_limit: String,
    /// Seed time limit override, in seconds.
    pub seed_time_limit: String,
    /// Cleanup seed ratio override.
    pub cleanup_seed_ratio_limit: String,
    /// Cleanup seed time override, in seconds.
    pub cleanup_seed_time_limit: String,
    /// Whether cleanup removes data.
    pub cleanup_remove_data: bool,
}

impl Default for LabelFormState {
    fn default() -> Self {
        Self {
            name: String::new(),
            download_dir: String::new(),
            rate_limit_download: String::new(),
            rate_limit_upload: String::new(),
            queue_position: String::new(),
            auto_managed: AutoManagedChoice::Default,
            seed_ratio_limit: String::new(),
            seed_time_limit: String::new(),
            cleanup_seed_ratio_limit: String::new(),
            cleanup_seed_time_limit: String::new(),
            cleanup_remove_data: false,
        }
    }
}

impl LabelFormState {
    /// Build form state from an existing label entry.
    #[must_use]
    pub fn from_entry(entry: &TorrentLabelEntry) -> Self {
        let policy = &entry.policy;
        let (rate_limit_download, rate_limit_upload) = policy.rate_limit.as_ref().map_or_else(
            || (String::new(), String::new()),
            |rate_limit| {
                (
                    option_to_string(rate_limit.download_bps),
                    option_to_string(rate_limit.upload_bps),
                )
            },
        );
        let (cleanup_seed_ratio_limit, cleanup_seed_time_limit, cleanup_remove_data) =
            policy.cleanup.as_ref().map_or_else(
                || (String::new(), String::new(), false),
                |cleanup| {
                    (
                        option_to_string(cleanup.seed_ratio_limit),
                        option_to_string(cleanup.seed_time_limit),
                        cleanup.remove_data,
                    )
                },
            );
        Self {
            name: entry.name.clone(),
            download_dir: policy.download_dir.clone().unwrap_or_default(),
            rate_limit_download,
            rate_limit_upload,
            queue_position: option_to_string(policy.queue_position),
            auto_managed: AutoManagedChoice::from_option(policy.auto_managed),
            seed_ratio_limit: option_to_string(policy.seed_ratio_limit),
            seed_time_limit: option_to_string(policy.seed_time_limit),
            cleanup_seed_ratio_limit,
            cleanup_seed_time_limit,
            cleanup_remove_data,
        }
    }

    /// Convert the form state into a label policy payload.
    ///
    /// # Errors
    /// Returns an error when numeric fields are invalid or cleanup is enabled
    /// without a seed ratio/time threshold.
    pub fn to_policy(&self) -> Result<TorrentLabelPolicy, String> {
        let download_dir = self.download_dir.trim().to_string();
        let download_dir = if download_dir.is_empty() {
            None
        } else {
            Some(download_dir)
        };
        let rate_limit_download =
            parse_optional_u64("download rate limit", &self.rate_limit_download)?;
        let rate_limit_upload = parse_optional_u64("upload rate limit", &self.rate_limit_upload)?;
        let rate_limit = if rate_limit_download.is_some() || rate_limit_upload.is_some() {
            Some(TorrentRateLimit {
                download_bps: rate_limit_download,
                upload_bps: rate_limit_upload,
            })
        } else {
            None
        };
        let queue_position = parse_optional_i32("queue position", &self.queue_position)?;
        let seed_ratio_limit = parse_optional_f64("seed ratio limit", &self.seed_ratio_limit)?;
        let seed_time_limit = parse_optional_u64("seed time limit", &self.seed_time_limit)?;
        let cleanup_seed_ratio_limit =
            parse_optional_f64("cleanup seed ratio limit", &self.cleanup_seed_ratio_limit)?;
        let cleanup_seed_time_limit =
            parse_optional_u64("cleanup seed time limit", &self.cleanup_seed_time_limit)?;
        let cleanup = if cleanup_seed_ratio_limit.is_none() && cleanup_seed_time_limit.is_none() {
            if self.cleanup_remove_data {
                return Err("cleanup policy requires seed ratio or seed time".to_string());
            }
            None
        } else {
            Some(TorrentCleanupPolicy {
                seed_ratio_limit: cleanup_seed_ratio_limit,
                seed_time_limit: cleanup_seed_time_limit,
                remove_data: self.cleanup_remove_data,
            })
        };

        Ok(TorrentLabelPolicy {
            download_dir,
            rate_limit,
            queue_position,
            auto_managed: self.auto_managed.as_option(),
            seed_ratio_limit,
            seed_time_limit,
            cleanup,
        })
    }
}

fn option_to_string<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map(|inner| inner.to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{AutoManagedChoice, LabelFormState};
    use crate::models::{
        TorrentCleanupPolicy, TorrentLabelEntry, TorrentLabelPolicy, TorrentRateLimit,
    };

    #[test]
    fn to_policy_builds_rate_limit() {
        let form = LabelFormState {
            rate_limit_download: "1024".to_string(),
            rate_limit_upload: "2048".to_string(),
            ..LabelFormState::default()
        };
        let policy = form.to_policy().expect("policy should parse");
        let rate = policy.rate_limit.expect("rate limit");
        assert_eq!(rate.download_bps, Some(1024));
        assert_eq!(rate.upload_bps, Some(2048));
    }

    #[test]
    fn cleanup_requires_threshold() {
        let form = LabelFormState {
            cleanup_remove_data: true,
            ..LabelFormState::default()
        };
        let err = form.to_policy().expect_err("cleanup requires threshold");
        assert!(err.contains("cleanup policy requires"));
    }

    #[test]
    fn from_entry_maps_policy_fields() {
        let entry = TorrentLabelEntry {
            name: "movies".to_string(),
            policy: TorrentLabelPolicy {
                download_dir: Some("/data/movies".to_string()),
                rate_limit: Some(TorrentRateLimit {
                    download_bps: Some(4096),
                    upload_bps: None,
                }),
                queue_position: Some(2),
                auto_managed: Some(false),
                seed_ratio_limit: Some(1.5),
                seed_time_limit: Some(3600),
                cleanup: Some(TorrentCleanupPolicy {
                    seed_ratio_limit: Some(2.0),
                    seed_time_limit: None,
                    remove_data: true,
                }),
            },
        };
        let form = LabelFormState::from_entry(&entry);
        assert_eq!(form.name, "movies");
        assert_eq!(form.download_dir, "/data/movies");
        assert_eq!(form.rate_limit_download, "4096");
        assert_eq!(form.queue_position, "2");
        assert_eq!(form.auto_managed, AutoManagedChoice::Disabled);
        assert_eq!(form.seed_ratio_limit, "1.5");
        assert_eq!(form.seed_time_limit, "3600");
        assert_eq!(form.cleanup_seed_ratio_limit, "2");
        assert!(form.cleanup_remove_data);
    }
}
