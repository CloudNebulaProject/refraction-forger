use std::path::Path;

use spec_parser::schema::Target;
use tracing::info;

use crate::error::ForgeError;

/// Build an OCI container image from the staged rootfs.
pub fn build_oci(
    target: &Target,
    staging_root: &Path,
    output_dir: &Path,
) -> Result<(), ForgeError> {
    info!("Building OCI container image");

    // Create the tar.gz layer from staging
    let layer =
        forge_oci::tar_layer::create_layer(staging_root).map_err(|e| ForgeError::OciBuild(e.to_string()))?;

    info!(
        digest = %layer.digest,
        size = layer.data.len(),
        "Layer created"
    );

    // Build image options from target spec
    let mut options = forge_oci::manifest::ImageOptions::default();

    if let Some(ref ep) = target.entrypoint {
        options.entrypoint = Some(vec![ep.command.clone()]);
    }

    if let Some(ref env) = target.environment {
        options.env = env
            .vars
            .iter()
            .map(|v| format!("{}={}", v.key, v.value))
            .collect();
    }

    // Build manifest and config
    let (config_json, manifest_json) = forge_oci::manifest::build_manifest(&[layer.clone()], &options)
        .map_err(|e| ForgeError::OciBuild(e.to_string()))?;

    // Write OCI Image Layout
    let oci_output = output_dir.join(format!("{}-oci", target.name));
    forge_oci::layout::write_oci_layout(&oci_output, &[layer], &config_json, &manifest_json)
        .map_err(|e| ForgeError::OciBuild(e.to_string()))?;

    info!(path = %oci_output.display(), "OCI Image Layout written");
    Ok(())
}
