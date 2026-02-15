use std::path::Path;

use flate2::write::GzEncoder;
use flate2::Compression;
use spec_parser::schema::Target;
use tracing::info;
use walkdir::WalkDir;

use crate::error::ForgeError;

/// Build a tarball artifact from the staged rootfs.
pub fn build_artifact(
    target: &Target,
    staging_root: &Path,
    output_dir: &Path,
    _files_dir: &Path,
) -> Result<(), ForgeError> {
    let output_path = output_dir.join(format!("{}.tar.gz", target.name));
    info!(path = %output_path.display(), "Creating artifact tarball");

    let file = std::fs::File::create(&output_path)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut tar = tar::Builder::new(encoder);

    for entry in WalkDir::new(staging_root).follow_links(false) {
        let entry = entry.map_err(|e| ForgeError::Qcow2Build {
            step: "artifact_walk".to_string(),
            detail: e.to_string(),
        })?;

        let full_path = entry.path();
        let rel_path = full_path
            .strip_prefix(staging_root)
            .unwrap_or(full_path);

        if rel_path.as_os_str().is_empty() {
            continue;
        }

        if full_path.is_file() {
            tar.append_path_with_name(full_path, rel_path)
                .map_err(|e| ForgeError::Qcow2Build {
                    step: "artifact_tar".to_string(),
                    detail: e.to_string(),
                })?;
        } else if full_path.is_dir() {
            tar.append_dir(rel_path, full_path)
                .map_err(|e| ForgeError::Qcow2Build {
                    step: "artifact_tar_dir".to_string(),
                    detail: e.to_string(),
                })?;
        }
    }

    let encoder = tar.into_inner().map_err(|e| ForgeError::Qcow2Build {
        step: "artifact_tar_finish".to_string(),
        detail: e.to_string(),
    })?;

    encoder.finish().map_err(|e| ForgeError::Qcow2Build {
        step: "artifact_gz_finish".to_string(),
        detail: e.to_string(),
    })?;

    info!(path = %output_path.display(), "Artifact tarball created");
    Ok(())
}
