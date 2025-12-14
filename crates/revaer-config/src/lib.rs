#![forbid(unsafe_code)]
#![deny(
    warnings,
    dead_code,
    unused,
    unused_imports,
    unused_must_use,
    unreachable_pub,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]

//! Database-backed configuration facade built on `PostgreSQL`.
//!
//! Layout: `model.rs` (typed config models and changesets), `validate.rs`
//! (validation/parsing helpers), `service.rs` (`ConfigService` + `SettingsFacade`).

pub mod engine_profile;
pub mod model;
pub mod service;
pub(crate) mod validate;

pub use engine_profile::{
    EngineBehaviorConfig, EngineEncryptionPolicy, EngineIpv6Mode, EngineLimitsConfig,
    EngineNetworkConfig, EngineProfileEffective, EngineStorageConfig, IpFilterConfig, IpFilterRule,
    MAX_RATE_LIMIT_BPS, TrackerConfig, TrackerProxyConfig, TrackerProxyType,
    normalize_engine_profile,
};
pub use model::{
    ApiKeyAuth, ApiKeyPatch, ApiKeyRateLimit, AppMode, AppProfile, AppliedChanges, ConfigSnapshot,
    EngineProfile, FsPolicy, SecretPatch, SettingsChange, SettingsChangeset, SettingsPayload,
    SetupToken,
};
pub use service::{ConfigService, ConfigWatcher, SettingsFacade, SettingsStream};
pub use validate::ConfigError;
