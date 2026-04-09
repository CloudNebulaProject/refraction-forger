use std::path::Path;

use spec_parser::schema::{DistroFamily, ImageSpec};
use tracing::info;

use crate::error::BuilderError;

/// Push QCOW2 outputs to OCI registries from the host side.
///
/// Iterates over spec targets, finds QCOW2 files with `push_to` set,
/// and pushes them via `forge_oci::artifact`. This mirrors
/// `forge_engine::phase2::push_qcow2_if_configured()` but runs on the
/// host where `GITHUB_TOKEN` is available.
pub async fn push_qcow2_outputs(spec: &ImageSpec, output_dir: &Path) -> Result<(), BuilderError> {
    for target in &spec.targets {
        let Some(ref push_ref) = target.push_to else {
            continue;
        };

        let qcow2_path = output_dir.join(format!("{}.qcow2", target.name));

        if !qcow2_path.exists() {
            info!(
                target = %target.name,
                path = %qcow2_path.display(),
                "QCOW2 file not found — skipping push"
            );
            continue;
        }

        info!(
            reference = %push_ref,
            path = %qcow2_path.display(),
            "Pushing QCOW2 artifact to OCI registry (host-side)"
        );

        let qcow2_data =
            std::fs::read(&qcow2_path).map_err(|e| BuilderError::ArtifactPushFailed {
                detail: format!("failed to read QCOW2 file {}: {e}", qcow2_path.display()),
            })?;

        let distro = DistroFamily::from_distro_str(spec.distro.as_deref());
        let metadata = forge_oci::artifact::Qcow2Metadata {
            name: target.name.clone(),
            version: spec.metadata.version.clone(),
            architecture: "amd64".to_string(),
            os: distro.oci_os().to_string(),
            description: None,
        };

        let auth = forge_oci::artifact::resolve_ghcr_auth();

        forge_oci::artifact::push_qcow2_artifact(push_ref, qcow2_data, &metadata, &auth, &[])
            .await
            .map_err(|e| BuilderError::ArtifactPushFailed {
                detail: e.to_string(),
            })?;

        info!(reference = %push_ref, "Host-side push complete");
    }

    Ok(())
}
