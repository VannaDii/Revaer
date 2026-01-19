//! Settings feature pure helpers.
//!
//! # Design
//! - Provide deterministic helpers for settings transformation and validation.
//! - Invariants: field keys are scoped by section; empty input clears optional values.
//! - Failure modes: parsing returns typed errors instead of panicking.

use std::collections::{BTreeMap, HashSet};

use crate::features::settings::state::{
    AltSpeedValues, AppGroups, EngineGroups, FieldControl, FieldDraft, IpFilterValues, LabelKind,
    LabelPolicyEntryValues, NumericError, NumericKind, PeerClassEntry, SelectOptions,
    SettingsDraft, SettingsField, SettingsSection, SettingsStatus, StringListOptions,
    TrackerAnnounceValues, TrackerAuthValues, TrackerProxyValues, TrackerTlsValues, TrackerValues,
};
use crate::i18n::TranslationBundle;
use serde_json::{Map, Value};

const APP_INFO_FIELDS: &[&str] = &[
    "id",
    "instance_name",
    "mode",
    "auth_mode",
    "version",
    "http_port",
    "bind_addr",
    "local_networks",
    "immutable_keys",
];

const APP_TELEMETRY_FIELDS: &[&str] = &["telemetry"];

const APP_LABEL_FIELDS: &[&str] = &["label_policies"];

pub(crate) const LABEL_POLICIES_FIELD_KEY: &str = "app_profile.label_policies";

const DOWNLOAD_FIELDS: &[&str] = &[
    "download_root",
    "resume_dir",
    "sequential_default",
    "auto_managed",
    "max_active",
    "max_download_bps",
    "max_upload_bps",
    "connections_limit",
    "connections_limit_per_torrent",
    "unchoke_slots",
    "half_open_limit",
    "stats_interval_ms",
    "alt_speed",
];

const SEEDING_FIELDS: &[&str] = &[
    "seed_ratio_limit",
    "seed_time_limit",
    "auto_manage_prefer_seeds",
    "dont_count_slow_torrents",
    "super_seeding",
    "strict_super_seeding",
    "choking_algorithm",
    "seed_choking_algorithm",
    "optimistic_unchoke_slots",
];

const NETWORK_FIELDS: &[&str] = &[
    "listen_port",
    "listen_interfaces",
    "ipv6_mode",
    "dht",
    "dht_bootstrap_nodes",
    "dht_router_nodes",
    "encryption",
    "enable_lsd",
    "enable_upnp",
    "enable_natpmp",
    "enable_pex",
    "enable_outgoing_utp",
    "enable_incoming_utp",
    "force_proxy",
    "prefer_rc4",
    "allow_multiple_connections_per_ip",
    "outgoing_port_min",
    "outgoing_port_max",
    "peer_dscp",
    "tracker",
    "ip_filter",
    "peer_classes",
];

const STORAGE_FIELDS: &[&str] = &[
    "storage_mode",
    "use_partfile",
    "disk_read_mode",
    "disk_write_mode",
    "verify_piece_hashes",
    "cache_size",
    "cache_expiry",
    "coalesce_reads",
    "coalesce_writes",
    "use_disk_cache_pool",
    "max_queued_disk_bytes",
];

pub(crate) const WEEKDAYS: [(&str, &str); 7] = [
    ("mon", "settings.weekday.mon"),
    ("tue", "settings.weekday.tue"),
    ("wed", "settings.weekday.wed"),
    ("thu", "settings.weekday.thu"),
    ("fri", "settings.weekday.fri"),
    ("sat", "settings.weekday.sat"),
    ("sun", "settings.weekday.sun"),
];

#[must_use]
pub(crate) fn settings_status(
    snapshot: Option<&Value>,
    draft: &SettingsDraft,
    immutable_keys: &HashSet<String>,
) -> SettingsStatus {
    let Some(snapshot) = snapshot else {
        return SettingsStatus::default();
    };
    let mut status = SettingsStatus::default();
    for section in SettingsSection::all() {
        let Some(section_value) = snapshot.get(section.key()) else {
            continue;
        };
        let Some(map) = section_value.as_object() else {
            continue;
        };
        for (key, original) in map {
            let field_key = format!("{}.{}", section.key(), key);
            if let Some(state) = draft.fields.get(&field_key) {
                if state.error.is_some() {
                    status.has_errors = true;
                }
                if is_field_read_only(section, key, immutable_keys) {
                    continue;
                }
                if original != &state.value {
                    status.dirty_count = status.dirty_count.saturating_add(1);
                }
            }
        }
    }
    status
}

