//! Filesystem post-processing placeholder crate.

use anyhow::Result;
use revaer_config::FsPolicy;
use tracing::info;

pub struct FsOpsService;

impl FsOpsService {
    #[allow(clippy::missing_errors_doc)]
    pub fn apply_policy(&self, policy: &FsPolicy) -> Result<()> {
        info!("Applying filesystem policy at {}", policy.library_root);
        Ok(())
    }
}
