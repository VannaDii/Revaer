//! API helpers for label policies.
//!
//! # Design
//! - Keep HTTP calls localized to the feature layer.
//! - Reuse the shared ApiClient for auth and error handling.

use crate::features::labels::state::LabelKind;
use crate::models::{TorrentLabelEntry, TorrentLabelPolicy};
use crate::services::api::{ApiClient, ApiError};

/// Upsert a category or tag label policy.
pub(crate) async fn upsert_label(
    client: &ApiClient,
    kind: LabelKind,
    name: &str,
    policy: &TorrentLabelPolicy,
) -> Result<TorrentLabelEntry, ApiError> {
    match kind {
        LabelKind::Category => client.upsert_category(name, policy).await,
        LabelKind::Tag => client.upsert_tag(name, policy).await,
    }
}