#[must_use]
pub(crate) fn build_changeset_from_snapshot(
    snapshot: &Value,
    draft: &SettingsDraft,
    immutable_keys: &HashSet<String>,
) -> Option<Value> {
    let mut app_patch = None;
    let mut engine_patch = None;
    let mut fs_patch = None;
    for section in SettingsSection::all() {
        let Some(section_value) = snapshot.get(section.key()) else {
            continue;
        };
        let Some(map) = section_value.as_object() else {
            continue;
        };
        let mut updated_map = map.clone();
        let mut dirty = false;
        for (key, original) in map {
            if is_field_read_only(section, key, immutable_keys) {
                continue;
            }
            let field_key = format!("{}.{}", section.key(), key);
            let Some(current) = draft.fields.get(&field_key) else {
                continue;
            };
            if original == &current.value {
                continue;
            }
            updated_map.insert(key.clone(), current.value.clone());
            dirty = true;
        }
        if !dirty {
            continue;
        }
        match section {
            SettingsSection::AppProfile => app_patch = Some(updated_map),
            SettingsSection::EngineProfile => engine_patch = Some(updated_map),
            SettingsSection::FsPolicy => fs_patch = Some(updated_map),
        }
    }

    if app_patch.is_none() && engine_patch.is_none() && fs_patch.is_none() {
        return None;
    }

    let mut root = Map::new();
    if let Some(app_patch) = app_patch {
        root.insert("app_profile".to_string(), Value::Object(app_patch));
    }
    if let Some(engine_patch) = engine_patch {
        root.insert("engine_profile".to_string(), Value::Object(engine_patch));
    }
    if let Some(fs_patch) = fs_patch {
        root.insert("fs_policy".to_string(), Value::Object(fs_patch));
    }

    Some(Value::Object(root))
}

#[must_use]
pub(crate) fn changeset_disables_auth_bypass(changeset: &Value) -> bool {
    changeset
        .get("app_profile")
        .and_then(|profile| profile.get("auth_mode"))
        .and_then(Value::as_str)
        == Some("api_key")
}

#[must_use]
pub(crate) fn build_settings_draft(snapshot: &Value) -> SettingsDraft {
    let mut fields = BTreeMap::new();
    for section in SettingsSection::all() {
        let Some(section_value) = snapshot.get(section.key()) else {
            continue;
        };
        let Some(map) = section_value.as_object() else {
            continue;
        };
        for (key, value) in map {
            let field_key = format!("{}.{}", section.key(), key);
            fields.insert(
                field_key,
                FieldDraft {
                    value: value.clone(),
                    raw: value_to_raw(value),
                    error: None,
                },
            );
        }
    }
    SettingsDraft { fields }
}

#[must_use]
pub(crate) fn collect_section_fields(
    snapshot: Option<&Value>,
    section: SettingsSection,
) -> Vec<SettingsField> {
    let Some(snapshot) = snapshot else {
        return Vec::new();
    };
    let Some(section_value) = snapshot.get(section.key()) else {
        return Vec::new();
    };
    let Some(map) = section_value.as_object() else {
        return Vec::new();
    };
    let mut fields = map
        .keys()
        .map(|key| SettingsField {
            section,
            key: key.clone(),
        })
        .collect::<Vec<_>>();
    fields.sort_by(|a, b| a.key.cmp(&b.key));
    fields
}

#[must_use]
pub(crate) fn split_engine_fields(fields: Vec<SettingsField>) -> EngineGroups {
    let (downloads, remaining) = split_fields(fields, DOWNLOAD_FIELDS);
    let (seeding, remaining) = split_fields(remaining, SEEDING_FIELDS);
    let (network, remaining) = split_fields(remaining, NETWORK_FIELDS);
    let (storage, remaining) = split_fields(remaining, STORAGE_FIELDS);

    EngineGroups {
        downloads,
        seeding,
        network,
        storage,
        advanced: remaining,
    }
}

#[must_use]
pub(crate) fn split_app_fields(fields: Vec<SettingsField>) -> AppGroups {
    let (labels, remaining) = split_fields(fields, APP_LABEL_FIELDS);
    let (telemetry, remaining) = split_fields(remaining, APP_TELEMETRY_FIELDS);
    let (info, remaining) = split_fields(remaining, APP_INFO_FIELDS);
    AppGroups {
        info,
        telemetry,
        labels,
        other: remaining,
    }
}

#[must_use]
pub(crate) fn immutable_key_set(snapshot: Option<&Value>) -> HashSet<String> {
    let mut keys = HashSet::new();
    let Some(snapshot) = snapshot else {
        return keys;
    };
    let Some(app) = snapshot.get(SettingsSection::AppProfile.key()) else {
        return keys;
    };
    let Some(value) = app.get("immutable_keys") else {
        return keys;
    };
    let Some(entries) = value.as_array() else {
        return keys;
    };
    for entry in entries {
        if let Some(item) = entry.as_str() {
            keys.insert(item.to_string());
        }
    }
    keys
}

fn split_fields(
    fields: Vec<SettingsField>,
    names: &[&str],
) -> (Vec<SettingsField>, Vec<SettingsField>) {
    let name_set = names.iter().copied().collect::<HashSet<_>>();
    let mut selected = Vec::new();
    let mut remaining = Vec::new();
    for field in fields {
        if name_set.contains(field.key.as_str()) {
            selected.push(field);
        } else {
            remaining.push(field);
        }
    }
    (selected, remaining)
}

#[must_use]
pub(crate) fn map_string(map: &Map<String, Value>, key: &str) -> String {
    map.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

#[must_use]
pub(crate) fn map_bool(map: &Map<String, Value>, key: &str) -> bool {
    map.get(key).and_then(Value::as_bool).unwrap_or(false)
}

#[must_use]
pub(crate) fn map_array_strings(map: &Map<String, Value>, key: &str) -> Vec<String> {
    map.get(key).map(value_array_as_strings).unwrap_or_default()
}

pub(crate) fn set_optional_string(map: &mut Map<String, Value>, key: &str, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        map.remove(key);
    } else {
        map.insert(key.to_string(), Value::String(trimmed.to_string()));
    }
}

