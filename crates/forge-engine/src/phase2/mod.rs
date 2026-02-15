pub mod artifact;
pub mod oci;
pub mod qcow2;
pub mod qcow2_ext4;
pub mod qcow2_zfs;

use std::path::Path;

use spec_parser::schema::{Target, TargetKind};
use tracing::info;

use crate::error::ForgeError;

/// Execute Phase 2 for non-QCOW2 targets (OCI, Artifact).
///
/// QCOW2 targets are handled directly by the orchestrator in `lib.rs` via
/// the prepare/finalize/cleanup flow.
pub async fn execute(
    target: &Target,
    staging_root: &Path,
    files_dir: &Path,
    output_dir: &Path,
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
        TargetKind::Artifact => {
            artifact::build_artifact(target, staging_root, output_dir, files_dir)?;
        }
        TargetKind::Qcow2 => {
            unreachable!("QCOW2 targets are handled by the orchestrator via prepare/finalize/cleanup");
        }
    }

    info!(target = %target.name, "Phase 2 complete");
    Ok(())
}

/// Push a QCOW2 file to an OCI registry if `push_to` is set on the target.
pub async fn push_qcow2_if_configured(
    target: &Target,
    output_dir: &Path,
) -> Result<(), ForgeError> {
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

    Ok(())
}
