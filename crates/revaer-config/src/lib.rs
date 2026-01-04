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
    clippy::cargo,
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]
#![allow(clippy::multiple_crate_versions)]

//! Database-backed configuration facade built on `PostgreSQL`.
//!
//! Layout: `model.rs` (typed config models and changesets), `validate.rs`
//! (validation/parsing helpers), `loader.rs` (`ConfigService` + `SettingsFacade`).

#[cfg(not(target_arch = "wasm32"))]
pub mod defaults;
pub mod engine_profile;
pub mod error;
#[cfg(not(target_arch = "wasm32"))]
pub mod loader;
pub mod model;
pub mod validate;

pub use engine_profile::{
    EngineBehaviorConfig, EngineEncryptionPolicy, EngineIpv6Mode, EngineLimitsConfig,
    EngineNetworkConfig, EngineProfileEffective, EngineStorageConfig, IpFilterConfig, IpFilterRule,
    MAX_RATE_LIMIT_BPS, TrackerAuthConfig, TrackerConfig, TrackerProxyConfig, TrackerProxyType,
    normalize_engine_profile,
};
pub use error::{ConfigError, ConfigResult};
#[cfg(not(target_arch = "wasm32"))]
pub use loader::{ConfigService, ConfigWatcher, SettingsFacade, SettingsStream};
pub use model::{
    ApiKeyAuth, ApiKeyPatch, ApiKeyRateLimit, AppAuthMode, AppMode, AppProfile, AppliedChanges,
    ConfigSnapshot, EngineProfile, FsPolicy, LabelKind, LabelPolicy, SecretPatch, SettingsChange,
    SettingsChangeset, SettingsPayload, SetupToken, TelemetryConfig,
};