#[must_use]
pub(crate) fn ordered_weekdays(days: &[String]) -> Vec<String> {
    WEEKDAYS
        .iter()
        .filter(|(day, _)| days.iter().any(|entry| entry == *day))
        .map(|(day, _)| (*day).to_string())
        .collect()
}

pub(crate) fn apply_optional_numeric(
    raw: &str,
    kind: NumericKind,
) -> Result<Option<Value>, NumericError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    parse_numeric(kind, trimmed).map(Some)
}

#[must_use]
pub(crate) fn label_policy_entries(
    value: &Value,
    kind: LabelKind,
) -> Vec<(String, Map<String, Value>)> {
    let mut entries = Vec::new();
    let Some(list) = value.as_array() else {
        return entries;
    };
    for entry in list {
        let Some(map) = entry.as_object() else {
            continue;
        };
        let entry_kind = map
            .get("kind")
            .and_then(Value::as_str)
            .and_then(LabelKind::from_str);
        let Some(name) = map.get("name").and_then(Value::as_str) else {
            continue;
        };
        if entry_kind == Some(kind) {
            entries.push((name.to_string(), map.clone()));
        }
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

#[must_use]
pub(crate) fn label_policy_matches(entry: &Value, kind: LabelKind, name: &str) -> bool {
    let Some(map) = entry.as_object() else {
        return false;
    };
    let entry_kind = map
        .get("kind")
        .and_then(Value::as_str)
        .and_then(LabelKind::from_str);
    let entry_name = map.get("name").and_then(Value::as_str);
    entry_kind == Some(kind) && entry_name == Some(name)
}

pub(crate) fn normalize_label_policy_entry(
    kind: LabelKind,
    name: &str,
    policy: &mut Map<String, Value>,
) {
    policy.insert("kind".to_string(), Value::String(kind.key().to_string()));
    policy.insert("name".to_string(), Value::String(name.to_string()));
}

#[must_use]
pub(crate) fn label_policy_download_dir(
    draft: &SettingsDraft,
    kind: LabelKind,
    name: &str,
) -> Option<String> {
    draft
        .fields
        .get(LABEL_POLICIES_FIELD_KEY)
        .and_then(|field| field.value.as_array())
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| label_policy_matches(entry, kind, name))
        })
        .and_then(Value::as_object)
        .and_then(|policy| policy.get("download_dir"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[must_use]
pub(crate) fn validate_tracker_map(
    map: &Map<String, Value>,
    bundle: &TranslationBundle,
) -> Option<String> {
    if let Some(proxy_value) = map.get("proxy") {
        if proxy_value.is_null() {
            return None;
        }
        let Some(proxy) = proxy_value.as_object() else {
            return Some(bundle.text("settings.tracker.error_proxy"));
        };
        let host = proxy
            .get("host")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let port = proxy.get("port").and_then(Value::as_i64).unwrap_or(0);
        if host.is_empty() || !(1..=65_535).contains(&port) {
            return Some(bundle.text("settings.tracker.error_proxy"));
        }
    }

    if let Some(auth_value) = map.get("auth") {
        if auth_value.is_null() {
            return None;
        }
        let Some(auth) = auth_value.as_object() else {
            return Some(bundle.text("settings.tracker.error_auth"));
        };
        let has_secret = ["username_secret", "password_secret", "cookie_secret"]
            .iter()
            .any(|key| {
                auth.get(*key)
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty())
            });
        if !has_secret {
            return Some(bundle.text("settings.tracker.error_auth"));
        }
    }

    None
}

#[must_use]
pub(crate) fn peer_classes_from_value(map: &Map<String, Value>) -> Vec<PeerClassEntry> {
    let defaults = map
        .get("default")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(Value::as_u64)
                .filter_map(|value| u8::try_from(value).ok())
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let mut classes = Vec::new();
    if let Some(entries) = map.get("classes").and_then(Value::as_array) {
        for entry in entries {
            let Some(obj) = entry.as_object() else {
                continue;
            };
            let Some(id_value) = obj.get("id").and_then(Value::as_u64) else {
                continue;
            };
            let Ok(id) = u8::try_from(id_value) else {
                continue;
            };
            if id > 31 {
                continue;
            }
            let label = obj
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let download_priority = obj
                .get("download_priority")
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(1)
                .max(1);
            let upload_priority = obj
                .get("upload_priority")
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(1)
                .max(1);
            let connection_limit_factor = obj
                .get("connection_limit_factor")
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(100)
                .max(1);
            let ignore_unchoke_slots = obj
                .get("ignore_unchoke_slots")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let label = if label.is_empty() {
                format!("class_{id}")
            } else {
                label
            };
            classes.push(PeerClassEntry {
                id,
                label,
                download_priority,
                upload_priority,
                connection_limit_factor,
                ignore_unchoke_slots,
                is_default: defaults.contains(&id),
            });
        }
    }
    classes
}

#[must_use]
pub(crate) fn next_peer_class_id(classes: &[PeerClassEntry]) -> Option<u8> {
    (0..=31).find(|id| classes.iter().all(|entry| entry.id != *id))
}

pub(crate) fn parse_numeric(kind: NumericKind, value: &str) -> Result<Value, NumericError> {
    match kind {
        NumericKind::Integer => value
            .parse::<i64>()
            .map(Value::from)
            .map_err(|_| NumericError::Integer),
        NumericKind::Float => {
            let parsed = value.parse::<f64>().map_err(|_| NumericError::Float)?;
            serde_json::Number::from_f64(parsed)
                .map(Value::Number)
                .ok_or(NumericError::Float)
        }
    }
}

#[must_use]
pub(crate) fn value_to_raw(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Null | Value::Array(_) | Value::Object(_) => String::new(),
    }
}

