//! Nexus-based dashboard feature slice.

#[cfg(target_arch = "wasm32")]
mod disk_usage;
#[cfg(target_arch = "wasm32")]
mod global_summary;
#[cfg(target_arch = "wasm32")]
mod queue_summary;
#[cfg(target_arch = "wasm32")]
mod recent_events;
#[cfg(target_arch = "wasm32")]
mod shell;
#[cfg(target_arch = "wasm32")]
mod stats_cards;
#[cfg(target_arch = "wasm32")]
mod tracker_health;
#[cfg(target_arch = "wasm32")]
pub(crate) mod view;

#[cfg(target_arch = "wasm32")]
pub(crate) use view::DashboardPage;
