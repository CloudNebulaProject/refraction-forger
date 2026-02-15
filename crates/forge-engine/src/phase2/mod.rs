pub mod artifact;
pub mod oci;
pub mod qcow2;
pub mod qcow2_ext4;
pub mod qcow2_zfs;

use std::path::Path;

use spec_parser::schema::{Target, TargetKind};
use tracing::info;

use crate::error::ForgeError;
use crate::tools::ToolRunner;

/// Execute Phase 2: produce the target artifact from the staged rootfs.
///
/// After building the artifact, if a `push_to` reference is set on a QCOW2 target,
/// the QCOW2 file is automatically pushed as an OCI artifact.
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

    // Auto-push QCOW2 to OCI registry if push_to is set
    if target.kind == TargetKind::Qcow2 {
        if let Some(ref push_ref) = target.push_to {
            let qcow2_path = output_dir.join(format!("{}.qcow2", target.name));
            info!(
                reference = %push_ref,
                path = %qcow2_path.display(),
                "Auto-pushing QCOW2 artifact to OCI registry"
            );

            let qcow2_data = std::fs::read(&qcow2_path).map_err(|e| {
                ForgeError::ArtifactPushFailed {
                    reference: push_ref.clone(),
                    detail: format!("failed to read QCOW2 file: {e}"),
                }
            })?;

            let metadata = forge_oci::artifact::Qcow2Metadata {
                name: target.name.clone(),
                version: "latest".to_string(),
                architecture: "amd64".to_string(),
                os: "linux".to_string(),
                description: None,
            };

            let auth = forge_oci::artifact::resolve_ghcr_auth();

            forge_oci::artifact::push_qcow2_artifact(push_ref, qcow2_data, &metadata, &auth, &[])
                .await
                .map_err(|e| ForgeError::ArtifactPushFailed {
                    reference: push_ref.clone(),
                    detail: e.to_string(),
                })?;
        }
    }

    info!(target = %target.name, "Phase 2 complete");
    Ok(())
}
