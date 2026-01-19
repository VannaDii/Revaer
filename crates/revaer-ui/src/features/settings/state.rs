//! Settings feature state types.
//!
//! # Design
//! - Hold serializable, UI-facing state without side effects.
//! - Invariants: enums model known settings kinds; structs carry raw and parsed values.
//! - Failure modes: invalid data is represented via draft errors, not panics.

use std::collections::BTreeMap;

use crate::models::FsEntry;
use serde_json::Value;

/// Tab selection for the settings page.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsTab {
    Connection,
    Downloads,
    Seeding,
    Network,
    Storage,
    Labels,
    System,
}

impl SettingsTab {
    #[must_use]
    pub(crate) const fn all() -> [Self; 7] {
        [
            Self::Connection,
            Self::Downloads,
            Self::Seeding,
            Self::Network,
            Self::Storage,
            Self::Labels,
            Self::System,
        ]
    }

    #[must_use]
    pub(crate) const fn label_key(self) -> &'static str {
        match self {
            Self::Connection => "settings.tabs.connection",
            Self::Downloads => "settings.tabs.downloads",
            Self::Seeding => "settings.tabs.seeding",
            Self::Network => "settings.tabs.network",
            Self::Storage => "settings.tabs.storage",
            Self::Labels => "settings.tabs.labels",
            Self::System => "settings.tabs.system",
        }
    }

    #[must_use]
    pub(crate) const fn tab_id(self) -> &'static str {
        match self {
            Self::Connection => "settings-tab-connection",
            Self::Downloads => "settings-tab-downloads",
            Self::Seeding => "settings-tab-seeding",
            Self::Network => "settings-tab-network",
            Self::Storage => "settings-tab-storage",
            Self::Labels => "settings-tab-labels",
            Self::System => "settings-tab-system",
        }
    }

    #[must_use]
    pub(crate) const fn panel_id() -> &'static str {
        "settings-panel"
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsSection {
    AppProfile,
    EngineProfile,
    FsPolicy,
}

impl SettingsSection {
    #[must_use]
    pub(crate) const fn all() -> [Self; 3] {
        [Self::AppProfile, Self::EngineProfile, Self::FsPolicy]
    }

    #[must_use]
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::AppProfile => "app_profile",
            Self::EngineProfile => "engine_profile",
            Self::FsPolicy => "fs_policy",
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct FieldDraft {
    pub(crate) value: Value,
    pub(crate) raw: String,
    pub(crate) error: Option<String>,
}

#[derive(Clone, PartialEq, Default)]
pub(crate) struct SettingsDraft {
    pub(crate) fields: BTreeMap<String, FieldDraft>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct SettingsField {
    pub(crate) section: SettingsSection,
    pub(crate) key: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LabelKind {
    Category,
    Tag,
}

impl LabelKind {
    #[must_use]
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::Category => "category",
            Self::Tag => "tag",
        }
    }