#[must_use]
pub(crate) fn value_to_display(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(entries) => entries
            .iter()
            .map(value_to_display)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(map) => {
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort();
            keys.iter()
                .map(|key| {
                    let value = map.get(*key).map(value_to_display).unwrap_or_default();
                    format!("{key}: {value}")
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

#[must_use]
pub(crate) fn value_array_as_strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[must_use]
pub(crate) fn field_label(
    bundle: &TranslationBundle,
    section: SettingsSection,
    key: &str,
) -> String {
    let translation_key = format!("settings.fields.{}.{}", section.key(), key);
    let translated = bundle.text(&translation_key);
    if translated == translation_key || translated.starts_with("missing:") {
        humanize_key(key)
    } else {
        translated
    }
}

#[must_use]
pub(crate) fn humanize_key(key: &str) -> String {
    let mut out = String::new();
    for (idx, segment) in key.split('_').enumerate() {
        if segment.is_empty() {
            continue;
        }
        if idx > 0 {
            out.push(' ');
        }
        let mut chars = segment.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            for ch in chars {
                out.push(ch);
            }
        }
    }
    out
}

#[must_use]
pub(crate) fn is_field_read_only(
    section: SettingsSection,
    key: &str,
    immutable_keys: &HashSet<String>,
) -> bool {
    if matches!(
        (section, key),
        (
            SettingsSection::AppProfile
                | SettingsSection::EngineProfile
                | SettingsSection::FsPolicy,
            "id"
        ) | (
            SettingsSection::AppProfile,
            "version" | "mode" | "bind_addr" | "http_port" | "immutable_keys"
        ) | (SettingsSection::EngineProfile, "implementation")
    ) {
        return true;
    }
    let scoped = format!("{}.{}", section.key(), key);
    let scoped_wildcard = format!("{}.*", section.key());
    immutable_keys.contains(section.key())
        || immutable_keys.contains(key)
        || immutable_keys.contains(&scoped)
        || immutable_keys.contains(&scoped_wildcard)
}

#[must_use]
pub(crate) fn control_for_field(
    section: SettingsSection,
    key: &str,
    value: &Value,
) -> FieldControl {
    if matches!((section, key), (SettingsSection::AppProfile, "telemetry")) {
        return FieldControl::Telemetry;
    }
    if matches!(
        (section, key),
        (SettingsSection::AppProfile, "label_policies")
    ) {
        return FieldControl::LabelPolicies;
    }
    if matches!(
        (section, key),
        (SettingsSection::EngineProfile, "alt_speed")
    ) {
        return FieldControl::AltSpeed;
    }
    if matches!((section, key), (SettingsSection::EngineProfile, "tracker")) {
        return FieldControl::Tracker;
    }
    if matches!(
        (section, key),
        (SettingsSection::EngineProfile, "ip_filter")
    ) {
        return FieldControl::IpFilter;
    }
    if matches!(
        (section, key),
        (SettingsSection::EngineProfile, "peer_classes")
    ) {
        return FieldControl::PeerClasses;
    }
    if section == SettingsSection::FsPolicy && key == "allow_paths" {
        return FieldControl::PathList;
    }
    if is_directory_field(section, key) {
        return FieldControl::Path;
    }
    if let Some(options) = select_options(section, key, value) {
        return FieldControl::Select(options);
    }
    if let Some(kind) = numeric_kind(section, key) {
        return FieldControl::Number(kind);
    }
    if value.is_boolean() {
        return FieldControl::Toggle;
    }
    if value.is_array() {
        return FieldControl::StringList(
            string_list_options(section, key).unwrap_or_else(default_list_options),
        );
    }
    FieldControl::Text
}

#[must_use]
pub(crate) fn string_list_options(
    section: SettingsSection,
    key: &str,
) -> Option<StringListOptions> {
    let options = match (section, key) {
        (SettingsSection::AppProfile, "local_networks") => StringListOptions {
            placeholder: "settings.list.local_networks_placeholder",
            add_label: "settings.add",
            empty_label: "settings.list.empty",
        },
        (SettingsSection::EngineProfile, "listen_interfaces") => StringListOptions {
            placeholder: "settings.list.listen_interfaces_placeholder",
            add_label: "settings.add",
            empty_label: "settings.list.empty",
        },
        (SettingsSection::EngineProfile, "dht_bootstrap_nodes") => StringListOptions {
            placeholder: "settings.list.dht_bootstrap_placeholder",
            add_label: "settings.add",
            empty_label: "settings.list.empty",
        },
        (SettingsSection::EngineProfile, "dht_router_nodes") => StringListOptions {
            placeholder: "settings.list.dht_router_placeholder",
            add_label: "settings.add",
            empty_label: "settings.list.empty",
        },
        (SettingsSection::FsPolicy, "cleanup_keep") => StringListOptions {
            placeholder: "settings.list.cleanup_keep_placeholder",
            add_label: "settings.add",
            empty_label: "settings.list.empty",
        },
        (SettingsSection::FsPolicy, "cleanup_drop") => StringListOptions {
            placeholder: "settings.list.cleanup_drop_placeholder",
            add_label: "settings.add",
            empty_label: "settings.list.empty",
        },
        _ => return None,
    };
    Some(options)
}

#[must_use]
pub(crate) const fn default_list_options() -> StringListOptions {
    StringListOptions {
        placeholder: "settings.list.placeholder",
        add_label: "settings.add",
        empty_label: "settings.list.empty",
    }
}

#[must_use]
pub(crate) fn is_directory_field(section: SettingsSection, key: &str) -> bool {
    matches!(
        (section, key),
        (
            SettingsSection::EngineProfile,
            "download_root" | "resume_dir"
        ) | (SettingsSection::FsPolicy, "library_root")
    )
}

#[must_use]
pub(crate) fn select_options(
    section: SettingsSection,
    key: &str,
    value: &Value,
) -> Option<SelectOptions> {
    let allow_empty = value.is_null()
        || matches!(
            (section, key),
            (
                SettingsSection::EngineProfile,
                "disk_read_mode" | "disk_write_mode"
            )
        );
    let options = match (section, key) {
        (SettingsSection::AppProfile, "auth_mode") => vec![
            ("api_key".to_string(), "settings.option.api_key"),
            ("none".to_string(), "settings.option.no_auth"),
        ],
        (SettingsSection::EngineProfile, "ipv6_mode") => vec![
            ("disabled".to_string(), "settings.option.disabled"),
            ("enabled".to_string(), "settings.option.enabled"),
            ("prefer_v6".to_string(), "settings.option.prefer_v6"),
        ],
        (SettingsSection::EngineProfile, "encryption") => vec![
            ("require".to_string(), "settings.option.require"),
            ("prefer".to_string(), "settings.option.prefer"),
            ("disable".to_string(), "settings.option.disable"),
        ],
        (SettingsSection::EngineProfile, "choking_algorithm") => vec![
            ("fixed_slots".to_string(), "settings.option.fixed_slots"),
            ("rate_based".to_string(), "settings.option.rate_based"),
        ],
        (SettingsSection::EngineProfile, "seed_choking_algorithm") => vec![
            ("round_robin".to_string(), "settings.option.round_robin"),
            (
                "fastest_upload".to_string(),
                "settings.option.fastest_upload",
            ),
            ("anti_leech".to_string(), "settings.option.anti_leech"),
        ],
        (SettingsSection::EngineProfile, "storage_mode") => vec![
            ("sparse".to_string(), "settings.option.sparse"),
            ("allocate".to_string(), "settings.option.allocate"),
        ],
        (SettingsSection::EngineProfile, "disk_read_mode" | "disk_write_mode") => vec![
            (
                "enable_os_cache".to_string(),
                "settings.option.enable_os_cache",
            ),
            (
                "disable_os_cache".to_string(),
                "settings.option.disable_os_cache",
            ),
            ("write_through".to_string(), "settings.option.write_through"),
        ],
        (SettingsSection::FsPolicy, "par2") => vec![
            ("off".to_string(), "settings.option.off"),
            ("verify".to_string(), "settings.option.verify"),
            ("repair".to_string(), "settings.option.repair"),
        ],
        (SettingsSection::FsPolicy, "move_mode") => vec![
            ("copy".to_string(), "settings.option.copy"),
            ("move".to_string(), "settings.option.move"),
            ("hardlink".to_string(), "settings.option.hardlink"),
        ],
        _ => return None,
    };
    Some(SelectOptions {
        allow_empty,
        options,
    })
}

#[must_use]
pub(crate) fn numeric_kind(section: SettingsSection, key: &str) -> Option<NumericKind> {
    if section != SettingsSection::EngineProfile {
        return None;
    }
    match key {
        "listen_port"
        | "outgoing_port_min"
        | "outgoing_port_max"
        | "peer_dscp"
        | "max_active"
        | "connections_limit"
        | "connections_limit_per_torrent"
        | "unchoke_slots"
        | "half_open_limit"
        | "optimistic_unchoke_slots"
        | "cache_size"
        | "cache_expiry"
        | "max_download_bps"
        | "max_upload_bps"
        | "seed_time_limit"
        | "stats_interval_ms"
        | "max_queued_disk_bytes" => Some(NumericKind::Integer),
        "seed_ratio_limit" => Some(NumericKind::Float),
        _ => None,
    }
}

#[must_use]
pub(crate) fn alt_speed_values(map: &Map<String, Value>) -> AltSpeedValues {
    let download_bps = map
        .get("download_bps")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let upload_bps = map
        .get("upload_bps")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let schedule = map.get("schedule").and_then(Value::as_object).cloned();
    let schedule_enabled = schedule.is_some();
    let schedule_map = schedule.unwrap_or_default();
    let days = schedule_map
        .get("days")
        .map(value_array_as_strings)
        .unwrap_or_default();
    let start_time = schedule_map
        .get("start")
        .and_then(Value::as_str)
        .unwrap_or("00:00")
        .to_string();
    let end_time = schedule_map
        .get("end")
        .and_then(Value::as_str)
        .unwrap_or("23:59")
        .to_string();
    AltSpeedValues {
        download_bps,
        upload_bps,
        schedule_enabled,
        days,
        start_time,
        end_time,
    }
}

#[must_use]
pub(crate) fn label_policy_entry_values(policy: &Map<String, Value>) -> LabelPolicyEntryValues {
    let download_dir = policy
        .get("download_dir")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let queue_position = policy
        .get("queue_position")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let auto_managed = policy
        .get("auto_managed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let seed_ratio_limit = policy
        .get("seed_ratio_limit")
        .and_then(Value::as_f64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let seed_time_limit = policy
        .get("seed_time_limit")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let rate_download = policy
        .get("rate_limit_download_bps")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let rate_upload = policy
        .get("rate_limit_upload_bps")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let cleanup_seed_ratio = policy
        .get("cleanup_seed_ratio_limit")
        .and_then(Value::as_f64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let cleanup_seed_time = policy
        .get("cleanup_seed_time_limit")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let cleanup_remove = policy
        .get("cleanup_remove_data")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    LabelPolicyEntryValues {
        download_dir,
        queue_position,
        auto_managed,
        seed_ratio_limit,
        seed_time_limit,
        rate_download,
        rate_upload,
        cleanup_seed_ratio,
        cleanup_seed_time,
        cleanup_remove,
    }
}

#[must_use]
pub(crate) fn tracker_values(map: &Map<String, Value>) -> TrackerValues {
    let default_list = map_array_strings(map, "default");
    let extra_list = map_array_strings(map, "extra");
    let announce = TrackerAnnounceValues {
        replace: map_bool(map, "replace"),
        announce_to_all: map_bool(map, "announce_to_all"),
        user_agent: map_string(map, "user_agent"),
        announce_ip: map_string(map, "announce_ip"),
        listen_interface: map_string(map, "listen_interface"),
        request_timeout: map
            .get("request_timeout_ms")
            .and_then(Value::as_i64)
            .map(|value| value.to_string())
            .unwrap_or_default(),
    };
    let tls = TrackerTlsValues {
        cert: map_string(map, "ssl_cert"),
        private_key: map_string(map, "ssl_private_key"),
        ca_cert: map_string(map, "ssl_ca_cert"),
        verify: map
            .get("ssl_tracker_verify")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    };

    let proxy_value = map.get("proxy");
    let proxy_enabled = proxy_value.is_some_and(|value| !value.is_null());
    let proxy_map = proxy_value
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let proxy = TrackerProxyValues {
        enabled: proxy_enabled,
        host: map_string(&proxy_map, "host"),
        port: proxy_map
            .get("port")
            .and_then(Value::as_i64)
            .map(|value| value.to_string())
            .unwrap_or_default(),
        kind: map_string(&proxy_map, "kind"),
        user: map_string(&proxy_map, "username_secret"),
        pass: map_string(&proxy_map, "password_secret"),
        peers: map_bool(&proxy_map, "proxy_peers"),
    };

    let auth_value = map.get("auth");
    let auth_enabled = auth_value.is_some_and(|value| !value.is_null());
    let auth_map = auth_value
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let auth = TrackerAuthValues {
        enabled: auth_enabled,
        user: map_string(&auth_map, "username_secret"),
        pass: map_string(&auth_map, "password_secret"),
        cookie: map_string(&auth_map, "cookie_secret"),
    };

    TrackerValues {
        default_list,
        extra_list,
        announce,
        tls,
        proxy,
        auth,
    }
}

#[must_use]
pub(crate) fn ip_filter_values(map: &Map<String, Value>) -> IpFilterValues {
    let cidrs = map_array_strings(map, "cidrs");
    let blocklist_url = map_string(map, "blocklist_url");
    let etag = map_string(map, "etag");
    let last_updated = map_string(map, "last_updated_at");
    let last_error = map_string(map, "last_error");
    IpFilterValues {
        cidrs,
        blocklist_url,
        etag,
        last_updated,
        last_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};

    #[test]
    fn ordered_weekdays_respects_order() {
        let input = vec!["sun".to_string(), "mon".to_string()];
        assert_eq!(ordered_weekdays(&input), vec!["mon", "sun"]);
    }

    #[test]
    fn parse_numeric_rejects_invalid() {
        assert!(parse_numeric(NumericKind::Integer, "oops").is_err());
        assert!(parse_numeric(NumericKind::Float, "1.5").is_ok());
    }

    #[test]
    fn label_policy_entries_filters_kind() {
        let value = serde_json::json!([
            {"kind":"category","name":"tv"},
            {"kind":"tag","name":"hdr"}
        ]);
        let entries = label_policy_entries(&value, LabelKind::Category);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "tv");
    }

    #[test]
    fn settings_status_detects_dirty() {
        let snapshot = serde_json::json!({
            "app_profile": {"instance_name":"demo"}
        });
        let mut draft = build_settings_draft(&snapshot);
        if let Some(field) = draft.fields.get_mut("app_profile.instance_name") {
            field.value = Value::String("updated".to_string());
        }
        let status = settings_status(Some(&snapshot), &draft, &HashSet::new());
        assert!(status.dirty_count > 0);
    }

    #[test]
    fn validate_tracker_map_requires_proxy_host() {
        let bundle = TranslationBundle::new(DEFAULT_LOCALE);
        let map = serde_json::json!({"proxy": {"host": "", "port": 0}});
        let map = map.as_object().expect("map");
        assert!(validate_tracker_map(map, &bundle).is_some());
    }

    #[test]
    fn settings_changeset_groups_and_keys() {
        let snapshot = serde_json::json!({
            "app_profile": {
                "instance_name": "demo",
                "label_policies": [],
                "telemetry": {},
                "auth_mode": "none",
                "immutable_keys": ["engine_profile.listen_port"]
            },
            "engine_profile": {
                "download_root": "/downloads",
                "seed_ratio_limit": 1.5,
                "listen_port": 6881,
                "storage_mode": "sparse"
            },
            "fs_policy": {
                "library_root": "/library"
            }
        });
        let immutable_keys = immutable_key_set(Some(&snapshot));
        assert!(immutable_keys.contains("engine_profile.listen_port"));

        let mut draft = build_settings_draft(&snapshot);
        if let Some(field) = draft.fields.get_mut("app_profile.instance_name") {
            field.value = Value::String("updated".to_string());
        }
        let changeset =
            build_changeset_from_snapshot(&snapshot, &draft, &immutable_keys).expect("changeset");
        assert!(changeset.get("app_profile").is_some());
        assert!(changeset_disables_auth_bypass(&serde_json::json!({
            "app_profile": {"auth_mode": "api_key"}
        })));

        let app_fields = collect_section_fields(Some(&snapshot), SettingsSection::AppProfile);
        let app_groups = split_app_fields(app_fields);
        assert!(
            app_groups
                .info
                .iter()
                .any(|field| field.key == "instance_name")
        );
        assert!(
            app_groups
                .labels
                .iter()
                .any(|field| field.key == "label_policies")
        );
        assert!(
            app_groups
                .telemetry
                .iter()
                .any(|field| field.key == "telemetry")
        );
        assert!(app_groups.other.is_empty());

        let engine_fields = collect_section_fields(Some(&snapshot), SettingsSection::EngineProfile);
        let engine_groups = split_engine_fields(engine_fields);
        assert!(
            engine_groups
                .downloads
                .iter()
                .any(|field| field.key == "download_root")
        );
        assert!(
            engine_groups
                .seeding
                .iter()
                .any(|field| field.key == "seed_ratio_limit")
        );
        assert!(
            engine_groups
                .network
                .iter()
                .any(|field| field.key == "listen_port")
        );
        assert!(
            engine_groups
                .storage
                .iter()
                .any(|field| field.key == "storage_mode")
        );
        assert!(engine_groups.advanced.is_empty());
    }

    #[test]
    fn settings_value_helpers_smoke() {
        let mut map = Map::new();
        map.insert("name".to_string(), Value::String("demo".to_string()));
        map.insert("enabled".to_string(), Value::Bool(true));
        map.insert(
            "tags".to_string(),
            Value::Array(vec![
                Value::String("alpha".to_string()),
                Value::String("beta".to_string()),
            ]),
        );

        assert_eq!(map_string(&map, "name"), "demo");
        assert!(map_bool(&map, "enabled"));
        assert_eq!(map_array_strings(&map, "tags"), vec!["alpha", "beta"]);

        set_optional_string(&mut map, "name", " ");
        assert!(map.get("name").is_none());
        set_optional_string(&mut map, "name", " ok ");
        assert_eq!(map_string(&map, "name"), "ok");

        assert!(
            apply_optional_numeric("", NumericKind::Integer)
                .expect("parse empty")
                .is_none()
        );
        assert!(
            apply_optional_numeric("3", NumericKind::Integer)
                .expect("parse int")
                .is_some()
        );

        let array = serde_json::json!(["a", "b"]);
        assert_eq!(value_array_as_strings(&array), vec!["a", "b"]);
        let display = value_to_display(&serde_json::json!({"key":"value","num":1}));
        assert!(display.contains("key: value"));
    }

    #[test]
    fn label_policy_helpers_round_trip() {
        let mut policy = Map::new();
        normalize_label_policy_entry(LabelKind::Category, "tv", &mut policy);
        policy.insert(
            "download_dir".to_string(),
            Value::String("/media".to_string()),
        );
        let entry = Value::Object(policy.clone());
        assert!(label_policy_matches(&entry, LabelKind::Category, "tv"));

        let mut draft = SettingsDraft::default();
        draft.fields.insert(
            LABEL_POLICIES_FIELD_KEY.to_string(),
            FieldDraft {
                value: Value::Array(vec![Value::Object(policy)]),
                raw: String::new(),
                error: None,
            },
        );
        assert_eq!(
            label_policy_download_dir(&draft, LabelKind::Category, "tv").as_deref(),
            Some("/media")
        );
    }

    #[test]
    fn peer_class_helpers_keep_defaults() {
        let value = serde_json::json!({
            "default": [1],
            "classes": [
                {
                    "id": 0,
                    "label": "base",
                    "download_priority": 1,
                    "upload_priority": 2,
                    "connection_limit_factor": 1,
                    "ignore_unchoke_slots": false
                },
                {
                    "id": 1,
                    "label": "priority",
                    "download_priority": 3,
                    "upload_priority": 4,
                    "connection_limit_factor": 2,
                    "ignore_unchoke_slots": true
                }
            ]
        });
        let classes = peer_classes_from_value(value.as_object().expect("classes map"));
        assert_eq!(classes.len(), 2);
        assert!(
            classes
                .iter()
                .any(|entry| entry.id == 1 && entry.is_default)
        );
        assert_eq!(next_peer_class_id(&classes), Some(2));
    }

    #[test]
    fn field_label_and_control_helpers() {
        let bundle = TranslationBundle::new(DEFAULT_LOCALE);
        assert_eq!(humanize_key("seed_ratio_limit"), "Seed Ratio Limit");
        assert_eq!(
            field_label(&bundle, SettingsSection::AppProfile, "custom_key"),
            "Custom Key"
        );

        let telemetry = control_for_field(
            SettingsSection::AppProfile,
            "telemetry",
            &Value::Object(Map::new()),
        );
        assert!(matches!(telemetry, FieldControl::Telemetry));

        let alt_speed = control_for_field(
            SettingsSection::EngineProfile,
            "alt_speed",
            &Value::Object(Map::new()),
        );
        assert!(matches!(alt_speed, FieldControl::AltSpeed));

        let list_control = control_for_field(
            SettingsSection::FsPolicy,
            "cleanup_keep",
            &Value::Array(vec![]),
        );
        match list_control {
            FieldControl::StringList(options) => {
                assert_eq!(
                    options.placeholder,
                    "settings.list.cleanup_keep_placeholder"
                );
            }
            _ => panic!("expected string list control"),
        }

        let numeric = control_for_field(
            SettingsSection::EngineProfile,
            "listen_port",
            &Value::Number(serde_json::Number::from(6881)),
        );
        assert!(matches!(
            numeric,
            FieldControl::Number(NumericKind::Integer)
        ));

        let options = string_list_options(SettingsSection::EngineProfile, "listen_interfaces")
            .expect("string list options");
        assert_eq!(
            options.placeholder,
            "settings.list.listen_interfaces_placeholder"
        );
        let defaults = default_list_options();
        assert_eq!(defaults.placeholder, "settings.list.placeholder");

        assert!(is_directory_field(
            SettingsSection::EngineProfile,
            "download_root"
        ));
        assert!(!is_directory_field(
            SettingsSection::EngineProfile,
            "seed_ratio_limit"
        ));

        let select = select_options(SettingsSection::AppProfile, "auth_mode", &Value::Null)
            .expect("select options");
        assert!(select.allow_empty);

        assert!(matches!(
            numeric_kind(SettingsSection::EngineProfile, "listen_port"),
            Some(NumericKind::Integer)
        ));
        assert!(matches!(
            numeric_kind(SettingsSection::EngineProfile, "seed_ratio_limit"),
            Some(NumericKind::Float)
        ));
    }

    #[test]
    fn settings_value_structs_build_expected_defaults() {
        let alt_speed_map = serde_json::json!({
            "download_bps": 123,
            "upload_bps": 456,
            "schedule": {
                "days": ["mon", "tue"],
                "start": "08:00",
                "end": "17:00"
            }
        });
        let alt_speed = alt_speed_values(alt_speed_map.as_object().expect("alt speed map"));
        assert_eq!(alt_speed.download_bps, "123");
        assert_eq!(alt_speed.upload_bps, "456");
        assert!(alt_speed.schedule_enabled);
        assert_eq!(alt_speed.days, vec!["mon", "tue"]);

        let policy_map = serde_json::json!({
            "download_dir": "/media",
            "queue_position": 3,
            "auto_managed": true,
            "seed_ratio_limit": 1.5,
            "seed_time_limit": 42,
            "rate_limit_download_bps": 10,
            "rate_limit_upload_bps": 20,
            "cleanup_seed_ratio_limit": 1.0,
            "cleanup_seed_time_limit": 60,
            "cleanup_remove_data": true
        });
        let policy_values = label_policy_entry_values(policy_map.as_object().expect("policy map"));
        assert_eq!(policy_values.download_dir, "/media");
        assert_eq!(policy_values.queue_position, "3");
        assert!(policy_values.cleanup_remove);

        let tracker_map = serde_json::json!({
            "default": ["https://tracker.local/announce"],
            "extra": ["https://extra.local/announce"],
            "replace": true,
            "announce_to_all": false,
            "user_agent": "ua",
            "announce_ip": "1.2.3.4",
            "listen_interface": "eth0",
            "request_timeout_ms": 1500,
            "ssl_cert": "cert",
            "ssl_private_key": "key",
            "ssl_ca_cert": "ca",
            "ssl_tracker_verify": false,
            "proxy": {
                "host": "proxy",
                "port": 8080,
                "kind": "socks5",
                "username_secret": "user",
                "password_secret": "pass",
                "proxy_peers": true
            },
            "auth": {
                "username_secret": "user",
                "password_secret": "pass",
                "cookie_secret": "cookie"
            }
        });
        let tracker_values = tracker_values(tracker_map.as_object().expect("tracker map"));
        assert_eq!(tracker_values.default_list.len(), 1);
        assert_eq!(tracker_values.extra_list.len(), 1);
        assert_eq!(tracker_values.announce.user_agent, "ua");
        assert!(tracker_values.proxy.enabled);
        assert!(tracker_values.auth.enabled);

        let ip_filter_map = serde_json::json!({
            "cidrs": ["10.0.0.0/8"],
            "blocklist_url": "https://blocklist.local",
            "etag": "etag",
            "last_updated_at": "2024-01-01T00:00:00Z",
            "last_error": "timeout"
        });
        let ip_filter = ip_filter_values(ip_filter_map.as_object().expect("ip filter map"));
        assert_eq!(ip_filter.cidrs.len(), 1);
        assert_eq!(ip_filter.blocklist_url, "https://blocklist.local");
        assert_eq!(ip_filter.etag, "etag");
        assert_eq!(ip_filter.last_updated, "2024-01-01T00:00:00Z");
        assert_eq!(ip_filter.last_error, "timeout");
    }
}
