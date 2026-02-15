pub mod artifact;
pub mod oci;
pub mod qcow2;

use std::path::Path;

use spec_parser::schema::{Target, TargetKind};
use tracing::info;

use crate::error::ForgeError;
use crate::tools::ToolRunner;

/// Execute Phase 2: produce the target artifact from the staged rootfs.
pub async fn execute(
    target: &Target,
    staging_root: &Path,
    files_dir: &Path,
    output_dir: &Path,
    runner: &dyn ToolRunner,
) -> Result<(), ForgeError> {
    info!(
        target = %target.name,
        kind = %target.kind,
        "Starting Phase 2: target production"
    );

    match target.kind {
        TargetKind::Oci => {
            oci::build_oci(target, staging_root, output_dir)?;
        }
        TargetKind::Qcow2 => {
            qcow2::build_qcow2(target, staging_root, output_dir, runner).await?;
        }
        TargetKind::Artifact => {
            artifact::build_artifact(target, staging_root, output_dir, files_dir)?;
        }
    }

    info!(target = %target.name, "Phase 2 complete");
    Ok(())
}
