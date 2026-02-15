use std::path::PathBuf;

use tracing::info;

use crate::error::ForgeError;

/// Create a temporary staging directory for rootfs assembly.
pub fn create_staging() -> Result<(tempfile::TempDir, PathBuf), ForgeError> {
    let staging_dir = tempfile::TempDir::new().map_err(ForgeError::StagingSetup)?;
    let staging_root = staging_dir.path().to_path_buf();
    Ok((staging_dir, staging_root))
}

/// Extract a base tarball into the staging directory.
pub fn extract_base_tarball(tarball_path: &str, staging_root: &PathBuf) -> Result<(), ForgeError> {
    info!(tarball_path, "Extracting base tarball");

    let file =
        std::fs::File::open(tarball_path).map_err(|e| ForgeError::BaseExtract {
            path: tarball_path.to_string(),
            source: e,
        })?;

    // Try gzip first, fall back to plain tar
    let reader = std::io::BufReader::new(file);
    if tarball_path.ends_with(".gz") || tarball_path.ends_with(".tgz") {
        let decoder = flate2::read::GzDecoder::new(reader);
        let mut archive = tar::Archive::new(decoder);
        archive
            .unpack(staging_root)
            .map_err(|e| ForgeError::BaseExtract {
                path: tarball_path.to_string(),
                source: e,
            })?;
    } else {
        let mut archive = tar::Archive::new(reader);
        archive
            .unpack(staging_root)
            .map_err(|e| ForgeError::BaseExtract {
                path: tarball_path.to_string(),
                source: e,
            })?;
    }

    info!("Base tarball extracted");
    Ok(())
}
