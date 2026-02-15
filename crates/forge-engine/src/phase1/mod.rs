pub mod customizations;
pub mod overlays;
pub mod packages;
pub mod packages_apt;
pub mod staging;
pub mod variants;

use std::path::{Path, PathBuf};

use spec_parser::schema::{DistroFamily, ImageSpec};
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
/// Dispatches to the appropriate distro-specific path based on the `distro` field,
/// then applies common customizations and overlays.
pub async fn execute(
    spec: &ImageSpec,
    files_dir: &Path,
    runner: &dyn ToolRunner,
) -> Result<Phase1Result, ForgeError> {
    let distro = DistroFamily::from_distro_str(spec.distro.as_deref());
    info!(name = %spec.metadata.name, ?distro, "Starting Phase 1: rootfs assembly");

    // 1. Create staging directory
    let (staging_dir, staging_root) = staging::create_staging()?;
    let root = staging_root.to_str().unwrap();
    info!(root, "Staging directory created");

    // 2. Extract base tarball
    if let Some(ref base) = spec.base {
        staging::extract_base_tarball(base, &staging_root)?;
    }

    // 3. Distro-specific package management
    match distro {
        DistroFamily::OmniOS => execute_ips(spec, root, files_dir, runner).await?,
        DistroFamily::Ubuntu => execute_apt(spec, root, runner).await?,
    }

    // 4. Apply customizations (common)
    for customization in &spec.customizations {
        customizations::apply(customization, &staging_root)?;
    }

    // 5. Apply overlays (common)
    for overlay_block in &spec.overlays {
        overlays::apply_overlays(&overlay_block.actions, &staging_root, files_dir, runner).await?;
    }

    info!("Phase 1 complete: rootfs assembled");

    Ok(Phase1Result {
        staging_root,
        _staging_dir: staging_dir,
    })
}

/// IPS/OmniOS-specific Phase 1: pkg image-create, publishers, variants, certs, install.
async fn execute_ips(
    spec: &ImageSpec,
    root: &str,
    files_dir: &Path,
    runner: &dyn ToolRunner,
) -> Result<(), ForgeError> {
    info!("Executing IPS package management path");

    crate::tools::pkg::image_create(runner, root).await?;

    for publisher in &spec.repositories.publishers {
        crate::tools::pkg::set_publisher(runner, root, &publisher.name, &publisher.origin).await?;
    }

    if let Some(ref vars) = spec.variants {
        variants::apply_variants(runner, root, vars).await?;
    }

    if let Some(ref certs) = spec.certificates {
        for ca in &certs.ca {
            let certfile_path = files_dir.join(&ca.certfile);
            let certfile_str = certfile_path.to_str().unwrap_or(&ca.certfile);
            crate::tools::pkg::approve_ca_cert(runner, root, &ca.publisher, certfile_str).await?;
        }
    }

    if let Some(ref incorporation) = spec.incorporation {
        crate::tools::pkg::set_incorporation(runner, root, incorporation).await?;
    }

    packages::install_all(runner, root, &spec.packages).await?;

    Ok(())
}

/// APT/Ubuntu-specific Phase 1: debootstrap, add sources, apt update, apt install.
async fn execute_apt(
    spec: &ImageSpec,
    root: &str,
    runner: &dyn ToolRunner,
) -> Result<(), ForgeError> {
    info!("Executing APT package management path");

    // Determine suite and mirror from apt-mirror entries
    let first_mirror = spec.repositories.apt_mirrors.first();
    let suite = first_mirror
        .map(|m| m.suite.as_str())
        .unwrap_or("jammy");
    let mirror_url = first_mirror
        .map(|m| m.url.as_str())
        .unwrap_or("http://archive.ubuntu.com/ubuntu");
    let components = first_mirror.and_then(|m| m.components.as_deref());

    // Bootstrap the rootfs
    crate::tools::apt::debootstrap(runner, suite, root, mirror_url, components).await?;

    // Write sources.list with full component lists from all apt-mirror entries
    let source_entries: Vec<String> = spec
        .repositories
        .apt_mirrors
        .iter()
        .map(|m| {
            let components = m.components.as_deref().unwrap_or("main");
            format!("deb {} {} {}", m.url, m.suite, components)
        })
        .collect();
    if !source_entries.is_empty() {
        crate::tools::apt::write_sources_list(root, &source_entries).await?;
    }

    // Update package lists
    crate::tools::apt::update(runner, root).await?;

    // Install packages
    packages_apt::install_all(runner, root, &spec.packages).await?;

    Ok(())
}
