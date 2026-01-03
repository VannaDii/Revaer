//! Domain models for filesystem post-processing.
//!
//! # Design
//! - Keep request/response types lightweight and copyable.
//! - Avoid embedding IO handles; callers supply references.

use std::path::Path;

use revaer_config::FsPolicy;
use uuid::Uuid;

/// Immutable inputs provided to the filesystem pipeline for a completed torrent.
#[derive(Copy, Clone)]
pub struct FsOpsRequest<'a> {
    /// Identifier of the torrent the operation applies to.
    pub torrent_id: Uuid,
    /// Absolute staging path that contains the downloaded payload.
    pub source_path: &'a Path,
    /// Filesystem policy snapshot describing how to handle the payload.
    pub policy: &'a FsPolicy,
}
