pub mod customizations;
pub mod overlays;
pub mod packages;
pub mod staging;
pub mod variants;

use std::path::{Path, PathBuf};

use spec_parser::schema::ImageSpec;
use tracing::info;

use crate::error::ForgeError;
use crate::tools::ToolRunner;

/// Result of Phase 1: a populated staging directory ready for Phase 2.
pub struct Phase1Result {
    /// Path to the staging root containing the assembled rootfs.
    pub staging_root: PathBuf,
    /// The tempdir handle -- dropping it cleans up the staging dir.
    pub _staging_dir: tempfile::TempDir,
}

/// Execute Phase 1: assemble a rootfs in a staging directory from the spec.
///
/// Steps:
/// 1. Create staging directory
/// 2. Extract base tarball (if specified)
/// 3. Apply IPS variants
/// 4. Configure package repositories and install packages
/// 5. Apply customizations (users, groups)
/// 6. Apply overlays (files, dirs, symlinks, shadow, devfsadm)
pub async fn execute(
    spec: &ImageSpec,
    files_dir: &Path,
    runner: &dyn ToolRunner,
) -> Result<Phase1Result, ForgeError> {
    info!(name = %spec.metadata.name, "Starting Phase 1: rootfs assembly");

    // 1. Create staging directory
    let (staging_dir, staging_root) = staging::create_staging()?;
    let root = staging_root.to_str().unwrap();
    info!(root, "Staging directory created");

    // 2. Extract base tarball
    if let Some(ref base) = spec.base {
        staging::extract_base_tarball(base, &staging_root)?;
    }

    // 3. Create IPS image and configure publishers
    crate::tools::pkg::image_create(runner, root).await?;

    for publisher in &spec.repositories.publishers {
        crate::tools::pkg::set_publisher(runner, root, &publisher.name, &publisher.origin).await?;
    }

    // 4. Apply variants
    if let Some(ref vars) = spec.variants {
        variants::apply_variants(runner, root, vars).await?;
    }

    // 5. Approve CA certificates
    if let Some(ref certs) = spec.certificates {
        for ca in &certs.ca {
            let certfile_path = files_dir.join(&ca.certfile);
            let certfile_str = certfile_path.to_str().unwrap_or(&ca.certfile);
            crate::tools::pkg::approve_ca_cert(runner, root, &ca.publisher, certfile_str).await?;
        }
    }

    // 6. Set incorporation
    if let Some(ref incorporation) = spec.incorporation {
        crate::tools::pkg::set_incorporation(runner, root, incorporation).await?;
    }

    // 7. Install packages
    packages::install_all(runner, root, &spec.packages).await?;

    // 8. Apply customizations
    for customization in &spec.customizations {
        customizations::apply(customization, &staging_root)?;
    }

    // 9. Apply overlays
    for overlay_block in &spec.overlays {
        overlays::apply_overlays(&overlay_block.actions, &staging_root, files_dir, runner).await?;
    }

    info!("Phase 1 complete: rootfs assembled");

    Ok(Phase1Result {
        staging_root,
        _staging_dir: staging_dir,
    })
}
