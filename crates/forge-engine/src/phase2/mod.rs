pub mod artifact;
pub mod oci;
pub mod qcow2;
pub mod qcow2_ext4;
pub mod qcow2_zfs;

use std::path::Path;

use spec_parser::schema::{DistroFamily, Target, TargetKind};
use tracing::info;

use crate::error::ForgeError;

/// Resolve the output artifact base name shared by every Phase 2 producer:
/// the `--output-name` override if present, otherwise the target's own name.
pub fn artifact_base<'a>(target: &'a Target, output_name: Option<&'a str>) -> &'a str {
    output_name.unwrap_or(&target.name)
}

/// Execute Phase 2 for non-QCOW2 targets (OCI, Artifact).
///
/// QCOW2 targets are handled directly by the orchestrator in `lib.rs` via
/// the prepare/finalize/cleanup flow.
pub async fn execute(
    target: &Target,
    staging_root: &Path,
    files_dir: &Path,
    output_dir: &Path,
    distro: &DistroFamily,
    output_name: Option<&str>,
) -> Result<(), ForgeError> {
    info!(
        target = %target.name,
        kind = %target.kind,
        "Starting Phase 2: target production"
    );

    match target.kind {
        TargetKind::Oci => {
            oci::build_oci(target, staging_root, output_dir, distro, output_name)?;
        }
        TargetKind::Artifact => {
            artifact::build_artifact(target, staging_root, output_dir, files_dir, output_name)?;
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
    distro: &DistroFamily,
    version: &str,
    output_name: Option<&str>,
) -> Result<(), ForgeError> {
    if let Some(ref push_ref) = target.push_to {
        let qcow2_path = output_dir.join(format!("{}.qcow2", artifact_base(target, output_name)));
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
            version: version.to_string(),
            architecture: "amd64".to_string(),
            os: distro.oci_os().to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use spec_parser::schema::TargetKind;

    fn target(name: &str) -> Target {
        Target {
            name: name.to_string(),
            kind: TargetKind::Qcow2,
            disk_size: None,
            bootloader: None,
            filesystem: None,
            push_to: None,
            entrypoint: None,
            environment: None,
            pool: None,
        }
    }

    // `artifact_base` is the single source of truth every Phase 2 producer
    // (qcow2-ext4, qcow2-zfs, oci, artifact tar) uses to name its output.
    #[test]
    fn artifact_base_defaults_to_target_name() {
        let t = target("vm");
        assert_eq!(artifact_base(&t, None), "vm");
    }

    #[test]
    fn artifact_base_uses_output_name_override() {
        let t = target("vm");
        assert_eq!(artifact_base(&t, Some("solstice-ubuntu-22.04")), "solstice-ubuntu-22.04");
    }

    // Show the resulting filenames per format, with and without the override.
    #[test]
    fn output_filenames_across_formats() {
        let t = target("vm");

        let base = artifact_base(&t, None);
        assert_eq!(format!("{base}.qcow2"), "vm.qcow2"); // qcow2-ext4 / qcow2-zfs
        assert_eq!(format!("{base}.raw"), "vm.raw");
        assert_eq!(format!("{base}.tar.gz"), "vm.tar.gz"); // artifact
        assert_eq!(format!("{base}-oci"), "vm-oci"); // oci layout dir

        let base = artifact_base(&t, Some("img"));
        assert_eq!(format!("{base}.qcow2"), "img.qcow2");
        assert_eq!(format!("{base}.raw"), "img.raw");
        assert_eq!(format!("{base}.tar.gz"), "img.tar.gz");
        assert_eq!(format!("{base}-oci"), "img-oci");
    }
}