    #[must_use]
    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "category" => Some(Self::Category),
            "tag" => Some(Self::Tag),
            _ => None,
        }
    }

    #[must_use]
    pub(crate) const fn label_key(self) -> &'static str {
        match self {
            Self::Category => "settings.labels.categories",
            Self::Tag => "settings.labels.tags",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum PathPickerTarget {
    Single(String),
    AllowPaths(String),
    LabelPolicy { kind: LabelKind, name: String },
}

#[derive(Clone, PartialEq, Default)]
pub(crate) struct PathBrowserState {
    pub(crate) open: bool,
    pub(crate) target: Option<PathPickerTarget>,
    pub(crate) path: String,
    pub(crate) input: String,
    pub(crate) entries: Vec<FsEntry>,
    pub(crate) parent: Option<String>,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NumericKind {
    Integer,
    Float,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NumericError {
    Integer,
    Float,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct SelectOptions {
    pub(crate) allow_empty: bool,
    pub(crate) options: Vec<(String, &'static str)>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct StringListOptions {
    pub(crate) placeholder: &'static str,
    pub(crate) add_label: &'static str,
    pub(crate) empty_label: &'static str,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum FieldControl {
    Toggle,
    Select(SelectOptions),
    Number(NumericKind),
    Text,
    Path,
    PathList,
    StringList(StringListOptions),
    Telemetry,
    LabelPolicies,
    AltSpeed,
    Tracker,
    IpFilter,
    PeerClasses,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SettingsStatus {
    pub(crate) dirty_count: usize,
    pub(crate) has_errors: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct AltSpeedValues {
    pub(crate) download_bps: String,
    pub(crate) upload_bps: String,
    pub(crate) schedule_enabled: bool,
    pub(crate) days: Vec<String>,
    pub(crate) start_time: String,
    pub(crate) end_time: String,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct LabelPolicyEntryValues {
    pub(crate) download_dir: String,
    pub(crate) queue_position: String,
    pub(crate) auto_managed: bool,
    pub(crate) seed_ratio_limit: String,
    pub(crate) seed_time_limit: String,
    pub(crate) rate_download: String,
    pub(crate) rate_upload: String,
    pub(crate) cleanup_seed_ratio: String,
    pub(crate) cleanup_seed_time: String,
    pub(crate) cleanup_remove: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct TrackerValues {
    pub(crate) default_list: Vec<String>,
    pub(crate) extra_list: Vec<String>,
    pub(crate) announce: TrackerAnnounceValues,
    pub(crate) tls: TrackerTlsValues,
    pub(crate) proxy: TrackerProxyValues,
    pub(crate) auth: TrackerAuthValues,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct TrackerAnnounceValues {
    pub(crate) replace: bool,
    pub(crate) announce_to_all: bool,
    pub(crate) user_agent: String,
    pub(crate) announce_ip: String,
    pub(crate) listen_interface: String,
    pub(crate) request_timeout: String,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct TrackerTlsValues {
    pub(crate) cert: String,
    pub(crate) private_key: String,
    pub(crate) ca_cert: String,
    pub(crate) verify: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct TrackerProxyValues {
    pub(crate) enabled: bool,
    pub(crate) host: String,
    pub(crate) port: String,
    pub(crate) kind: String,
    pub(crate) user: String,
    pub(crate) pass: String,
    pub(crate) peers: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct TrackerAuthValues {
    pub(crate) enabled: bool,
    pub(crate) user: String,
    pub(crate) pass: String,
    pub(crate) cookie: String,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct IpFilterValues {
    pub(crate) cidrs: Vec<String>,
    pub(crate) blocklist_url: String,
    pub(crate) etag: String,
    pub(crate) last_updated: String,
    pub(crate) last_error: String,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PeerClassEntry {
    pub(crate) id: u8,
    pub(crate) label: String,
    pub(crate) download_priority: u16,
    pub(crate) upload_priority: u16,
    pub(crate) connection_limit_factor: u16,
    pub(crate) ignore_unchoke_slots: bool,
    pub(crate) is_default: bool,
}

pub(crate) struct EngineGroups {
    pub(crate) downloads: Vec<SettingsField>,
    pub(crate) seeding: Vec<SettingsField>,
    pub(crate) network: Vec<SettingsField>,
    pub(crate) storage: Vec<SettingsField>,
    pub(crate) advanced: Vec<SettingsField>,
}

pub(crate) struct AppGroups {
    pub(crate) info: Vec<SettingsField>,
    pub(crate) telemetry: Vec<SettingsField>,
    pub(crate) labels: Vec<SettingsField>,
    pub(crate) other: Vec<SettingsField>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_tab_metadata_is_stable() {
        let tabs = SettingsTab::all();
        assert_eq!(tabs.len(), 7);
        assert_eq!(
            SettingsTab::Connection.label_key(),
            "settings.tabs.connection"
        );
        assert_eq!(SettingsTab::System.tab_id(), "settings-tab-system");
        assert_eq!(SettingsTab::panel_id(), "settings-panel");
    }

    #[test]
    fn label_kind_keys_are_distinct() {
        assert_eq!(
            LabelKind::Category.label_key(),
            "settings.labels.categories"
        );
        assert_eq!(LabelKind::Tag.label_key(), "settings.labels.tags");
    }

    #[test]
    fn path_browser_state_defaults() {
        let mut state = PathBrowserState::default();
        assert!(!state.open);
        state.target = Some(PathPickerTarget::Single("/tmp".to_string()));
        assert!(matches!(state.target, Some(PathPickerTarget::Single(_))));
        let allow = PathPickerTarget::AllowPaths("/data".to_string());
        assert!(matches!(allow, PathPickerTarget::AllowPaths(_)));
        let label = PathPickerTarget::LabelPolicy {
            kind: LabelKind::Tag,
            name: "demo".to_string(),
        };
        assert!(matches!(label, PathPickerTarget::LabelPolicy { .. }));
    }
}
